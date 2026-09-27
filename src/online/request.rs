//! `JevRequestBuilder` (PRD §23.2): pure, no I/O. One `noul` question per state item, with
//! stable ids `q0..qn` in item order.

use super::{SemanticStage, prompt};
use serde::Serialize;
use serde::ser::SerializeMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JevRequest {
    pub model: String,
    pub state: JevState,
    pub questions: Questions,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JevState {
    pub query: String,
    pub guidance: &'static str,
    pub items: Vec<StateItem>,
}

/// A candidate as the classifier sees it: a relative path and bounded text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StateItem {
    pub id: String,
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JevQuestion {
    #[serde(rename = "type")]
    pub kind: JevQuestionType,
    pub instructions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JevQuestionType {
    Noul,
}

/// Questions by id, serialized as a JSON object in insertion order.
#[derive(Debug, Clone, PartialEq)]
pub struct Questions(pub Vec<(String, JevQuestion)>);

impl Serialize for Questions {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (id, q) in &self.0 {
            map.serialize_entry(id, q)?;
        }
        map.end()
    }
}

/// The request asking `stage`'s question about each item; question `qN` is about item N.
pub fn build(model: &str, query: &str, stage: SemanticStage, items: Vec<StateItem>) -> JevRequest {
    let questions = items
        .iter()
        .enumerate()
        .map(|(n, item)| {
            (
                format!("q{n}"),
                JevQuestion {
                    kind: JevQuestionType::Noul,
                    instructions: prompt::instructions(stage, &item.id),
                },
            )
        })
        .collect();
    JevRequest {
        model: model.into(),
        state: JevState {
            query: query.into(),
            guidance: prompt::GUIDANCE,
            items,
        },
        questions: Questions(questions),
    }
}
