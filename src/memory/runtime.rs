//! The memory worker's lifecycle (PRD jev-mem §4, §8.1): started by `serve --memory` and by
//! nothing else, stopped with the server, and run on demand by `memory drain --online`. No
//! process with the credential outlives the one that was authorized; what is pending stays
//! durable on disk for the next one.

use super::controller::{Config, Worker};
use super::identity;
use super::publish::MemoryConfig;
use super::store::{Limits, Refusal, Store};
use super::time::{Clock, SystemClock};
use crate::cli::ServeArgs;
use crate::online::classifier::MemoryClassifier;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// `memory drain` stops after this long, or after [`DRAIN_JOBS`] jobs (PRD jev-mem §4).
pub const DRAIN_DEADLINE: Duration = Duration::from_secs(60);
pub const DRAIN_JOBS: usize = 20;
/// Retention runs when the worker starts and then once an hour while it is active.
const SWEEP_EVERY_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
/// The pinned model when no `--jev-model` was given.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";

pub struct Runtime {
    store: Arc<Store>,
    workspace_id: String,
    worker: Arc<Worker>,
    config: Config,
    publish: MemoryConfig,
    cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<()>>,
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
    let store = Arc::new(Store::with_limits(state_dir, &workspace_id, limits));
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
    let worker = Arc::new(Worker::new(store.clone(), classifier, config.clone()));
    let publish = MemoryConfig {
        worker: Some(worker.metrics_handle()),
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
    }))
}

impl Runtime {
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// What the broker's tools publish to.
    pub fn publish(&self) -> &MemoryConfig {
        &self.publish
    }

    /// Runs the worker in the background until the runtime is dropped: incorporate the spool,
    /// run every ready job, then wait `tick`.
    pub fn start(&mut self, tick: Duration) {
        let (store, worker, cancel) =
            (self.store.clone(), self.worker.clone(), self.cancel.clone());
        self.task = Some(tokio::spawn(async move {
            let mut swept_at: Option<u64> = None;
            loop {
                let now = SystemClock.now_ms();
                if swept_at.is_none_or(|at| now.saturating_sub(at) >= SWEEP_EVERY_MS) {
                    let _ = store.sweep(now);
                    swept_at = Some(now);
                }
                let _ = store.ingest();
                while let Ok(Some(_)) = worker.run_once(SystemClock.now_ms()).await {
                    if cancel.is_cancelled() {
                        return;
                    }
                }
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(tick) => {}
                }
            }
        }));
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drained {
    pub jobs: usize,
    pub stop: DrainStop,
}

/// `memory drain`: incorporates the spool and runs ready jobs, at most `max_jobs` and for at
/// most `deadline`; a job cut by the deadline stays pending.
pub async fn drain(
    store: &Store,
    worker: &Worker,
    clock: &dyn Clock,
    max_jobs: usize,
    deadline: Duration,
) -> Result<Drained, Refusal> {
    let _ = store.ingest();
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
            Ok(Ok(None)) => {
                return Ok(Drained {
                    jobs,
                    stop: DrainStop::Empty,
                });
            }
            Ok(Err(e)) => return Err(e),
        }
    }
}
