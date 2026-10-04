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

// ---------------------------------------------------------------- the read (§10, steps 1–8)

use super::index;
use super::model::{Edge, Record};
use super::prompts::{self, Stage};
use super::store::State;
use crate::online::classifier::MemoryClassifier;
use crate::online::reader::WorkspaceReader;
use crate::online::request::StateRequest;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

/// Expansions in all; the same twelve [`route`] splits among the views.
pub const MAX_EXPANSIONS: usize = EXPANSIONS;
/// Nodes examined and scored in a read.
pub const MAX_VISITED: usize = 16;
/// Edges looked at, kept or not.
pub const MAX_EDGES_SEEN: usize = 128;
/// Candidates each scoring request carries.
pub const SCORING_BATCH: usize = 8;
/// The selected memories expansion starts from.
pub const BEAM: usize = 4;
/// A candidate needs at least this relevance, and this score.
pub const ENTRY: f64 = 0.60;

/// What a broker started with `--memory` reads with.
#[derive(Clone)]
pub struct ReadSetup {
    pub store: Arc<super::store::Store>,
    pub classifier: Arc<dyn MemoryClassifier>,
    pub cfg: ReadConfig,
}

impl std::fmt::Debug for ReadSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadSetup")
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

/// How a read chooses (`--memory-selection`, D-141). `Deterministic` is the evaluation's
/// control: the local anchors alone, no classifier, and an identity of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    Jev,
    Deterministic,
}

#[derive(Debug, Clone)]
pub struct ReadConfig {
    pub model: String,
    pub deadline: Duration,
    /// Requests a read may send (`--memory-read-request-limit`); never retried.
    pub request_limit: usize,
    /// Each request gets at most this, and never more than the time left.
    pub attempt_timeout: Duration,
    /// Questions a read may ask in all (PRD jev-mem §8.3: 6 + 2 × 32 + 4).
    pub max_questions: usize,
    /// The MCP call's cancellation.
    pub cancel: Option<tokio_util::sync::CancellationToken>,
    pub selection: Selection,
}

impl Default for ReadConfig {
    fn default() -> Self {
        Self {
            model: "jev-1.13.0".into(),
            deadline: Duration::from_millis(750),
            request_limit: 4,
            attempt_timeout: Duration::from_millis(250),
            max_questions: 74,
            cancel: None,
            selection: Selection::Jev,
        }
    }
}

/// Why a read ended (PRD jev-mem §10). Only `Sufficient` and `LowExpectedGain` are the
/// classifier's judgement; the rest are limits or failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Sufficient,
    LowExpectedGain,
    Empty,
    Deadline,
    RequestLimit,
    QuestionLimit,
    GraphLimit,
    Cancelled,
    ProviderError,
    BudgetOmitted,
    /// `--memory-selection deterministic`: the local ranking chose, no classifier judged.
    Deterministic,
}

/// A memory the read selected, with how it got there.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub record: Record,
    pub score: f64,
    /// The four scoring answers: relevance, new information, relation usefulness, support.
    /// `None` when no classifier scored it (`--memory-selection deterministic`).
    pub scores: Option<[f64; 4]>,
    /// The edge that reached it; `None` for an anchor.
    pub via: Option<Edge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    /// Best first.
    pub memories: Vec<Found>,
    pub stop: StopReason,
    pub requests: usize,
    pub questions: usize,
    /// Nodes scored.
    pub visited: usize,
    pub expansions: usize,
    pub edges_seen: usize,
    /// An answer the read depended on was unknown.
    pub partial: bool,
    /// No request was allowed (`--memory-read-request-limit 0`): only a cache could serve.
    pub degraded: bool,
    /// Memories left out because a source changed since they were observed.
    pub stale_omitted: usize,
    /// Observations in the spool, not incorporated yet: not awaited.
    pub pending_writes: usize,
    /// How it chose: what its memories' `basis` says.
    pub selection: Selection,
}

/// A candidate to score: the node and, after expansion, the edge that reached it.
struct Candidate {
    record: Record,
    anchor: f64,
    via: Option<Edge>,
}

fn candidate_item(c: &Candidate) -> Value {
    let mut item = json!({"id": c.record.node_id, "content": c.record.content});
    if let Some(e) = &c.via {
        item["relation"] = json!({
            "graph": e.graph, "relation": e.relation, "source": e.source, "target": e.target,
        });
    }
    item
}

/// Weighted sum of PRD jev-mem §10.6: a product decision, not the paper's Eq. 23.
fn weighted(s: [f64; 4], anchor: f64) -> f64 {
    0.40 * s[0] + 0.20 * s[1] + 0.15 * s[2] + 0.15 * s[3] + 0.10 * anchor
}

struct Run<'a> {
    classifier: &'a dyn MemoryClassifier,
    cfg: &'a ReadConfig,
    until: tokio::time::Instant,
    requests: usize,
    questions: usize,
    partial: bool,
}

impl Run<'_> {
    async fn ask(
        &mut self,
        state: Value,
        questions: Vec<crate::online::request::JevQuestion>,
    ) -> Result<Vec<Decision>, StopReason> {
        let cancel = self.cfg.cancel.clone().unwrap_or_default();
        if cancel.is_cancelled() {
            return Err(StopReason::Cancelled);
        }
        if self.requests >= self.cfg.request_limit {
            return Err(StopReason::RequestLimit);
        }
        if self.questions + questions.len() > self.cfg.max_questions {
            return Err(StopReason::QuestionLimit);
        }
        let left = self
            .until
            .saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return Err(StopReason::Deadline);
        }
        let left = left.min(self.cfg.attempt_timeout);
        let req = StateRequest::new(&self.cfg.model, state, questions);
        self.requests += 1;
        self.questions += req.questions.0.len();
        // No retry on the read path: a failure ends the read and keeps what it validated.
        tokio::select! {
            _ = cancel.cancelled() => Err(StopReason::Cancelled),
            answer = tokio::time::timeout(left, self.classifier.decide(&req)) => match answer {
                Err(_) => Err(StopReason::Deadline),
                Ok(Err(_)) => Err(StopReason::ProviderError),
                Ok(Ok(d)) => Ok(d),
            },
        }
    }

    /// Scores `batch`; the ones that pass the gate, with their score.
    async fn score(
        &mut self,
        query: &str,
        evidence: &[Found],
        batch: Vec<Candidate>,
    ) -> Result<Vec<Found>, StopReason> {
        let state = json!({
            "query": query,
            "evidence": evidence.iter().map(|f| &f.record.content).collect::<Vec<_>>(),
            "candidates": batch.iter().map(candidate_item).collect::<Vec<_>>(),
        });
        let questions = (0..batch.len())
            .flat_map(|i| {
                prompts::questions(Stage::Scoring, i)
                    .into_iter()
                    .map(|(_, q)| q)
            })
            .collect();
        let decisions = self.ask(state, questions).await?;
        let mut out = vec![];
        for (c, d) in batch.into_iter().zip(decisions.chunks(4)) {
            let known: Vec<f64> = d.iter().filter_map(Decision::probability).collect();
            let [rel, new, use_, sup] = known[..] else {
                self.partial = true;
                continue;
            };
            let scores = [rel, new, use_, sup];
            let score = weighted(scores, c.anchor);
            if rel >= ENTRY && score >= ENTRY {
                out.push(Found {
                    record: c.record,
                    score,
                    scores: Some(scores),
                    via: c.via,
                });
            }
        }
        Ok(out)
    }
}

/// Best first; with `recency`, a tie goes to the later observation; then the id.
fn rank(found: &mut [Found], recency: bool) {
    found.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| match recency {
                true => b.record.ingest_seq.cmp(&a.record.ingest_seq),
                false => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.record.node_id.cmp(&b.record.node_id))
    });
}

/// Reads memory for `query` (PRD jev-mem §10), over the snapshot alone: no source is checked.
pub async fn read(
    state: &State,
    query: &str,
    classifier: &dyn MemoryClassifier,
    cfg: &ReadConfig,
) -> Read {
    read_with(
        Arc::new(state.clone()),
        query,
        classifier,
        cfg,
        Local::Inline,
    )
    .await
}

/// Where a read's local work runs: the ranking of the anchors and the check of each memory's
/// sources (PRD jev-mem §8.2, blocking I/O in a bounded executor).
#[derive(Clone)]
enum Local {
    /// On the snapshot alone, in place: nothing is read from disk.
    Inline,
    /// Off the async threads, under the read's deadline, checking sources in this workspace.
    Disk(Arc<WorkspaceReader>),
}

impl Local {
    /// `work`, or `None` once `until` passes: on the disk, in the blocking pool, so that neither
    /// the async thread nor the structural answer beside it waits on it.
    async fn run<T: Send + 'static>(
        &self,
        until: tokio::time::Instant,
        work: impl FnOnce(&Local) -> T + Send + 'static,
    ) -> Option<T> {
        match self {
            Local::Inline => Some(work(self)),
            Local::Disk(_) => {
                let local = self.clone();
                let blocking = tokio::task::spawn_blocking(move || work(&local));
                tokio::time::timeout_at(until, blocking).await.ok()?.ok()
            }
        }
    }

    /// Whether every source of `r` still has the bytes it was observed with, and is eligible.
    fn fresh(&self, r: &Record) -> bool {
        match self {
            Local::Inline => true,
            Local::Disk(reader) => r.sources.iter().all(|s| {
                reader
                    .snapshot(&s.path)
                    .is_ok_and(|snap| snap.content_hash == s.sha256)
            }),
        }
    }

    /// `items` whose memories are fresh, and how many were not; `None` once `until` passes. Only
    /// what a read is about to use is checked, never the whole store.
    async fn keep_fresh<T: Send + 'static>(
        &self,
        items: Vec<T>,
        record: fn(&T) -> &Record,
        until: tokio::time::Instant,
    ) -> Option<(Vec<T>, usize)> {
        let stop = until.into_std();
        self.run(until, move |local| {
            let mut kept = Vec::with_capacity(items.len());
            let mut stale = 0;
            for item in items {
                if matches!(local, Local::Disk(_)) && std::time::Instant::now() >= stop {
                    return None;
                }
                match local.fresh(record(&item)) {
                    true => kept.push(item),
                    false => stale += 1,
                }
            }
            Some((kept, stale))
        })
        .await
        .flatten()
    }
}

/// [`read`], with the memories whose sources are no longer what they were left out (step 1):
/// history, not current advice. Every local step counts against the deadline.
async fn read_with(
    state: Arc<State>,
    query: &str,
    classifier: &dyn MemoryClassifier,
    cfg: &ReadConfig,
    local: Local,
) -> Read {
    let until = tokio::time::Instant::now() + cfg.deadline;
    let mut run = Run {
        classifier,
        cfg,
        until,
        requests: 0,
        questions: 0,
        partial: false,
    };
    let mut out = Read {
        memories: vec![],
        stop: StopReason::Empty,
        requests: 0,
        questions: 0,
        visited: 0,
        expansions: 0,
        edges_seen: 0,
        partial: false,
        degraded: false,
        stale_omitted: 0,
        pending_writes: 0,
        selection: cfg.selection,
    };
    let (ranked, q) = (state.clone(), query.to_string());
    let Some(anchors) = local
        .run(until, move |_| {
            index::anchors(&ranked, &q)
                .into_iter()
                .filter_map(|(id, rrf)| ranked.nodes.get(&id).map(|r| (r.clone(), rrf)))
                .collect::<Vec<_>>()
        })
        .await
    else {
        out.stop = StopReason::Deadline;
        return finish(out, run);
    };
    let Some((anchors, stale)) = local.keep_fresh(anchors, |(r, _)| r, until).await else {
        out.stop = StopReason::Deadline;
        return finish(out, run);
    };
    if cfg.selection == Selection::Deterministic {
        // The evaluation's control: the fresh anchors in their local order, nothing asked.
        out.stale_omitted = stale;
        out.memories = anchors
            .into_iter()
            .map(|(record, rrf)| Found {
                record,
                score: rrf,
                scores: None,
                via: None,
            })
            .collect();
        out.stop = StopReason::Deterministic;
        return finish(out, run);
    }
    out.stale_omitted += stale;
    if anchors.is_empty() {
        return out;
    }
    // There is no decision cache yet: with no request allowed, nothing can be validated.
    if cfg.request_limit == 0 {
        out.degraded = true;
        out.stop = StopReason::RequestLimit;
        return out;
    }

    // 1. Routing, on the query alone.
    let routing = prompts::questions(Stage::Routing, 0);
    let questions = routing.iter().map(|(_, q)| q.clone()).collect();
    let answers = match run.ask(json!({"query": query}), questions).await {
        Ok(a) => a,
        Err(reason) => {
            out.stop = reason;
            return finish(out, run);
        }
    };
    let by_name: BTreeMap<&str, Decision> = routing.iter().map(|(n, _)| *n).zip(answers).collect();
    let route = route(&by_name);
    run.partial |= route.partial;

    // 2. The anchors, scored.
    let best = anchors.first().map_or(1.0, |a| a.1).max(f64::MIN_POSITIVE);
    let batch: Vec<Candidate> = anchors
        .into_iter()
        .map(|(record, rrf)| Candidate {
            record,
            anchor: rrf / best,
            via: None,
        })
        .collect();
    let mut visited: BTreeSet<String> = batch.iter().map(|c| c.record.node_id.clone()).collect();
    out.visited = visited.len();
    let mut selected = match run.score(query, &[], batch).await {
        Ok(s) => s,
        Err(reason) => {
            out.stop = reason;
            return finish(out, run);
        }
    };
    rank(&mut selected, route.recency);

    // 3. Expansion from the beam, through the active views, under every limit.
    let mut left: BTreeMap<Graph, usize> = route.budget.iter().copied().collect();
    let mut frontier: Vec<String> = selected
        .iter()
        .take(BEAM)
        .map(|f| f.record.node_id.clone())
        .collect();
    let mut queued: Vec<Candidate> = vec![];
    let mut limited = false;
    'depth: for _ in 0..route.depth {
        let mut next = vec![];
        for from in &frontier {
            for edge in state.edges.values() {
                let other = match (edge.source == *from, edge.target == *from) {
                    (true, _) => edge.target.clone(),
                    (false, true) if matches!(edge.graph, Graph::Semantic | Graph::Entity) => {
                        edge.source.clone()
                    }
                    (false, true) => {
                        out.edges_seen += 1;
                        continue;
                    }
                    _ => continue,
                };
                out.edges_seen += 1;
                if out.edges_seen > MAX_EDGES_SEEN {
                    out.edges_seen = MAX_EDGES_SEEN;
                    limited = true;
                    break 'depth;
                }
                // A view that is off has no budget, and its edges are not followed.
                let Some(budget) = left.get_mut(&edge.graph) else {
                    continue;
                };
                if visited.contains(&other) {
                    continue;
                }
                if *budget == 0
                    || out.expansions >= MAX_EXPANSIONS
                    || visited.len() >= MAX_VISITED
                    || queued.len() >= SCORING_BATCH
                {
                    limited = true;
                    continue;
                }
                let Some(record) = state.nodes.get(&other) else {
                    continue;
                };
                *budget -= 1;
                out.expansions += 1;
                visited.insert(other.clone());
                next.push((other.clone(), edge.score.unwrap_or(0.0)));
                queued.push(Candidate {
                    record: record.clone(),
                    anchor: 0.0,
                    via: Some(edge.clone()),
                });
            }
        }
        // The next depth starts from a beam too, never from every node just reached: the ones
        // reached by the strongest relations, a tie by id.
        next.sort_by(|a: &(String, f64), b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        next.truncate(BEAM);
        frontier = next.into_iter().map(|(id, _)| id).collect();
    }
    out.visited = visited.len();
    // The last request goes to stopping: an expansion that would take it waits.
    if !queued.is_empty() && run.requests + 2 > cfg.request_limit {
        queued.clear();
        limited = true;
    }
    let Some((queued, stale)) = local.keep_fresh(queued, |c| &c.record, until).await else {
        out.stop = StopReason::Deadline;
        out.memories = selected;
        return finish(out, run);
    };
    out.stale_omitted += stale;
    if !queued.is_empty() {
        let more = match run.score(query, &selected, queued).await {
            Ok(m) => m,
            Err(reason) => {
                out.stop = reason;
                out.memories = selected;
                return finish(out, run);
            }
        };
        selected.extend(more);
        rank(&mut selected, route.recency);
    }
    out.memories = selected;

    // 4. Stopping, on the evidence gathered.
    let stopping = prompts::questions(Stage::Stopping, 0);
    let evidence: Vec<&String> = out.memories.iter().map(|f| &f.record.content).collect();
    let state_json = json!({"query": query, "evidence": evidence, "depth": route.depth});
    let answers = run
        .ask(
            state_json,
            stopping.iter().map(|(_, q)| q.clone()).collect(),
        )
        .await;
    let by_name: BTreeMap<&str, Decision> = match answers {
        Ok(a) => stopping.iter().map(|(n, _)| *n).zip(a).collect(),
        Err(reason) => {
            out.stop = reason;
            return finish(out, run);
        }
    };
    run.partial |= by_name.values().any(|d| d.probability().is_none());
    let p = |n: &str| by_name.get(n).and_then(Decision::probability);
    out.stop = match (
        p("evidence_sufficient"),
        p("missing_evidence"),
        p("contradiction"),
        p("continue_useful"),
    ) {
        (Some(s), Some(m), Some(c), _) if s >= 0.95 && m < 0.15 && c < 0.15 => {
            StopReason::Sufficient
        }
        (.., Some(g)) if g < 0.15 => StopReason::LowExpectedGain,
        _ if limited => StopReason::GraphLimit,
        _ => StopReason::RequestLimit,
    };
    finish(out, run)
}

fn finish(mut out: Read, run: Run<'_>) -> Read {
    out.requests = run.requests;
    out.questions = run.questions;
    out.partial |= run.partial;
    out
}

/// How a query's `--jev-request-limit` is shared (PRD jev-mem §8.2): memory takes at most four
/// slots, never more than its own limit nor than there are; discovery keeps the rest.
pub fn slots(read_limit: usize, jev_request_limit: usize) -> (usize, usize) {
    let memory = read_limit.min(4).min(jev_request_limit);
    (memory, jev_request_limit - memory)
}

/// A read of `state` that leaves out memories whose sources changed since they were observed,
/// checked off the async threads and only for what the read is about to use.
pub async fn read_fresh(
    state: Arc<State>,
    reader: Arc<WorkspaceReader>,
    query: &str,
    classifier: &dyn MemoryClassifier,
    cfg: &ReadConfig,
) -> Read {
    read_with(state, query, classifier, cfg, Local::Disk(reader)).await
}

/// Step 9, right before delivery: keeps the memories whose node is still in `now` in the same
/// generation and whose sources are unchanged; the stale ones are counted. `false` when the
/// sources could not all be checked by `until`: then nothing read can go out.
pub async fn revalidate(
    read: &mut Read,
    now: &State,
    reader: Arc<WorkspaceReader>,
    until: tokio::time::Instant,
) -> bool {
    read.memories.retain(|f| {
        now.nodes
            .get(&f.record.node_id)
            .is_some_and(|n| n.generation == f.record.generation)
    });
    let found = std::mem::take(&mut read.memories);
    match Local::Disk(reader)
        .keep_fresh(found, |f| &f.record, until)
        .await
    {
        Some((kept, stale)) => {
            read.memories = kept;
            read.stale_omitted += stale;
            true
        }
        None => false,
    }
}

/// The memories of `read` as the envelope carries them (PRD jev-mem §11).
pub fn items(read: &Read) -> Vec<crate::model::MemoryItem> {
    use crate::model::{MemoryItem, MemoryScores, MemorySource, Untrusted};
    let snake = |v: serde_json::Value| v.as_str().unwrap_or_default().to_string();
    read.memories
        .iter()
        .map(|f| MemoryItem {
            id: f.record.node_id.clone(),
            text: Untrusted {
                untrusted_repository_data: f.record.content.clone(),
            },
            kind: snake(json!(f.record.kind)),
            sources: f
                .record
                .sources
                .iter()
                .map(|s| MemorySource {
                    path: s.path.clone(),
                    sha256: s.sha256.clone(),
                })
                .collect(),
            observed_at_ms: f.record.observed_at_ms,
            time_basis: snake(json!(f.record.timestamp_role)),
            basis: match read.selection {
                Selection::Jev => "jev_scored",
                Selection::Deterministic => "deterministic_rank",
            },
            derived_from: f
                .record
                .derived_from
                .iter()
                .map(|p| p.node_id.clone())
                .collect(),
            stale: false,
            why_included: match &f.via {
                None => "matched the task's words or files".into(),
                Some(e) => format!(
                    "related by {} to a memory that matched the task",
                    snake(json!(e.graph))
                ),
            },
            scores: f.scores.map(|s| MemoryScores {
                relevance: s[0],
                new_information: s[1],
                relation_usefulness: s[2],
                supports_current_evidence: s[3],
                score: f.score,
            }),
        })
        .collect()
}

/// The schema of `provenance.memory`.
pub const MEMORY_SCHEMA: &str = "ripwire-broker.memory/v1";

/// `provenance.memory` for `read`.
pub fn provenance(read: &Read) -> crate::model::MemoryProvenance {
    crate::model::MemoryProvenance {
        schema_version: MEMORY_SCHEMA,
        stop_reason: json!(read.stop).as_str().unwrap_or_default().to_string(),
        requests: read.requests,
        questions: read.questions,
        visited: read.visited,
        stale_omitted: read.stale_omitted,
        pending_writes: read.pending_writes,
        partial: read.partial,
        degraded: read.degraded,
        assessment_before_truncation: false,
    }
}

/// Puts `read` into `env` (PRD jev-mem §10.9): what fits memory's share of the budget and was not
/// delivered to this session before, with `provenance.memory`. A sufficiency judged on memories the
/// budget then cut is marked; a read whose memories all gave way is `budget_omitted`. Returns the
/// ids delivered, for the session to remember.
pub fn attach(
    env: &mut crate::model::Envelope,
    read: &Read,
    seen: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut p = provenance(read);
    // In place while memories are fitted, so the room they take is room the record leaves.
    env.provenance.memory = Some(p.clone());
    let fit = crate::budget::add_memories(env, items(read), seen);
    if fit.omitted > 0
        && matches!(
            read.stop,
            StopReason::Sufficient | StopReason::LowExpectedGain
        )
    {
        p.assessment_before_truncation = true;
    }
    if fit.delivered.is_empty() && fit.omitted > 0 {
        p.stop_reason = json!(StopReason::BudgetOmitted)
            .as_str()
            .unwrap_or_default()
            .into();
    }
    env.provenance.memory = Some(p);
    fit.delivered
}
