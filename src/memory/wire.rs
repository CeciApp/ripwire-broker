//! Memory requests to the classifier (PRD jev-mem §8.2): at most 32 questions and 38,000 bytes
//! each, split before sending and never through a pair, so every answer about a pair arrives in
//! the same response as the rest of that pair's answers.

use crate::online::request::{JevQuestion, StateRequest};
use serde_json::Value;

pub const MAX_QUESTIONS: usize = 32;
pub const MAX_REQUEST_BYTES: usize = crate::online::request::MAX_REQUEST_BYTES;

/// One indivisible unit: the state item it is about and its named questions.
#[derive(Debug, Clone)]
pub struct Group {
    pub item: Value,
    pub questions: Vec<(String, JevQuestion)>,
}

/// One request and, for each of its questions in order, the group and the question's name.
#[derive(Debug, Clone)]
pub struct Batch {
    pub request: StateRequest,
    pub keys: Vec<(usize, String)>,
}

/// A group too large or with too many questions to fit any request on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooLarge;

/// `base` (an object) is shared by every request; each request adds its groups' items under
/// `list_key`, in order.
pub fn batches(
    model: &str,
    base: &Value,
    list_key: &str,
    groups: Vec<Group>,
) -> Result<Vec<Batch>, TooLarge> {
    let mut out = Vec::new();
    let mut current: Vec<(usize, Group)> = Vec::new();
    for (index, group) in groups.into_iter().enumerate() {
        let mut trial = current.clone();
        trial.push((index, group.clone()));
        if fits(model, base, list_key, &trial) {
            current = trial;
            continue;
        }
        if current.is_empty() {
            return Err(TooLarge);
        }
        out.push(assemble(
            model,
            base,
            list_key,
            std::mem::take(&mut current),
        ));
        let alone = vec![(index, group)];
        if !fits(model, base, list_key, &alone) {
            return Err(TooLarge);
        }
        current = alone;
    }
    if !current.is_empty() {
        out.push(assemble(model, base, list_key, current));
    }
    Ok(out)
}

fn fits(model: &str, base: &Value, list_key: &str, groups: &[(usize, Group)]) -> bool {
    let batch = assemble(model, base, list_key, groups.to_vec());
    batch.request.questions.0.len() <= MAX_QUESTIONS
        && serde_json::to_string(&batch.request).is_ok_and(|s| s.len() <= MAX_REQUEST_BYTES)
}

fn assemble(model: &str, base: &Value, list_key: &str, groups: Vec<(usize, Group)>) -> Batch {
    let mut state = base.clone();
    let mut keys = Vec::new();
    let mut questions = Vec::new();
    let mut items = Vec::new();
    for (index, group) in groups {
        items.push(group.item);
        for (name, q) in group.questions {
            keys.push((index, name));
            questions.push(q);
        }
    }
    if let Some(obj) = state.as_object_mut() {
        obj.insert(list_key.into(), Value::Array(items));
    }
    Batch {
        request: StateRequest::new(model, state, questions),
        keys,
    }
}
