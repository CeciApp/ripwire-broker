//! Validation of a classifier response (PRD §23.2, CA-ONLINE-10). A probability that is
//! absent or invalid is unknown, never clamped into `[0, 1]` and never read as `false`.

use serde_json::Value;

/// A response that breaks the contract as a whole; every question it answers is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidResponse {
    /// Not JSON, or no `model`/`answers` object.
    Malformed,
    /// Answered by a model other than the pinned one.
    WrongModel,
    /// An answer for a question that was not asked: the id correspondence is broken.
    UnknownQuestion,
}

impl std::fmt::Display for InvalidResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Malformed => "malformed classifier response",
            Self::WrongModel => "classifier answered with another model",
            Self::UnknownQuestion => "classifier answered a question it was not asked",
        })
    }
}

/// The probability of each question in `ids`, in that order; `None` is unknown.
pub fn parse_answers(
    model: &str,
    ids: &[String],
    body: &str,
) -> Result<Vec<Option<f64>>, InvalidResponse> {
    let v: Value = serde_json::from_str(body).map_err(|_| InvalidResponse::Malformed)?;
    let (Some(got), Some(answers)) = (v["model"].as_str(), v["answers"].as_object()) else {
        return Err(InvalidResponse::Malformed);
    };
    if got != model {
        return Err(InvalidResponse::WrongModel);
    }
    if answers.keys().any(|k| !ids.contains(k)) {
        return Err(InvalidResponse::UnknownQuestion);
    }
    Ok(ids
        .iter()
        .map(|id| {
            let a = answers.get(id)?;
            if a["type"] != "noul" {
                return None;
            }
            a["noul"]
                .as_f64()
                .filter(|p| p.is_finite() && (0.0..=1.0).contains(p))
        })
        .collect())
}
