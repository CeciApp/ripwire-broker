//! `JevRequestBuilder` (PRD §23.2): pure, no I/O. One `noul` question per state item, with
//! stable ids `q0..qn` in item order.

use super::{SemanticStage, prompt};
use serde::Serialize;
use serde::ser::SerializeMap;

/// A request closes before passing either limit (v0.1 §11.7).
pub const MAX_QUESTIONS: usize = 128;
pub const MAX_REQUEST_BYTES: usize = 38_000;
/// Evidence (`source_selection`) batches: at most 8 units and about 14 KiB of text (v0.1 §9.6).
pub const MAX_EVIDENCE_UNITS: usize = 8;
pub const EVIDENCE_BATCH_BYTES: usize = 14 * 1024;

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

fn json_len<T: Serialize>(v: &T) -> usize {
    serde_json::to_string(v).map_or(usize::MAX, |s| s.len())
}

/// Bytes the `n`-th item adds to a request (its state entry, its question and the commas).
fn item_bytes(stage: SemanticStage, n: usize, item: &StateItem) -> usize {
    let question = JevQuestion {
        kind: JevQuestionType::Noul,
        instructions: prompt::instructions(stage, &item.id),
    };
    let separators = if n == 0 { 0 } else { 2 };
    json_len(item) + format!("\"q{n}\":").len() + json_len(&question) + separators
}

/// The exact length of `build(model, query, stage, items)` as JSON, computed item by item.
pub fn request_bytes(model: &str, query: &str, stage: SemanticStage, items: &[StateItem]) -> usize {
    let base = json_len(&build(model, query, stage, vec![]));
    items.iter().enumerate().fold(base, |sum, (n, item)| {
        sum.saturating_add(item_bytes(stage, n, item))
    })
}

/// Splits `items` into requests under the limits, keeping their order. An item that cannot be
/// sent even alone is returned apart, for a `request_too_large` limitation.
pub fn batches(
    model: &str,
    query: &str,
    stage: SemanticStage,
    items: Vec<StateItem>,
) -> (Vec<JevRequest>, Vec<StateItem>) {
    let base = request_bytes(model, query, stage, &[]);
    let evidence = stage == SemanticStage::SourceSelection;
    let (mut sent, mut too_large) = (vec![], vec![]);
    let (mut open, mut bytes, mut text) = (Vec::<StateItem>::new(), base, 0usize);
    for item in items {
        let grows = |open: &[StateItem], bytes: usize, text: usize| {
            let n = open.len();
            let within_evidence = !evidence
                || n == 0
                || (n < MAX_EVIDENCE_UNITS && text + item.text.len() <= EVIDENCE_BATCH_BYTES);
            n < MAX_QUESTIONS
                && within_evidence
                && bytes.saturating_add(item_bytes(stage, n, &item)) <= MAX_REQUEST_BYTES
        };
        if !grows(&open, bytes, text) && !open.is_empty() {
            sent.push(build(model, query, stage, std::mem::take(&mut open)));
            (bytes, text) = (base, 0);
        }
        if grows(&open, bytes, text) {
            bytes += item_bytes(stage, open.len(), &item);
            text += item.text.len();
            open.push(item);
        } else {
            too_large.push(item);
        }
    }
    if !open.is_empty() {
        sent.push(build(model, query, stage, open));
    }
    (sent, too_large)
}
