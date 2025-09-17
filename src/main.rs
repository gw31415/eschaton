use std::{
    borrow::Cow, collections::HashMap, iter::once, mem::MaybeUninit, ops::AddAssign, sync::Arc,
    thread::available_parallelism,
};

use rand::seq::SliceRandom;
use tabled::settings::Style;

mod eschaton;

fn to_string(i: usize, count: usize) -> String {
    if i == 0 {
        String::from("0")
    } else {
        format!("{:.2}", i as f64 / count as f64)
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = tabled::builder::Builder::new();
        builder.push_record(["平均残席数", "①", "②", "③", "④", "⑤", "⑥"]);
        let slots = &self.vacants;
        builder.push_record(
            once(Cow::Borrowed("院内内科")).chain(
                slots
                    .iter()
                    .map(|s| to_string(s.inner_medical, self.count).into()),
            ),
        );
        builder.push_record(
            once(Cow::Borrowed("院内外科")).chain(
                slots
                    .iter()
                    .map(|s| to_string(s.inner_surgical, self.count).into()),
            ),
        );
        builder.push_record(
            once(Cow::Borrowed("院外内科")).chain(
                slots
                    .iter()
                    .map(|s| to_string(s.outer_medical, self.count).into()),
            ),
        );
        builder.push_record(
            once(Cow::Borrowed("院外外科")).chain(
                slots
                    .iter()
                    .map(|s| to_string(s.outer_surgical, self.count).into()),
            ),
        );
        std::fmt::Display::fmt(builder.build().with(Style::modern()), f)?;
        writeln!(f)?;

        writeln!(
            f,
            "TRIAL: {}, SUCCESS: {} ({:.3} %)",
            self.count,
            self.success,
            self.success as f64 / self.count as f64 * 100.0,
        )?;

        let mut fails: Vec<(_, _)> = self.fails.iter().collect();
        fails.sort_by(|(_, a), (_, b)| b.cmp(a));
        let mut builder = tabled::builder::Builder::new();
        for (i, (name, fail)) in fails.into_iter().enumerate() {
            let rate = 100.0 * *fail as f64 / self.count as f64;
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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct State {
    pub count: usize,
    pub success: usize,
    pub fails: HashMap<String, usize>,
    pub vacants: eschaton::HospitalTableInner,
}

impl Default for State {
    fn default() -> Self {
        State {
            count: 0,
            success: 0,
            fails: Default::default(),
            vacants: std::array::from_fn(|_| eschaton::TermVacants {
                inner_medical: 0,
                inner_surgical: 0,
                outer_medical: 0,
                outer_surgical: 0,
            }),
        }
    }
}

struct Trial {
    pub victims: Vec<eschaton::Student>,
    pub vacants: eschaton::HospitalTableInner,
}

impl AddAssign<Trial> for State {
    fn add_assign(&mut self, rhs: Trial) {
        let Trial {
            victims,
            vacants,
        } = rhs;
        self.count += 1;
        if victims.is_empty() {
            self.success += 1;
        }
        for student in victims {
            self.fails
                .entry(student.into_name())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
        for (a, b) in self.vacants.iter_mut().zip(vacants.iter()) {
            *a += b;
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Trial>(4096);
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
            if let Some(trial) = rx.recv().await {
                if last_shown.elapsed() > stdout_duration {
                    last_shown = tokio::time::Instant::now();
                    if last_saved.elapsed() > save_duration {
                        last_saved = tokio::time::Instant::now();
                        tokio::fs::write("state.json", serde_json::to_vec_pretty(&state).unwrap())
                            .await
                            .unwrap();
                    }
                    println!("{state}");
                }
                state += trial;
            } else {
                tokio::fs::write("state.json", serde_json::to_vec_pretty(&state).unwrap())
                    .await
                    .unwrap();
                println!("Successfully saved state.json");
            }
        }
    });
    let mut reserves = csv::Reader::from_reader(include_bytes!("./reserves.csv").as_slice());
    let mut maybeuninit: [MaybeUninit<eschaton::TermVacants>; 6] =
        std::array::from_fn(|_| MaybeUninit::uninit());
    for (i, row) in reserves.deserialize().enumerate() {
        maybeuninit
            .get_mut(i)
            .expect("Too many rows in reserves.csv")
            .write(row.expect("Failed to parse reserves.csv"));
    }
    let (db, students) = {
        let mut db = eschaton::HospitalTable::new(unsafe {
            std::mem::transmute::<[MaybeUninit<eschaton::TermVacants>; 6], [eschaton::TermVacants; 6]>(
                maybeuninit,
            )
        });
        let students = Arc::new(
            csv::Reader::from_reader(include_bytes!("./students.csv").as_slice())
                .deserialize::<eschaton::InitStudentOption>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
                .into_iter()
                .map(|s| db.init_student(s))
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
                let mut db: eschaton::HospitalTable = db.as_ref().clone();
                let trial: Trial = {
                    let mut rng = rand::rng();
                    let mut alives: Vec<eschaton::Student> = students.as_ref().clone();
                    let mut victims = Vec::new();
                    loop {
                        alives.shuffle(&mut rng);
                        let mut alives2 = Vec::new();
                        for mut student in alives {
                            if student.done() {
                                continue;
                            }
                            if db.random_select(&mut student, &mut rng).is_ok() {
                                &mut alives2
                            } else {
                                &mut victims
                            }
                            .push(student);
                        }
                        if alives2.is_empty() {
                            break Trial {
                                victims,
                                vacants: db.into_inner(),
                            };
                        }
                        alives = alives2;
                    }
                };
                let _ = tx.send(trial).await;
            }
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
}
