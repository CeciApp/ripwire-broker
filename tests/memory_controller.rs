//! Seam: the memory controller (PRD jev-mem §7, §8): what it asks the classifier and what it
//! does with the answers. A classifier stand-in, no network.
use ripwire_broker::memory::prompts::{self, Stage};
use ripwire_broker::online::request::JevQuestionType;

#[test]
fn every_stage_names_its_state_fields_and_carries_the_untrusted_guidance() {
    assert_eq!(prompts::VERSION, "memory-prompts/v1");
    assert_ne!(prompts::VERSION, ripwire_broker::notes::PROMPT_VERSION);

    let expected: [(Stage, &[&str], usize, usize); 8] = [
        (
            Stage::Typing,
            &["episodic", "semantic", "procedural", "preference"],
            4,
            0,
        ),
        (
            Stage::Relations,
            &["semantic", "caused_by", "causes", "same_episode"],
            4,
            0,
        ),
        (Stage::Alias, &["alias"], 1, 0),
        (Stage::ImplicitTime, &["time"], 0, 1),
        (
            Stage::Routing,
            &[
                "semantic",
                "temporal",
                "causal",
                "entity",
                "multi_hop_need",
                "recency_importance",
            ],
            6,
            0,
        ),
        (
            Stage::Scoring,
            &[
                "relevance",
                "new_information",
                "relation_usefulness",
                "supports_current_evidence",
            ],
            4,
            0,
        ),
        (
            Stage::Stopping,
            &[
                "evidence_sufficient",
                "continue_useful",
                "missing_evidence",
                "contradiction",
            ],
            4,
            0,
        ),
        (
            Stage::Consolidation,
            &[
                "redundancy",
                "contradiction",
                "obsolescence",
                "link_usefulness",
                "representation",
            ],
            4,
            1,
        ),
    ];
    for (stage, names, nouls, choices) in expected {
        let questions = prompts::questions(stage, 2);
        let got: Vec<&str> = questions.iter().map(|(n, _)| *n).collect();
        assert_eq!(got, names, "{stage:?}");
        let count = |k| questions.iter().filter(|(_, q)| q.kind == k).count();
        assert_eq!(
            (count(JevQuestionType::Noul), count(JevQuestionType::Choice)),
            (nouls, choices),
            "{stage:?}"
        );

        let fields = prompts::state_fields(stage);
        assert!(!fields.is_empty(), "{stage:?}");
        for field in fields {
            assert!(
                questions
                    .iter()
                    .any(|(_, q)| q.instructions.contains(field)),
                "{stage:?}: no question names `{field}`"
            );
        }
        for (name, q) in &questions {
            let text = &q.instructions;
            assert!(
                text.contains("true:") && text.contains("false:"),
                "{stage:?}/{name}: explicit criteria"
            );
            assert!(text.contains("untrusted"), "{stage:?}/{name}: the guidance");
            for (other, _) in questions.iter().filter(|(o, _)| o != name) {
                assert!(
                    !text.contains(&format!("answer to {other}"))
                        && !text.contains("other question"),
                    "{stage:?}/{name} leans on {other}"
                );
            }
        }
    }
    // A per-candidate stage points at the candidate it is about.
    let relations = prompts::questions(Stage::Relations, 3);
    assert!(
        relations
            .iter()
            .all(|(_, q)| q.instructions.contains("candidates[3]"))
    );
    let time = &prompts::questions(Stage::ImplicitTime, 0)[0].1;
    let options: Vec<&str> = time
        .criteria
        .as_ref()
        .unwrap()
        .0
        .iter()
        .map(|(o, _)| o.as_str())
        .collect();
    assert_eq!(
        options,
        [
            "before",
            "after",
            "during",
            "contains",
            "overlaps",
            "same_time",
            "unknown"
        ],
        "direction new_memory → candidate"
    );
    let representation = &prompts::questions(Stage::Consolidation, 0)[4].1;
    let options: Vec<&str> = representation
        .criteria
        .as_ref()
        .unwrap()
        .0
        .iter()
        .map(|(o, _)| o.as_str())
        .collect();
    assert_eq!(options, ["keep_separate", "merge", "promote", "uncertain"]);
}

// ---------------------------------------------------------------- the worker (PRD jev-mem §7, §8.1; T2.7)

use async_trait::async_trait;
use ripwire_broker::memory::controller::{self, Config};
use ripwire_broker::memory::model::{EdgeBasis, EnrichmentState, Graph, Record};
use ripwire_broker::memory::store::{State, Store};
use ripwire_broker::online::classifier::{ClassifyError, MemoryClassifier};
use ripwire_broker::online::request::StateRequest;
use ripwire_broker::online::response::Decision;
use serde_json::json;
use std::sync::Mutex;

fn id(n: u64) -> String {
    format!("{n:064}")
}

fn rec(n: u64, content: &str, entities: &[&str]) -> Record {
    serde_json::from_value(json!({
        "schema_version": 1, "policy_version": "memory-policy/v1",
        "node_id": id(n), "content_hash": id(n), "workspace_id": "w", "event_key": "e",
        "kind": "edit_observation", "content": content, "observed_at_ms": n, "ingest_seq": 0,
        "timestamp_role": "observation",
        "entities": entities.iter().map(|e| json!({"id": e, "kind": "file", "path": e})).collect::<Vec<_>>(),
        "expires_at_ms": u64::MAX, "generation": 0
    }))
    .unwrap()
}

fn timed(n: u64, content: &str, entities: &[&str]) -> Record {
    let mut r = rec(n, content, entities);
    r.temporal_references = vec![
        serde_json::from_value(json!(
            {"start_ms": 1, "precision": "day", "evidence_id": "s0"}
        ))
        .unwrap(),
    ];
    r
}

fn stored(dir: &std::path::Path, records: &[Record]) -> Store {
    let store = Store::new(dir, &"c".repeat(64));
    for r in records {
        store.enqueue(r).unwrap();
    }
    store.ingest().unwrap();
    store
}

type Answer = Box<dyn Fn(&str) -> Decision + Send + Sync>;

/// Answers each question from its instruction text, and keeps every request.
struct Fake {
    answer: Answer,
    seen: Mutex<Vec<StateRequest>>,
    fail: bool,
}

impl Fake {
    fn new(answer: impl Fn(&str) -> Decision + Send + Sync + 'static) -> Self {
        Fake {
            answer: Box::new(answer),
            seen: Mutex::default(),
            fail: false,
        }
    }
    fn count(&self) -> (usize, usize) {
        use ripwire_broker::online::request::JevQuestionType::*;
        let seen = self.seen.lock().unwrap();
        let all = seen
            .iter()
            .flat_map(|r| r.questions.0.iter().map(|(_, q)| q.kind));
        all.fold((0, 0), |(n, c), k| match k {
            Noul => (n + 1, c),
            Choice => (n, c + 1),
        })
    }
}

#[async_trait]
impl MemoryClassifier for Fake {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.seen.lock().unwrap().push(req.clone());
        if self.fail {
            return Err(ClassifyError::Network);
        }
        Ok(req
            .questions
            .0
            .iter()
            .map(|(_, q)| (self.answer)(&q.instructions))
            .collect())
    }
}

fn noul(p: f64) -> Decision {
    Decision::Noul { probability: p }
}

fn config(k: usize) -> Config {
    Config {
        model: "jev-1.13.0".into(),
        candidates: k,
    }
}

/// Enriches the one job ready, as the worker would.
async fn enrich(store: &Store, fake: &Fake, k: usize) -> controller::Enriched {
    let lease = store.lease_next(0).unwrap().expect("a job");
    controller::enrich(store, fake, lease, &config(k), 1_000)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_node_exists_before_any_inference() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &[rec(1, "cache layer", &["e1"])]);
    let s = store.load().unwrap();
    let node = &s.nodes[&id(1)];
    assert_eq!(node.types.episodic, None, "not typed yet");
    assert_eq!(node.enrichment.state, EnrichmentState::Pending);
    assert!(s.edges.is_empty());

    let mut down = Fake::new(|_| noul(0.9));
    down.fail = true;
    enrich(&store, &down, 4).await;
    let s = store.load().unwrap();
    assert!(
        s.nodes.contains_key(&id(1)),
        "a provider failure keeps the node"
    );
    assert_eq!(s.nodes[&id(1)].enrichment.state, EnrichmentState::Failed);
}

#[test]
fn candidates_are_deterministic_and_capped_at_k() {
    let mut s = State::default();
    for n in 1..=12 {
        let mut r = rec(n, &format!("note {n}"), &["shared"]);
        r.ingest_seq = n;
        s.nodes.insert(id(n), r);
    }
    let mut far = rec(99, "unrelated words entirely", &["other"]);
    far.ingest_seq = 99;
    s.nodes.insert(id(99), far);

    assert!(controller::candidates(&s, &id(1), 0).is_empty());
    let four = controller::candidates(&s, &id(1), 4);
    assert_eq!(
        four,
        [id(2), id(3), id(4), id(5)],
        "nearest first among equals"
    );
    assert_eq!(
        controller::candidates(&s, &id(6), 4),
        [id(5), id(7), id(4), id(8)],
        "by distance in the sequence, then id"
    );
    let ten = controller::candidates(&s, &id(1), 10);
    assert_eq!(ten.len(), 10);
    assert!(!ten.contains(&id(1)), "never itself");
    assert_eq!(controller::candidates(&s, &id(1), 10), ten, "deterministic");
    assert!(
        !controller::candidates(&s, &id(1), 20).contains(&id(99)),
        "no signal, no candidate"
    );
}

#[tokio::test]
async fn write_cost_matches_the_formula() {
    for k in [0usize, 4, 10] {
        let dir = tempfile::tempdir().unwrap();
        let mut records = vec![timed(100, "cache layer eviction", &["e-new"])];
        // Candidates: half share the entity (no alias question), half only words (alias asked);
        // one of them carries a temporal reference (an implicit time question).
        for n in 1..=k as u64 {
            let entities: &[&str] = if n % 2 == 0 { &["e-new"] } else { &["e-other"] };
            records.push(match n {
                1 => timed(n, "cache layer eviction", entities),
                _ => rec(n, "cache layer eviction", entities),
            });
        }
        let store = stored(dir.path(), &records);
        let fake = Fake::new(|_| noul(0.1));
        // The newest node is enriched; its candidates are the others.
        let lease = loop {
            let l = store.lease_next(0).unwrap().unwrap();
            if l.node_id() == id(100) {
                break l;
            }
            store
                .finish(l, ripwire_broker::memory::queue::Outcome::Done)
                .unwrap();
        };
        controller::enrich(&store, &fake, lease, &config(k), 1_000)
            .await
            .unwrap();
        let alias = (1..=k).filter(|n| n % 2 == 1).count();
        let time = usize::from(k > 0);
        assert_eq!(fake.count(), (4 + 4 * k + alias, time), "K = {k}");
        // A split request still names its own candidates: `candidates[i]` exists in it.
        for req in fake.seen.lock().unwrap().iter() {
            let n = req.state["candidates"].as_array().map_or(0, Vec::len);
            for (_, q) in &req.questions.0 {
                for i in 0..20 {
                    if q.instructions.contains(&format!("candidates[{i}]")) {
                        assert!(i < n, "K = {k}: candidates[{i}] in a request of {n}");
                    }
                }
            }
        }
    }
}

/// The question kinds, told apart by their instruction text.
fn named(text: &str) -> &'static str {
    match () {
        _ if text.contains("cause, enable or explain the event in new_memory") => "caused_by",
        _ if text.contains("Does the event in new_memory.content cause") => "causes",
        _ if text.contains("semantic link") => "semantic",
        _ if text.contains("same working episode") => "same_episode",
        _ if text.contains("explicitly refer to the same") => "alias",
        _ if text.contains("temporal relation") => "time",
        _ => "typing",
    }
}

#[tokio::test]
async fn an_edge_needs_060_and_absence_is_not_zero() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[
            rec(1, "cache layer one", &["e"]),
            rec(2, "cache layer two", &["e"]),
            rec(3, "cache layer three", &["e"]),
            rec(9, "cache layer new", &["e"]),
        ],
    );
    let fake = Fake::new(|text| match named(text) {
        "semantic" if text.contains("candidates[0]") => noul(0.60),
        "semantic" if text.contains("candidates[1]") => noul(0.59),
        "semantic" => Decision::Unknown {
            reason: ripwire_broker::online::response::Unknown::Absent,
        },
        _ => noul(0.0),
    });
    let lease = loop {
        let l = store.lease_next(0).unwrap().unwrap();
        if l.node_id() == id(9) {
            break l;
        }
        store
            .finish(l, ripwire_broker::memory::queue::Outcome::Done)
            .unwrap();
    };
    controller::enrich(&store, &fake, lease, &config(4), 1_000)
        .await
        .unwrap();
    let s = store.load().unwrap();
    let semantic: Vec<_> = s
        .edges
        .values()
        .filter(|e| e.graph == Graph::Semantic)
        .collect();
    assert_eq!(
        semantic.len(),
        1,
        "only 0.60 makes an edge; 0.59 and unknown do not"
    );
    assert_eq!(semantic[0].score, Some(0.60));
    assert_eq!(semantic[0].basis, EdgeBasis::JevInference);
    assert_eq!(
        s.nodes[&id(9)].enrichment.state,
        EnrichmentState::Partial,
        "an unknown is not a no"
    );
}

#[test]
fn caused_by_and_causes_point_in_opposite_directions() {
    let (new, cand) = (rec(2, "new", &["a"]), rec(1, "old", &["b"]));
    let edges = |caused_by: f64, causes: f64| {
        let answers = [("caused_by", noul(caused_by)), ("causes", noul(causes))].into();
        controller::pair_edges(&new, &cand, &answers, &config(4))
    };
    let e = edges(0.9, 0.1);
    let causal: Vec<_> = e.iter().filter(|e| e.graph == Graph::Causal).collect();
    assert_eq!(causal.len(), 1);
    assert_eq!(
        (causal[0].source.as_str(), causal[0].target.as_str()),
        (id(1).as_str(), id(2).as_str()),
        "candidate → new"
    );
    let e = edges(0.1, 0.9);
    let causal: Vec<_> = e.iter().filter(|e| e.graph == Graph::Causal).collect();
    assert_eq!(
        (causal[0].source.as_str(), causal[0].target.as_str()),
        (id(2).as_str(), id(1).as_str()),
        "new → candidate"
    );
}

#[tokio::test]
async fn alias_is_asked_only_when_ids_do_not_intersect_and_never_merges_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[
            rec(1, "cache layer", &["shared"]),
            rec(2, "cache layer", &["old-name"]),
            rec(9, "cache layer", &["shared", "new-name"]),
        ],
    );
    let fake = Fake::new(|text| match named(text) {
        "alias" => noul(0.95),
        _ => noul(0.0),
    });
    let lease = loop {
        let l = store.lease_next(0).unwrap().unwrap();
        if l.node_id() == id(9) {
            break l;
        }
        store
            .finish(l, ripwire_broker::memory::queue::Outcome::Done)
            .unwrap();
    };
    controller::enrich(&store, &fake, lease, &config(4), 1_000)
        .await
        .unwrap();
    let asked: Vec<String> = fake
        .seen
        .lock()
        .unwrap()
        .iter()
        .flat_map(|r| r.questions.0.iter().map(|(_, q)| q.instructions.clone()))
        .filter(|t| named(t) == "alias")
        .collect();
    assert_eq!(asked.len(), 1, "only for the pair without a shared id");
    let s = store.load().unwrap();
    let entity: Vec<_> = s
        .edges
        .values()
        .filter(|e| e.graph == Graph::Entity)
        .collect();
    assert!(
        entity
            .iter()
            .any(|e| e.basis == EdgeBasis::Deterministic && e.target == id(1))
    );
    assert!(
        entity.iter().any(|e| e.basis == EdgeBasis::JevInference
            && e.relation == "alias"
            && e.target == id(2))
    );
    let ids = |n| {
        s.nodes[&id(n)]
            .entities
            .iter()
            .map(|e| e.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        (ids(2), ids(9)),
        (
            vec!["old-name".to_string()],
            vec!["shared".into(), "new-name".into()]
        ),
        "never merged"
    );
}

#[tokio::test]
async fn implicit_time_is_a_choice_and_unknown_adds_no_edge() {
    for (picked, edges) in [("after", 1), ("unknown", 0)] {
        let dir = tempfile::tempdir().unwrap();
        let store = stored(
            dir.path(),
            &[
                timed(1, "cache layer", &["e"]),
                timed(9, "cache layer", &["e"]),
            ],
        );
        let fake = Fake::new(move |text| match named(text) {
            "time" => Decision::Choice {
                selected: picked.into(),
                probabilities: [
                    (
                        "after".to_string(),
                        if picked == "after" { 0.8 } else { 0.2 },
                    ),
                    (
                        "unknown".to_string(),
                        if picked == "unknown" { 0.8 } else { 0.2 },
                    ),
                ]
                .into(),
                confidence: None,
            },
            _ => noul(0.0),
        });
        let lease = loop {
            let l = store.lease_next(0).unwrap().unwrap();
            if l.node_id() == id(9) {
                break l;
            }
            store
                .finish(l, ripwire_broker::memory::queue::Outcome::Done)
                .unwrap();
        };
        controller::enrich(&store, &fake, lease, &config(4), 1_000)
            .await
            .unwrap();
        let s = store.load().unwrap();
        let temporal: Vec<_> = s
            .edges
            .values()
            .filter(|e| e.graph == Graph::Temporal && e.basis == EdgeBasis::JevInference)
            .collect();
        assert_eq!(temporal.len(), edges, "{picked}");
        if let Some(e) = temporal.first() {
            assert_eq!(
                (e.relation.as_str(), e.source.as_str()),
                ("after", id(9).as_str()),
                "new → candidate"
            );
        }
    }
}

// ---------------------------------------------------------------- pair transactions (PRD jev-mem §12; T2.8)

/// A fake that runs `during` on its first relations request, as another process would.
struct Meanwhile {
    inner: Fake,
    during: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

#[async_trait]
impl MemoryClassifier for Meanwhile {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        if req.state.get("candidates").is_some()
            && let Some(f) = self.during.lock().unwrap().take()
        {
            f();
        }
        self.inner.decide(req).await
    }
}

async fn enrich_node(store: &Store, fake: &dyn MemoryClassifier, n: u64) {
    let lease = loop {
        let l = store.lease_next(0).unwrap().unwrap();
        if l.node_id() == id(n) {
            break l;
        }
        store
            .finish(l, ripwire_broker::memory::queue::Outcome::Done)
            .unwrap();
    };
    controller::enrich(store, fake, lease, &config(4), 1_000)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_timeout_in_the_middle_of_a_pair_commits_nothing_of_that_pair() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])],
    );
    let fake = Fake::new(|text| match named(text) {
        "semantic" => noul(0.9),
        "caused_by" => Decision::Unknown {
            reason: ripwire_broker::online::response::Unknown::Absent,
        },
        _ => noul(0.0),
    });
    enrich_node(&store, &fake, 9).await;
    let s = store.load().unwrap();
    assert!(
        s.edges
            .values()
            .all(|e| e.basis == EdgeBasis::Deterministic),
        "one decision missing, none of the pair's inferences: {:?}",
        s.edges.values().map(|e| &e.relation).collect::<Vec<_>>()
    );
    assert!(
        s.edges.values().any(|e| e.relation == "shared_entity"),
        "the deterministic one stays"
    );
}

#[tokio::test]
async fn a_late_answer_for_an_old_generation_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])],
    );
    let path = dir.path().to_path_buf();
    let fake = Meanwhile {
        inner: Fake::new(|_| noul(0.9)),
        during: Mutex::new(Some(Box::new(move || {
            // Forgotten, its tombstone over, and observed again: the same id, a new generation.
            let other = Store::new(&path, &"c".repeat(64));
            other.forget(&id(9), 5).unwrap();
            other.sweep(10).unwrap();
            other.enqueue(&rec(9, "cache layer", &["e"])).unwrap();
            other.ingest().unwrap();
        }))),
    };
    enrich_node(&store, &fake, 9).await;
    let s = store.load().unwrap();
    assert_eq!(
        s.nodes[&id(9)].types.episodic,
        None,
        "the answers were about the old node"
    );
    assert!(s.edges.is_empty());
}

#[tokio::test]
async fn an_answer_for_a_forgotten_node_does_not_recreate_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])],
    );
    let path = dir.path().to_path_buf();
    let fake = Meanwhile {
        inner: Fake::new(|_| noul(0.9)),
        during: Mutex::new(Some(Box::new(move || {
            Store::new(&path, &"c".repeat(64))
                .forget(&id(9), u64::MAX)
                .unwrap();
        }))),
    };
    enrich_node(&store, &fake, 9).await;
    let s = store.load().unwrap();
    assert!(!s.nodes.contains_key(&id(9)));
    assert!(
        s.edges
            .values()
            .all(|e| e.source != id(9) && e.target != id(9))
    );
}

#[test]
fn the_enrichment_counter_increments_exactly_once_per_node() {
    use ripwire_broker::memory::queue::Outcome;
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &[rec(1, "cache layer", &["e"])]);
    let first = store.lease_next(0).unwrap().unwrap();
    store
        .finish(first, Outcome::Retry { not_before_ms: 0 })
        .unwrap();
    assert_eq!(
        store.load().unwrap().enriched,
        0,
        "a run that did not finish is not an enrichment"
    );
    let second = store.lease_next(0).unwrap().unwrap();
    store.finish(second, Outcome::Done).unwrap();
    assert_eq!(store.load().unwrap().enriched, 1);
    // Brought back by hand and run again: still one node.
    store
        .commit_enrichment(
            &id(1),
            store.load().unwrap().nodes[&id(1)].generation,
            None,
            EnrichmentState::Complete,
            vec![],
            &[],
        )
        .unwrap();
    assert!(
        !store.retry_failed(&id(1)).unwrap(),
        "only a failed job comes back"
    );
    assert_eq!(
        store.load().unwrap().enriched,
        1,
        "once per node, however many runs"
    );
}

#[tokio::test]
async fn an_edge_to_a_candidate_forgotten_meanwhile_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])],
    );
    let path = dir.path().to_path_buf();
    let fake = Meanwhile {
        inner: Fake::new(|_| noul(0.9)),
        during: Mutex::new(Some(Box::new(move || {
            Store::new(&path, &"c".repeat(64))
                .forget(&id(1), u64::MAX)
                .unwrap();
        }))),
    };
    enrich_node(&store, &fake, 9).await;
    let s = store.load().unwrap();
    assert!(s.nodes.contains_key(&id(9)), "the node itself stays");
    assert!(
        s.edges
            .values()
            .all(|e| e.target != id(1) && e.source != id(1)),
        "nothing points at a forgotten node"
    );
}

// ---------------------------------------------------------------- provider failures (PRD jev-mem §8.2, §12; T2.9)

use ripwire_broker::memory::controller::Worker;
use std::collections::VecDeque;
use std::sync::Arc;

/// Fails as scripted, one entry per request, then answers every Noul with 0.1.
struct Scripted {
    script: Mutex<VecDeque<Option<ClassifyError>>>,
    seen: Mutex<Vec<StateRequest>>,
    hang: bool,
}

impl Scripted {
    fn new(script: Vec<Option<ClassifyError>>) -> Arc<Self> {
        Arc::new(Scripted {
            script: Mutex::new(script.into()),
            seen: Mutex::default(),
            hang: false,
        })
    }
    fn sent(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

#[async_trait]
impl MemoryClassifier for Scripted {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.seen.lock().unwrap().push(req.clone());
        if self.hang {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
        if let Some(Some(e)) = self.script.lock().unwrap().pop_front() {
            return Err(e);
        }
        Ok(req.questions.0.iter().map(|_| noul(0.1)).collect())
    }
}

fn worker(store: &Arc<Store>, classifier: Arc<dyn MemoryClassifier>) -> Worker {
    Worker::new(store.clone(), classifier, config(4))
}

#[tokio::test]
async fn auth_failures_suspend_the_worker_until_reauthorized() {
    for status in [401, 403] {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
        let fake = Scripted::new(vec![Some(ClassifyError::Auth(status))]);
        let w = worker(&store, fake.clone());
        let ran = w.run_once(1_000).await.unwrap().expect("one job");
        assert_eq!(ran.state, EnrichmentState::Failed);
        assert!(w.is_suspended(), "{status}");
        assert!(
            w.run_once(1_000_000).await.unwrap().is_none(),
            "{status}: nothing more is sent"
        );
        assert_eq!(fake.sent(), 1, "{status}: never retried");
        w.reauthorize();
        assert!(
            w.run_once(1_000_000).await.unwrap().is_some(),
            "{status}: a new credential resumes"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_429_waits_only_inside_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let fake = Scripted::new(vec![Some(ClassifyError::RateLimited {
        retry_after: Some("2".into()),
    })]);
    let started = tokio::time::Instant::now();
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        ran.state,
        EnrichmentState::Complete,
        "waited 2 s, inside the 5 s budget, and retried"
    );
    assert!(started.elapsed() >= std::time::Duration::from_secs(2));
    assert_eq!(fake.sent(), 2);

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let fake = Scripted::new(vec![Some(ClassifyError::RateLimited {
        retry_after: Some("30".into()),
    })]);
    let started = tokio::time::Instant::now();
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ran.state, EnrichmentState::Failed);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "no wait past the budget"
    );
    assert_eq!(fake.sent(), 1);
    let job = &store.load().unwrap().jobs[&id(1)];
    assert_eq!(
        job.not_before_ms,
        1_000 + 30_000,
        "the provider's cooldown is kept for the next run"
    );

    // A second 429 inside the budget is not waited for again.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let twice = ClassifyError::RateLimited {
        retry_after: Some("1".into()),
    };
    let fake = Scripted::new(vec![Some(twice.clone()), Some(twice)]);
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (ran.state, fake.sent()),
        (EnrichmentState::Failed, 2),
        "one wait at most"
    );
}

#[tokio::test(start_paused = true)]
async fn a_huge_retry_after_neither_crashes_the_worker_nor_pins_the_job() {
    // The largest delay a header can carry, and ten years.
    for retry_after in ["18446744073709551615", "315360000"] {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
        let fake = Scripted::new(vec![Some(ClassifyError::RateLimited {
            retry_after: Some(retry_after.into()),
        })]);
        let ran = worker(&store, fake.clone())
            .run_once(1_000)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ran.state, EnrichmentState::Failed, "{retry_after}");
        let job = &store.load().unwrap().jobs[&id(1)];
        assert_eq!(
            job.not_before_ms,
            1_000 + controller::MAX_COOLDOWN.as_millis() as u64,
            "{retry_after}: asked again after the longest cooldown the worker keeps"
        );
    }
}

#[tokio::test]
async fn a_5xx_is_retried_once_inside_the_four_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let fake = Scripted::new(vec![Some(ClassifyError::Server(503))]);
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (ran.state, fake.sent()),
        (EnrichmentState::Complete, 2),
        "one retry"
    );

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let fake = Scripted::new(vec![
        Some(ClassifyError::Server(503)),
        Some(ClassifyError::Server(502)),
    ]);
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (ran.state, fake.sent()),
        (EnrichmentState::Failed, 2),
        "never a second retry"
    );

    // Every request failing, with candidates: four attempts in all for the job.
    let dir = tempfile::tempdir().unwrap();
    let records: Vec<Record> = (1..=12).map(|n| rec(n, "cache layer", &["e"])).collect();
    let store = Arc::new(stored(dir.path(), &records));
    let mut script = vec![None];
    script.extend(vec![Some(ClassifyError::Timeout); 40]);
    let fake = Scripted::new(script);
    let w = Worker::new(store.clone(), fake.clone(), config(10));
    for _ in 1..=11 {
        let l = store.lease_next(0).unwrap().unwrap();
        store
            .finish(l, ripwire_broker::memory::queue::Outcome::Done)
            .unwrap();
    }
    let _ = w.run_once(1_000).await.unwrap();
    assert!(fake.sent() <= 4, "{} attempts", fake.sent());
}

#[tokio::test]
async fn cancellation_stops_http_and_leaves_the_job_pending() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let fake = Arc::new(Scripted {
        script: Mutex::default(),
        seen: Mutex::default(),
        hang: true,
    });
    let w = worker(&store, fake.clone());
    let cut = tokio::time::timeout(std::time::Duration::from_millis(100), w.run_once(1_000)).await;
    assert!(cut.is_err(), "cancelled while the request was in flight");
    assert_eq!(
        store.load().unwrap().nodes[&id(1)].enrichment.state,
        EnrichmentState::Pending
    );
    let again = store
        .lease_next(1_000)
        .unwrap()
        .expect("the job is free for the next run");
    assert_eq!(again.run(), 2);
}

#[tokio::test]
async fn no_model_swap_and_no_silent_heuristic_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(2, "cache layer", &["e"])],
    ));
    let fake = Scripted::new(vec![
        Some(ClassifyError::Network),
        Some(ClassifyError::Network),
    ]);
    let ran = worker(&store, fake.clone())
        .run_once(1_000)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ran.state, EnrichmentState::Failed);
    assert!(
        fake.seen
            .lock()
            .unwrap()
            .iter()
            .all(|r| r.model == "jev-1.13.0"),
        "never another model"
    );
    let s = store.load().unwrap();
    let node = s
        .nodes
        .values()
        .find(|n| n.enrichment.state == EnrichmentState::Failed)
        .unwrap();
    assert_eq!(node.types.episodic, None, "nothing guessed in its place");
    assert!(
        s.edges
            .values()
            .all(|e| e.basis == EdgeBasis::Deterministic)
    );
}

#[tokio::test]
async fn one_remote_job_per_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(
        dir.path(),
        &[rec(1, "cache", &["a"]), rec(2, "layer", &["b"])],
    ));
    let hanging = Arc::new(Scripted {
        script: Mutex::default(),
        seen: Mutex::default(),
        hang: true,
    });
    let busy = Arc::new(worker(&store, hanging));
    let running = {
        let busy = busy.clone();
        tokio::spawn(async move { busy.run_once(1_000).await })
    };
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Another worker of the same workspace, in this or another process, waits its turn.
    let other = Store::new(dir.path(), &"c".repeat(64));
    let quick = Scripted::new(vec![]);
    let second = Worker::new(Arc::new(other), quick.clone(), config(4));
    assert!(
        second.run_once(1_000).await.unwrap().is_none(),
        "one remote job at a time"
    );
    assert_eq!(quick.sent(), 0);

    running.abort();
    let _ = running.await;
    assert!(
        second.run_once(1_000).await.unwrap().is_some(),
        "free once the first is gone"
    );
}

// ---------------------------------------------------------------- lifecycle (PRD jev-mem §4; T2.11)

use ripwire_broker::cli::{self, Command};
use ripwire_broker::memory::runtime::{self, DrainStop};

fn serve(argv: &[&str]) -> cli::ServeArgs {
    match cli::parse(argv.iter().map(|s| s.to_string()).collect()) {
        Ok(Command::Serve(s)) => s,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn online_alone_never_processes_old_memory_jobs() {
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let w = ws.path().to_str().unwrap();
    let fake = Scripted::new(vec![]);
    let online = serve(&["--workspace", w, "--online"]);
    assert!(
        runtime::from_serve(&online, state.path(), Some(fake.clone()))
            .unwrap()
            .is_none()
    );
    assert!(
        !state.path().join("memory").exists(),
        "not even a store is opened"
    );
    assert_eq!(fake.sent(), 0);

    let memory = serve(&["--workspace", w, "--memory"]);
    assert!(
        runtime::from_serve(&memory, state.path(), None).is_err(),
        "--memory needs the classifier"
    );
}

#[tokio::test]
async fn memory_and_online_memory_start_one_worker() {
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let w = ws.path().to_str().unwrap();
    let fake = Scripted::new(vec![]);
    let a = runtime::from_serve(
        &serve(&["--workspace", w, "--memory"]),
        state.path(),
        Some(fake.clone()),
    )
    .unwrap()
    .unwrap();
    let b = runtime::from_serve(
        &serve(&[
            "--workspace",
            w,
            "--online",
            "--memory",
            "--memory-write-candidates",
            "4",
        ]),
        state.path(),
        Some(fake),
    )
    .unwrap()
    .unwrap();
    assert_eq!(a.workspace_id(), b.workspace_id());
    assert_eq!(a.config().candidates, b.config().candidates);
    assert_eq!(a.publish().retention_ms, 30 * 24 * 60 * 60 * 1000);
}

#[tokio::test]
async fn the_server_leaves_no_process_with_the_credential_on_exit() {
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let fake = Scripted::new(vec![]);
    let args = serve(&["--workspace", ws.path().to_str().unwrap(), "--memory"]);
    let mut rt = runtime::from_serve(&args, state.path(), Some(fake.clone()))
        .unwrap()
        .unwrap();
    let store = Store::new(state.path(), rt.workspace_id());
    rt.start(std::time::Duration::from_millis(20));

    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while store.load().unwrap().enriched == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the worker incorporated and enriched it"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let sent = fake.sent();
    drop(rt); // the server stops

    store.enqueue(&rec(2, "another layer", &["f"])).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(fake.sent(), sent, "no worker outlives the server");
    assert_eq!(
        store.pending().unwrap(),
        1,
        "the new one waits, durable, for the next process"
    );
}

#[tokio::test(start_paused = true)]
async fn drain_stops_at_60s_or_20_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let records: Vec<Record> = (1..=25)
        .map(|n| rec(n, &format!("note {n}"), &[]))
        .collect();
    let store = Arc::new(stored(dir.path(), &records));
    let fast = Scripted::new(vec![]);
    let w = Worker::new(store.clone(), fast, config(0));
    let drained = runtime::drain(
        &store,
        &w,
        &ripwire_broker::memory::time::SystemClock,
        20,
        runtime::DRAIN_DEADLINE,
    )
    .await
    .unwrap();
    assert_eq!((drained.jobs, drained.stop), (20, DrainStop::Jobs));
    assert_eq!(runtime::DRAIN_DEADLINE, std::time::Duration::from_secs(60));

    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &records));
    let slow = Arc::new(Slow(Scripted::new(vec![])));
    let w = Worker::new(store.clone(), slow, config(0));
    let started = tokio::time::Instant::now();
    let drained = runtime::drain(
        &store,
        &w,
        &ripwire_broker::memory::time::SystemClock,
        20,
        runtime::DRAIN_DEADLINE,
    )
    .await
    .unwrap();
    assert_eq!(drained.stop, DrainStop::Deadline);
    assert!(drained.jobs < 20 && started.elapsed() <= std::time::Duration::from_secs(61));
}

/// Takes 25 s per request: the third job of a drain straddles its 60 s.
struct Slow(Arc<Scripted>);

#[async_trait]
impl MemoryClassifier for Slow {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        tokio::time::sleep(std::time::Duration::from_secs(25)).await;
        self.0.decide(req).await
    }
}

// ---------------------------------------------------------------- cost metrics (PRD jev-mem §8.3, §14; T2.13)

#[tokio::test]
async fn questions_attempts_bytes_and_cache_are_counted_per_operation_without_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(
        dir.path(),
        &[
            rec(1, "secret-ish cache layer text", &["src/cache.rs"]),
            rec(2, "secret-ish cache layer text two", &["src/cache.rs"]),
        ],
    ));
    let fake = Scripted::new(vec![Some(ClassifyError::Server(503))]);
    let w = worker(&store, fake.clone());
    while w.run_once(1_000).await.unwrap().is_some() {}

    let m = w.metrics();
    assert_eq!(
        m.typing.questions,
        4 * 2 + 4,
        "two nodes typed, one retried"
    );
    assert_eq!((m.typing.attempts, m.typing.retries), (3, 1));
    assert_eq!(m.typing.failures.get("server"), Some(&1));
    assert!(m.relations.attempts >= 1 && m.relations.questions >= 4);
    assert!(m.typing.bytes_sent > 0 && m.relations.bytes_sent > 0);
    assert_eq!(m.jobs_done, 2);
    assert_eq!(
        fake.sent() as u64,
        m.typing.attempts + m.relations.attempts,
        "every attempt counted"
    );

    let shown = serde_json::to_string(&m).unwrap();
    for leak in ["secret-ish", "cache layer", "src/cache.rs", "Bearer"] {
        assert!(!shown.contains(leak), "{leak} in {shown}");
    }

    // The 24-hour budget it used is on disk, for `memory status` and any other process.
    let (attempts, questions) = store
        .ledger()
        .unwrap()
        .entries
        .iter()
        .fold((0, 0), |(a, q), e| (a + e.1, q + e.2));
    assert_eq!(
        (attempts as u64, questions as u64),
        (
            m.typing.attempts + m.relations.attempts,
            m.typing.questions + m.relations.questions
        )
    );
}

// ---------------------------------------------------------------- review of phase 2 (D-138)

#[tokio::test]
async fn a_spent_quota_keeps_jobs_pending_and_uses_no_run() {
    let dir = tempfile::tempdir().unwrap();
    let limits = ripwire_broker::memory::store::Limits {
        attempts_per_day: 0,
        ..Default::default()
    };
    let store = Store::with_limits(dir.path(), &"c".repeat(64), limits);
    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    store.ingest().unwrap();
    let store = Arc::new(store);
    let fake = Scripted::new(vec![]);
    let w = worker(&store, fake.clone());
    for minute in 0..5u64 {
        let _ = w.run_once(minute * 3_600_000).await.unwrap();
    }
    let job = &store.load().unwrap().jobs[&id(1)];
    assert_eq!(
        job.state,
        ripwire_broker::memory::queue::JobState::Pending,
        "kept, not failed"
    );
    assert_eq!(job.runs, 0, "a run that sent nothing does not count");
    assert_eq!(fake.sent(), 0);
    assert_eq!(w.metrics().typing.quota_refusals, 5);
}

#[tokio::test]
async fn a_relations_failure_leaves_the_job_pending_and_is_not_counted() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(
        dir.path(),
        &[rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])],
    ));
    for _ in 0..1 {
        let l = store.lease_next(0).unwrap().unwrap();
        store
            .finish(l, ripwire_broker::memory::queue::Outcome::Done)
            .unwrap();
    }
    let before = store.load().unwrap().enriched;
    // Typing answers; the relations request fails twice (one retry).
    let fake = Scripted::new(vec![
        None,
        Some(ClassifyError::Network),
        Some(ClassifyError::Network),
    ]);
    let w = worker(&store, fake.clone());
    let ran = w.run_once(1_000).await.unwrap().unwrap();
    assert_eq!(ran.state, EnrichmentState::Partial);
    let s = store.load().unwrap();
    assert_eq!(
        s.jobs[&id(9)].state,
        ripwire_broker::memory::queue::JobState::Pending,
        "the pairs are still owed"
    );
    assert_eq!(
        s.enriched, before,
        "not counted until its planned stages finish"
    );
    assert!(
        s.nodes[&id(9)].types.episodic.is_some(),
        "what did arrive is kept"
    );

    let again = w.run_once(1_000_000).await.unwrap().unwrap();
    assert_eq!(again.state, EnrichmentState::Complete);
    let s = store.load().unwrap();
    assert_eq!(
        (s.jobs[&id(9)].state, s.enriched),
        (ripwire_broker::memory::queue::JobState::Done, before + 1)
    );
}

#[tokio::test]
async fn a_run_that_paid_for_typing_keeps_its_count_when_the_quota_runs_out() {
    use ripwire_broker::memory::store::Limits;
    // Room in the 24-hour quota for the typing request's four questions, not for the relations.
    let dir = tempfile::tempdir().unwrap();
    let limits = Limits {
        questions_per_day: 4,
        ..Limits::default()
    };
    let store = Store::with_limits(dir.path(), &"c".repeat(64), limits);
    for r in [rec(1, "cache layer", &["e"]), rec(9, "cache layer", &["e"])] {
        store.enqueue(&r).unwrap();
    }
    store.ingest().unwrap();
    let store = Arc::new(store);
    let l = store.lease_next(0).unwrap().unwrap();
    store
        .finish(l, ripwire_broker::memory::queue::Outcome::Done)
        .unwrap();
    let fake = Scripted::new(vec![None]);
    worker(&store, fake.clone()).run_once(1_000).await.unwrap();
    assert_eq!(fake.sent(), 1, "typing went out");
    let job = &store.load().unwrap().jobs[&id(9)];
    assert_eq!(
        job.runs, 1,
        "a run that sent something is a run: it is not given back"
    );
}

#[test]
fn commits_stay_inside_the_edge_and_snapshot_caps() {
    use ripwire_broker::memory::model::Edge;
    use ripwire_broker::memory::store::Limits;
    assert_eq!(Limits::default().max_edges, 32_000, "PRD jev-mem §6");
    let edge = |s: u64, t: u64, relation: &str| Edge {
        source: id(s),
        target: id(t),
        graph: Graph::Semantic,
        relation: relation.into(),
        basis: EdgeBasis::JevInference,
        score: Some(0.9),
        model: Some("m".into()),
        prompt_version: None,
        policy: "p".into(),
        generation: 0,
    };
    let make = |limits: Limits| {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::with_limits(dir.path(), &"c".repeat(64), limits);
        for n in 1..=3 {
            store.enqueue(&rec(n, "cache layer", &["e"])).unwrap();
        }
        store.ingest().unwrap();
        (dir, store)
    };

    let (_d, store) = make(Limits {
        max_edges: 2,
        ..Default::default()
    });
    let g = store.load().unwrap().nodes[&id(1)].generation;
    let three = vec![edge(1, 2, "a"), edge(1, 3, "b"), edge(2, 3, "c")];
    store
        .commit_enrichment(&id(1), g, None, EnrichmentState::Complete, three, &[])
        .unwrap();
    assert_eq!(
        store.load().unwrap().edges.len(),
        2,
        "never past the edge cap"
    );

    let (d, store) = make(Limits::default());
    let size = std::fs::metadata(
        d.path()
            .join("memory")
            .join("c".repeat(64))
            .join("snapshot.json"),
    )
    .unwrap()
    .len();
    let tight = Store::with_limits(
        d.path(),
        &"c".repeat(64),
        Limits {
            snapshot_bytes: size + 400,
            ..Default::default()
        },
    );
    let g = store.load().unwrap().nodes[&id(1)].generation;
    let wide: Vec<Edge> = (0..20)
        .map(|i| edge(1, 2, &format!("{i}{}", "r".repeat(100))))
        .collect();
    let types = ripwire_broker::memory::model::Types {
        episodic: Some(0.5),
        ..Default::default()
    };
    tight
        .commit_enrichment(&id(1), g, Some(types), EnrichmentState::Complete, wide, &[])
        .unwrap();
    let s = tight.load().unwrap();
    assert!(
        s.edges.is_empty(),
        "the edges that would overflow the snapshot are left out"
    );
    assert_eq!(
        s.nodes[&id(1)].types.episodic,
        Some(0.5),
        "what fits is kept"
    );
}

#[tokio::test]
async fn drain_says_when_it_could_not_run_instead_of_reporting_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &[rec(1, "cache layer", &["e"])]));
    let clock = ripwire_broker::memory::time::SystemClock;
    let held = store.remote_slot().unwrap().unwrap(); // a running server
    let w = Worker::new(store.clone(), Scripted::new(vec![]), config(4));
    let busy = runtime::drain(&store, &w, &clock, 20, runtime::DRAIN_DEADLINE)
        .await
        .unwrap();
    assert_eq!((busy.jobs, busy.stop), (0, DrainStop::Busy));
    drop(held);

    let refused = Worker::new(
        store.clone(),
        Scripted::new(vec![Some(ClassifyError::Auth(401))]),
        config(4),
    );
    let suspended = runtime::drain(&store, &refused, &clock, 20, runtime::DRAIN_DEADLINE)
        .await
        .unwrap();
    assert_eq!(suspended.stop, DrainStop::Suspended);
}

#[tokio::test]
async fn serve_with_memory_reads_with_its_flags() {
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let args = serve(&[
        "--workspace",
        ws.path().to_str().unwrap(),
        "--memory",
        "--memory-read-deadline-ms",
        "400",
        "--memory-read-request-limit",
        "2",
    ]);
    let rt = runtime::from_serve(&args, state.path(), Some(Scripted::new(vec![])))
        .unwrap()
        .unwrap();
    let read = rt
        .publish()
        .read
        .as_ref()
        .expect("context_for_task reads memory");
    assert_eq!(read.cfg.deadline, std::time::Duration::from_millis(400));
    assert_eq!(read.cfg.request_limit, 2);
    assert_eq!(read.cfg.model, runtime::DEFAULT_MODEL);
    assert_eq!(
        read.store.dir(),
        Store::new(state.path(), rt.workspace_id()).dir()
    );
}

#[tokio::test]
async fn deterministic_memory_collects_and_ingests_but_never_enriches() {
    use ripwire_broker::memory::retrieve::Selection;
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let fake = Scripted::new(vec![]);
    let args = serve(&[
        "--workspace",
        ws.path().to_str().unwrap(),
        "--memory",
        "--memory-selection",
        "deterministic",
    ]);
    let mut rt = runtime::from_serve(&args, state.path(), Some(fake.clone()))
        .unwrap()
        .unwrap();
    let read = rt.publish().read.as_ref().expect("it reads memory");
    assert_eq!(read.cfg.selection, Selection::Deterministic);
    let store = Store::new(state.path(), rt.workspace_id());
    rt.start(std::time::Duration::from_millis(20));

    // The same collection and store: the observation is incorporated...
    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while store.load().unwrap().nodes.is_empty() {
        assert!(std::time::Instant::now() < deadline, "incorporated");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // ...and never sent to the classifier.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(fake.sent(), 0);
    assert_eq!(store.load().unwrap().enriched, 0);
}

/// A store the worker cannot use is said on stderr instead of the memory going quiet, and once,
/// not on every tick (D-146). The real server, with a snapshot that no longer parses; nothing is
/// sent, since deterministic selection only ingests and sweeps.
#[cfg(feature = "online")]
#[test]
fn the_worker_says_once_on_stderr_when_it_cannot_use_the_store() {
    use std::io::BufRead;
    let (ws, xdg) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let ws_path = ws.path().canonicalize().unwrap();
    let id = ripwire_broker::memory::identity::workspace_id(&ws_path).unwrap();
    let store = Store::new(&xdg.path().join("ripwire-broker"), &id);
    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    store.ingest().unwrap();
    store.enqueue(&rec(2, "queue layer", &["f"])).unwrap();
    std::fs::write(store.dir().join("snapshot.json"), "{not json").unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ripwire-broker"))
        .args(["--workspace", ws_path.to_str().unwrap()])
        .args(["--ripwire", "/nonexistent/ripwire", "--memory"])
        .args(["--memory-selection", "deterministic"])
        .env("XDG_STATE_HOME", xdg.path())
        .env("RIPWIRE_BROKER_JEV_API_KEY", "tok-unused")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let err = child.stderr.take().unwrap();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });

    // The worker ticks every 5 s: past the second tick, a repeat would be there.
    let until = std::time::Instant::now() + std::time::Duration::from_millis(6_500);
    let mut lines = vec![];
    while let Ok(line) = rx.recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
    {
        lines.push(line);
    }
    let _ = child.kill();
    let _ = child.wait();

    let worker: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("memory worker"))
        .collect();
    for stage in ["retention", "ingest"] {
        let said = worker.iter().filter(|l| l.contains(stage)).count();
        assert_eq!(said, 1, "{stage}: {lines:#?}");
    }
    assert!(worker.iter().all(|l| l.contains("Corrupt")), "{worker:#?}");
}

/// A provider that refuses the credential stops the worker until the server restarts, and the
/// worker says so, once (D-146): the job that met the 401 comes back as a run, and every call after
/// it finds the worker suspended, neither of which is a failure of the store.
#[tokio::test]
async fn a_refused_credential_is_said_once() {
    let (ws, state) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let fake = Scripted::new(vec![Some(ClassifyError::Auth(401))]);
    let args = serve(&["--workspace", ws.path().to_str().unwrap(), "--memory"]);
    let mut rt = runtime::from_serve(&args, state.path(), Some(fake.clone()))
        .unwrap()
        .unwrap();
    let store = Store::new(state.path(), rt.workspace_id());
    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let tx = std::sync::Mutex::new(tx);
    rt.start_reporting(std::time::Duration::from_millis(20), move |line| {
        let _ = tx.lock().unwrap().send(line.to_string());
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fake.sent() == 0 {
        assert!(std::time::Instant::now() < deadline, "the job was sent");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // Several ticks of a suspended worker.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    drop(rt);

    let said: Vec<String> = rx.try_iter().collect();
    assert_eq!(said.len(), 1, "{said:#?}");
    assert!(said[0].contains("refused the credential"), "{said:#?}");
}

/// `serve --state-dir DIR` keeps memory where a hook run with the same `--state-dir` put its spool
/// (D-147): without it the server only knew the default directory, and that spool was never read.
#[cfg(feature = "online")]
#[test]
fn serve_reads_the_spool_of_the_state_dir_it_is_given() {
    let (ws, dir, xdg) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let ws_path = ws.path().canonicalize().unwrap();
    let id = ripwire_broker::memory::identity::workspace_id(&ws_path).unwrap();
    let store = Store::new(dir.path(), &id);
    store.enqueue(&rec(1, "cache layer", &["e"])).unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_ripwire-broker"))
        .args(["--workspace", ws_path.to_str().unwrap()])
        .args(["--ripwire", "/nonexistent/ripwire", "--memory"])
        .args(["--memory-selection", "deterministic", "--state-dir"])
        .arg(dir.path())
        .env("XDG_STATE_HOME", xdg.path())
        .env("RIPWIRE_BROKER_JEV_API_KEY", "tok-unused")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut ingested = false;
    while !ingested && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        ingested = store.load().is_ok_and(|s| !s.nodes.is_empty());
    }
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        ingested,
        "the server never incorporated the spool under --state-dir"
    );
}

/// A clock that, the first time it is read, takes the workspace's remote slot, as a `serve`
/// starting right after a drain's first check would.
struct SlotTaker {
    store: Arc<Store>,
    held: std::sync::Mutex<Option<std::fs::File>>,
}

impl ripwire_broker::memory::time::Clock for SlotTaker {
    fn now_ms(&self) -> u64 {
        let mut held = self.held.lock().unwrap();
        if held.is_none() {
            *held = Some(self.store.remote_slot().unwrap().expect("free at first"));
        }
        ripwire_broker::memory::time::SystemClock.now_ms()
    }
}

/// A drain whose slot was taken after its first check says busy, not empty, while jobs remain
/// (D-150): it only probed the slot, released it at once, and read the worker's "nothing" as an
/// empty queue.
#[tokio::test]
async fn a_drain_that_loses_the_slot_says_busy_not_empty() {
    let dir = tempfile::tempdir().unwrap();
    let records: Vec<Record> = (1..=3).map(|n| rec(n, &format!("note {n}"), &[])).collect();
    let store = Arc::new(stored(dir.path(), &records));
    let w = Worker::new(store.clone(), Scripted::new(vec![]), config(0));
    let clock = SlotTaker {
        store: store.clone(),
        held: std::sync::Mutex::new(None),
    };

    let drained = runtime::drain(&store, &w, &clock, 20, runtime::DRAIN_DEADLINE)
        .await
        .unwrap();

    assert_eq!((drained.jobs, drained.stop), (0, DrainStop::Busy));
}
