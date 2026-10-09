//! Memory in `context_for_task` (PRD jev-mem §10, §13): the snapshot kept warm between calls and
//! loaded off the async threads, one load at a time, and every failure turned into a limitation so
//! the structural answer always goes out.

use super::queue::Ledger;
use super::retrieve::{self, Read, ReadConfig, ReadSetup, StopReason};
use super::store::{State, Unavailable, Version};
use super::time::{Clock, SystemClock};
use crate::model::{Basis, Limitation, Source};
use crate::online::reader::WorkspaceReader;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

type Loaded = Option<Result<Arc<State>, Unavailable>>;

#[derive(Default)]
struct Warm {
    /// The last snapshot loaded, with the version of the file it came from.
    state: Option<(Option<Version>, Arc<State>)>,
    /// The load under way; there is never more than one.
    loading: Option<watch::Receiver<Loaded>>,
}

/// What a read gave: memories when it got to run, and what kept it short.
#[derive(Debug, Default)]
pub struct Recalled {
    pub read: Option<Read>,
    pub limitations: Vec<Limitation>,
}

enum Miss {
    /// The load did not finish in the read's time; it goes on, for the next call.
    Cold,
    Unavailable(Unavailable),
}

/// A read for `--memory-debug-log` (D-164): the task only by a short hash and its length, the
/// memories by id, and why the read stopped.
fn said(query: &str, got: &Recalled, took: std::time::Duration) -> String {
    use sha2::Digest as _;
    let hash = format!("{:x}", sha2::Sha256::digest(query.as_bytes()));
    let mut line = format!("task#{} ({} chars)", &hash[..8], query.chars().count());
    match &got.read {
        Some(r) => {
            let kept: Vec<&str> = r
                .memories
                .iter()
                .map(|f| super::debug::short(&f.record.node_id))
                .collect();
            line.push_str(&format!(
                " kept={} [{}] visited={} requests={} stop={}",
                kept.len(),
                kept.join(" "),
                r.visited,
                r.requests,
                super::debug::name(&r.stop)
            ));
            if r.degraded {
                line.push_str(" degraded");
            }
            if r.partial {
                line.push_str(" partial");
            }
            if r.stale_omitted > 0 {
                line.push_str(&format!(" stale_omitted={}", r.stale_omitted));
            }
        }
        None => line.push_str(" no read"),
    }
    for l in &got.limitations {
        line.push_str(&format!(" limitation={}", l.kind));
    }
    line.push_str(&format!(" {}ms", took.as_millis()));
    line
}

/// Of the read's deadline, what the checks right before delivery keep for themselves: 50 ms, or a
/// fifth of a shorter deadline.
fn revalidation(deadline: std::time::Duration) -> std::time::Duration {
    std::time::Duration::from_millis(50).min(deadline / 5)
}

/// Requests a read of this process sent, until they are written to the quota.
struct Spent {
    attempts: u32,
    questions: u32,
    /// [`WRITING`], then [`WRITTEN`] or [`FAILED`].
    state: Arc<AtomicU8>,
}

const WRITING: u8 = 0;
const WRITTEN: u8 = 1;
/// The quota could not be written (its lock stayed busy, an I/O error): the next charge carries it.
const FAILED: u8 = 2;

pub struct Recall {
    setup: ReadSetup,
    reader: Arc<WorkspaceReader>,
    warm: Arc<Mutex<Warm>>,
    spent: Mutex<Vec<Spent>>,
}

impl Recall {
    pub fn new(setup: ReadSetup, workspace: &std::path::Path) -> Result<Self, String> {
        Ok(Self {
            setup,
            reader: Arc::new(WorkspaceReader::new(workspace)?),
            warm: Arc::default(),
            spent: Mutex::default(),
        })
    }

    /// Reads memory for `query` inside the read's deadline, the snapshot load and the checks right
    /// before delivery included. Never fails: what goes wrong is a limitation.
    pub async fn read(&self, query: &str) -> Recalled {
        let started = std::time::Instant::now();
        let got = self.read_now(query).await;
        self.setup
            .store
            .debug("read", || said(query, &got, started.elapsed()));
        got
    }

    async fn read_now(&self, query: &str) -> Recalled {
        let cfg = &self.setup.cfg;
        let until = tokio::time::Instant::now() + cfg.deadline;
        let left = || until.saturating_duration_since(tokio::time::Instant::now());
        // Taken before the quota is read: what is written in between is counted twice, never
        // missed.
        let in_flight = self.in_flight();
        let store = self.setup.store.clone();
        let on_disk = tokio::task::spawn_blocking(move || (store.pending(), store.quota_file()));
        let state = match self.state(cfg.deadline).await {
            Ok(copy) => copy,
            Err(miss) => {
                return Recalled {
                    read: None,
                    limitations: vec![missed(miss)],
                };
            }
        };
        let (pending, ledger) = match tokio::time::timeout_at(until, on_disk).await {
            Ok(Ok((pending, ledger))) => (pending.unwrap_or(0), ledger.ok()),
            _ => (0, None),
        };
        let now_ms = SystemClock.now_ms();
        // Never past the 24-hour quota: with none left, or none known, no new inference (PRD
        // jev-mem §8.2).
        // Before anything was charged to its own file, the quota is the one the snapshot has.
        let ledger = ledger.map(|l| l.unwrap_or_else(|| state.ledger.clone()));
        let (attempts, questions) =
            ledger.map_or((0, 0), |l| self.quota_left(&l, in_flight, now_ms));
        let exhausted = attempts == 0 || questions == 0;
        // The requests stop early enough to leave the last checks their time.
        let cfg = ReadConfig {
            deadline: left().saturating_sub(revalidation(cfg.deadline)),
            request_limit: match exhausted {
                true => 0,
                false => cfg.request_limit.min(attempts),
            },
            max_questions: cfg.max_questions.min(questions),
            ..cfg.clone()
        };
        let classifier = &*self.setup.classifier;
        let mut read =
            retrieve::read_fresh(state, self.reader.clone(), query, classifier, &cfg).await;
        self.charge(now_ms, &read);
        let checked = match self.state(left()).await {
            Ok(now) => retrieve::revalidate(&mut read, &now, self.reader.clone(), until).await,
            Err(_) => false,
        };
        let mut limitations = vec![];
        if exhausted && read.degraded {
            limitations.push(quota_spent());
        }
        if !checked {
            // What was read cannot be checked against the current generation: nothing goes out.
            read.memories.clear();
            read.partial = true;
            limitations.push(unchecked());
        }
        read.pending_writes = pending;
        if matches!(
            read.stop,
            StopReason::ProviderError | StopReason::Deadline | StopReason::Cancelled
        ) {
            limitations.push(stopped_early(read.stop));
        }
        Recalled {
            read: Some(read),
            limitations,
        }
    }

    /// What reads of this process sent and the store's quota does not have yet.
    fn in_flight(&self) -> (u32, u32) {
        let mut spent = self.spent.lock().unwrap();
        spent.retain(|s| s.state.load(Ordering::Acquire) != WRITTEN);
        spent.iter().fold((0u32, 0u32), |(a, q), s| {
            (a.saturating_add(s.attempts), q.saturating_add(s.questions))
        })
    }

    /// Attempts and questions the quota has left: what `ledger` records, and `in_flight`.
    fn quota_left(&self, ledger: &Ledger, in_flight: (u32, u32), now_ms: u64) -> (usize, usize) {
        let limits = self.setup.store.limits();
        let (attempts, questions) = ledger.used(now_ms);
        (
            limits
                .attempts_per_day
                .saturating_sub(attempts.saturating_add(in_flight.0)) as usize,
            limits
                .questions_per_day
                .saturating_sub(questions.saturating_add(in_flight.1)) as usize,
        )
    }

    /// Charges what `read` sent to the quota, off the answer's path, with what earlier reads
    /// could not write: a busy lock or an I/O error delays a spend, never loses it (D-150).
    fn charge(&self, now_ms: u64, read: &Read) {
        let (mut attempts, mut questions) = (read.requests as u32, read.questions as u32);
        let state = Arc::new(AtomicU8::new(WRITING));
        {
            let mut spent = self.spent.lock().unwrap();
            spent.retain(|s| match s.state.load(Ordering::Acquire) {
                FAILED => {
                    attempts = attempts.saturating_add(s.attempts);
                    questions = questions.saturating_add(s.questions);
                    false
                }
                _ => true,
            });
            if attempts == 0 && questions == 0 {
                return;
            }
            spent.push(Spent {
                attempts,
                questions,
                state: state.clone(),
            });
        }
        let store = self.setup.store.clone();
        tokio::task::spawn_blocking(move || {
            let done = match store.spend(now_ms, attempts, questions) {
                Ok(()) => WRITTEN,
                Err(_) => FAILED,
            };
            state.store(done, Ordering::Release);
        });
    }

    /// The current snapshot: the warm copy while its version is current, otherwise a load, awaited
    /// for at most `within`. A load outlives the wait and warms the copy for the next call.
    async fn state(&self, within: std::time::Duration) -> Result<Arc<State>, Miss> {
        let store = self.setup.store.clone();
        let mut loaded = {
            let mut warm = self.warm.lock().unwrap();
            if let Some((version, state)) = &warm.state
                && *version == store.snapshot_version()
            {
                return Ok(state.clone());
            }
            // A load whose sender is gone panicked: it is started again, never waited on.
            match warm.loading.as_ref().filter(|l| l.has_changed().is_ok()) {
                Some(loading) => loading.clone(),
                None => {
                    let (tx, rx) = watch::channel(None);
                    warm.loading = Some(rx.clone());
                    let warm = self.warm.clone();
                    tokio::task::spawn_blocking(move || {
                        let version = store.snapshot_version();
                        let state = store.load().map(Arc::new);
                        let mut w = warm.lock().unwrap();
                        w.loading = None;
                        // The version is read before the snapshot, which a writer replaces
                        // before the version: a write in between leaves a newer copy under an
                        // older version, loaded again on the next call, never an older copy
                        // under a newer one.
                        if let Ok(s) = &state {
                            w.state = Some((version, s.clone()));
                        }
                        let _ = tx.send(Some(state));
                    });
                    rx
                }
            }
        };
        match tokio::time::timeout(within, loaded.wait_for(Option::is_some)).await {
            Ok(Ok(state)) => state
                .clone()
                .unwrap_or(Err(Unavailable::Io))
                .map_err(Miss::Unavailable),
            Ok(Err(_)) => Err(Miss::Unavailable(Unavailable::Io)),
            Err(_) => Err(Miss::Cold),
        }
    }
}

fn limitation(kind: &'static str, detail: String) -> Limitation {
    Limitation {
        kind,
        detail,
        source: Source {
            verb: "memory",
            basis: Basis::BrokerInference,
        },
    }
}

fn missed(miss: Miss) -> Limitation {
    match miss {
        Miss::Cold => limitation(
            "memory_cold",
            "the memory snapshot is still loading; memories will be in a later answer".into(),
        ),
        Miss::Unavailable(why) => limitation(
            "memory_unavailable",
            format!(
                "memory could not be read ({}); the rest of the answer is unaffected",
                why.as_str()
            ),
        ),
    }
}

fn unchecked() -> Limitation {
    incomplete("the memories read could not be checked again")
}

fn quota_spent() -> Limitation {
    incomplete("the 24-hour inference quota is spent")
}

fn stopped_early(stop: StopReason) -> Limitation {
    let why = serde_json::json!(stop);
    incomplete(&format!(
        "the read stopped early ({})",
        why.as_str().unwrap_or_default()
    ))
}

/// The most a read ever adds to `limitations`, for the budget's reserve: two `memory_incomplete`
/// at once, which is wider than `memory_cold` or `memory_unavailable` alone. A read stopped early
/// sent requests, so the quota was not spent: the two never go together.
pub fn widest_limitations() -> Vec<Limitation> {
    let width = |l: &Limitation| serde_json::to_string(l).map_or(0, |s| s.len());
    let (stopped, spent) = (stopped_early(StopReason::ProviderError), quota_spent());
    let second = match width(&stopped) >= width(&spent) {
        true => stopped,
        false => spent,
    };
    vec![unchecked(), second]
}

fn incomplete(why: &str) -> Limitation {
    limitation(
        "memory_incomplete",
        format!("{why}; the rest of the answer is unaffected"),
    )
}
