//! Consolidation (PRD jev-mem §9): after 20 enrichments since the last round, or once pairs
//! have waited 24 hours, a round asks the classifier about a few pairs of neighbouring
//! observations and records what it decided. The counter, the time the oldest pending pair
//! appeared, the cursor and the decisions are all in the snapshot, so a crash loses none of them;
//! with no process running nothing is scheduled, and the next one to open the store runs what
//! came due. Originals are never deleted.

use super::controller::{self, Budget, Failure};
use super::identity;
use super::metrics::Operation;
use super::model::Kind;
use super::prompts::{self, Stage};
use super::queue::JobState;
use super::store::{Refusal, State, Store};
use crate::online::classifier::MemoryClassifier;
use crate::online::request::StateRequest;
use crate::online::response::Decision;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// A round is due after this many enrichments since the last one...
pub const EVERY_ENRICHMENTS: u64 = 20;
/// ...or once a pair has been pending this long.
pub const PENDING_FOR_MS: u64 = 24 * 60 * 60 * 1000;
/// Pairs a round asks about.
pub const MAX_PAIRS: usize = 4;
/// The neighbours of an observation it is paired with: the write's candidates.
const NEIGHBOURS: usize = 4;
/// Questions about one pair: four Nouls and the representation Choice.
const PER_PAIR: usize = 5;

/// Two observations, by id in ascending order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pair {
    pub first: String,
    pub second: String,
}

impl Pair {
    fn new(a: &str, b: &str) -> Self {
        let (first, second) = if a < b { (a, b) } else { (b, a) };
        Pair {
            first: first.into(),
            second: second.into(),
        }
    }

    /// Where the cursor points.
    pub fn key(&self) -> String {
        format!("{}:{}", self.first, self.second)
    }
}

/// The representation Choice; nothing to do with a Git merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationDecision {
    KeepSeparate,
    Merge,
    Promote,
    Uncertain,
}

impl RepresentationDecision {
    fn parse(option: &str) -> Option<Self> {
        Some(match option {
            "keep_separate" => Self::KeepSeparate,
            "merge" => Self::Merge,
            "promote" => Self::Promote,
            "uncertain" => Self::Uncertain,
            _ => return None,
        })
    }
}

/// What a round decided about one pair, recorded only when every answer was valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairDecision {
    pub pair: Pair,
    /// The content hashes of `pair.first` and `pair.second` when it was asked.
    pub hashes: (String, String),
    /// Which of the two was observed later.
    pub newer: String,
    pub redundancy: f64,
    pub contradiction: f64,
    /// That the newer one makes the older one obsolete.
    pub obsolescence: f64,
    pub link_usefulness: f64,
    pub representation: RepresentationDecision,
    /// Of the chosen option, not the Choice's confidence.
    pub probability: f64,
    pub model: String,
    pub prompt_version: String,
}

/// The consolidation state, in the same snapshot as the memories.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Consolidation {
    /// `State::enriched` when the last round ran.
    pub counted: u64,
    /// Since when a pair has been waiting for a round.
    pub pending_since_ms: Option<u64>,
    /// The last pair a round asked about: the next one starts after it.
    pub cursor: Option<String>,
    /// By [`decision_key`]: a pair is decided once per content, model and prompt.
    pub decisions: BTreeMap<String, PairDecision>,
}

impl Consolidation {
    /// Forgets what was decided about nodes that are gone.
    pub(crate) fn drop_nodes(&mut self, gone: &BTreeSet<String>) {
        self.decisions
            .retain(|_, d| !gone.contains(&d.pair.first) && !gone.contains(&d.pair.second));
    }
}

/// `None` when either node is gone.
fn decision_key(state: &State, pair: &Pair, model: &str) -> Option<String> {
    let (a, b) = (
        state.nodes.get(&pair.first)?,
        state.nodes.get(&pair.second)?,
    );
    Some(identity::hash(&[
        "memory/v1/pair",
        &a.node_id,
        &a.content_hash,
        &b.node_id,
        &b.content_hash,
        model,
        prompts::VERSION,
    ]))
}

/// Whether a round is due at `now_ms`. Only a process asks: nothing runs in between.
pub fn due(state: &State, now_ms: u64) -> bool {
    let c = &state.consolidation;
    state.enriched.saturating_sub(c.counted) >= EVERY_ENRICHMENTS
        || c.pending_since_ms
            .is_some_and(|since| now_ms >= since.saturating_add(PENDING_FOR_MS))
}

/// An original observation whose planned enrichment finished.
fn consolidable(state: &State, id: &str) -> bool {
    state
        .nodes
        .get(id)
        .is_some_and(|r| r.kind != Kind::DerivedNote)
        && state
            .jobs
            .get(id)
            .is_some_and(|j| j.state == JobState::Done)
}

/// The pairs not decided yet, in a stable order: each enriched observation with its write
/// candidates (shared entities, then shared words, then the nearest earlier one).
pub fn pending(state: &State, model: &str) -> Vec<Pair> {
    let mut pairs = BTreeSet::new();
    for id in state.nodes.keys().filter(|id| consolidable(state, id)) {
        for c in controller::candidates(state, id, NEIGHBOURS) {
            if consolidable(state, &c) {
                pairs.insert(Pair::new(id, &c));
            }
        }
    }
    pairs
        .into_iter()
        .filter(|p| {
            decision_key(state, p, model)
                .is_some_and(|k| !state.consolidation.decisions.contains_key(&k))
        })
        .collect()
}

/// `pairs` starting after the cursor, then around: the same pairs are not asked every time.
fn after_cursor(pairs: Vec<Pair>, cursor: Option<&str>) -> Vec<Pair> {
    let Some(cursor) = cursor else {
        return pairs;
    };
    let (later, earlier): (Vec<Pair>, Vec<Pair>) =
        pairs.into_iter().partition(|p| p.key().as_str() > cursor);
    later.into_iter().chain(earlier).collect()
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The pinned model.
    pub model: String,
}

impl Config {
    pub fn new(model: &str) -> Self {
        Config {
            model: model.into(),
        }
    }
}

/// What one round did; counts and ids only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Round {
    pub asked: Vec<Pair>,
    pub decided: usize,
    pub requests: usize,
    /// The provider refused the credential (401/403).
    pub auth_failed: bool,
    pub metrics: Operation,
}

/// The decision about one pair from its five answers; `None` unless all of them are valid.
fn decided(
    state: &State,
    pair: &Pair,
    answers: &[Decision],
    model: &str,
) -> Option<(String, PairDecision)> {
    let [
        redundancy,
        contradiction,
        obsolescence,
        link_usefulness,
        representation,
    ] = answers
    else {
        return None;
    };
    let Decision::Choice { selected, .. } = representation else {
        return None;
    };
    let (a, b) = (
        state.nodes.get(&pair.first)?,
        state.nodes.get(&pair.second)?,
    );
    let newer = match a.ingest_seq > b.ingest_seq {
        true => &a.node_id,
        false => &b.node_id,
    };
    let decision = PairDecision {
        pair: pair.clone(),
        hashes: (a.content_hash.clone(), b.content_hash.clone()),
        newer: newer.clone(),
        redundancy: redundancy.probability()?,
        contradiction: contradiction.probability()?,
        obsolescence: obsolescence.probability()?,
        link_usefulness: link_usefulness.probability()?,
        representation: RepresentationDecision::parse(selected)?,
        probability: representation.probability()?,
        model: model.into(),
        prompt_version: prompts::VERSION.into(),
    };
    Some((decision_key(state, pair, model)?, decision))
}

/// One round, when one is due (`None` otherwise). Nothing holds the store's lock while the
/// classifier answers; the result is committed in one new snapshot.
pub async fn round(
    store: &Store,
    classifier: &dyn MemoryClassifier,
    cfg: &Config,
    now_ms: u64,
) -> Result<Option<Round>, Refusal> {
    let state = store.load()?;
    if !due(&state, now_ms) {
        return Ok(None);
    }
    let asked: Vec<Pair> = after_cursor(
        pending(&state, &cfg.model),
        state.consolidation.cursor.as_deref(),
    )
    .into_iter()
    .take(MAX_PAIRS)
    .collect();
    let mut round = Round {
        asked: asked.clone(),
        ..Round::default()
    };
    let mut decisions = Vec::new();
    // The cursor moves on once the pairs were asked, whatever the answers were.
    let mut cursor = None;
    if !asked.is_empty() {
        let items: Vec<_> = asked
            .iter()
            .map(|p| {
                let (a, b) = (&state.nodes[&p.first], &state.nodes[&p.second]);
                let (older, newer) = if a.ingest_seq > b.ingest_seq {
                    (b, a)
                } else {
                    (a, b)
                };
                json!({"newer": controller::item(newer), "older": controller::item(older)})
            })
            .collect();
        let questions = (0..asked.len())
            .flat_map(|i| prompts::questions(Stage::Consolidation, i))
            .map(|(_, q)| q)
            .collect();
        let req = StateRequest::new(&cfg.model, json!({"pairs": items}), questions);
        let mut budget = Budget {
            attempts_left: controller::MAX_ATTEMPTS,
            deadline: tokio::time::Instant::now() + controller::RUN_DEADLINE,
            sent: 0,
        };
        match controller::send(
            store,
            classifier,
            &req,
            now_ms,
            &mut budget,
            &mut round.metrics,
        )
        .await
        {
            Ok(answers) => {
                cursor = asked.last().map(Pair::key);
                decisions = asked
                    .iter()
                    .zip(answers.chunks(PER_PAIR))
                    .filter_map(|(p, a)| decided(&state, p, a, &cfg.model))
                    .collect();
            }
            Err(failure) => {
                round.auth_failed = matches!(failure, Failure::Auth);
                if !matches!(failure, Failure::Quota | Failure::Busy) {
                    cursor = asked.last().map(Pair::key);
                }
            }
        }
        round.requests = budget.sent;
    }
    round.decided = store.commit_round(state.enriched, cursor, decisions, &cfg.model, now_ms)?;
    Ok(Some(round))
}
