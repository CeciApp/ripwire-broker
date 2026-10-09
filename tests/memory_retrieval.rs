//! Seam: reading memory back (PRD jev-mem §10): the local index, routing, scoring, stopping and
//! what `context_for_task` gets. A classifier stand-in, no network.
use ripwire_broker::memory::index;
use ripwire_broker::memory::model::Record;
use ripwire_broker::memory::store::State;
use serde_json::json;

fn id(n: u64) -> String {
    format!("{n:064}")
}

fn rec(n: u64, content: &str, paths: &[&str]) -> Record {
    serde_json::from_value(json!({
        "schema_version": 1, "policy_version": "memory-policy/v1",
        "node_id": id(n), "content_hash": id(n), "workspace_id": "w", "event_key": "e",
        "kind": "edit_observation", "content": content, "observed_at_ms": n, "ingest_seq": n,
        "timestamp_role": "observation",
        "entities": paths.iter().map(|p| json!({"id": format!("e:{p}"), "kind": "file", "path": p})).collect::<Vec<_>>(),
        "expires_at_ms": u64::MAX, "generation": 1
    }))
    .unwrap()
}

fn state(records: Vec<Record>) -> State {
    let mut s = State::default();
    for r in records {
        s.nodes.insert(r.node_id.clone(), r);
    }
    s
}

#[test]
fn tokens_are_unicode_alphanumeric_lowercase_without_stemming() {
    assert_eq!(
        index::tokens("Caching cachés Café_42 src/cache.rs"),
        ["caching", "cachés", "café", "42", "src", "cache", "rs"],
        "split on anything not alphanumeric, lowercased, nothing stemmed"
    );
    assert!(index::tokens("  --- ").is_empty());
}

fn three() -> State {
    state(vec![
        rec(1, "cache eviction policy", &["src/cache.rs"]),
        rec(2, "cache warmup", &["src/warm.rs"]),
        rec(3, "router table", &["src/router.rs"]),
    ])
}

#[test]
fn lexical_rank_is_the_sum_of_idf_over_shared_terms() {
    let ranked = index::lexical_rank(&three(), "cache eviction in src/router.rs");
    let ln = f64::ln;
    // N = 3; df(cache) = 2, df(eviction) = df(router) = 1; idf(t) = ln(1 + N / (1 + df)).
    let expected = [
        (id(1), ln(2.0) + ln(2.5)),
        (id(3), ln(2.5)),
        (id(2), ln(2.0)),
    ];
    assert_eq!(ranked.len(), 3);
    for ((got, score), (want, value)) in ranked.iter().zip(expected) {
        assert_eq!(got, &want);
        assert!((score - value).abs() < 1e-12, "{score} vs {value}");
    }
    let twins = state(vec![rec(5, "cache", &[]), rec(4, "cache", &[])]);
    let ids: Vec<String> = index::lexical_rank(&twins, "cache")
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ids, [id(4), id(5)], "a tie goes by id");
}

#[test]
fn rrf_fuses_lexical_and_entity_ranks_into_at_most_eight_anchors() {
    let anchors = index::anchors(&three(), "cache eviction in src/router.rs");
    // Lexical ranks: 1, 3, 2 (ids 1, 3, 2). Entity ranks: only id 3, first. RRF k = 60.
    let expected = [
        (id(3), 1.0 / 62.0 + 1.0 / 61.0),
        (id(1), 1.0 / 61.0),
        (id(2), 1.0 / 63.0),
    ];
    assert_eq!(anchors.len(), 3);
    for ((got, score), (want, value)) in anchors.iter().zip(expected) {
        assert_eq!(got, &want);
        assert!((score - value).abs() < 1e-12, "{score} vs {value}");
    }
    let many = state((1..=12).map(|n| rec(n, "cache", &[])).collect());
    assert_eq!(index::anchors(&many, "cache").len(), index::MAX_ANCHORS);
    assert_eq!(index::MAX_ANCHORS, 8);
    assert!(index::anchors(&three(), "nothing matches").is_empty());
}

#[test]
fn an_entity_is_named_only_by_the_whole_path() {
    let s = state(vec![rec(1, "x", &["a.rs"]), rec(2, "y", &["src/lib.rs"])]);
    let named = |q: &str| -> Vec<String> {
        index::entity_rank(&s, q)
            .into_iter()
            .map(|(i, _)| i)
            .collect()
    };
    assert!(
        named("what changed in src/data.rs?").is_empty(),
        "a.rs is not data.rs"
    );
    assert!(
        named("see src/lib.rs.orig").is_empty(),
        "nor is a longer path"
    );
    assert_eq!(
        named("see src/lib.rs."),
        [id(2)],
        "trailing punctuation is not part of it"
    );
    assert_eq!(named("in `src/lib.rs`, and (a.rs)"), [id(1), id(2)]);
}

// ---------------------------------------------------------------- routing (PRD jev-mem §10, step 4; T3.2)

use ripwire_broker::memory::model::Graph;
use ripwire_broker::memory::retrieve::{self, Route};
use ripwire_broker::online::response::{Decision, Unknown};
use std::collections::BTreeMap;

fn p(v: f64) -> Decision {
    Decision::Noul { probability: v }
}

fn unknown() -> Decision {
    Decision::Unknown {
        reason: Unknown::Absent,
    }
}

fn routed(answers: &[(&'static str, Decision)]) -> Route {
    let map: BTreeMap<&str, Decision> = answers.iter().cloned().collect();
    retrieve::route(&map)
}

#[test]
fn views_activate_at_010_and_unknown_never_activates() {
    let r = routed(&[
        ("semantic", p(0.10)),
        ("temporal", p(0.0999)),
        ("causal", unknown()),
        ("entity", p(0.5)),
        ("multi_hop_need", p(0.2)),
        ("recency_importance", p(0.1)),
    ]);
    let views: Vec<Graph> = r.budget.iter().map(|(g, _)| *g).collect();
    assert_eq!(views, [Graph::Semantic, Graph::Entity]);
    assert!(routed(&[]).budget.is_empty(), "no answer, no view");
}

#[test]
fn the_budget_of_twelve_is_split_by_largest_remainder_in_fixed_tie_order() {
    assert_eq!(retrieve::EXPANSIONS, 12);
    // One each, then 8 by need: 2.29, 2.29, 2.29, 1.14 → floors 2, 2, 2, 1 and one left over,
    // which three equal remainders tie for: the fixed order gives it to semantic.
    let r = routed(&[
        ("entity", p(0.5)),
        ("causal", p(1.0)),
        ("temporal", p(1.0)),
        ("semantic", p(1.0)),
    ]);
    assert_eq!(
        r.budget,
        [
            (Graph::Semantic, 4),
            (Graph::Temporal, 3),
            (Graph::Causal, 3),
            (Graph::Entity, 2)
        ]
    );
    let r = routed(&[("semantic", p(0.3)), ("entity", p(0.7))]);
    assert_eq!(
        r.budget,
        [(Graph::Semantic, 4), (Graph::Entity, 8)],
        "1 + 3 and 1 + 7"
    );
}

#[test]
fn depth_is_one_unless_multi_hop_is_at_least_half() {
    let at = |m: Decision| routed(&[("semantic", p(0.9)), ("multi_hop_need", m)]);
    assert_eq!((at(p(0.49)).depth, at(p(0.49)).partial), (1, false));
    assert_eq!(at(p(0.5)).depth, 2);
    let unsure = at(unknown());
    assert_eq!(
        (unsure.depth, unsure.partial),
        (1, true),
        "unknown: depth 1, and said so"
    );
    assert!(routed(&[("recency_importance", p(0.5))]).recency);
    assert!(!routed(&[("recency_importance", p(0.49))]).recency);
}

// ---------------------------------------------------------------- scoring and expansion (§10, steps 5–7; T3.3)

use async_trait::async_trait;
use ripwire_broker::memory::model::{Edge, EdgeBasis};
use ripwire_broker::memory::prompts::{self, Stage};
use ripwire_broker::memory::retrieve::{Read, ReadConfig, StopReason};
use ripwire_broker::online::classifier::{ClassifyError, MemoryClassifier};
use ripwire_broker::online::request::StateRequest;
use std::sync::Mutex;

/// The stage, question name and candidate index of an instruction of `memory-prompts/v1`.
fn which(text: &str) -> (Stage, &'static str, usize) {
    for stage in [Stage::Routing, Stage::Scoring, Stage::Stopping] {
        for i in 0..16 {
            for (name, q) in prompts::questions(stage, i) {
                if q.instructions == text {
                    return (stage, name, i);
                }
            }
        }
    }
    panic!("not a read question: {text}")
}

type Answer = Box<dyn Fn(Stage, &str, Option<&str>) -> Decision + Send + Sync>;

/// Answers each read question from its stage, name and the candidate it is about.
struct Reader {
    answer: Answer,
    seen: Mutex<Vec<StateRequest>>,
}

impl Reader {
    fn new(answer: impl Fn(Stage, &str, Option<&str>) -> Decision + Send + Sync + 'static) -> Self {
        Reader {
            answer: Box::new(answer),
            seen: Mutex::default(),
        }
    }
    /// Every candidate id sent for scoring, in order.
    fn scored(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| {
                r.state
                    .get("candidates")
                    .and_then(|c| c.as_array())
                    .cloned()
            })
            .flatten()
            .map(|c| c["id"].as_str().unwrap().to_string())
            .collect()
    }
}

#[async_trait]
impl MemoryClassifier for Reader {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.seen.lock().unwrap().push(req.clone());
        Ok(req
            .questions
            .0
            .iter()
            .map(|(_, q)| {
                let (stage, name, i) = which(&q.instructions);
                let cand = req.state["candidates"]
                    .get(i)
                    .and_then(|c| c["id"].as_str());
                (self.answer)(
                    stage,
                    name,
                    if stage == Stage::Scoring { cand } else { None },
                )
            })
            .collect())
    }
}

fn edge(s: u64, t: u64, graph: Graph) -> Edge {
    Edge {
        source: id(s),
        target: id(t),
        graph,
        relation: "r".into(),
        basis: EdgeBasis::JevInference,
        score: Some(0.9),
        model: Some("m".into()),
        prompt_version: None,
        policy: "p".into(),
        generation: 1,
    }
}

fn with_edges(records: Vec<Record>, edges: Vec<Edge>) -> State {
    let mut s = state(records);
    for e in edges {
        s.edges.insert(e.key(), e);
    }
    s
}

/// Routing: the views given, depth by `multi_hop`; scoring from `score`; stopping: low gain.
fn reader(
    views: &'static [&'static str],
    multi_hop: f64,
    score: impl Fn(&str, &str) -> Decision + Send + Sync + 'static,
) -> Reader {
    Reader::new(move |stage, name, cand| match stage {
        Stage::Routing if views.contains(&name) => p(1.0),
        Stage::Routing if name == "multi_hop_need" => p(multi_hop),
        Stage::Routing => p(0.0),
        Stage::Scoring => score(cand.unwrap(), name),
        _ => p(0.0),
    })
}

async fn read(s: &State, query: &str, r: &Reader) -> Read {
    retrieve::read(s, query, r, &ReadConfig::default()).await
}

#[tokio::test]
async fn the_score_is_the_weighted_sum_and_needs_four_valid_answers() {
    let s = state(vec![
        rec(1, "cache eviction policy", &[]),
        rec(2, "cache warmup", &[]),
        rec(3, "cache store", &[]),
        rec(4, "cache misc", &[]),
    ]);
    let (a, b, c, d) = (id(1), id(2), id(3), id(4));
    let r = reader(&[], 0.0, move |cand, name| {
        let v = |rel, new, use_, sup| match name {
            "relevance" => rel,
            "new_information" => new,
            "relation_usefulness" => use_,
            _ => sup,
        };
        match cand {
            x if x == a => p(v(0.9, 0.5, 0.5, 0.5)),
            x if x == b => p(v(0.59, 1.0, 1.0, 1.0)),
            x if x == c && name == "new_information" => unknown(),
            x if x == c => p(0.9),
            x if x == d => p(v(0.6, 0.1, 0.1, 0.1)),
            _ => unreachable!(),
        }
    });
    let got = read(&s, "cache eviction", &r).await;
    let ids: Vec<&str> = got
        .memories
        .iter()
        .map(|f| f.record.node_id.as_str())
        .collect();
    assert_eq!(
        ids,
        [id(1).as_str()],
        "B: relevance 0.59; C: an unknown answer; D: score below 0.60"
    );
    // 0.40·0.9 + 0.20·0.5 + 0.15·0.5 + 0.15·0.5 + 0.10·1.0 (the best anchor).
    assert!(
        (got.memories[0].score - 0.71).abs() < 1e-9,
        "{}",
        got.memories[0].score
    );
}

#[tokio::test]
async fn no_node_is_scored_twice_and_every_visit_is_counted() {
    let s = with_edges(
        vec![rec(1, "eviction", &[]), rec(2, "b", &[]), rec(3, "c", &[])],
        vec![
            edge(1, 2, Graph::Semantic),
            edge(2, 1, Graph::Semantic),
            edge(1, 3, Graph::Semantic),
            edge(3, 2, Graph::Semantic),
        ],
    );
    let r = reader(&["semantic"], 0.9, |_, _| p(0.9));
    let got = read(&s, "eviction", &r).await;
    let scored = r.scored();
    let mut unique = scored.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(scored.len(), unique.len(), "scored twice: {scored:?}");
    assert_eq!(got.visited, 3);
    assert!(
        got.edges_seen >= 3,
        "every edge looked at counts, kept or not: {}",
        got.edges_seen
    );
}

#[tokio::test]
async fn expansion_respects_twelve_sixteen_depth_two_128_and_beam_four() {
    // A chain from one anchor: depth 2 reaches two hops, never a third.
    let s = with_edges(
        vec![
            rec(1, "eviction", &[]),
            rec(2, "b", &[]),
            rec(3, "c", &[]),
            rec(4, "d", &[]),
        ],
        vec![
            edge(1, 2, Graph::Semantic),
            edge(2, 3, Graph::Semantic),
            edge(3, 4, Graph::Semantic),
        ],
    );
    let r = reader(&["semantic"], 0.9, |_, _| p(0.9));
    read(&s, "eviction", &r).await;
    assert!(!r.scored().contains(&id(4)), "depth 2 at most");

    // A fan-out: 40 neighbours of one anchor.
    let mut records = vec![rec(1, "eviction", &[])];
    let mut edges = vec![];
    for n in 100..140 {
        records.push(rec(n, "other", &[]));
        edges.push(edge(1, n, Graph::Semantic));
    }
    let s = with_edges(records, edges);
    let r = reader(&["semantic"], 0.9, |_, _| p(0.9));
    let got = read(&s, "eviction", &r).await;
    assert!(got.expansions <= retrieve::MAX_EXPANSIONS && retrieve::MAX_EXPANSIONS == 12);
    assert!(got.visited <= 16, "{}", got.visited);
    assert!(got.edges_seen <= 128, "{}", got.edges_seen);

    // 200 edges out of the anchor, in a view that is off: each one looked at still counts.
    let mut records = vec![rec(1, "eviction", &[])];
    let mut edges = vec![];
    for n in 1000..1200 {
        records.push(rec(n, "other", &[]));
        edges.push(edge(1, n, Graph::Causal));
    }
    let s = with_edges(records, edges);
    let r = reader(&["semantic"], 0.9, |_, _| p(0.9));
    let got = read(&s, "eviction", &r).await;
    assert_eq!(got.edges_seen, 128, "looking stops at the cap");

    // Six anchors, each with its own neighbour: only the best four are expanded.
    let mut records = vec![];
    let mut edges = vec![];
    for n in 1..=6 {
        records.push(rec(n, "eviction", &[]));
        records.push(rec(50 + n, "z", &[]));
        edges.push(edge(n, 50 + n, Graph::Semantic));
    }
    let s = with_edges(records, edges);
    let r = reader(&["semantic"], 0.0, |_, _| p(0.9));
    read(&s, "eviction", &r).await;
    let scored = r.scored();
    assert!(scored.contains(&id(51)) && scored.contains(&id(54)));
    assert!(
        !scored.contains(&id(55)) && !scored.contains(&id(56)),
        "beam 4: {scored:?}"
    );
}

#[tokio::test]
async fn depth_two_expands_only_from_a_beam_of_the_four_strongest_relations() {
    // One anchor and six neighbours; only the two reached by the weakest relations have
    // neighbours of their own.
    let mut records = vec![rec(1, "eviction", &[])];
    let mut edges = vec![];
    for n in 2..=7 {
        records.push(rec(n, "z", &[]));
        let mut e = edge(1, n, Graph::Semantic);
        e.score = Some(if n >= 6 { 0.7 } else { 0.9 });
        edges.push(e);
    }
    for (from, to) in [(6, 16), (7, 17)] {
        records.push(rec(to, "z", &[]));
        edges.push(edge(from, to, Graph::Semantic));
    }
    let s = with_edges(records, edges);
    let r = reader(&["semantic"], 0.9, |_, _| p(0.9));
    read(&s, "eviction", &r).await;
    let scored = r.scored();
    assert!(
        scored.contains(&id(7)),
        "depth 1 is all expanded: {scored:?}"
    );
    assert!(
        !scored.contains(&id(16)) && !scored.contains(&id(17)),
        "depth 2 starts from the four strongest only: {scored:?}"
    );
}

#[tokio::test]
async fn recency_only_breaks_ties() {
    let mut x = rec(10, "x", &[]);
    x.ingest_seq = 1;
    let mut y = rec(20, "y", &[]);
    y.ingest_seq = 9;
    let z = rec(5, "z", &[]);
    let s = with_edges(
        vec![rec(1, "eviction", &[]), x, y, z],
        vec![
            edge(1, 10, Graph::Semantic),
            edge(1, 20, Graph::Semantic),
            edge(1, 5, Graph::Semantic),
        ],
    );
    let s = &s;
    let order = |recency: f64| {
        let r = Reader::new(move |stage, name, cand| match (stage, name) {
            (Stage::Routing, "semantic") => p(1.0),
            (Stage::Routing, "recency_importance") => p(recency),
            (Stage::Routing, _) => p(0.0),
            (Stage::Scoring, _) if cand == Some(id(5).as_str()) => p(1.0),
            (Stage::Scoring, _) => p(0.9),
            _ => p(0.0),
        });
        async move {
            let got = read(s, "eviction", &r).await;
            got.memories
                .iter()
                .map(|f| f.record.node_id.clone())
                .collect::<Vec<_>>()
        }
    };
    let with = order(0.9).await;
    let without = order(0.1).await;
    assert_eq!(with[..2], [id(1), id(5)], "a higher score stays ahead");
    assert_eq!(with[2..], [id(20), id(10)], "recency breaks the tie");
    assert_eq!(without[2..], [id(10), id(20)], "otherwise the id does");
}

#[tokio::test]
async fn only_active_views_are_expanded_with_their_direction() {
    let s = with_edges(
        vec![
            rec(1, "eviction", &[]),
            rec(2, "b", &[]),
            rec(3, "c", &[]),
            rec(4, "d", &[]),
        ],
        vec![
            edge(1, 2, Graph::Causal),
            edge(3, 1, Graph::Causal),
            edge(1, 4, Graph::Semantic),
        ],
    );
    let r = reader(&["causal"], 0.0, |_, _| p(0.9));
    read(&s, "eviction", &r).await;
    assert_eq!(
        r.scored(),
        [id(1), id(2)],
        "the causal edge out of the anchor only"
    );
    let r = reader(&["semantic"], 0.0, |_, _| p(0.9));
    read(&s, "eviction", &r).await;
    assert_eq!(r.scored(), [id(1), id(4)]);
}

// ---------------------------------------------------------------- stop reasons (§10, step 8; T3.4)

/// One anchor that passes; stopping answers from `stop`; the rest as given.
fn stopping(stop: impl Fn(&str) -> Decision + Send + Sync + 'static) -> Reader {
    Reader::new(move |stage, name, _| match stage {
        Stage::Routing => p(0.0),
        Stage::Scoring => p(0.9),
        _ => stop(name),
    })
}

fn one() -> State {
    state(vec![rec(1, "eviction policy", &[])])
}

async fn stop_of(s: &State, r: &Reader, cfg: ReadConfig) -> Read {
    retrieve::read(s, "eviction", r, &cfg).await
}

#[tokio::test]
async fn sufficient() {
    let r = stopping(|n| match n {
        "evidence_sufficient" => p(0.95),
        "continue_useful" => p(0.9),
        _ => p(0.14),
    });
    let got = stop_of(&one(), &r, ReadConfig::default()).await;
    assert_eq!(got.stop, StopReason::Sufficient);
    assert_eq!(got.memories.len(), 1);
    let short = stopping(|n| match n {
        "evidence_sufficient" => p(0.949),
        "continue_useful" => p(0.9),
        _ => p(0.14),
    });
    assert_ne!(
        stop_of(&one(), &short, ReadConfig::default()).await.stop,
        StopReason::Sufficient
    );
}

#[tokio::test]
async fn low_expected_gain() {
    let r = stopping(|n| match n {
        "evidence_sufficient" => p(0.5),
        "continue_useful" => p(0.14),
        _ => p(0.5),
    });
    assert_eq!(
        stop_of(&one(), &r, ReadConfig::default()).await.stop,
        StopReason::LowExpectedGain
    );
}

#[tokio::test]
async fn empty() {
    let r = stopping(|_| p(0.0));
    let got = retrieve::read(&one(), "nothing in common", &r, &ReadConfig::default()).await;
    assert_eq!(
        (got.stop, got.requests),
        (StopReason::Empty, 0),
        "no anchor, no request"
    );
}

/// Never answers.
struct Silent;

#[async_trait]
impl MemoryClassifier for Silent {
    async fn decide(&self, _: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn deadline() {
    let got = retrieve::read(&one(), "eviction", &Silent, &ReadConfig::default()).await;
    assert_eq!(got.stop, StopReason::Deadline);
    assert!(got.degraded, "the local anchors went out instead (D-156)");
}

#[tokio::test]
async fn request_limit() {
    // Two requests route and score; the read stops at the limit instead of asking a third. With
    // one, it never asks at all (`a_single_request_is_not_spent_on_a_read_that_cannot_deliver`).
    let r = stopping(|_| p(0.5));
    let cfg = ReadConfig {
        request_limit: 2,
        ..Default::default()
    };
    let got = stop_of(&one(), &r, cfg).await;
    assert_eq!((got.stop, got.requests), (StopReason::RequestLimit, 2));
}

#[tokio::test]
async fn question_limit() {
    let r = stopping(|_| p(0.5));
    let cfg = ReadConfig {
        max_questions: 6,
        ..Default::default()
    };
    let got = stop_of(&one(), &r, cfg).await;
    assert_eq!(
        (got.stop, got.questions),
        (StopReason::QuestionLimit, 6),
        "routing fit, scoring did not"
    );
}

#[tokio::test]
async fn graph_limit() {
    let mut records = vec![rec(1, "eviction", &[])];
    let mut edges = vec![];
    for n in 100..120 {
        records.push(rec(n, "other", &[]));
        edges.push(edge(1, n, Graph::Semantic));
    }
    let s = with_edges(records, edges);
    let r = Reader::new(|stage, name, _| match (stage, name) {
        (Stage::Routing, "semantic") => p(1.0),
        (Stage::Routing, _) => p(0.0),
        (Stage::Scoring, _) => p(0.9),
        (_, "continue_useful") => p(0.9),
        _ => p(0.5),
    });
    let got = stop_of(&s, &r, ReadConfig::default()).await;
    assert_eq!(
        got.stop,
        StopReason::GraphLimit,
        "more neighbours than the expansions allow"
    );
}

#[tokio::test]
async fn cancelled() {
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    let cfg = ReadConfig {
        cancel: Some(cancel),
        ..Default::default()
    };
    let got = retrieve::read(&one(), "eviction", &Silent, &cfg).await;
    assert_eq!(
        (got.stop, got.requests),
        (StopReason::Cancelled, 0),
        "nothing is sent"
    );
}

/// Fails every request.
struct Down;

#[async_trait]
impl MemoryClassifier for Down {
    async fn decide(&self, _: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        Err(ClassifyError::Server(503))
    }
}

#[tokio::test]
async fn provider_error() {
    let got = retrieve::read(&one(), "eviction", &Down, &ReadConfig::default()).await;
    assert_eq!(
        (got.stop, got.requests),
        (StopReason::ProviderError, 1),
        "never retried"
    );
}

#[tokio::test]
async fn without_a_valid_stopping_answer_it_is_never_sufficient() {
    let r = stopping(|n| match n {
        "evidence_sufficient" => p(0.99),
        "missing_evidence" => unknown(),
        _ => p(0.0),
    });
    let got = stop_of(&one(), &r, ReadConfig::default()).await;
    assert_ne!(
        got.stop,
        StopReason::Sufficient,
        "an unknown answer is never a yes"
    );
    assert!(got.partial);
}

// ---------------------------------------------------------------- deadline and requests (§8.2, §10; T3.5)

/// Answers like `reader`, after `delay` per request.
struct Slow(Reader, std::time::Duration);

#[async_trait]
impl MemoryClassifier for Slow {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        tokio::time::sleep(self.1).await;
        self.0.decide(req).await
    }
}

fn expandable() -> State {
    with_edges(
        vec![rec(1, "eviction", &[]), rec(2, "b", &[])],
        vec![edge(1, 2, Graph::Semantic)],
    )
}

#[tokio::test(start_paused = true)]
async fn the_deadline_cuts_the_requests() {
    let started = tokio::time::Instant::now();
    let got = retrieve::read(&one(), "eviction", &Silent, &ReadConfig::default()).await;
    assert_eq!(got.stop, StopReason::Deadline);
    assert!(
        started.elapsed() <= std::time::Duration::from_millis(860),
        "{:?}",
        started.elapsed()
    );

    // Two answers at 240 ms each, then the third would start after the 480 ms: never sent.
    let slow = Slow(
        reader(&["semantic"], 0.0, |_, _| p(0.9)),
        std::time::Duration::from_millis(240),
    );
    let cfg = ReadConfig {
        deadline: std::time::Duration::from_millis(480),
        ..Default::default()
    };
    let got = retrieve::read(&expandable(), "eviction", &slow, &cfg).await;
    assert_eq!((got.stop, got.requests), (StopReason::Deadline, 2));
    assert_eq!(
        got.memories.len(),
        1,
        "what was validated before the deadline is kept"
    );
    assert!(
        got.memories[0].scores.is_some() && !got.degraded,
        "validated memories are not replaced by the local ranking"
    );
}

#[tokio::test(start_paused = true)]
async fn each_attempt_gets_at_most_450ms_and_there_are_no_automatic_retries() {
    let slow = Slow(
        reader(&[], 0.0, |_, _| p(0.9)),
        std::time::Duration::from_millis(500),
    );
    let started = tokio::time::Instant::now();
    let got = retrieve::read(&one(), "eviction", &slow, &ReadConfig::default()).await;
    assert_eq!(
        (got.stop, got.requests),
        (StopReason::Deadline, 1),
        "cut at 450 ms, and not tried again"
    );
    assert!(
        started.elapsed() >= std::time::Duration::from_millis(450),
        "the attempt is not cut before its 450 ms: {:?}",
        started.elapsed()
    );
    assert!(
        started.elapsed() <= std::time::Duration::from_millis(460),
        "{:?}",
        started.elapsed()
    );
}

/// Discovery's requests take 300 ms; memory's, through `Slow`, take what it says.
struct Contended(Slow);

#[async_trait]
impl ripwire_broker::online::classifier::Classifier for Contended {
    fn model(&self) -> &str {
        "jev-1.13.0"
    }
    async fn classify(
        &self,
        req: &ripwire_broker::online::request::JevRequest,
    ) -> Result<Vec<Option<f64>>, ClassifyError> {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(vec![Some(0.5); req.questions.0.len()])
    }
}

#[async_trait]
impl MemoryClassifier for Contended {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.0.decide(req).await
    }
}

#[tokio::test(start_paused = true)]
async fn waiting_for_a_slot_is_not_spent_from_the_attempt() {
    use ripwire_broker::online::SemanticStage;
    use ripwire_broker::online::classifier::{Classifier, Shared};
    use ripwire_broker::online::request::{StateItem, build};
    // Each memory request answers in 100 ms, well inside its 450 ms, but discovery holds the
    // four slots for the first 300 ms. The 450 ms are the HTTP attempt's (PRD jev-mem §8.2):
    // the queue comes out of the read's 850 ms, which still leave 550 ms for three requests.
    let slow = Slow(
        reader(&[], 0.0, |_, _| p(0.9)),
        std::time::Duration::from_millis(100),
    );
    let shared = Arc::new(Shared::new(Arc::new(Contended(slow)), 4));
    let mut discovery = tokio::task::JoinSet::new();
    for n in 0..4 {
        let d = shared.clone();
        discovery.spawn(async move {
            let item = StateItem {
                id: format!("i{n}"),
                path: "a.rs".into(),
                text: "x".into(),
            };
            d.classify(&build(
                "jev-1.13.0",
                "q",
                SemanticStage::FileAdmission,
                vec![item],
            ))
            .await
        });
    }
    tokio::task::yield_now().await;

    let got = retrieve::read(&one(), "eviction", shared.as_ref(), &ReadConfig::default()).await;
    assert_ne!(got.stop, StopReason::Deadline, "requests {}", got.requests);
    assert_eq!(
        got.memories.len(),
        1,
        "the anchor was scored once a slot was free"
    );
    assert!(!got.degraded, "scored by Jev, not the local fallback");
    while let Some(done) = discovery.join_next().await {
        done.unwrap().unwrap();
    }
}

/// Answers routing like `Reader`; never answers a request that scores candidates.
struct SilentScoring(Reader);

#[async_trait]
impl MemoryClassifier for SilentScoring {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        if req.state.get("candidates").is_some() {
            return std::future::pending().await;
        }
        self.0.decide(req).await
    }
}

fn delivered_bases(read: &Read) -> Vec<String> {
    retrieve::items(read)
        .into_iter()
        .map(|i| i.basis.to_string())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn a_read_jev_cannot_answer_in_time_delivers_the_local_anchors_degraded() {
    // D-156: Jev never answers routing; the anchors the local index found go out in its order,
    // as the deterministic arm would deliver them, and the read says it is degraded.
    let s = state(vec![
        rec(1, "eviction policy", &[]),
        rec(2, "eviction", &[]),
    ]);
    let got = retrieve::read(&s, "eviction", &Silent, &ReadConfig::default()).await;
    assert_eq!((got.stop, got.requests), (StopReason::Deadline, 1));
    assert!(got.degraded);
    assert_eq!(got.memories.len(), 2);
    assert!(got.memories.iter().all(|f| f.scores.is_none()));
    assert_eq!(delivered_bases(&got), ["deterministic_rank"; 2]);
    assert!(retrieve::provenance(&got).degraded);

    // Routing answered, scoring never did: the same fallback.
    let r = SilentScoring(reader(&[], 0.0, |_, _| p(0.9)));
    let got = retrieve::read(&one(), "eviction", &r, &ReadConfig::default()).await;
    assert_eq!((got.stop, got.requests), (StopReason::Deadline, 2));
    assert!(got.degraded);
    assert_eq!(delivered_bases(&got), ["deterministic_rank"]);
}

#[tokio::test]
async fn a_provider_error_is_not_a_late_answer_and_delivers_nothing() {
    // Only a deadline falls back: a refused or broken request says nothing about the anchors.
    let got = retrieve::read(&one(), "eviction", &Down, &ReadConfig::default()).await;
    assert_eq!(got.stop, StopReason::ProviderError);
    assert!(got.memories.is_empty() && !got.degraded);
}

#[tokio::test]
async fn memories_jev_scored_say_so() {
    let r = stopping(|_| p(0.5));
    let got = retrieve::read(&one(), "eviction", &r, &ReadConfig::default()).await;
    assert_eq!(delivered_bases(&got), ["jev_scored"]);
    assert!(!got.degraded);
}

#[tokio::test]
async fn the_fourth_request_is_reserved_for_stopping() {
    let is_stopping = |r: &StateRequest| r.state.get("depth").is_some();
    let r = reader(&["semantic"], 0.0, |_, _| p(0.9));
    let got = retrieve::read(&expandable(), "eviction", &r, &ReadConfig::default()).await;
    assert_eq!(got.requests, 4, "routing, anchors, expansion, stopping");

    let r = reader(&["semantic"], 0.0, |_, _| p(0.9));
    let cfg = ReadConfig {
        request_limit: 3,
        ..Default::default()
    };
    let got = retrieve::read(&expandable(), "eviction", &r, &cfg).await;
    let last_is_stopping = is_stopping(r.seen.lock().unwrap().last().unwrap());
    assert_eq!(got.requests, 3);
    assert!(last_is_stopping, "the expansion gave way to stopping");
    assert!(!r.scored().contains(&id(2)));
}

#[tokio::test]
async fn request_limit_zero_serves_only_cache_and_says_degraded() {
    let r = reader(&[], 0.0, |_, _| p(0.9));
    let cfg = ReadConfig {
        request_limit: 0,
        ..Default::default()
    };
    let got = retrieve::read(&one(), "eviction", &r, &cfg).await;
    assert!(got.degraded);
    assert_eq!(
        (got.stop, got.requests, got.memories.len()),
        (StopReason::RequestLimit, 0, 0)
    );
    assert!(
        r.seen.lock().unwrap().is_empty(),
        "no cache yet, and nothing sent"
    );
}

/// One request cannot both route and score, so it is never spent (D-150): the read paid for the
/// routing and stopped at the limit before it could deliver anything.
#[tokio::test]
async fn a_single_request_is_not_spent_on_a_read_that_cannot_deliver() {
    let r = reader(&[], 0.0, |_, _| p(0.9));
    let cfg = ReadConfig {
        request_limit: 1,
        ..Default::default()
    };

    let got = retrieve::read(&one(), "eviction", &r, &cfg).await;

    assert!(got.degraded);
    assert_eq!((got.stop, got.requests), (StopReason::RequestLimit, 0));
    assert!(r.seen.lock().unwrap().is_empty(), "nothing sent");
}

#[test]
fn memory_takes_at_most_four_slots_of_the_jev_request_limit() {
    assert_eq!(
        retrieve::slots(4, 24),
        (4, 20),
        "memory, then what discovery keeps"
    );
    assert_eq!(retrieve::slots(4, 3), (3, 0), "never beyond the limit");
    assert_eq!(retrieve::slots(0, 24), (0, 24));
    assert_eq!(retrieve::slots(9, 24), (4, 20), "four at most");
}

// ---------------------------------------------------------------- stale sources, pending writes (§10, steps 1 and 9; T3.6)

mod common;

use ripwire_broker::memory::admission::{self, Draft, Event, Outcome, Stamp, Tests};
use ripwire_broker::memory::recall::Recall;
use ripwire_broker::memory::retrieve::ReadSetup;
use ripwire_broker::memory::{identity, store::Store};
use ripwire_broker::online::reader::WorkspaceReader;
use std::sync::Arc;

/// An observation of `path` in `root`, with the hash of its bytes now.
fn observed(root: &std::path::Path, ws: &str, path: &str, words: &str) -> Record {
    let reader = WorkspaceReader::new(root).unwrap();
    let draft = Draft {
        event_key: words.into(),
        event: Event::AfterEdit,
        outcome: Outcome::AnalysisCompleted,
        tests: Tests::Unknown,
        scope: vec![path.into()],
        evidence: vec![words.into()],
    };
    let stamp = Stamp {
        observed_at_ms: 1,
        ingest_seq: 0,
        generation: 0,
        retention_ms: u64::MAX / 2,
    };
    admission::admit(&reader, ws, &draft, stamp).unwrap()
}

fn passing() -> Arc<Reader> {
    Arc::new(reader(&[], 0.0, |_, _| p(0.9)))
}

/// What `context_for_task` reads with: the warm snapshot, sources checked.
fn recall(
    store: &Arc<Store>,
    root: &std::path::Path,
    classifier: Arc<dyn MemoryClassifier>,
) -> Recall {
    let setup = ReadSetup {
        store: store.clone(),
        classifier,
        cfg: ReadConfig::default(),
    };
    Recall::new(setup, root).unwrap()
}

async fn recalled(
    store: &Arc<Store>,
    root: &std::path::Path,
    classifier: Arc<dyn MemoryClassifier>,
    query: &str,
) -> Read {
    recall(store, root, classifier)
        .read(query)
        .await
        .read
        .expect("the snapshot loads")
}

#[tokio::test]
async fn a_memory_whose_source_changed_is_omitted_and_counted_stale() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    common::write(root.path(), "src/b.rs", "fn b() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store
        .enqueue(&observed(root.path(), &ws, "src/b.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    common::write(root.path(), "src/a.rs", "fn a() { changed() }\n");

    let r = passing();
    let got = recalled(&store, root.path(), r.clone(), "eviction").await;
    assert_eq!(
        got.stale_omitted, 1,
        "src/a.rs changed since it was observed"
    );
    assert_eq!(
        r.scored().len(),
        1,
        "a stale memory is not even sent for scoring"
    );
    let paths: Vec<&str> = got
        .memories
        .iter()
        .map(|f| f.record.sources[0].path.as_str())
        .collect();
    assert_eq!(paths, ["src/b.rs"], "old advice is not served as current");
}

#[tokio::test]
async fn only_what_a_read_is_about_to_use_has_its_sources_checked() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    common::write(root.path(), "src/hit.rs", "fn hit() {}\n");
    store
        .enqueue(&observed(root.path(), &ws, "src/hit.rs", "eviction"))
        .unwrap();
    // Thirty memories the query has nothing to do with, every one of them stale.
    for n in 0..30 {
        let path = format!("src/m{n}.rs");
        common::write(root.path(), &path, "fn m() {}\n");
        store
            .enqueue(&observed(root.path(), &ws, &path, &format!("unrelated{n}")))
            .unwrap();
        common::write(root.path(), &path, "fn m() { changed() }\n");
    }
    store.ingest().unwrap();
    let got = recalled(&store, root.path(), passing(), "eviction").await;
    assert_eq!(got.memories.len(), 1);
    assert_eq!(
        got.stale_omitted, 0,
        "the unrelated ones are never looked at, so never counted"
    );
}

#[tokio::test]
async fn checking_sources_counts_against_the_deadline() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    // Anchors over large sources: hashing them all takes far longer than the deadline.
    let big = "x".repeat(7 * 1024 * 1024);
    for n in 0..4 {
        let path = format!("src/big{n}.rs");
        common::write(root.path(), &path, &big);
        store
            .enqueue(&observed(root.path(), &ws, &path, "eviction"))
            .unwrap();
    }
    store.ingest().unwrap();
    let setup = ReadSetup {
        store: store.clone(),
        classifier: passing(),
        cfg: ReadConfig {
            deadline: std::time::Duration::from_millis(60),
            ..ReadConfig::default()
        },
    };
    let recall = Recall::new(setup, root.path()).unwrap();
    recall.read("warm up").await;
    let started = std::time::Instant::now();
    let got = recall.read("eviction").await;
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_millis(400),
        "the read keeps to its deadline, local work included: {took:?}"
    );
    let read = got.read.unwrap();
    assert_eq!(read.stop, StopReason::Deadline);
    assert!(read.memories.is_empty());

    // With all the time it needs, the hashing still runs beside the async thread, never on it:
    // a timer on this same thread fires on time.
    let setup = ReadSetup {
        store: store.clone(),
        classifier: passing(),
        cfg: ReadConfig {
            deadline: std::time::Duration::from_secs(30),
            ..ReadConfig::default()
        },
    };
    let patient = Recall::new(setup, root.path()).unwrap();
    patient.read("warm up").await;
    let started = std::time::Instant::now();
    let (_, ticked) = tokio::join!(patient.read("eviction"), async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        started.elapsed()
    });
    assert!(
        ticked < std::time::Duration::from_millis(150),
        "the async thread was held for {ticked:?}"
    );
}

#[tokio::test]
async fn a_stale_memory_reached_by_a_relation_is_not_scored_either() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    common::write(root.path(), "src/b.rs", "fn b() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    let (a, b) = (
        observed(root.path(), &ws, "src/a.rs", "eviction"),
        observed(root.path(), &ws, "src/b.rs", "unrelated"),
    );
    store.enqueue(&a).unwrap();
    store.enqueue(&b).unwrap();
    store.ingest().unwrap();
    let mut state = store.load().unwrap();
    let mut related = edge(0, 0, Graph::Semantic);
    (related.source, related.target) = (a.node_id.clone(), b.node_id.clone());
    state.edges.insert(related.key(), related);
    store.publish(&state).unwrap();
    common::write(root.path(), "src/b.rs", "fn b() { changed() }\n");

    let r = Arc::new(reader(&["semantic"], 0.0, |_, _| p(0.9)));
    let got = recalled(&store, root.path(), r.clone(), "eviction").await;
    assert_eq!(
        r.scored(),
        std::slice::from_ref(&a.node_id),
        "b is stale: never sent"
    );
    assert_eq!(got.stale_omitted, 1);
}

/// Answers at once, except the stopping request, which never comes back.
struct NeverStops(Reader);

#[async_trait]
impl MemoryClassifier for NeverStops {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        if req.state.get("depth").is_some() {
            std::future::pending::<()>().await;
        }
        self.0.decide(req).await
    }
}

#[tokio::test]
async fn the_last_checks_keep_their_time_when_the_provider_takes_all_of_it() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    let setup = ReadSetup {
        store: store.clone(),
        classifier: Arc::new(NeverStops(reader(&[], 0.0, |_, _| p(0.9)))),
        cfg: ReadConfig {
            deadline: std::time::Duration::from_millis(200),
            ..ReadConfig::default()
        },
    };
    let recall = Recall::new(setup, root.path()).unwrap();
    recall.read("warm up").await;
    let got = recall.read("eviction").await;
    let read = got.read.unwrap();
    assert_eq!(read.stop, StopReason::Deadline);
    assert_eq!(
        read.memories.len(),
        1,
        "scored before the deadline, and checked again inside it: {:?}",
        got.limitations
    );
}

#[tokio::test]
async fn a_short_deadline_still_leaves_the_requests_most_of_it() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    // `--memory-read-deadline-ms` goes down to 1: the time kept for the last checks shrinks with it.
    let setup = ReadSetup {
        store: store.clone(),
        classifier: passing(),
        cfg: ReadConfig {
            deadline: std::time::Duration::from_millis(40),
            ..ReadConfig::default()
        },
    };
    let recall = Recall::new(setup, root.path()).unwrap();
    recall.read("warm up").await;
    let read = recall.read("eviction").await.read.unwrap();
    assert!(read.requests > 0, "{read:?}");
    assert_eq!(read.memories.len(), 1, "{read:?}");
}

#[tokio::test]
async fn hashes_and_generation_are_revalidated_right_before_delivery() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    let record = observed(root.path(), &ws, "src/a.rs", "eviction");
    store.enqueue(&record).unwrap();
    store.ingest().unwrap();

    // Forgotten while the second read, served from the warm snapshot, was waiting on the
    // provider.
    let (other, id_) = (store.clone(), record.node_id.clone());
    let stops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = stops.clone();
    let forgetting = Arc::new(Reader::new(move |stage, name, _| {
        if stage == Stage::Stopping
            && name == "evidence_sufficient"
            && counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1
        {
            other.forget(&id_, u64::MAX).unwrap();
        }
        p(0.9)
    }));
    let warm = recall(&store, root.path(), forgetting);
    let first = warm.read("eviction").await.read.unwrap();
    assert_eq!(first.memories.len(), 1, "delivered while it is there");
    let got = warm.read("eviction").await.read.unwrap();
    assert!(
        got.memories.is_empty(),
        "a node gone by delivery time is not delivered"
    );

    // Forgotten, its tombstone over, and observed again while the read waited: the same id in a
    // new generation is not what was scored.
    let store = Arc::new(Store::new(st.path(), &format!("{ws}3")));
    store.enqueue(&record).unwrap();
    store.ingest().unwrap();
    let (other, again) = (store.clone(), record.clone());
    let replaced = Arc::new(Reader::new(move |stage, _, _| {
        if stage == Stage::Stopping {
            other.forget(&again.node_id, 5).unwrap();
            other.sweep(10).unwrap();
            other.enqueue(&again).unwrap();
            other.ingest().unwrap();
        }
        p(0.9)
    }));
    let got = recalled(&store, root.path(), replaced, "eviction").await;
    assert!(got.memories.is_empty(), "another generation of the node");

    // Edited while the read was waiting: stale by delivery time.
    let store = Arc::new(Store::new(st.path(), &format!("{ws}2")));
    store.enqueue(&record).unwrap();
    store.ingest().unwrap();
    let path = root.path().to_path_buf();
    let editing = Arc::new(Reader::new(move |stage, _, _| {
        if stage == Stage::Stopping {
            common::write(&path, "src/a.rs", "fn a() { edited() }\n");
        }
        p(0.9)
    }));
    let got = recalled(&store, root.path(), editing, "eviction").await;
    assert!(got.memories.is_empty());
    assert_eq!(got.stale_omitted, 1);
}

#[tokio::test]
async fn a_snapshot_rewritten_with_the_same_size_and_time_is_still_seen() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    // A store without the record of its generation: only the file says what changed.
    std::fs::remove_file(store.dir().join("generation")).unwrap();
    let warm = recall(&store, root.path(), passing());
    assert_eq!(warm.read("eviction").await.read.unwrap().memories.len(), 1);

    // Replaced through a rename, same length, same modification time: another memory.
    let snapshot = store.dir().join("snapshot.json");
    let before = std::fs::metadata(&snapshot).unwrap().modified().unwrap();
    let text = std::fs::read_to_string(&snapshot).unwrap();
    assert!(text.contains("eviction"));
    let next = store.dir().join("snapshot.next");
    std::fs::write(&next, text.replace("eviction", "zzzzzzzz")).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&next)
        .unwrap()
        .set_modified(before)
        .unwrap();
    std::fs::rename(&next, &snapshot).unwrap();
    assert_eq!(
        std::fs::metadata(&snapshot).unwrap().modified().unwrap(),
        before
    );
    let got = warm.read("eviction").await.read.unwrap();
    assert!(
        got.memories.is_empty(),
        "the copy kept is not served after the file changed"
    );
}

#[tokio::test]
async fn the_generation_is_recorded_beside_every_snapshot() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Store::new(st.path(), &ws);
    let record = observed(root.path(), &ws, "src/a.rs", "eviction");
    store.enqueue(&record).unwrap();
    store.ingest().unwrap();
    let recorded = || std::fs::read_to_string(store.dir().join("generation")).unwrap();
    assert_eq!(recorded(), store.load().unwrap().generation.to_string());
    store.charge(1, 1, 1).unwrap();
    assert_eq!(recorded(), "1", "a charge changes no memory");
    store.forget(&record.node_id, u64::MAX).unwrap();
    assert_eq!(recorded(), store.load().unwrap().generation.to_string());
    assert_eq!(recorded(), "2");
}

#[tokio::test]
async fn pending_writes_are_reported_and_not_awaited() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction later"))
        .unwrap();
    let got = recalled(&store, root.path(), passing(), "eviction").await;
    assert_eq!(got.pending_writes, 1);
    assert_eq!(got.memories.len(), 1, "only what is incorporated");
    assert_eq!(
        store.pending().unwrap(),
        1,
        "a read never incorporates the spool"
    );
}

#[tokio::test]
async fn another_worktree_of_the_same_head_sees_nothing() {
    let (repo, st, other) = (
        common::sample_repo(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let wt = other.path().join("wt");
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-q", wt.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    let main = identity::workspace_id(repo.path()).unwrap();
    let store = Store::new(st.path(), &main);
    store
        .enqueue(&observed(repo.path(), &main, "src/auth.py", "eviction"))
        .unwrap();
    store.ingest().unwrap();

    let linked = Arc::new(Store::new(st.path(), &identity::workspace_id(&wt).unwrap()));
    let got = recalled(&linked, &wt, passing(), "eviction").await;
    assert_eq!((got.stop, got.memories.len()), (StopReason::Empty, 0));
}

/// A read's spend that could not reach the quota file is written by a later read, never lost
/// (D-150): the quota lock was busy for its whole wait, the entry stayed in memory forever, and
/// other processes, or this one after a restart, never saw those requests.
#[tokio::test]
async fn a_spend_that_missed_the_quota_file_is_written_later() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws));
    store
        .enqueue(&observed(root.path(), &ws, "src/a.rs", "eviction"))
        .unwrap();
    store.ingest().unwrap();
    let recall = recall(&store, root.path(), passing());
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    };
    let used = || {
        store
            .quota_file()
            .unwrap()
            .map_or((0, 0), |l| l.used(now()))
    };

    // Someone else holds the quota for longer than a spend waits.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(store.dir().join("quota.lock"))
        .unwrap();
    lock.lock().unwrap();
    let first = recall.read("eviction").await.read.unwrap();
    assert!(first.requests > 0);
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert_eq!(used(), (0, 0), "the first spend could not be written");
    drop(lock);

    let second = recall.read("eviction").await.read.unwrap();
    let expected = (
        (first.requests + second.requests) as u32,
        (first.questions + second.questions) as u32,
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while used() != expected && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert_eq!(used(), expected, "both reads are on the quota file");
}

// ---------------------------------------------------------------- --memory-debug-log (D-164)

#[tokio::test]
async fn a_read_is_logged_by_its_outcome_and_never_by_the_task_text() {
    let (root, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    common::write(root.path(), "src/a.rs", "fn a() {}\n");
    let ws = identity::workspace_id(root.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &ws).with_debug_log("test").unwrap());
    let record = observed(root.path(), &ws, "src/a.rs", "eviction");
    store.enqueue(&record).unwrap();
    store.ingest().unwrap();
    let recall = recall(&store, root.path(), passing());
    let task = "why does the cache evict too early";
    let got = recall.read(task).await;
    assert!(got.read.is_some());

    let log =
        std::fs::read_to_string(store.dir().join(ripwire_broker::memory::debug::FILE)).unwrap();
    let read = log
        .lines()
        .find(|l| l.get(24..).and_then(|r| r.split_whitespace().nth(1)) == Some("read"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(read.contains("task#"), "the task by a hash: {read}");
    assert!(read.contains("stop="), "{read}");
    assert!(!log.contains(task) && !log.contains("evict too"), "{log}");
}
