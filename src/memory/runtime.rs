//! The memory worker's lifecycle (PRD jev-mem §4, §8.1): started by `serve --memory` and by
//! nothing else, stopped with the server, and run on demand by `memory drain --online`. No
//! process with the credential outlives the one that was authorized; what is pending stays
//! durable on disk for the next one.

use super::controller::{Config, Worker};
use super::identity;
use super::publish::MemoryConfig;
use super::retrieve::{ReadConfig, ReadSetup, Selection};
use super::store::{Limits, Refusal, Store};
use super::time::{Clock, SystemClock};
use crate::cli::ServeArgs;
use crate::online::classifier::MemoryClassifier;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinError;
use tokio_util::sync::CancellationToken;

/// `memory drain` stops after this long, or after [`DRAIN_JOBS`] jobs (PRD jev-mem §4).
pub const DRAIN_DEADLINE: Duration = Duration::from_secs(60);
pub const DRAIN_JOBS: usize = 20;
/// Retention runs when the worker starts and then once an hour while it is active.
const SWEEP_EVERY_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub use crate::online::DEFAULT_MODEL;
/// Earlier memories a new one is compared with when no `--memory-write-candidates` is given.
pub const DEFAULT_WRITE_CANDIDATES: usize = 4;

pub struct Runtime {
    store: Arc<Store>,
    workspace_id: String,
    worker: Arc<Worker>,
    config: Config,
    publish: MemoryConfig,
    cancel: CancellationToken,
    /// The worker's task; a separate one watches it, to say if it panicked.
    task: Option<tokio::task::AbortHandle>,
    /// `--memory-selection`: in `Deterministic` nothing is enriched or consolidated.
    selection: Selection,
}

/// The runtime `serve` gets: `None` without `--memory`, whatever else is on. `--online` alone
/// never opens a store nor processes a job left by an earlier process.
pub fn from_serve(
    args: &ServeArgs,
    state_dir: &Path,
    classifier: Option<Arc<dyn MemoryClassifier>>,
) -> Result<Option<Runtime>, String> {
    let Some(memory) = &args.memory else {
        return Ok(None);
    };
    let classifier = classifier.ok_or("--memory needs the online classifier")?;
    let workspace_id = identity::workspace_id(&args.workspace)?;
    let limits = Limits {
        max_nodes: memory.max_nodes,
        ..Limits::default()
    };
    let mut store = Store::with_limits(state_dir, &workspace_id, limits);
    if memory.debug_log {
        let dir = store.dir().to_path_buf();
        store = store.with_debug_log("serve").map_err(|e| {
            format!(
                "--memory-debug-log: {}: {e}",
                dir.join(super::debug::FILE).display()
            )
        })?;
    }
    let store = Arc::new(store);
    let config = Config {
        model: args
            .online
            .as_ref()
            .map_or(DEFAULT_MODEL.into(), |o| o.model.clone()),
        candidates: memory.write_candidates,
    };
    let publish = MemoryConfig::new(
        store.clone(),
        workspace_id.clone(),
        u64::from(memory.retention_days) * DAY_MS,
    );
    let read = ReadSetup {
        store: store.clone(),
        classifier: classifier.clone(),
        cfg: ReadConfig {
            model: config.model.clone(),
            deadline: memory.read_deadline,
            request_limit: memory.read_request_limit,
            selection: memory.selection,
            ..ReadConfig::default()
        },
    };
    let worker = Arc::new(Worker::new(store.clone(), classifier, config.clone()));
    let publish = MemoryConfig {
        worker: Some(worker.metrics_handle()),
        read: Some(read),
        debug: store.debug_log().cloned(),
        ..publish
    };
    Ok(Some(Runtime {
        store,
        workspace_id,
        worker,
        config,
        publish,
        cancel: CancellationToken::new(),
        task: None,
        selection: memory.selection,
    }))
}

impl Runtime {
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// `--summarizer-cmd`: consolidation rounds may write derived notes with it.
    pub fn set_summarizer(&self, summarizer: Arc<dyn crate::summarizer::Summarizer>) {
        self.worker.set_summarizer(summarizer);
    }

    /// What the broker's tools publish to.
    pub fn publish(&self) -> &MemoryConfig {
        &self.publish
    }

    /// Runs the worker in the background until the runtime is dropped: incorporate the spool,
    /// run every ready job and a consolidation round that came due, then wait `tick`. With
    /// `--memory-selection deterministic` only the spool and retention run: nothing is sent. The
    /// disk work (spool, retention) runs in the blocking pool, and a stage that starts failing is
    /// said on stderr (D-146).
    pub fn start(&mut self, tick: Duration) {
        self.start_reporting(tick, |line| eprintln!("{line}"));
    }

    /// [`Runtime::start`], with what the worker says going to `say` instead of stderr.
    pub fn start_reporting(&mut self, tick: Duration, say: impl Fn(&str) + Send + Sync + 'static) {
        let (store, worker, cancel) =
            (self.store.clone(), self.worker.clone(), self.cancel.clone());
        let enrich = self.selection == Selection::Jev;
        // What the worker says on stderr also goes to the debug log, where the rest of it is.
        let log = self.store.debug_log().cloned();
        let say: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line: &str| {
            say(line);
            if let Some(log) = &log {
                log.event("worker", line);
            }
        });
        let watch = say.clone();
        let task = tokio::spawn(async move {
            let mut swept_at: Option<u64> = None;
            let mut said = Said::new(say);
            loop {
                let now = SystemClock.now_ms();
                if swept_at.is_none_or(|at| now.saturating_sub(at) >= SWEEP_EVERY_MS) {
                    let s = store.clone();
                    said.note(
                        "retention",
                        tokio::task::spawn_blocking(move || s.sweep(now)).await,
                    );
                    swept_at = Some(now);
                }
                let s = store.clone();
                said.note(
                    "ingest",
                    tokio::task::spawn_blocking(move || s.ingest()).await,
                );
                if enrich {
                    loop {
                        let ran = worker.run_once(SystemClock.now_ms()).await;
                        let done = !matches!(ran, Ok(Some(_)));
                        said.note("enrichment", Ok(ran));
                        if done {
                            break;
                        }
                        if cancel.is_cancelled() {
                            return;
                        }
                    }
                    let round = worker.consolidate(SystemClock.now_ms()).await;
                    said.note("consolidation", Ok(round));
                    // A 401/403 comes back as a run, not a refusal, and suspends the worker for
                    // the life of the process.
                    if worker.is_suspended() {
                        said.failed(
                            "provider",
                            "the provider refused the credential (401/403); jobs wait for a \
                             server restart"
                                .into(),
                        );
                    }
                }
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(tick) => {}
                }
            }
        });
        self.task = Some(task.abort_handle());
        // A worker that panics is gone for the life of the server: said once, never silent
        // (D-151). A task aborted by `Drop` is not a panic, and says nothing.
        tokio::spawn(async move {
            if task.await.is_err_and(|e| e.is_panic()) {
                watch(
                    "ripwire-broker: memory worker: stopped: it panicked; memory is collected but \
                     not processed until the server restarts",
                );
            }
        });
    }
}

/// What each stage of the worker last said: a store that stays broken is said once, when the
/// stage starts failing or fails differently, and a success clears it. A store held by another
/// writer is not a failure.
struct Said {
    last: std::collections::HashMap<&'static str, String>,
    say: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Said {
    fn new(say: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        Self {
            last: Default::default(),
            say,
        }
    }

    fn note<T>(&mut self, stage: &'static str, ran: Result<Result<T, Refusal>, JoinError>) {
        match ran {
            Ok(Ok(_)) | Ok(Err(Refusal::Locked)) => {
                self.last.remove(stage);
            }
            Ok(Err(refusal)) => self.failed(stage, format!("{refusal:?}")),
            Err(_) => self.failed(stage, "panicked".into()),
        }
    }

    /// Says `why` for `stage`, unless it is what the stage said last.
    fn failed(&mut self, stage: &'static str, why: String) {
        if self.last.get(stage) != Some(&why) {
            (self.say)(&format!("ripwire-broker: memory worker: {stage}: {why}"));
            self.last.insert(stage, why);
        }
    }
}

impl Drop for Runtime {
    /// The server stops: so does the worker, mid-request if need be. An interrupted job's lease
    /// goes with it, and the next process takes the job again.
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainStop {
    /// Nothing ready is left.
    Empty,
    Jobs,
    Deadline,
    /// Another worker (a running `serve --memory`) holds the workspace's remote slot.
    Busy,
    /// The provider refused the credential.
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drained {
    pub jobs: usize,
    pub stop: DrainStop,
}

/// No job is ready: a consolidation round that came due runs, if a whole round's time is left
/// (none while the worker is suspended). It is bounded by its own deadline and never cut: a
/// round cut after it paid would be bought again by the next drain. Then how the drain stops.
async fn idle(
    worker: &Worker,
    clock: &dyn Clock,
    until: tokio::time::Instant,
) -> Result<DrainStop, Refusal> {
    let left = until.saturating_duration_since(tokio::time::Instant::now());
    if left >= super::consolidate::DEADLINE {
        worker.consolidate(clock.now_ms()).await?;
    }
    Ok(match worker.is_suspended() {
        true => DrainStop::Suspended,
        false => DrainStop::Empty,
    })
}

/// `memory drain`: incorporates the spool and runs ready jobs, at most `max_jobs` and for at
/// most `deadline`, then a consolidation round if one came due. A job cut by the deadline has
/// used its run (runs are counted on disk), and its lease is free for the next process.
pub async fn drain(
    store: &Store,
    worker: &Worker,
    clock: &dyn Clock,
    max_jobs: usize,
    deadline: Duration,
) -> Result<Drained, Refusal> {
    let _ = store.ingest();
    if store.remote_slot()?.is_none() {
        return Ok(Drained {
            jobs: 0,
            stop: DrainStop::Busy,
        });
    }
    let until = tokio::time::Instant::now() + deadline;
    let mut jobs = 0;
    loop {
        if jobs >= max_jobs {
            return Ok(Drained {
                jobs,
                stop: DrainStop::Jobs,
            });
        }
        let left = until.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return Ok(Drained {
                jobs,
                stop: DrainStop::Deadline,
            });
        }
        match tokio::time::timeout(left, worker.run_once(clock.now_ms())).await {
            Err(_) => {
                return Ok(Drained {
                    jobs,
                    stop: DrainStop::Deadline,
                });
            }
            Ok(Ok(Some(_))) => jobs += 1,
            // "Nothing" is also what a worker says when another process took the slot after the
            // check above: that is a busy drain, not an empty queue (D-150).
            Ok(Ok(None)) if store.remote_slot()?.is_none() => {
                return Ok(Drained {
                    jobs,
                    stop: DrainStop::Busy,
                });
            }
            Ok(Ok(None)) => {
                let stop = idle(worker, clock, until).await?;
                return Ok(Drained { jobs, stop });
            }
            Ok(Err(e)) => return Err(e),
        }
    }
}
