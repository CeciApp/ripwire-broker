//! Removes repeated symbols, bodies, tests and limitations (RF-06). Entries must already be
//! in priority order: the first occurrence wins.

use crate::normalize::Entry;
use std::collections::HashSet;

pub fn dedup(entries: Vec<Entry>) -> Vec<Entry> {
    let mut seen: HashSet<String> = HashSet::new();
    entries
        .into_iter()
        .filter(|e| {
            let keys = keys(e);
            let fresh = keys.iter().all(|k| !seen.contains(k));
            if fresh {
                seen.extend(keys);
            }
            fresh
        })
        .collect()
}

fn keys(e: &Entry) -> Vec<String> {
    match e {
        Entry::Item(_, i) => {
            let mut k = match &i.symbol {
                Some(sym) => vec![format!("sym:{}::{sym}", i.path)],
                None => vec![format!("file:{}:{:?}", i.path, i.line)],
            };
            if let Some(line) = i.line {
                k.push(format!("loc:{}:{line}", i.path));
            }
            if let Some(c) = &i.content {
                k.push(format!("body:{}", c.untrusted_repository_data.trim()));
            }
            k
        }
        Entry::Test(_, t) => vec![format!("test:{}", t.path)],
        Entry::Risk(_, r) => vec![format!("risk:{}:{:?}:{:?}", r.kind, r.path, r.symbol)],
        Entry::Limitation(l) => vec![format!("lim:{}:{}", l.kind, l.detail)],
    }
}
