use std::{
    collections::HashSet,
    fmt::Display,
    ops::{BitAnd, SubAssign},
};

// 院外、院内、外科、内科はそれぞれ3つずつ選択する必要がある
#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
pub enum Hospital {
    InnerMedical,
    InnerSurgical,
    OuterSurgical,
    OuterMedical,
}

impl Display for Hospital {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Hospital::InnerMedical => "🏠💊",
                Hospital::InnerSurgical => "🏠🔪",
                Hospital::OuterMedical => "🚙💊",
                Hospital::OuterSurgical => "🚙🔪",
            }
        )
    }
}

#[derive(Debug)]
pub enum Course {
    Eschaton,
    Avoidance,
}

impl Display for Course {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Course::Eschaton => "💀",
                Course::Avoidance => "👍",
            }
        )
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug)]
pub struct HospitalSlots {
    pub inner_medical: usize,
    pub inner_surgical: usize,
    pub outer_medical: usize,
    pub outer_surgical: usize,
}

impl SubAssign<Hospital> for HospitalSlots {
    fn sub_assign(&mut self, rhs: Hospital) {
        match rhs {
            Hospital::InnerMedical => self.inner_medical -= 1,
            Hospital::InnerSurgical => self.inner_surgical -= 1,
            Hospital::OuterMedical => self.outer_medical -= 1,
            Hospital::OuterSurgical => self.outer_surgical -= 1,
        }
    }
}

impl HospitalSlots {
    pub fn len(&self) -> usize {
        self.inner_medical + self.inner_surgical + self.outer_medical + self.outer_surgical
    }
    pub fn is_empty(&self) -> bool {
        [
            self.inner_medical,
            self.inner_surgical,
            self.outer_medical,
            self.outer_surgical,
        ]
        .iter()
        .all(|&x| x == 0)
    }
    pub fn new(
        inner_medical: usize,
        inner_surgical: usize,
        outer_surgical: usize,
        outer_medical: usize,
    ) -> Self {
        HospitalSlots {
            inner_medical,
            inner_surgical,
            outer_medical,
            outer_surgical,
        }
    }
    pub fn zero() -> Self {
        Self::new(0, 0, 0, 0)
    }
    pub fn infinite() -> Self {
        Self::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX)
    }
    pub fn count(&self, h: Hospital) -> usize {
        match h {
            Hospital::InnerMedical => self.inner_medical,
            Hospital::InnerSurgical => self.inner_surgical,
            Hospital::OuterMedical => self.outer_medical,
            Hospital::OuterSurgical => self.outer_surgical,
        }
    }
}

impl BitAnd<&Self> for HospitalSlots {
    type Output = Self;
    fn bitand(self, rhs: &Self) -> Self::Output {
        HospitalSlots::new(
            self.inner_medical.min(rhs.inner_medical),
            self.inner_surgical.min(rhs.inner_surgical),
            self.outer_surgical.min(rhs.outer_surgical),
            self.outer_medical.min(rhs.outer_medical),
        )
    }
}

impl From<&Course> for HospitalSlots {
    fn from(value: &Course) -> Self {
        match value {
            Course::Eschaton => HospitalSlots {
                inner_medical: 1,
                inner_surgical: 2,
                outer_medical: 2,
                outer_surgical: 1,
            },
            Course::Avoidance => HospitalSlots {
                inner_medical: 2,
                inner_surgical: 1,
                outer_medical: 1,
                outer_surgical: 2,
            },
        }
    }
}

/// 学生
#[derive(Debug, Clone, Default)]
pub struct Student {
    name: String,
    selection: [Option<Hospital>; 6],
}

#[derive(serde::Deserialize, Debug)]
pub struct StudentRecord {
    name: String,
    term1: String,
    term2: String,
    term3: String,
    term4: String,
    term5: String,
    term6: String,
}

impl StudentRecord {
    pub fn extract(self) -> (String, [Option<Hospital>; 6]) {
        let StudentRecord {
            name,
            term1,
            term2,
            term3,
            term4,
            term5,
            term6,
        } = self;
        let selection = [term1, term2, term3, term4, term5, term6].map(|term| match term.trim() {
            "院内内科" => Some(Hospital::InnerMedical),
            "院内外科" => Some(Hospital::InnerSurgical),
            "院外内科" => Some(Hospital::OuterMedical),
            "院外外科" => Some(Hospital::OuterSurgical),
            "" => None,
            _ => panic!("未定義の病院名が指定されました: {term}"),
        });
        (name, selection)
    }
}

impl Student {
    /// 名前を取得
    pub fn get_name(&self) -> &str {
        &self.name
    }
    /// 名前に変換
    pub fn into_name(self) -> String {
        self.name
    }
    /// 選択状況を取得
    pub fn get_selection(&self) -> &[Option<Hospital>; 6] {
        &self.selection
    }
    /// コースが推定できる場合は返す
    pub fn course(&self) -> Option<Course> {
        let mut already_shown = HashSet::new();
        for h in self.selection.iter().filter_map(|x| x.as_ref()) {
            if !already_shown.insert(h) {
                return Some(match h {
                    Hospital::InnerMedical => Course::Avoidance,
                    Hospital::InnerSurgical => Course::Eschaton,
                    Hospital::OuterMedical => Course::Eschaton,
                    Hospital::OuterSurgical => Course::Avoidance,
                });
            }
        }
        None
    }
    /// 選択可能な学期のインデックスを返す
    pub fn selectable_terms(&self) -> impl Iterator<Item = usize> {
        self.selection
            .iter()
            .enumerate()
            .filter_map(|(i, x)| if x.is_none() { Some(i) } else { None })
    }
    fn slots(&self) -> HospitalSlots {
        if let Some(course) = self.course() {
            let mut slot = HospitalSlots::from(&course);
            for selection in self.selection.iter().flatten() {
                slot -= *selection;
            }
            slot
        } else {
            HospitalSlots::infinite()
        }
    }
    pub fn done(&self) -> bool {
        self.selection.iter().all(|x| x.is_some())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Database([HospitalSlots; 6]);

impl BitAnd<&Student> for &Database {
    type Output = Database;
    fn bitand(self, rhs: &Student) -> Self::Output {
        let mut hospitals: [HospitalSlots; 6] = core::array::from_fn(|_| HospitalSlots::zero());
        let slots = rhs.slots();
        for term in rhs.selectable_terms() {
            hospitals[term] = self.0[term].clone() & &slots;
        }
        Database(hospitals)
    }
}

impl Database {
    pub fn new(slots: [HospitalSlots; 6]) -> Self {
        Database(slots)
    }
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(HospitalSlots::is_empty)
    }
    pub fn len(&self) -> usize {
        self.0.iter().map(HospitalSlots::len).sum()
    }
    pub fn get_slots(&self) -> &[HospitalSlots; 6] {
        &self.0
    }
    fn index(&self, mut i: usize) -> Option<(usize, Hospital)> {
        let sizes: [usize; 6] = self.0.clone().map(|slot| slot.len());
        for (term, &size) in sizes.iter().enumerate() {
            if i < size {
                let slot = &self.0[term];
                let mut count = 0;
                for &hospital in [
                    Hospital::InnerMedical,
                    Hospital::InnerSurgical,
                    Hospital::OuterSurgical,
                    Hospital::OuterMedical,
                ]
                .iter()
                {
                    count += slot.count(hospital);
                    if i < count {
                        return Some((term, hospital));
                    }
                }
            } else {
                i -= size;
            }
        }
        // If we reach here, i > self.len() or self.is_empty()
        None
    }
    pub fn random_select(
        &mut self,
        student: &mut Student,
        mut rng: impl rand::Rng,
    ) -> Result<(), ()> {
        let choices = &*self & student;
        if choices.is_empty() {
            return Err(());
        }
        let rnd_index = { rng.random_range(0..choices.len()) };
        let (term, hospital) = choices.index(rnd_index).ok_or(())?;
        self.0[term] -= hospital;
        student.selection[term] = Some(hospital);
        Ok(())
    }
    pub fn new_student(&mut self, name: String, selection: [Option<Hospital>; 6]) -> Student {
        for (term, hospital) in selection.iter().enumerate() {
            if let Some(hospital) = hospital {
                self.0[term] -= *hospital;
            }
        }
        Student { name, selection }
    }
}
