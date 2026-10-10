//! Validation of a classifier response (PRD §23.2, CA-ONLINE-10). A probability that is
//! absent or invalid is unknown, never clamped into `[0, 1]` and never read as `false`.

use serde_json::Value;

/// A response that breaks the contract as a whole; every question it answers is unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidResponse {
    /// Not JSON, or no `model`/`answers` object.
    Malformed,
    /// Answered by a model other than the pinned one.
    WrongModel,
    /// An answer for a question that was not asked: the id correspondence is broken.
    UnknownQuestion,
    /// The same question answered twice: which answer counts is undecidable.
    DuplicateQuestion,
    /// The provider's envelope says the request failed (D-166): its errors, already sanitized,
    /// capped and without the credential. The parsers never produce it.
    Refused(String),
}

impl std::fmt::Display for InvalidResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Malformed => "malformed classifier response",
            Self::WrongModel => "classifier answered with another model",
            Self::UnknownQuestion => "classifier answered a question it was not asked",
            Self::DuplicateQuestion => "classifier answered a question twice",
            Self::Refused(errors) => return write!(f, "provider refused the request: {errors}"),
        })
    }
}

/// The probability of each question in `ids`, in that order; `None` is unknown.
pub fn parse_answers(
    model: &str,
    ids: &[String],
    body: &str,
) -> Result<Vec<Option<f64>>, InvalidResponse> {
    // Read in order, as the memory path does, so that a question answered twice is seen
    // instead of keeping whichever came last.
    let body: Body = serde_json::from_str(body).map_err(|_| InvalidResponse::Malformed)?;
    if body.model != model {
        return Err(InvalidResponse::WrongModel);
    }
    let mut answers: std::collections::BTreeMap<&str, &Value> = Default::default();
    for (id, v) in &body.answers.0 {
        if !ids.contains(id) {
            return Err(InvalidResponse::UnknownQuestion);
        }
        if answers.insert(id, v).is_some() {
            return Err(InvalidResponse::DuplicateQuestion);
        }
    }
    Ok(ids
        .iter()
        .map(|id| {
            let a = answers.get(id.as_str())?;
            if a["type"] != "noul" {
                return None;
            }
            a["noul"]
                .as_f64()
                .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
        })
        .collect())
}

// --- Typed decisions for memory requests (PRD jev-mem §7, CA-8) ---

use super::request::{JevQuestionType, StateRequest};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Choice probabilities may miss a sum of 1 by at most this much; never renormalized.
pub const CHOICE_SUM_TOLERANCE: f64 = 1e-3;

/// Why one decision is unknown; the rest of the batch stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// No answer, or no value in it.
    Absent,
    /// A Noul answered as a Choice or the other way round, or a value of the wrong JSON type.
    WrongType,
    /// A probability outside `[0, 1]` or not finite.
    OutOfRange,
    /// A Choice whose options are not exactly the ones asked, whose selection is not one of
    /// them, or whose probabilities do not sum to 1.
    BadChoice,
}

/// One answer, validated. An unknown is never read as a probability of 0.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Noul {
        probability: f64,
    },
    Choice {
        selected: String,
        probabilities: BTreeMap<String, f64>,
        confidence: Option<f64>,
    },
    Unknown {
        reason: Unknown,
    },
}

impl Decision {
    /// What a gate compares: a Noul's probability, or the selected option's probability (not
    /// `confidence`). `None` when unknown.
    pub fn probability(&self) -> Option<f64> {
        match self {
            Self::Noul { probability } => Some(*probability),
            Self::Choice {
                selected,
                probabilities,
                ..
            } => probabilities.get(selected).copied(),
            Self::Unknown { .. } => None,
        }
    }
}

/// Answers in the order they arrived, so a repeated id can be seen.
struct Entries(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Entries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of answers")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Entries, A::Error> {
                let mut out = Vec::new();
                while let Some(entry) = map.next_entry::<String, Value>()? {
                    out.push(entry);
                }
                Ok(Entries(out))
            }
        }
        d.deserialize_map(Visit)
    }
}

#[derive(Deserialize)]
struct Body {
    model: String,
    answers: Entries,
}

fn unit(v: &Value) -> Result<f64, Unknown> {
    let p = v.as_f64().ok_or(Unknown::WrongType)?;
    match p.is_finite() && (0.0..=1.0).contains(&p) {
        true => Ok(p),
        false => Err(Unknown::OutOfRange),
    }
}

/// The decision of each question of `req`, in order. A wrong model or a broken id
/// correspondence rejects the whole batch.
pub fn parse_decisions(req: &StateRequest, body: &str) -> Result<Vec<Decision>, InvalidResponse> {
    let body: Body = serde_json::from_str(body).map_err(|_| InvalidResponse::Malformed)?;
    if body.model != req.model {
        return Err(InvalidResponse::WrongModel);
    }
    let mut answers: BTreeMap<&str, &Value> = BTreeMap::new();
    for (id, v) in &body.answers.0 {
        if !req.questions.0.iter().any(|(asked, _)| asked == id) {
            return Err(InvalidResponse::UnknownQuestion);
        }
        if answers.insert(id, v).is_some() {
            return Err(InvalidResponse::DuplicateQuestion);
        }
    }
    Ok(req
        .questions
        .0
        .iter()
        .map(|(id, q)| {
            let decided = match answers.get(id.as_str()) {
                None => Err(Unknown::Absent),
                Some(a) => decide(q.kind, q.criteria.as_ref(), a),
            };
            decided.unwrap_or_else(|reason| Decision::Unknown { reason })
        })
        .collect())
}

fn decide(
    kind: JevQuestionType,
    criteria: Option<&super::request::Criteria>,
    a: &Value,
) -> Result<Decision, Unknown> {
    let answered = a["type"].as_str();
    match kind {
        JevQuestionType::Noul => {
            if answered != Some("noul") {
                return Err(Unknown::WrongType);
            }
            let v = a.get("noul").ok_or(Unknown::Absent)?;
            Ok(Decision::Noul {
                probability: unit(v)?,
            })
        }
        JevQuestionType::Choice => {
            if answered != Some("choice") {
                return Err(Unknown::WrongType);
            }
            let selected = a.get("choice").ok_or(Unknown::Absent)?;
            let selected = selected.as_str().ok_or(Unknown::WrongType)?;
            let given = a["probabilities"].as_object().ok_or(Unknown::Absent)?;
            let asked: Vec<&str> = criteria
                .map(|c| c.0.iter().map(|(o, _)| o.as_str()).collect())
                .unwrap_or_default();
            if given.len() != asked.len() || asked.iter().any(|o| !given.contains_key(*o)) {
                return Err(Unknown::BadChoice);
            }
            if !asked.contains(&selected) {
                return Err(Unknown::BadChoice);
            }
            let mut probabilities = BTreeMap::new();
            for (option, p) in given {
                probabilities.insert(option.clone(), unit(p)?);
            }
            let sum: f64 = probabilities.values().sum();
            if (sum - 1.0).abs() > CHOICE_SUM_TOLERANCE {
                return Err(Unknown::BadChoice);
            }
            let confidence = a.get("confidence").map(unit).transpose()?;
            Ok(Decision::Choice {
                selected: selected.into(),
                probabilities,
                confidence,
            })
        }
    }
}
