//! Reading memory for `context_for_task` (PRD jev-mem §10): routing to the relational views,
//! expanding from the anchors under fixed limits, scoring candidates, and stopping on evidence.

use super::model::Graph;
use crate::online::response::Decision;
use std::collections::BTreeMap;

/// Expansions a read may spend, shared among the active views.
pub const EXPANSIONS: usize = 12;
/// A view's routing probability activates it from here on.
pub const VIEW_THRESHOLD: f64 = 0.10;
/// The four views, in the order a tie goes.
const VIEWS: [(Graph, &str); 4] = [
    (Graph::Semantic, "semantic"),
    (Graph::Temporal, "temporal"),
    (Graph::Causal, "causal"),
    (Graph::Entity, "entity"),
];

/// Where a read goes, from the routing answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// Active views and their expansions, in the fixed order.
    pub budget: Vec<(Graph, usize)>,
    /// 1, or 2 when connecting several facts is needed.
    pub depth: usize,
    /// Recency may break ties between candidates.
    pub recency: bool,
    /// An answer the route depends on was unknown.
    pub partial: bool,
}

fn known(answers: &BTreeMap<&str, Decision>, name: &str) -> Option<f64> {
    answers.get(name).and_then(Decision::probability)
}

/// The route for the routing answers, by name. An unknown need never activates a view; an unknown
/// `multi_hop_need` keeps the depth at 1 and marks the route partial.
pub fn route(answers: &BTreeMap<&str, Decision>) -> Route {
    let active: Vec<(Graph, f64)> = VIEWS
        .iter()
        .filter_map(|(g, name)| known(answers, name).map(|p| (*g, p)))
        .filter(|(_, p)| *p >= VIEW_THRESHOLD)
        .collect();
    let multi_hop = known(answers, "multi_hop_need");
    Route {
        budget: split(&active),
        depth: match multi_hop {
            Some(p) if p >= 0.5 => 2,
            _ => 1,
        },
        recency: known(answers, "recency_importance").is_some_and(|p| p >= 0.5),
        partial: multi_hop.is_none(),
    }
}

/// One expansion per active view, the rest in proportion to need (exponent 1.0), rounded by
/// largest remainder; equal remainders go in the fixed order of the views.
fn split(active: &[(Graph, f64)]) -> Vec<(Graph, usize)> {
    if active.is_empty() {
        return vec![];
    }
    let rest = EXPANSIONS.saturating_sub(active.len());
    let total: f64 = active.iter().map(|(_, p)| p).sum();
    let quotas: Vec<f64> = active
        .iter()
        .map(|(_, p)| rest as f64 * p / total)
        .collect();
    let mut shares: Vec<usize> = quotas.iter().map(|q| q.floor() as usize).collect();
    let mut order: Vec<usize> = (0..active.len()).collect();
    // Stable: equal remainders keep the fixed order of `VIEWS`.
    order.sort_by(|a, b| {
        (quotas[*b] - quotas[*b].floor()).total_cmp(&(quotas[*a] - quotas[*a].floor()))
    });
    let left = rest - shares.iter().sum::<usize>();
    for i in order.into_iter().take(left) {
        shares[i] += 1;
    }
    active
        .iter()
        .zip(shares)
        .map(|((g, _), n)| (*g, n + 1))
        .collect()
}
