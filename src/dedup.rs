//! Removes repeated symbols, bodies, tests and limitations (RF-06). Entries must already be
//! in priority order: the first occurrence wins.

use crate::normalize::Entry;
use std::collections::HashSet;

/// A repeated entry goes. An item that is new but whose body already went out stays, without
/// its body: two different symbols may well share one (`fn default() -> Self { Self::new() }`),
/// and RF-06 forbids repeating the body, not naming the second symbol.
pub fn dedup(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen: HashSet<String> = HashSet::new();
    entries
        .into_iter()
        .filter_map(|mut e| {
            let (identity, body) = keys(&e);
            if identity.iter().any(|k| seen.contains(k)) {
                return None;
            }
            seen.extend(identity);
            if let (Some(body), Entry::Item(_, i)) = (body, &mut e)
                && !seen.insert(body)
            {
                i.content = None;
            }
            Some(e)
        })
        .collect()
}

/// What makes an entry the same as another, and its body's key when it has one.
fn keys(e: &Entry) -> (Vec<String>, Option<String>) {
    match e {
        Entry::Item(_, i) => {
            let mut k = match &i.symbol {
                Some(sym) => vec![format!("sym:{}::{sym}", i.path)],
                None => vec![format!("file:{}:{:?}", i.path, i.line)],
            };
            if let Some(line) = i.line {
                k.push(format!("loc:{}:{line}", i.path));
            }
            let body = i
                .content
                .as_ref()
                .map(|c| format!("body:{}", c.untrusted_repository_data.trim()));
            (k, body)
        }
        Entry::Test(_, t) => (vec![format!("test:{}", t.path)], None),
        Entry::Risk(_, r) => (
            vec![format!("risk:{}:{:?}:{:?}", r.kind, r.path, r.symbol)],
            None,
        ),
        Entry::Limitation(l) => (vec![format!("lim:{}:{}", l.kind, l.detail)], None),
    }
}
