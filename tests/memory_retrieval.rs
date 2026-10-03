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
}

#[tokio::test]
async fn request_limit() {
    let r = stopping(|_| p(0.5));
    let cfg = ReadConfig {
        request_limit: 1,
        ..Default::default()
    };
    let got = stop_of(&one(), &r, cfg).await;
    assert_eq!(
        (got.stop, got.requests, got.memories.len()),
        (StopReason::RequestLimit, 1, 0)
    );
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
async fn budget_omitted() {
    let r = stopping(|n| match n {
        "evidence_sufficient" => p(0.99),
        _ => p(0.0),
    });
    let mut got = stop_of(&one(), &r, ReadConfig::default()).await;
    assert_eq!(got.stop, StopReason::Sufficient);
    retrieve::fit(&mut got, 3, 1);
    assert!(got.memories.is_empty());
    assert_eq!(
        got.stop,
        StopReason::BudgetOmitted,
        "nothing fit the budget"
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
