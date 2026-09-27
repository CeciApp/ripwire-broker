//! Fits an envelope under its token budget (RF-05). Estimate: 4 bytes of JSON per token.

use crate::model::{Envelope, Item, Limitation, Note};
use crate::normalize::Entry;

pub fn estimate_tokens(env: &Envelope) -> u32 {
    serde_json::to_string(env)
        .map(|s| s.len().div_ceil(4) as u32)
        .unwrap_or(u32::MAX)
}

fn over(env: &Envelope) -> bool {
    estimate_tokens(env) > env.budget.requested_tokens
}

enum Added {
    Item,
    Test,
    Risk,
}

/// Adds entries in priority order while they fit; limitations are always kept (PRD 10.2 #1).
pub fn fill(env: &mut Envelope, mut entries: Vec<Entry>) {
    entries.sort_by_key(|e| match e {
        Entry::Limitation(_) => 0,
        Entry::Item(p, _) | Entry::Test(p, _) | Entry::Risk(p, _) => *p,
    });
    let entries = crate::dedup::dedup(entries);
    let total = entries
        .iter()
        .filter(|e| !matches!(e, Entry::Limitation(_)))
        .count();
    let mut order: Vec<Added> = Vec::new();
    for entry in entries {
        match entry {
            Entry::Limitation(l) => env.limitations.push(l),
            Entry::Item(_, item) => {
                if try_item(env, item) {
                    order.push(Added::Item);
                }
            }
            Entry::Test(_, t) => {
                env.tests.push(t);
                if over(env) {
                    env.tests.pop();
                } else {
                    order.push(Added::Test);
                }
            }
            Entry::Risk(_, r) => {
                env.risks.push(r);
                if over(env) {
                    env.risks.pop();
                } else {
                    order.push(Added::Risk);
                }
            }
        }
    }
    finish(env, order, total);
}

/// Whole item first; if it does not fit, the same item without its body (bodies on demand).
fn try_item(env: &mut Envelope, item: Item) -> bool {
    let slim = item.content.is_some().then(|| Item {
        content: None,
        ..item.clone()
    });
    env.items.push(item);
    if !over(env) {
        return true;
    }
    env.items.pop();
    if let Some(slim) = slim {
        env.items.push(slim);
        if !over(env) {
            return true;
        }
        env.items.pop();
    }
    false
}

/// Records the counts and, if the bookkeeping itself overflows, drops the latest additions.
fn finish(env: &mut Envelope, mut order: Vec<Added>, total: usize) {
    loop {
        let shown = env.items.len() + env.tests.len() + env.risks.len();
        env.budget.shown = shown;
        env.budget.omitted = total - shown;
        env.budget.truncated = env.budget.omitted > 0;
        env.budget.next_step = next_step(env);
        env.budget.estimated_tokens = env.budget.requested_tokens; // widest value while measuring
        if !over(env) || order.is_empty() {
            break;
        }
        match order.pop() {
            Some(Added::Item) => {
                env.items.pop();
            }
            Some(Added::Test) => {
                env.tests.pop();
            }
            Some(Added::Risk) => {
                env.risks.pop();
            }
            None => break,
        }
    }
    env.budget.estimated_tokens = estimate_tokens(env);
}

fn next_step(env: &Envelope) -> Option<String> {
    env.budget.truncated.then(|| {
        format!(
            "{} items omitted; call again with a larger budget_tokens (more than {}) or a narrower task",
            env.budget.omitted, env.budget.requested_tokens
        )
    })
}

/// Adds notes after the items (peripheral context, PRD 10.2 #9) and the note limitations.
/// Over budget, the last notes give way first, then the lowest-ranked items, which are
/// counted as omitted.
pub fn add_notes(env: &mut Envelope, notes: Vec<Note>, limitations: Vec<Limitation>) {
    let mut left_out = 0;
    env.notes.extend(notes);
    env.limitations.extend(limitations);
    let mut counted = 0;
    loop {
        if left_out != counted {
            env.limitations.retain(|l| l.kind != "notes_omitted");
            env.limitations.push(crate::notes::omitted(left_out));
            counted = left_out;
        }
        env.budget.estimated_tokens = env.budget.requested_tokens; // widest value while measuring
        if !over(env) {
            break;
        }
        if env.notes.pop().is_some() {
            left_out += 1;
            continue;
        }
        let dropped =
            env.items.pop().is_some() || env.tests.pop().is_some() || env.risks.pop().is_some();
        if !dropped {
            break;
        }
        env.budget.shown -= 1;
        env.budget.omitted += 1;
        env.budget.truncated = true;
        env.budget.next_step = next_step(env);
    }
    env.budget.estimated_tokens = estimate_tokens(env);
}
