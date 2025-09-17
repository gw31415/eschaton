use std::{
    collections::HashMap, iter::once, mem::MaybeUninit, ops::AddAssign, sync::Arc,
    thread::available_parallelism,
};

use rand::seq::SliceRandom;
use tabled::settings::Style;

mod eschaton;

struct Report<'a> {
    state: &'a State,
    student: &'a Option<&'a eschaton::Student>,
    db: &'a eschaton::Database,
}

impl std::fmt::Display for Report<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = tabled::builder::Builder::new();
        builder.push_record(["LAST REPORT", "①", "②", "③", "④", "⑤", "⑥"]);
        let slots = self.db.get_slots();
        builder.push_record(
            once("院内内科".to_string()).chain(slots.iter().map(|s| s.inner_medical.to_string())),
        );
        builder.push_record(
            once("院内外科".to_string()).chain(slots.iter().map(|s| s.inner_surgical.to_string())),
        );
        builder.push_record(
            once("院外内科".to_string()).chain(slots.iter().map(|s| s.outer_medical.to_string())),
        );
        builder.push_record(
            once("院外外科".to_string()).chain(slots.iter().map(|s| s.outer_surgical.to_string())),
        );
        if let Some(student) = &self.student {
            let student_name = {
                let name = student.get_name();
                let width = unicode_width::UnicodeWidthStr::width(name);
                let mut result = String::with_capacity(20);
                result.push_str(name);
                result.push_str(&" ".repeat(20 - width));
                result
            };
            builder.push_record(once(student_name).chain(student.get_selection().iter().map(
                |s| {
                    if let Some(hospital) = s {
                        hospital.to_string()
                    } else {
                        "    ".to_string()
                    }
                },
            )));
        }
        let mut table = builder.build();
        std::fmt::Display::fmt(table.with(Style::modern()), f)?;
        writeln!(f)?;

        writeln!(
            f,
            "TRIAL: {}, SUCCESS: {} ({:.3} %)",
            self.state.count,
            self.state.success,
            self.state.success as f64 / self.state.count as f64 * 100.0,
        )?;

        let mut fails: Vec<(_, _)> = self.state.fails.iter().collect();
        fails.sort_by(|(_, a), (_, b)| b.cmp(a));
        let mut builder = tabled::builder::Builder::new();
        for (i, (name, fail)) in fails.into_iter().enumerate() {
            let rate = 100.0 * *fail as f64 / self.state.count as f64;
            builder.push_record([
                (i + 1).to_string(),
                name.to_string(),
                fail.to_string(),
                format!("{rate:.3} %"),
            ]);
        }

        let mut table = builder.build();
        std::fmt::Display::fmt(table.with(Style::empty()), f)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct State {
    pub count: usize,
    pub success: usize,
    pub fails: HashMap<String, usize>,
}

impl AddAssign<&Self> for State {
    fn add_assign(&mut self, rhs: &Self) {
        self.count += rhs.count;
        for (name, count) in &rhs.fails {
            self.fails
                .entry(name.clone())
                .and_modify(|c| *c += count)
                .or_insert(*count);
        }
    }
}

impl AddAssign<Vec<eschaton::Student>> for State {
    fn add_assign(&mut self, rhs: Vec<eschaton::Student>) {
        self.count += 1;
        if rhs.is_empty() {
            self.success += 1;
        }
        for student in rhs {
            self.fails
                .entry(student.into_name())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<(Vec<eschaton::Student>, eschaton::Database)>(4096);
    tokio::spawn(async move {
        let stdout_duration = std::time::Duration::from_millis(1000 / 5);
        let save_duration = std::time::Duration::from_secs(5);

        let mut state = {
            if tokio::fs::metadata("state.json").await.is_ok() {
                let data = tokio::fs::read("state.json").await.unwrap();
                serde_json::from_slice::<State>(&data).unwrap()
            } else {
                Default::default()
            }
        };
        let mut last_shown = tokio::time::Instant::now();
        let mut last_saved = tokio::time::Instant::now();
        loop {
            if let Some((students, db)) = rx.recv().await {
                if last_shown.elapsed() > stdout_duration {
                    last_shown = tokio::time::Instant::now();
                    if last_saved.elapsed() > save_duration {
                        last_saved = tokio::time::Instant::now();
                        tokio::fs::write("state.json", serde_json::to_vec_pretty(&state).unwrap())
                            .await
                            .unwrap();
                    }
                    let report = Report {
                        state: &state,
                        student: &students.first(),
                        db: &db,
                    };
                    println!("{report}");
                }
                state += students;
            } else {
                tokio::fs::write("state.json", serde_json::to_vec_pretty(&state).unwrap())
                    .await
                    .unwrap();
                println!("Successfully saved state.json");
            }
        }
    });
    let mut reserves = csv::Reader::from_reader(include_bytes!("./reserves.csv").as_slice());
    let mut maybeuninit: [MaybeUninit<eschaton::HospitalSlots>; 6] =
        std::array::from_fn(|_| MaybeUninit::uninit());
    for (i, row) in reserves.deserialize().enumerate() {
        maybeuninit
            .get_mut(i)
            .expect("Too many rows in reserves.csv")
            .write(row.expect("Failed to parse reserves.csv"));
    }
    let (db, students) = {
        let mut db = eschaton::Database::new(unsafe {
            std::mem::transmute::<
                [MaybeUninit<eschaton::HospitalSlots>; 6],
                [eschaton::HospitalSlots; 6],
            >(maybeuninit)
        });
        let students = Arc::new(
            csv::Reader::from_reader(include_bytes!("./students.csv").as_slice())
                .deserialize::<eschaton::StudentRecord>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .into_iter()
                .map(|s| {
                    let (name, selection) = s.extract();
                    db.new_student(name, selection)
                })
                .collect::<Vec<_>>(),
        );
        (Arc::new(db), students)
    };
    for _ in 0..available_parallelism().unwrap().into() {
        let tx = tx.clone();
        let db = db.clone();
        let students = students.clone();
        tokio::spawn(async move {
            loop {
                let mut db: eschaton::Database = db.as_ref().clone();
                let result: (Vec<_>, eschaton::Database) = {
                    let mut rng = rand::rng();
                    let mut students: Vec<eschaton::Student> = {
                        let mut s = students.as_ref().clone();
                        s.shuffle(&mut rng);
                        s
                    }
                    .clone();
                    let mut eschatons = Vec::new();
                    'game: loop {
                        let mut undone_students = Vec::new();
                        for mut student in students {
                            if student.done() {
                                continue;
                            } else if db.random_select(&mut student, &mut rng).is_err() {
                                eschatons.push(student);
                            } else {
                                undone_students.push(student);
                            }
                        }
                        if undone_students.is_empty() {
                            break 'game (eschatons, db);
                        }
                        students = undone_students;
                    }
                };
                let _ = tx.send(result).await;
            }
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
}
