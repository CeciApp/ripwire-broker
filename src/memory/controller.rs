//! The worker's enrichment of one node (PRD jev-mem §7, §8.1): typing, a bounded and
//! deterministic set of candidates, and the relations between the node and each of them. The
//! node exists before any inference; nothing here holds the store's lock while waiting on the
//! network, and every request is charged to the 24-hour quota before it is sent.

use super::model::{Edge, EdgeBasis, EnrichmentState, Graph, POLICY_VERSION, Record, Types};
use super::prompts::{self, Stage};
use super::queue::{Lease, Outcome};
use super::store::{Refusal, State, Store};
use super::wire::{self, Group};
use crate::online::classifier::{ClassifyError, MemoryClassifier};
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
    /// The provider refused the credential (401/403).
    pub auth_failed: bool,
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
    // A pair commits whole (PRD jev-mem §12): one decision missing, and none of its inferences
    // count. The deterministic edge above stands either way.
    if answers.is_empty() || answers.values().any(|d| d.probability().is_none()) {
        return out;
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

/// Classifier attempts per job, retries and splits included (PRD jev-mem §8.2).
pub const MAX_ATTEMPTS: u32 = 4;
/// The run's deadline; a 429 only waits inside it.
pub const RUN_DEADLINE: std::time::Duration = std::time::Duration::from_millis(5_000);

/// Why a request got no decisions.
enum Failure {
    /// 401/403: the worker stops until reauthorized.
    Auth,
    /// A 429 whose wait does not fit the run: try again no sooner than this many ms.
    Cooldown(u64),
    /// Out of attempts, of quota, or a failure not worth retrying.
    GaveUp,
}

struct Budget {
    attempts_left: u32,
    deadline: tokio::time::Instant,
    sent: usize,
}

/// One request, with at most one retry: for a transient failure, or a 429 whose wait fits the
/// run. Every attempt is charged to the 24-hour quota first.
async fn send(
    store: &Store,
    classifier: &dyn MemoryClassifier,
    req: &StateRequest,
    now_ms: u64,
    budget: &mut Budget,
) -> Result<Vec<Decision>, Failure> {
    let mut retried = false;
    loop {
        if budget.attempts_left == 0 {
            return Err(Failure::GaveUp);
        }
        if store
            .charge(now_ms, 1, req.questions.0.len() as u32)
            .is_err()
        {
            return Err(Failure::GaveUp);
        }
        budget.attempts_left -= 1;
        budget.sent += 1;
        let error = match classifier.decide(req).await {
            Ok(d) => return Ok(d),
            Err(e) => e,
        };
        match error {
            ClassifyError::Auth(_) => return Err(Failure::Auth),
            ClassifyError::RateLimited { retry_after } => {
                let wait = retry_after
                    .as_deref()
                    .and_then(|v| {
                        crate::online::retry_after::parse(v, std::time::SystemTime::now())
                    })
                    .unwrap_or(RUN_DEADLINE);
                let fits = tokio::time::Instant::now() + wait <= budget.deadline;
                if retried || budget.attempts_left == 0 || !fits {
                    return Err(Failure::Cooldown(wait.as_millis() as u64));
                }
                tokio::time::sleep(wait).await;
            }
            e if e.is_transient() && !retried && budget.attempts_left > 0 => {}
            _ => return Err(Failure::GaveUp),
        }
        retried = true;
    }
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
            auth_failed: false,
        });
    };
    let mut budget = Budget {
        attempts_left: MAX_ATTEMPTS,
        deadline: tokio::time::Instant::now() + RUN_DEADLINE,
        sent: 0,
    };
    let mut partial = false;

    // Typing: four Nouls on the observation alone. Without it the run failed.
    let typing = StateRequest::new(
        &cfg.model,
        json!({"observation": node.content}),
        prompts::questions(Stage::Typing, 0)
            .into_iter()
            .map(|(_, q)| q)
            .collect(),
    );
    let types = match send(store, classifier, &typing, now_ms, &mut budget).await {
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
        Err(failure) => {
            let (wait, auth_failed) = match failure {
                Failure::Auth => (RETRY_AFTER_MS, true),
                Failure::Cooldown(ms) => (ms, false),
                Failure::GaveUp => (RETRY_AFTER_MS, false),
            };
            store.commit_enrichment(
                &node.node_id,
                node.generation,
                None,
                EnrichmentState::Failed,
                vec![],
            )?;
            store.finish(
                lease,
                Outcome::Retry {
                    not_before_ms: now_ms + wait,
                },
            )?;
            return Ok(Enriched {
                state: EnrichmentState::Failed,
                requests: budget.sent,
                edges: 0,
                auth_failed,
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
    let mut auth_failed = false;
    let batches = wire::batches(&cfg.model, &base, "candidates", groups).unwrap_or_default();
    for batch in batches {
        if auth_failed {
            partial = true;
            break;
        }
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
        match send(store, classifier, &req, now_ms, &mut budget).await {
            Ok(decisions) => {
                for ((g, name), d) in batch.keys.iter().zip(decisions) {
                    partial |= d.probability().is_none();
                    answers[*g].insert(prompts::name_of(name), d);
                }
            }
            Err(Failure::Auth) => auth_failed = true,
            Err(_) => partial = true,
        }
    }
    let edges: Vec<Edge> = chosen
        .iter()
        .zip(&answers)
        .flat_map(|(c, a)| pair_edges(&node, c, a, cfg))
        .collect();
    let count = edges.len();
    let outcome = match partial || auth_failed {
        true => EnrichmentState::Partial,
        false => EnrichmentState::Complete,
    };
    store.commit_enrichment(&node.node_id, node.generation, Some(types), outcome, edges)?;
    store.finish(lease, Outcome::Done)?;
    Ok(Enriched {
        state: outcome,
        requests: budget.sent,
        edges: count,
        auth_failed,
    })
}

/// The process's memory worker (PRD jev-mem §8.1): one job at a time, and nothing more sent
/// after the provider refused the credential, until `reauthorize` (a new process is one).
pub struct Worker {
    store: std::sync::Arc<Store>,
    classifier: std::sync::Arc<dyn MemoryClassifier>,
    cfg: Config,
    suspended: std::sync::atomic::AtomicBool,
}

impl Worker {
    pub fn new(
        store: std::sync::Arc<Store>,
        classifier: std::sync::Arc<dyn MemoryClassifier>,
        cfg: Config,
    ) -> Self {
        Self {
            store,
            classifier,
            cfg,
            suspended: Default::default(),
        }
    }

    /// Runs the next ready job; `None` when there is none or the worker is suspended.
    pub async fn run_once(&self, now_ms: u64) -> Result<Option<Enriched>, Refusal> {
        if self.is_suspended() {
            return Ok(None);
        }
        let Some(_slot) = self.store.remote_slot()? else {
            return Ok(None);
        };
        let Some(lease) = self.store.lease_next(now_ms)? else {
            return Ok(None);
        };
        let ran = enrich(&self.store, &*self.classifier, lease, &self.cfg, now_ms).await?;
        if ran.auth_failed {
            self.suspended
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(Some(ran))
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn reauthorize(&self) {
        self.suspended
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}
