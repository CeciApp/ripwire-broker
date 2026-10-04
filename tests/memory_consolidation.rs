//! Seam: consolidation (PRD jev-mem §9): when a round is due, which pairs it asks about, what it
//! records and what it never touches. A classifier stand-in, no network.
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
