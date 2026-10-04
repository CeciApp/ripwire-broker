//! Seam: consolidation (PRD jev-mem §9): when a round is due, which pairs it asks about, what it
//! records and what it never touches. A classifier stand-in, no network.
mod common;

use async_trait::async_trait;
use ripwire_broker::memory::consolidate::{self, Config};
use ripwire_broker::memory::controller::{self, Worker};
use ripwire_broker::memory::model::Record;
use ripwire_broker::memory::prompts::{self, Stage};
use ripwire_broker::memory::queue::Outcome;
use ripwire_broker::memory::runtime;
use ripwire_broker::memory::store::Store;
use ripwire_broker::memory::time::Clock;
use ripwire_broker::online::classifier::{ClassifyError, MemoryClassifier};
use ripwire_broker::online::request::StateRequest;
use ripwire_broker::online::response::Decision;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MODEL: &str = "jev-1.13.0";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const T0: u64 = 1_000_000;

fn ws() -> String {
    "c".repeat(64)
}

fn id(n: u64) -> String {
    format!("{n:064}")
}

fn rec(n: u64, content: &str, entities: &[&str]) -> Record {
    serde_json::from_value(json!({
        "schema_version": 1, "policy_version": "memory-policy/v1",
        "node_id": id(n), "content_hash": id(n), "workspace_id": ws(), "event_key": "e",
        "kind": "edit_observation", "content": content, "observed_at_ms": n, "ingest_seq": 0,
        "timestamp_role": "observation",
        "entities": entities.iter().map(|e| json!({"id": e, "kind": "file", "path": e})).collect::<Vec<_>>(),
        "expires_at_ms": u64::MAX, "generation": 0
    }))
    .unwrap()
}

fn stored(dir: &std::path::Path, records: &[Record]) -> Store {
    let store = Store::new(dir, &ws());
    for r in records {
        store.enqueue(r).unwrap();
    }
    store.ingest().unwrap();
    store
}

/// `n` jobs finish their planned stages at `at_ms`, as the worker would end them.
fn enrich(store: &Store, n: usize, at_ms: u64) {
    for _ in 0..n {
        let lease = store.lease_next(at_ms).unwrap().expect("a job");
        store.finish(lease, Outcome::Done).unwrap();
    }
}

/// `n` observations of the same file: every one is a neighbour of every other.
fn same_file(n: u64) -> Vec<Record> {
    (1..=n)
        .map(|i| rec(i, &format!("cache layer change {i}"), &["src/cache.rs"]))
        .collect()
}

type Answer = Box<dyn Fn(usize, &str) -> Decision + Send + Sync>;

/// Answers each consolidation question by pair and name, and keeps every request.
struct Fake {
    answer: Answer,
    names: HashMap<String, (usize, &'static str)>,
    seen: Mutex<Vec<StateRequest>>,
}

impl Fake {
    fn new(answer: impl Fn(usize, &str) -> Decision + Send + Sync + 'static) -> Self {
        let names = (0..8)
            .flat_map(|i| {
                prompts::questions(Stage::Consolidation, i)
                    .into_iter()
                    .map(move |(n, q)| (q.instructions, (i, n)))
            })
            .collect();
        Fake {
            answer: Box::new(answer),
            names,
            seen: Mutex::default(),
        }
    }

    fn requests(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

#[async_trait]
impl MemoryClassifier for Fake {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.seen.lock().unwrap().push(req.clone());
        Ok(req
            .questions
            .0
            .iter()
            .map(|(_, q)| {
                let (pair, name) = self.names[&q.instructions];
                (self.answer)(pair, name)
            })
            .collect())
    }
}

fn noul(p: f64) -> Decision {
    Decision::Noul { probability: p }
}

fn choice(selected: &str, p: f64) -> Decision {
    let options = ["keep_separate", "merge", "promote", "uncertain"];
    let rest = (1.0 - p) / 3.0;
    Decision::Choice {
        selected: selected.into(),
        probabilities: options
            .iter()
            .map(|o| (o.to_string(), if *o == selected { p } else { rest }))
            .collect::<BTreeMap<_, _>>(),
        confidence: None,
    }
}

/// Keeps every pair separate, with nothing redundant, contradictory or obsolete.
fn separate(_: usize, name: &str) -> Decision {
    match name {
        "representation" => choice("keep_separate", 0.9),
        _ => noul(0.1),
    }
}

fn unknown(_: usize, _: &str) -> Decision {
    Decision::Unknown {
        reason: ripwire_broker::online::response::Unknown::Absent,
    }
}

// ---------------------------------------------------------------- cadence (T4.1, CA-7)

#[test]
fn a_crash_at_write_19_keeps_the_counter_at_19() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(20));
    enrich(&store, 19, T0);
    drop(store);

    // The process died after its 19th enrichment: the next one reads the same disk.
    let store = Store::new(dir.path(), &ws());
    let s = store.load().unwrap();
    assert_eq!(s.enriched, 19, "the counter is on disk");
    assert!(!consolidate::due(&s, T0 + 1), "19 is not 20");

    enrich(&store, 1, T0);
    assert!(consolidate::due(&store.load().unwrap(), T0 + 1));
}

#[tokio::test]
async fn pairs_pending_for_24h_trigger_without_the_twentieth_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(2));
    enrich(&store, 2, T0);
    let s = store.load().unwrap();
    assert_eq!(consolidate::pending(&s, MODEL).len(), 1);
    assert!(!consolidate::due(&s, T0 + DAY_MS - 1));
    assert!(consolidate::due(&s, T0 + DAY_MS), "pending for 24 h");

    let fake = Fake::new(separate);
    let round = consolidate::round(&store, &fake, &Config::new(MODEL), T0 + DAY_MS)
        .await
        .unwrap()
        .expect("a round was due");
    assert_eq!(round.decided, 1);
    let s = store.load().unwrap();
    assert!(consolidate::pending(&s, MODEL).is_empty());
    assert!(
        !consolidate::due(&s, T0 + 3 * DAY_MS),
        "nothing pending, nothing due"
    );
    let again = consolidate::round(&store, &fake, &Config::new(MODEL), T0 + 3 * DAY_MS)
        .await
        .unwrap();
    assert!(again.is_none());
    assert_eq!(fake.requests(), 1);
}

struct At(u64);

impl Clock for At {
    fn now_ms(&self) -> u64 {
        self.0
    }
}

#[tokio::test(start_paused = true)]
async fn no_background_timer_runs_without_a_process() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(dir.path(), &same_file(20)));
    enrich(&store, 20, T0);
    let snapshot = store.dir().join("snapshot.json");
    let before = std::fs::read(&snapshot).unwrap();

    // Two days with no process: nothing was left behind to run.
    tokio::time::advance(Duration::from_secs(2 * DAY_MS / 1000)).await;
    tokio::task::yield_now().await;
    assert_eq!(std::fs::read(&snapshot).unwrap(), before);

    // The next process to open the store runs what came due.
    let fake = Arc::new(Fake::new(separate));
    let worker = Worker::new(
        store.clone(),
        fake.clone(),
        controller::Config {
            model: MODEL.into(),
            candidates: 4,
        },
    );
    runtime::drain(
        &store,
        &worker,
        &At(T0 + 2 * DAY_MS),
        20,
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    assert_eq!(fake.requests(), 1, "one round");
    let s = store.load().unwrap();
    assert_eq!(s.consolidation.counted, 20);
    assert!(!s.consolidation.decisions.is_empty());
}

#[tokio::test]
async fn the_cursor_does_not_repeat_the_same_pairs() {
    let dir = tempfile::tempdir().unwrap();
    // Four observations of one file: six pairs, more than a round asks about.
    let store = stored(dir.path(), &same_file(4));
    enrich(&store, 4, T0);
    let all = consolidate::pending(&store.load().unwrap(), MODEL);
    assert_eq!(all.len(), 6);

    // No answer is usable: every pair stays pending, and the cursor alone moves the rounds on.
    let fake = Fake::new(unknown);
    let cfg = Config::new(MODEL);
    let first = consolidate::round(&store, &fake, &cfg, T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.asked, all[..4]);
    assert_eq!(first.decided, 0);
    assert_eq!(consolidate::pending(&store.load().unwrap(), MODEL), all);

    let second = consolidate::round(&store, &fake, &cfg, T0 + 2 * DAY_MS)
        .await
        .unwrap()
        .expect("still pending a day later");
    assert_eq!(
        second.asked,
        [&all[4..], &all[..2]].concat(),
        "after the cursor first, then around"
    );
}

// ---------------------------------------------------------------- a round (T4.2, CA-14)

/// Answers like `inner`, each request after `delay`, or fails each one with `fail`.
struct Slow<F> {
    inner: F,
    delay: Duration,
    fail: Option<fn() -> ClassifyError>,
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl<F: MemoryClassifier> MemoryClassifier for Slow<F> {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        match self.fail {
            Some(error) => Err(error()),
            None => self.inner.decide(req).await,
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_round_asks_at_most_four_pairs_twenty_questions_in_five_seconds() {
    assert_eq!(consolidate::MAX_PAIRS, 4);
    assert_eq!(consolidate::MAX_QUESTIONS, 20);
    assert_eq!(consolidate::DEADLINE, Duration::from_secs(5));
    assert_eq!(consolidate::MAX_ATTEMPTS, 4);
    let cfg = Config::new(MODEL);

    // Ten pairs pending: one round asks about four of them, twenty questions.
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(5));
    enrich(&store, 5, T0);
    assert_eq!(
        consolidate::pending(&store.load().unwrap(), MODEL).len(),
        10
    );
    let fake = Fake::new(separate);
    let round = consolidate::round(&store, &fake, &cfg, T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(round.asked.len(), 4);
    let seen = fake.seen.lock().unwrap().clone();
    let questions: usize = seen.iter().map(|r| r.questions.0.len()).sum();
    assert_eq!(questions, 20);
    assert_eq!(round.decided, 4);

    // A provider slower than the round: it ends at five seconds, with nothing decided.
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(5));
    enrich(&store, 5, T0);
    let slow = Slow {
        inner: Fake::new(separate),
        delay: Duration::from_secs(30),
        fail: None,
        calls: Default::default(),
    };
    let start = tokio::time::Instant::now();
    let round = consolidate::round(&store, &slow, &cfg, T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    assert!(
        start.elapsed() <= consolidate::DEADLINE + Duration::from_millis(10),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(round.decided, 0);
    assert_eq!(
        consolidate::pending(&store.load().unwrap(), MODEL).len(),
        10
    );

    // A provider that keeps failing: a retry, inside the four attempts, and no more.
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(5));
    enrich(&store, 5, T0);
    let failing = Slow {
        inner: Fake::new(separate),
        delay: Duration::ZERO,
        fail: Some(|| ClassifyError::Network),
        calls: Default::default(),
    };
    let round = consolidate::round(&store, &failing, &cfg, T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    let calls = failing.calls.load(std::sync::atomic::Ordering::SeqCst);
    assert!((2..=4).contains(&calls), "{calls} attempts");
    assert_eq!(round.metrics.attempts as usize, calls);
    assert_eq!(round.metrics.retries, 1);
}

#[tokio::test]
async fn originals_are_never_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(3));
    enrich(&store, 3, T0);
    let before = store.load().unwrap().nodes;

    // Everything says the two are one: redundant, the newer making the older obsolete, merge.
    let fake = Fake::new(|_, name| match name {
        "representation" => choice("merge", 0.99),
        "contradiction" => noul(0.0),
        _ => noul(0.99),
    });
    let round = consolidate::round(&store, &fake, &Config::new(MODEL), T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(round.decided, 3);
    let s = store.load().unwrap();
    assert_eq!(s.nodes, before, "every original, unchanged");
    assert!(
        s.consolidation
            .decisions
            .values()
            .all(|d| d.obsolescence > 0.9 && d.redundancy > 0.9)
    );
}

#[tokio::test]
async fn links_and_decisions_work_without_a_summarizer() {
    use ripwire_broker::memory::consolidate::RepresentationDecision;
    use ripwire_broker::memory::model::{EdgeBasis, Graph, Kind};
    let dir = tempfile::tempdir().unwrap();
    let store = stored(dir.path(), &same_file(3));
    enrich(&store, 3, T0);
    let pairs = consolidate::pending(&store.load().unwrap(), MODEL);
    assert_eq!(pairs.len(), 3);

    // Only the first pair is worth linking; the others are decided all the same.
    let fake = Fake::new(|pair, name| match (pair, name) {
        (_, "representation") => choice("promote", 0.95),
        (0, "link_usefulness") => noul(0.9),
        (_, "link_usefulness") => noul(0.3),
        _ => noul(0.2),
    });
    let round = consolidate::round(&store, &fake, &Config::new(MODEL), T0 + DAY_MS)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(round.decided, 3);

    let s = store.load().unwrap();
    let decisions: Vec<_> = s.consolidation.decisions.values().collect();
    assert_eq!(decisions.len(), 3);
    for d in &decisions {
        assert_eq!(d.representation, RepresentationDecision::Promote);
        assert_eq!(d.probability, 0.95);
        assert_eq!(
            (d.redundancy, d.contradiction, d.obsolescence),
            (0.2, 0.2, 0.2)
        );
    }
    let links: Vec<_> = s
        .edges
        .values()
        .filter(|e| e.relation == consolidate::LINK)
        .collect();
    assert_eq!(links.len(), 1, "one pair was worth linking");
    let link = links[0];
    let mut ends = [link.source.clone(), link.target.clone()];
    ends.sort();
    assert_eq!(ends, [pairs[0].first.clone(), pairs[0].second.clone()]);
    assert_eq!(
        (link.graph, link.basis, link.score),
        (Graph::Semantic, EdgeBasis::JevInference, Some(0.9))
    );
    assert!(
        s.nodes.values().all(|r| r.kind != Kind::DerivedNote),
        "no note without a summarizer"
    );
}

#[tokio::test]
async fn a_pair_missing_any_one_answer_is_not_decided() {
    for missing in [
        "redundancy",
        "contradiction",
        "obsolescence",
        "link_usefulness",
        "representation",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = stored(dir.path(), &same_file(3));
        enrich(&store, 3, T0);
        let pairs = consolidate::pending(&store.load().unwrap(), MODEL);
        // The first pair lacks one answer; absence is never a zero.
        let fake = Fake::new(move |pair, name| match (pair, name) {
            (0, n) if n == missing => unknown(0, n),
            (_, "representation") => choice("merge", 0.9),
            _ => noul(0.9),
        });
        let round = consolidate::round(&store, &fake, &Config::new(MODEL), T0 + DAY_MS)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(round.decided, 2, "{missing}");
        assert_eq!(
            consolidate::pending(&store.load().unwrap(), MODEL),
            [pairs[0].clone()],
            "{missing}: still pending"
        );
        let links = store.load().unwrap().edges.len();
        assert_eq!(links, 2, "{missing}: no link for the undecided pair");
    }
}

// ---------------------------------------------------------------- derived notes (T4.3, CA-14)

use common::summarizer::FakeSummarizer;
use ripwire_broker::memory::model::Kind;

const NOTE: &str = "Observed twice: a cache layer change in src/cache.rs; tests unknown.";

/// Two enriched observations of `src/cache.rs` with these contents, and one round of `answer`
/// with `summarizer`.
async fn consolidated(
    contents: [&str; 2],
    answer: impl Fn(usize, &str) -> Decision + Send + Sync + 'static,
    summarizer: Option<Arc<FakeSummarizer>>,
) -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = stored(
        dir.path(),
        &[
            rec(1, contents[0], &["src/cache.rs"]),
            rec(2, contents[1], &["src/cache.rs"]),
        ],
    );
    enrich(&store, 2, T0);
    let mut cfg = Config::new(MODEL);
    if let Some(s) = summarizer {
        cfg = cfg.with_summarizer(s);
    }
    consolidate::round(&store, &Fake::new(answer), &cfg, T0 + DAY_MS)
        .await
        .unwrap()
        .expect("due");
    (dir, store)
}

fn gated(
    representation: &'static str,
    p: f64,
    contradiction: f64,
) -> impl Fn(usize, &str) -> Decision {
    move |_, name| match name {
        "representation" => choice(representation, p),
        "contradiction" => noul(contradiction),
        _ => noul(0.5),
    }
}

fn notes(store: &Store) -> Vec<Record> {
    let s = store.load().unwrap();
    s.nodes
        .into_values()
        .filter(|r| r.kind == Kind::DerivedNote)
        .collect()
}

const TWO: [&str; 2] = ["cache layer change one", "cache layer change two"];

#[tokio::test]
async fn only_merge_or_promote_at_085_with_contradiction_below_085_calls_the_summarizer() {
    let cases = [
        ("merge", 0.85, 0.84, true),
        ("promote", 0.99, 0.0, true),
        ("merge", 0.84, 0.0, false),
        ("merge", 0.99, 0.85, false),
        ("keep_separate", 0.99, 0.0, false),
        ("uncertain", 0.99, 0.0, false),
    ];
    for (representation, p, contradiction, calls) in cases {
        let summarizer = Arc::new(FakeSummarizer::replying(NOTE));
        let (_dir, store) = consolidated(
            TWO,
            gated(representation, p, contradiction),
            Some(summarizer.clone()),
        )
        .await;
        let case = format!("{representation} {p}, contradiction {contradiction}");
        let s = store.load().unwrap();
        let decision = s.consolidation.decisions.values().next().expect("decided");
        let notes = notes(&store);
        assert_eq!(summarizer.prompts().len(), usize::from(calls), "{case}");
        assert_eq!(notes.len(), usize::from(calls), "{case}");
        if !calls {
            assert_eq!(decision.note, None, "{case}");
            continue;
        }
        let prompt = &summarizer.prompts()[0];
        assert!(
            prompt.contains(TWO[0]) && prompt.contains(TWO[1]),
            "{case}: whole parents"
        );
        let note = &notes[0];
        assert_eq!(note.content, NOTE);
        let parents: Vec<(&str, &str)> = note
            .derived_from
            .iter()
            .map(|p| (p.node_id.as_str(), p.content_hash.as_str()))
            .collect();
        let (one, two) = (&s.nodes[&id(1)], &s.nodes[&id(2)]);
        assert_eq!(
            parents,
            [
                (one.node_id.as_str(), one.content_hash.as_str()),
                (two.node_id.as_str(), two.content_hash.as_str())
            ]
        );
        assert_eq!(note.enrichment.model.as_deref(), Some("fake-model"));
        assert_eq!(
            note.enrichment.prompt_version.as_deref(),
            Some(consolidate::NOTE_PROMPT_VERSION)
        );
        assert_eq!(
            decision.note.as_deref(),
            Some(note.node_id.as_str()),
            "the authorizing decision"
        );
        assert!(
            s.nodes.contains_key(&id(1)) && s.nodes.contains_key(&id(2)),
            "parents stay"
        );
    }
}

#[tokio::test]
async fn parents_that_do_not_fit_are_not_summarized() {
    for (size, calls) in [(1_100, 0), (900, 1)] {
        let (a, b) = (
            format!("one {}", "x".repeat(size)),
            format!("two {}", "y".repeat(size)),
        );
        let summarizer = Arc::new(FakeSummarizer::replying(NOTE));
        let (_dir, store) = consolidated(
            [&a, &b],
            gated("merge", 0.99, 0.0),
            Some(summarizer.clone()),
        )
        .await;
        assert_eq!(
            summarizer.prompts().len(),
            calls,
            "parents of {size} characters"
        );
        assert_eq!(notes(&store).len(), calls);
        assert_eq!(store.load().unwrap().consolidation.decisions.len(), 1);
    }
}

#[tokio::test]
async fn a_note_naming_unknown_paths_or_ids_is_discarded() {
    let replies = [
        (
            "Merged: the change also touched src/other.rs.".to_string(),
            false,
        ),
        (
            "Merged; see docs/internal/README for more.".to_string(),
            false,
        ),
        (format!("Same change as {}.", id(7)), false),
        (
            format!("Merged {} and {} in src/cache.rs.", id(1), id(2)),
            true,
        ),
        (NOTE.to_string(), true),
    ];
    for (reply, kept) in replies {
        let summarizer = Arc::new(FakeSummarizer::replying(&reply));
        let (_dir, store) =
            consolidated(TWO, gated("merge", 0.99, 0.0), Some(summarizer.clone())).await;
        assert_eq!(summarizer.prompts().len(), 1);
        assert_eq!(notes(&store).len(), usize::from(kept), "{reply}");
    }
}

#[tokio::test]
async fn a_timeout_or_invalid_output_keeps_the_pair_separate_and_changes_no_gate() {
    let summarizers = [
        FakeSummarizer::failing("timeout after 60000 ms"),
        FakeSummarizer::replying(&"a".repeat(601)),
        FakeSummarizer::replying("  \n "),
    ];
    for summarizer in summarizers {
        let summarizer = Arc::new(summarizer);
        let (_dir, store) =
            consolidated(TWO, gated("merge", 0.99, 0.0), Some(summarizer.clone())).await;
        assert_eq!(summarizer.prompts().len(), 1);
        let s = store.load().unwrap();
        assert_eq!(s.nodes.len(), 2, "the two stay separate");
        assert_eq!(s.nodes[&id(1)].content, TWO[0]);
        assert_eq!(s.nodes[&id(2)].content, TWO[1]);
        let d = s.consolidation.decisions.values().next().unwrap();
        assert_eq!(
            (
                d.representation,
                d.probability,
                d.contradiction,
                d.note.clone()
            ),
            (consolidate::RepresentationDecision::Merge, 0.99, 0.0, None),
            "the decision is as the classifier made it"
        );
        assert!(s.consolidation.notes.is_empty());
    }
}

#[tokio::test]
async fn without_a_trusted_version_cmd_generated_notes_are_not_cached_on_disk() {
    for versioned in [false, true] {
        let mut fake = FakeSummarizer::replying(NOTE);
        if versioned {
            fake = fake.versioned();
        }
        let summarizer = Arc::new(fake);
        let (_dir, store) =
            consolidated(TWO, gated("merge", 0.99, 0.0), Some(summarizer.clone())).await;
        let first = notes(&store);
        assert_eq!(first.len(), 1);
        assert_eq!(
            store.load().unwrap().consolidation.notes.len(),
            usize::from(versioned)
        );

        // Another classifier model decides the same pair again, a day later.
        let mut s = store.load().unwrap();
        s.consolidation.pending_since_ms = Some(T0);
        s.generation += 1;
        store.publish(&s).unwrap();
        let cfg = Config::new("jev-2.0.0").with_summarizer(summarizer.clone());
        let round = consolidate::round(
            &store,
            &Fake::new(gated("merge", 0.99, 0.0)),
            &cfg,
            T0 + 2 * DAY_MS,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(round.decided, 1);
        let calls = if versioned { 1 } else { 2 };
        assert_eq!(summarizer.prompts().len(), calls, "versioned: {versioned}");
        let s = store.load().unwrap();
        assert!(
            s.consolidation
                .decisions
                .values()
                .all(|d| d.note.as_deref() == Some(first[0].node_id.as_str())),
            "the same note either way"
        );
        if versioned {
            // Forgetting a parent takes the note and what pointed at it.
            store.forget(&id(1), u64::MAX).unwrap();
            let s = store.load().unwrap();
            assert!(!s.nodes.contains_key(&first[0].node_id));
            assert!(s.consolidation.notes.is_empty());
            assert!(s.consolidation.decisions.is_empty());
        }
    }
}

#[tokio::test]
async fn the_worker_gives_its_rounds_the_servers_summarizer() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(stored(
        dir.path(),
        &[
            rec(1, TWO[0], &["src/cache.rs"]),
            rec(2, TWO[1], &["src/cache.rs"]),
        ],
    ));
    enrich(&store, 2, T0);
    let summarizer = Arc::new(FakeSummarizer::replying(NOTE));
    let worker = Worker::new(
        store.clone(),
        Arc::new(Fake::new(gated("merge", 0.99, 0.0))),
        controller::Config {
            model: MODEL.into(),
            candidates: 4,
        },
    );
    worker.set_summarizer(summarizer.clone());
    let round = worker.consolidate(T0 + DAY_MS).await.unwrap().expect("due");
    assert_eq!((round.decided, round.notes), (1, 1));
    assert_eq!(summarizer.prompts().len(), 1);
    assert_eq!(notes(&store).len(), 1);
    let m = worker.metrics();
    assert_eq!((m.rounds, m.notes, m.notes_rejected), (1, 1, 0));
}
