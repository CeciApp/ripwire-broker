//! Fits an envelope under its token budget (RF-05). Estimate: 4 bytes of JSON per token.

use crate::model::{Envelope, Item, Limitation, Note};
use crate::normalize::Entry;
use serde::Serialize;

pub fn estimate_tokens(env: &Envelope) -> u32 {
    serde_json::to_string(env)
        .map(|s| s.len().div_ceil(4) as u32)
        .unwrap_or(u32::MAX)
}

fn over(env: &Envelope) -> bool {
    estimate_tokens(env) > env.budget.requested_tokens
}

/// The envelope's compact JSON length. `usize::MAX` when it cannot be serialized, so an
/// unserializable envelope is over any budget, as `estimate_tokens` already had it by
/// answering `u32::MAX`.
fn json_len(env: &Envelope) -> usize {
    serde_json::to_string(env)
        .map(|s| s.len())
        .unwrap_or(usize::MAX)
}

/// Bytes that appending `value` to an array already holding `count` elements adds to the
/// envelope's JSON: the element, plus the comma when it is not the first. Exact for serde_json's
/// compact form, where an empty array is `[]` and `n` elements cost `sum + n - 1` inside the
/// brackets, and `items`, `tests`, `risks` and `limitations` are never skipped when empty.
/// `None` when the value cannot be serialized — a non-finite float — which the caller treats as
/// not fitting, the same answer the whole-envelope estimate gave.
fn added<T: Serialize>(value: &T, count: usize) -> Option<usize> {
    let body = serde_json::to_string(value).ok()?.len();
    Some(body + usize::from(count > 0))
}

/// `len` grown by `value`, or `None` when that would pass `budget`. Checking the sum is the same
/// decision `over()` made after pushing, without serializing the envelope again (D-099).
fn fits<T: Serialize>(value: &T, count: usize, len: usize, budget: u32) -> Option<usize> {
    let next = len.checked_add(added(value, count)?)?;
    (next.div_ceil(4) <= budget as usize).then_some(next)
}

enum Added {
    Item,
    Test,
    Risk,
}

/// Room held back from the entries so that the bookkeeping written *after* the fitting decisions
/// never has to evict one of them (D-103). Two sentences are written that late and neither is
/// accounted for while entries are being fitted:
///
/// - `next_step`, which `finish` writes once it knows how many entries were omitted;
/// - the `notes_omitted` limitation, which `add_notes` writes even when not one note fits.
///
/// Both are bounded, so the reserve is measured from their widest form rather than guessed. It
/// only applies when a summarizer is configured: without one, `add_notes` never runs, and the
/// `next_step` overshoot costs nobody an item because nothing is added after it.
pub fn notes_reserve() -> u32 {
    // Widest *reachable* form, not widest representable: at most `MAX_GROUPS` notes are ever
    // written, and `next_step` counts entries, which `MAX_BUDGET_TOKENS` bounds far below
    // `usize::MAX`. Reserving for unreachable digits would cost items for nothing.
    let widest_note_record = crate::notes::omitted(crate::notes::MAX_GROUPS);
    let widest_next_step = format!(
        "{} items omitted; call again with a larger budget_tokens (more than {}) or a narrower task",
        crate::broker::MAX_BUDGET_TOKENS,
        crate::broker::MAX_BUDGET_TOKENS
    );
    let tokens = |n: usize| (n + 1).div_ceil(4) as u32;
    let record = serde_json::to_string(&widest_note_record)
        .map(|s| tokens(s.len()))
        .unwrap_or(0);
    record + tokens(widest_next_step.len())
}

/// Adds entries in priority order while they fit; limitations are always kept (PRD 10.2 #1).
/// `reserve` is budget held back from the entries, for bookkeeping a later step must be able to
/// write without evicting anything already decided.
pub fn fill(env: &mut Envelope, entries: Vec<Entry>, reserve: u32) {
    fill_to(
        env,
        entries,
        env.budget.requested_tokens.saturating_sub(reserve),
    );
}

fn fill_to(env: &mut Envelope, mut entries: Vec<Entry>, budget: u32) {
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
    // The envelope's JSON length, carried along and grown by each entry that goes in, instead
    // of re-serialized whole for every candidate. Only the four arrays below change while this
    // loop runs, and each change is accounted for exactly, so the fit decisions are the ones
    // the whole-envelope estimate made (D-099). `finish` recomputes from scratch afterwards.
    let mut len = json_len(env);
    for entry in entries {
        match entry {
            // Always kept (PRD 10.2 #1), so this one is not a fit decision: it is accounted
            // for and pushed.
            Entry::Limitation(l) => {
                len =
                    added(&l, env.limitations.len()).map_or(usize::MAX, |d| len.saturating_add(d));
                env.limitations.push(l);
            }
            Entry::Item(_, item) => {
                if try_item(env, item, &mut len, budget) {
                    order.push(Added::Item);
                }
            }
            Entry::Test(_, t) => {
                if let Some(next) = fits(&t, env.tests.len(), len, budget) {
                    len = next;
                    env.tests.push(t);
                    order.push(Added::Test);
                }
            }
            Entry::Risk(_, r) => {
                if let Some(next) = fits(&r, env.risks.len(), len, budget) {
                    len = next;
                    env.risks.push(r);
                    order.push(Added::Risk);
                }
            }
        }
    }
    finish(env, order, total);
}

/// Whole item first; if it does not fit, the same item without its body (bodies on demand).
/// Nothing is pushed speculatively any more, so the item no longer has to be cloned to keep a
/// slim copy of it around.
fn try_item(env: &mut Envelope, item: Item, len: &mut usize, budget: u32) -> bool {
    if let Some(next) = fits(&item, env.items.len(), *len, budget) {
        *len = next;
        env.items.push(item);
        return true;
    }
    if item.content.is_some() {
        let slim = Item {
            content: None,
            ..item
        };
        if let Some(next) = fits(&slim, env.items.len(), *len, budget) {
            *len = next;
            env.items.push(slim);
            return true;
        }
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

/// Memory's share of an answer (PRD jev-mem §8.2): at most this many memories…
pub const MEMORY_MAX_ITEMS: usize = 3;
/// …this many tokens…
pub const MEMORY_MAX_TOKENS: u32 = 600;
/// …and this percentage of the requested budget; always inside what is left of it.
pub const MEMORY_SHARE_PERCENT: u32 = 20;

/// What fitting memories into an envelope did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryFit {
    pub delivered: Vec<String>,
    /// Left out for the budget.
    pub omitted: usize,
    /// Left out because this session already received them.
    pub already: usize,
}

/// Adds `items`, best first, inside memory's share and inside what the envelope has left: memory
/// is what gives way, never an item, a test or a risk already there. `seen` says which ids this
/// session already received.
pub fn add_memories(
    env: &mut Envelope,
    items: Vec<crate::model::MemoryItem>,
    seen: &dyn Fn(&str) -> bool,
) -> MemoryFit {
    let requested = env.budget.requested_tokens;
    let left = requested.saturating_sub(estimate_tokens(env));
    let cap = MEMORY_MAX_TOKENS
        .min(requested * MEMORY_SHARE_PERCENT / 100)
        .min(left);
    let mut fit = MemoryFit::default();
    for item in items {
        if seen(&item.id) {
            fit.already += 1;
            continue;
        }
        env.memories.push(item);
        // Measured on the envelope itself: the item, its comma and the `memories` key.
        let used = estimate_tokens(env).saturating_sub(requested - left);
        if env.memories.len() > MEMORY_MAX_ITEMS || used > cap {
            env.memories.pop();
            fit.omitted += 1;
            continue;
        }
        fit.delivered.push(
            env.memories
                .last()
                .map(|m| m.id.clone())
                .unwrap_or_default(),
        );
    }
    fit
}
