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
