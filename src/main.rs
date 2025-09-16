use std::{
    collections::HashMap, iter::once, mem::MaybeUninit, ops::AddAssign, sync::Arc,
    thread::available_parallelism,
};

use rand::seq::SliceRandom;
use tabled::settings::Style;

mod eschaton;

struct Report<'a> {
    state: &'a State,
    student: &'a Option<eschaton::Student>,
    db: &'a eschaton::Database,
}

impl std::fmt::Display for Report<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = tabled::builder::Builder::new();
        let fail: usize = self.state.fails.values().sum();
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
            "TRIAL: {}, SUCCESS: {} ({:.8} %)",
            self.state.count,
            self.state.count - fail,
            100.0 - (fail as f64 / self.state.count as f64 * 100.0)
        )?;

        let mut fails: Vec<(_, _)> = self.state.fails.iter().collect();
        fails.sort_by(|(_, a), (_, b)| b.cmp(a));
        let mut builder = tabled::builder::Builder::new();
        for (i, (name, fail)) in fails.into_iter().enumerate().take(40) {
            let rate = 100.0 * *fail as f64 / self.state.count as f64;
            builder.push_record([
                (i + 1).to_string(),
                name.to_string(),
                fail.to_string(),
                format!("{rate:.8} %"),
            ]);
        }

        let mut table = builder.build();
        std::fmt::Display::fmt(table.with(Style::empty()), f)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct State {
    pub count: usize,
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

impl AddAssign<Option<eschaton::Student>> for State {
    fn add_assign(&mut self, rhs: Option<eschaton::Student>) {
        self.count += 1;
        if let Some(student) = rhs {
            self.fails
                .entry(student.into_name())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<
        Result<eschaton::Database, (eschaton::Student, eschaton::Database)>,
    >(4096);
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
            if let Some(recv) = rx.recv().await {
                let (student, db) = match recv {
                    Ok(db) => (None, db),
                    Err((student, db)) => (Some(student), db),
                };
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
                        student: &student,
                        db: &db,
                    };
                    println!("{report}");
                }
                state += student;
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
                let result = {
                    let mut rng = rand::rng();
                    let mut students: Vec<eschaton::Student> = {
                        let mut s = students.as_ref().clone();
                        s.shuffle(&mut rng);
                        s
                    }
                    .clone();
                    'game: loop {
                        for student in students.iter_mut() {
                            if student.done() {
                                continue;
                            }
                            if db.random_select(student, &mut rng).is_err() {
                                break 'game Err((std::mem::take(student), db));
                            }
                        }
                        students.retain(|s| !s.done());
                        if students.is_empty() {
                            break 'game Ok(db);
                        }
                    }
                };
                let _ = tx.send(result).await;
            }
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
}
