use std::{
    collections::HashSet,
    fmt::Display,
    ops::{AddAssign, BitAnd, SubAssign},
    str::FromStr,
};

use serde_with::{DeserializeFromStr, NoneAsEmptyString, serde_as};

// 院外、院内、外科、内科はそれぞれ3つずつ選択する必要がある
#[derive(Eq, PartialEq, Hash, Clone, Copy, DeserializeFromStr)]
pub enum HospitalType {
    InnerMedical,
    InnerSurgical,
    OuterSurgical,
    OuterMedical,
}

impl FromStr for HospitalType {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "院内内科" => Ok(HospitalType::InnerMedical),
            "院内外科" => Ok(HospitalType::InnerSurgical),
            "院外内科" => Ok(HospitalType::OuterMedical),
            "院外外科" => Ok(HospitalType::OuterSurgical),
            _ => Err("Could not parse the &str as HospitalType"),
        }
    }
}

impl Display for HospitalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                HospitalType::InnerMedical => "🏠💊",
                HospitalType::InnerSurgical => "🏠🔪",
                HospitalType::OuterMedical => "🚙💊",
                HospitalType::OuterSurgical => "🚙🔪",
            }
        )
    }
}

pub enum Course {
    /// 院外外科1、院外内科2、院内外科2、院内内科1
    Eschaton,
    /// 院外外科2、院外内科1、院内外科1、院内内科2
    Avoidance,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct TermVacants {
    pub inner_medical: usize,
    pub inner_surgical: usize,
    pub outer_medical: usize,
    pub outer_surgical: usize,
}

impl SubAssign<HospitalType> for TermVacants {
    fn sub_assign(&mut self, rhs: HospitalType) {
        match rhs {
            HospitalType::InnerMedical => self.inner_medical -= 1,
            HospitalType::InnerSurgical => self.inner_surgical -= 1,
            HospitalType::OuterMedical => self.outer_medical -= 1,
            HospitalType::OuterSurgical => self.outer_surgical -= 1,
        }
    }
}

impl TermVacants {
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
        TermVacants {
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
    pub fn count(&self, h: HospitalType) -> usize {
        match h {
            HospitalType::InnerMedical => self.inner_medical,
            HospitalType::InnerSurgical => self.inner_surgical,
            HospitalType::OuterMedical => self.outer_medical,
            HospitalType::OuterSurgical => self.outer_surgical,
        }
    }
}

impl BitAnd<&Self> for TermVacants {
    type Output = Self;
    fn bitand(self, rhs: &Self) -> Self::Output {
        TermVacants::new(
            self.inner_medical.min(rhs.inner_medical),
            self.inner_surgical.min(rhs.inner_surgical),
            self.outer_surgical.min(rhs.outer_surgical),
            self.outer_medical.min(rhs.outer_medical),
        )
    }
}

impl AddAssign<&Self> for TermVacants {
    fn add_assign(&mut self, rhs: &Self) {
        self.inner_medical += rhs.inner_medical;
        self.inner_surgical += rhs.inner_surgical;
        self.outer_medical += rhs.outer_medical;
        self.outer_surgical += rhs.outer_surgical;
    }
}

impl From<&Course> for TermVacants {
    fn from(value: &Course) -> Self {
        match value {
            Course::Eschaton => TermVacants {
                inner_medical: 1,
                inner_surgical: 2,
                outer_medical: 2,
                outer_surgical: 1,
            },
            Course::Avoidance => TermVacants {
                inner_medical: 2,
                inner_surgical: 1,
                outer_medical: 1,
                outer_surgical: 2,
            },
        }
    }
}

/// 学生
#[derive(Clone, Default)]
pub struct Student {
    name: String,
    selection: [Option<HospitalType>; 6],
}

#[serde_as]
#[derive(serde::Deserialize)]
pub struct InitStudentOption {
    pub name: String,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term1: Option<HospitalType>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term2: Option<HospitalType>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term3: Option<HospitalType>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term4: Option<HospitalType>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term5: Option<HospitalType>,
    #[serde_as(as = "NoneAsEmptyString")]
    pub term6: Option<HospitalType>,
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
    pub fn get_selection(&self) -> &[Option<HospitalType>; 6] {
        &self.selection
    }
    /// コースが推定できる場合は返す
    pub fn course(&self) -> Option<Course> {
        let mut already_shown = HashSet::new();
        for h in self.selection.iter().filter_map(|x| x.as_ref()) {
            if !already_shown.insert(h) {
                return Some(match h {
                    HospitalType::InnerMedical => Course::Avoidance,
                    HospitalType::InnerSurgical => Course::Eschaton,
                    HospitalType::OuterMedical => Course::Eschaton,
                    HospitalType::OuterSurgical => Course::Avoidance,
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
    /// 現時点で残り必要な病院の数を返す
    fn required_hospitals(&self) -> TermVacants {
        if let Some(course) = self.course() {
            let mut slot = TermVacants::from(&course);
            for selection in self.selection.iter().flatten() {
                slot -= *selection;
            }
            slot
        } else {
            TermVacants::infinite()
        }
    }
    /// 必要単位を全て満たした学生
    pub fn done(&self) -> bool {
        self.selection.iter().all(|x| x.is_some())
    }
}

pub type HospitalTableInner = [TermVacants; 6];

#[derive(Clone)]
pub struct HospitalTable(HospitalTableInner);

impl BitAnd<&Student> for &HospitalTable {
    type Output = HospitalTable;
    fn bitand(self, rhs: &Student) -> Self::Output {
        let mut hospitals: HospitalTableInner = core::array::from_fn(|_| TermVacants::zero());
        let slots = rhs.required_hospitals();
        for term in rhs.selectable_terms() {
            hospitals[term] = self.0[term].clone() & &slots;
        }
        HospitalTable(hospitals)
    }
}

impl HospitalTable {
    pub fn into_inner(self) -> HospitalTableInner {
        self.0
    }
    pub fn as_inner(&self) -> &HospitalTableInner {
        &self.0
    }
    pub fn new(slots: HospitalTableInner) -> Self {
        HospitalTable(slots)
    }
    pub fn is_empty(&self) -> bool {
        self.0.iter().all(TermVacants::is_empty)
    }
    pub fn len(&self) -> usize {
        self.0.iter().map(TermVacants::len).sum()
    }
    fn index(&self, mut i: usize) -> Option<(usize, HospitalType)> {
        let sizes: [usize; 6] = self.0.clone().map(|slot| slot.len());
        for (term, &size) in sizes.iter().enumerate() {
            if i < size {
                let slot = &self.0[term];
                let mut count = 0;
                for &hospital in [
                    HospitalType::InnerMedical,
                    HospitalType::InnerSurgical,
                    HospitalType::OuterSurgical,
                    HospitalType::OuterMedical,
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
    /// ランダムに学生を選択する
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
    pub fn init_student(&mut self, student: InitStudentOption) -> Student {
        let InitStudentOption {
            name,
            term1,
            term2,
            term3,
            term4,
            term5,
            term6,
        } = student;
        let selection = [term1, term2, term3, term4, term5, term6];

        for (term, hospital) in selection.iter().enumerate() {
            if let Some(hospital) = hospital {
                self.0[term] -= *hospital;
            }
        }
        Student { name, selection }
    }
}
