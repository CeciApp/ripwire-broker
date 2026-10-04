//! The local index over memories (PRD jev-mem §10, steps 2–3): words and exact entities, fused
//! by reciprocal rank into the anchors a read starts from. No embedding, nothing remote; the
//! ranking is a pure function of the snapshot and the query.

use super::store::State;
use std::collections::{BTreeMap, BTreeSet};

/// Anchors a read starts from.
pub const MAX_ANCHORS: usize = 8;
/// The `k` of reciprocal rank fusion, `Σ 1 / (k + rank)` with ranks from 1.
const RRF_K: f64 = 60.0;

/// Segments of Unicode alphanumeric characters, lowercased, in order; nothing is stemmed.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn ordered(mut scored: Vec<(String, f64)>) -> Vec<(String, f64)> {
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored
}

/// Nodes sharing a word with `query`, scored by the sum of `idf(t) = ln(1 + N / (1 + df(t)))`
/// over the distinct shared words; best first, a tie by id.
pub fn lexical_rank(state: &State, query: &str) -> Vec<(String, f64)> {
    let words: BTreeMap<&str, BTreeSet<String>> = state
        .nodes
        .iter()
        .map(|(id, r)| (id.as_str(), tokens(&r.content).into_iter().collect()))
        .collect();
    let mut df: BTreeMap<&str, usize> = BTreeMap::new();
    for set in words.values() {
        for t in set {
            *df.entry(t.as_str()).or_default() += 1;
        }
    }
    let n = state.nodes.len() as f64;
    let idf = |t: &str| (1.0 + n / (1.0 + df.get(t).copied().unwrap_or(0) as f64)).ln();
    let asked: BTreeSet<String> = tokens(query).into_iter().collect();
    let scored = words
        .iter()
        .filter_map(|(id, set)| {
            let shared: Vec<&String> = set.intersection(&asked).collect();
            (!shared.is_empty()).then(|| (id.to_string(), shared.iter().map(|t| idf(t)).sum()))
        })
        .collect();
    ordered(scored)
}

/// The words of `query` that may be paths: split on whitespace, quotes and brackets, with the
/// punctuation that ends a sentence taken off.
fn named_paths(query: &str) -> BTreeSet<&str> {
    query
        .split(|c: char| c.is_whitespace() || "\"'`()[]{}<>,;".contains(c))
        .map(|w| w.trim_end_matches(['.', ':', '!', '?']))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Nodes whose entity paths the query names exactly, a whole path and not part of one, by how
/// many it names; a tie by id.
pub fn entity_rank(state: &State, query: &str) -> Vec<(String, f64)> {
    let named_in_query = named_paths(query);
    let scored = state
        .nodes
        .iter()
        .filter_map(|(id, r)| {
            let named = r
                .entities
                .iter()
                .filter(|e| named_in_query.contains(e.path.as_str()))
                .count();
            (named > 0).then(|| (id.clone(), named as f64))
        })
        .collect();
    ordered(scored)
}

/// The lexical and entity rankings fused by reciprocal rank: at most [`MAX_ANCHORS`].
pub fn anchors(state: &State, query: &str) -> Vec<(String, f64)> {
    let mut fused: BTreeMap<String, f64> = BTreeMap::new();
    for ranking in [lexical_rank(state, query), entity_rank(state, query)] {
        for (rank, (id, _)) in ranking.into_iter().enumerate() {
            *fused.entry(id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
        }
    }
    let mut out = ordered(fused.into_iter().collect());
    out.truncate(MAX_ANCHORS);
    out
}
