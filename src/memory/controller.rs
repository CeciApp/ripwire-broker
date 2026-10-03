//! The worker's enrichment of one node (PRD jev-mem §7, §8.1): typing, a bounded and
//! deterministic set of candidates, and the relations between the node and each of them. The
//! node exists before any inference; nothing here holds the store's lock while waiting on the
//! network, and every request is charged to the 24-hour quota before it is sent.

use super::model::{Edge, EdgeBasis, EnrichmentState, Graph, POLICY_VERSION, Record, Types};
use super::prompts::{self, Stage};
use super::queue::{Lease, Outcome};
use super::store::{Refusal, State, Store};
use super::wire::{self, Group};
use crate::online::classifier::MemoryClassifier;
use crate::online::request::StateRequest;
use crate::online::response::Decision;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// An inferred edge needs at least this probability (PRD jev-mem §7).
pub const EDGE_THRESHOLD: f64 = 0.60;
/// A failed run is retried no sooner than this.
const RETRY_AFTER_MS: u64 = 60_000;

#[derive(Debug, Clone)]
pub struct Config {
    /// The pinned model.
    pub model: String,
    /// `K`: existing memories a new one is compared with (`--memory-write-candidates`).
    pub candidates: usize,
}

/// What one run did; counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Enriched {
    pub state: EnrichmentState,
    pub requests: usize,
    pub edges: usize,
}

fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn entity_ids(r: &Record) -> BTreeSet<&str> {
    r.entities.iter().map(|e| e.id.as_str()).collect()
}

/// Up to `k` nodes to compare `node_id` with: those sharing an entity or a word, and the
/// nearest earlier observation. Ordered by shared entities, then shared words, then distance
/// in the ingestion sequence, then id: the same store always gives the same list.
pub fn candidates(state: &State, node_id: &str, k: usize) -> Vec<String> {
    let Some(node) = state.nodes.get(node_id) else {
        return vec![];
    };
    let (ids, words) = (entity_ids(node), tokens(&node.content));
    let nearest = state
        .nodes
        .values()
        .filter(|r| r.node_id != node_id && r.ingest_seq < node.ingest_seq)
        .max_by_key(|r| (r.ingest_seq, std::cmp::Reverse(r.node_id.clone())))
        .map(|r| r.node_id.clone());
    let mut scored: Vec<(usize, usize, u64, &str)> = state
        .nodes
        .values()
        .filter(|r| r.node_id != node_id)
        .filter_map(|r| {
            let shared = entity_ids(r).intersection(&ids).count();
            let common = tokens(&r.content).intersection(&words).count();
            let near = nearest.as_deref() == Some(r.node_id.as_str());
            (shared > 0 || common > 0 || near).then_some((
                shared,
                common,
                r.ingest_seq.abs_diff(node.ingest_seq),
                r.node_id.as_str(),
            ))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(b.3))
    });
    scored
        .into_iter()
        .take(k)
        .map(|s| s.3.to_string())
        .collect()
}

fn inferred(
    source: &str,
    target: &str,
    graph: Graph,
    relation: &str,
    p: f64,
    cfg: &Config,
) -> Edge {
    Edge {
        source: source.into(),
        target: target.into(),
        graph,
        relation: relation.into(),
        basis: EdgeBasis::JevInference,
        score: Some(p),
        model: Some(cfg.model.clone()),
        prompt_version: Some(prompts::VERSION.into()),
        policy: POLICY_VERSION.into(),
        generation: 0,
    }
}

/// The edges between `new` and `cand` that the answers, by question name, support. A shared
/// entity id is a deterministic edge; an inference needs [`EDGE_THRESHOLD`], and an unknown
/// answer adds nothing. `caused_by` points candidate → new, `causes` new → candidate.
pub fn pair_edges(
    new: &Record,
    cand: &Record,
    answers: &BTreeMap<&str, Decision>,
    cfg: &Config,
) -> Vec<Edge> {
    let (n, c) = (new.node_id.as_str(), cand.node_id.as_str());
    let mut out = Vec::new();
    if entity_ids(new)
        .intersection(&entity_ids(cand))
        .next()
        .is_some()
    {
        out.push(Edge {
            basis: EdgeBasis::Deterministic,
            score: None,
            model: None,
            prompt_version: None,
            ..inferred(n, c, Graph::Entity, "shared_entity", 0.0, cfg)
        });
    }
    let p = |name: &str| {
        answers
            .get(name)
            .and_then(Decision::probability)
            .filter(|p| *p >= EDGE_THRESHOLD)
    };
    if let Some(p) = p("semantic") {
        out.push(inferred(n, c, Graph::Semantic, "semantic", p, cfg));
    }
    if let Some(p) = p("caused_by") {
        out.push(inferred(c, n, Graph::Causal, "causes", p, cfg));
    }
    if let Some(p) = p("causes") {
        out.push(inferred(n, c, Graph::Causal, "causes", p, cfg));
    }
    if let Some(p) = p("same_episode") {
        out.push(inferred(n, c, Graph::Temporal, "same_episode", p, cfg));
    }
    if let Some(p) = p("alias") {
        out.push(inferred(n, c, Graph::Entity, "alias", p, cfg));
    }
    if let (Some(Decision::Choice { selected, .. }), Some(p)) = (answers.get("time"), p("time"))
        && selected != "unknown"
    {
        out.push(inferred(n, c, Graph::Temporal, selected, p, cfg));
    }
    out
}

fn item(r: &Record) -> Value {
    json!({
        "id": r.node_id,
        "content": r.content,
        "entities": r.entities.iter().map(|e| &e.id).collect::<Vec<_>>(),
        "temporal_references": r.temporal_references,
    })
}

/// The relation questions about a pair, at position `index` of a request.
fn pair_questions(
    new: &Record,
    cand: &Record,
    index: usize,
) -> Vec<(String, crate::online::request::JevQuestion)> {
    let mut stages = vec![Stage::Relations];
    if entity_ids(new)
        .intersection(&entity_ids(cand))
        .next()
        .is_none()
    {
        stages.push(Stage::Alias);
    }
    if !new.temporal_references.is_empty() && !cand.temporal_references.is_empty() {
        stages.push(Stage::ImplicitTime);
    }
    stages
        .into_iter()
        .flat_map(|s| prompts::questions(s, index))
        .map(|(name, q)| (name.to_string(), q))
        .collect()
}

/// One run of `lease`'s job. Errors are the store's; a provider failure is an outcome.
pub async fn enrich(
    store: &Store,
    classifier: &dyn MemoryClassifier,
    lease: Lease,
    cfg: &Config,
    now_ms: u64,
) -> Result<Enriched, Refusal> {
    let state = store.load()?;
    let Some(node) = state.nodes.get(lease.node_id()).cloned() else {
        store.finish(lease, Outcome::Done)?;
        return Ok(Enriched {
            state: EnrichmentState::Complete,
            requests: 0,
            edges: 0,
        });
    };
    let mut requests = 0;
    let mut partial = false;

    // Typing: four Nouls on the observation alone.
    let typing = StateRequest::new(
        &cfg.model,
        json!({"observation": node.content}),
        prompts::questions(Stage::Typing, 0)
            .into_iter()
            .map(|(_, q)| q)
            .collect(),
    );
    store.charge(now_ms, 1, typing.questions.0.len() as u32)?;
    requests += 1;
    let types = match classifier.decide(&typing).await {
        Ok(d) => {
            partial |= d.iter().any(|d| d.probability().is_none());
            let p = |i: usize| d.get(i).and_then(Decision::probability);
            Types {
                episodic: p(0),
                semantic: p(1),
                procedural: p(2),
                preference: p(3),
            }
        }
        Err(_) => {
            store.commit_enrichment(&node.node_id, None, EnrichmentState::Failed, vec![])?;
            store.finish(
                lease,
                Outcome::Retry {
                    not_before_ms: now_ms + RETRY_AFTER_MS,
                },
            )?;
            return Ok(Enriched {
                state: EnrichmentState::Failed,
                requests,
                edges: 0,
            });
        }
    };

    // Relations: every pair's questions, split into requests that never cut a pair.
    let chosen: Vec<Record> = candidates(&state, &node.node_id, cfg.candidates)
        .iter()
        .filter_map(|id| state.nodes.get(id).cloned())
        .collect();
    let groups: Vec<Group> = chosen
        .iter()
        .enumerate()
        .map(|(i, c)| Group {
            item: item(c),
            questions: pair_questions(&node, c, i),
        })
        .collect();
    let base = json!({"new_memory": item(&node)});
    let mut answers: Vec<BTreeMap<&str, Decision>> = vec![BTreeMap::new(); chosen.len()];
    let batches = wire::batches(&cfg.model, &base, "candidates", groups).unwrap_or_default();
    for batch in batches {
        // Within a request, a pair is `candidates[local]`: ask again with that index.
        let order: Vec<usize> = batch.keys.iter().map(|(g, _)| *g).fold(vec![], |mut v, g| {
            if v.last() != Some(&g) {
                v.push(g);
            }
            v
        });
        let questions = order
            .iter()
            .enumerate()
            .flat_map(|(local, g)| pair_questions(&node, &chosen[*g], local))
            .map(|(_, q)| q)
            .collect();
        let req = StateRequest::new(&cfg.model, batch.request.state.clone(), questions);
        store.charge(now_ms, 1, req.questions.0.len() as u32)?;
        requests += 1;
        match classifier.decide(&req).await {
            Ok(decisions) => {
                for ((g, name), d) in batch.keys.iter().zip(decisions) {
                    partial |= d.probability().is_none();
                    let name: &'static str = prompts::name_of(name);
                    answers[*g].insert(name, d);
                }
            }
            Err(_) => partial = true,
        }
    }
    let edges: Vec<Edge> = chosen
        .iter()
        .zip(&answers)
        .flat_map(|(c, a)| pair_edges(&node, c, a, cfg))
        .collect();
    let count = edges.len();
    let outcome = match partial {
        true => EnrichmentState::Partial,
        false => EnrichmentState::Complete,
    };
    store.commit_enrichment(&node.node_id, Some(types), outcome, edges)?;
    store.finish(lease, Outcome::Done)?;
    Ok(Enriched {
        state: outcome,
        requests,
        edges: count,
    })
}
