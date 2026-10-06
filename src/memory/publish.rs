//! Collection from the tools (PRD jev-mem §8.1): after an edit or a finish analysis, its
//! observation goes to the spool. The answer never waits for more than a short, fixed time;
//! past it the write carries on in the background and is counted as unconfirmed, not durable.

use super::admission::{self, Draft, Event, Outcome, Stamp, Tests};
use super::store::Spool;
use super::time::{Clock, SystemClock};
use crate::model::{Envelope, Status};
use crate::online::reader::WorkspaceReader;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What `--memory` gives the broker.
#[derive(Clone)]
pub struct MemoryConfig {
    pub spool: Arc<dyn Spool>,
    pub workspace_id: String,
    pub retention_ms: u64,
    /// Longest an answer waits for its observation to be durable (PRD jev-mem §8.2).
    pub wait: Duration,
    /// Writes still running in the background beyond which no new one starts.
    pub max_in_flight: usize,
    /// The worker's running cost, shown in the status resource; `None` without a worker.
    pub worker: Option<Arc<Mutex<super::metrics::Metrics>>>,
    /// What `context_for_task` reads memory with; `None` collects without reading.
    pub read: Option<super::retrieve::ReadSetup>,
    /// Where `serve` counts memory reads and stores for the status line (D-154); `None` elsewhere.
    pub activity: Option<Arc<crate::server_status::Activities>>,
}

impl std::fmt::Debug for MemoryConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryConfig")
            .field("workspace_id", &self.workspace_id)
            .field("retention_ms", &self.retention_ms)
            .finish_non_exhaustive()
    }
}

impl MemoryConfig {
    pub fn new(spool: Arc<dyn Spool>, workspace_id: String, retention_ms: u64) -> Self {
        Self {
            spool,
            workspace_id,
            retention_ms,
            wait: Duration::from_millis(25),
            max_in_flight: 4,
            worker: None,
            read: None,
            activity: None,
        }
    }
}

/// The `memory` field of the status resource: collection counts and the worker's cost.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MemoryStatus {
    #[serde(flatten)]
    pub counts: Counts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker: Option<super::metrics::Metrics>,
}

/// Counts only, for the status resource.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// Durable in the spool before the answer left.
    pub confirmed: u64,
    /// Still being written when the answer left, or not started because too many were.
    pub unconfirmed: u64,
    /// Refused by admission or by the store.
    pub rejected: u64,
}

pub struct Publisher {
    config: MemoryConfig,
    reader: Option<Arc<WorkspaceReader>>,
    in_flight: Arc<AtomicUsize>,
    counts: Arc<Mutex<Counts>>,
}

impl Publisher {
    pub fn new(config: MemoryConfig, workspace: &Path) -> Self {
        Self {
            config,
            reader: WorkspaceReader::new(workspace).ok().map(Arc::new),
            in_flight: Arc::new(AtomicUsize::new(0)),
            counts: Arc::default(),
        }
    }

    pub fn counts(&self) -> Counts {
        *self.counts.lock().unwrap()
    }

    /// Where `serve` counts memory activity for the status line (D-154).
    pub fn activity(&self) -> Option<&Arc<crate::server_status::Activities>> {
        self.config.activity.as_ref()
    }

    /// The status resource's `memory` field.
    pub fn status(&self) -> MemoryStatus {
        MemoryStatus {
            counts: self.counts(),
            worker: self
                .config
                .worker
                .as_ref()
                .map(|m| m.lock().unwrap().clone()),
        }
    }

    /// Publishes what `env` observed about `scope`. An answer the broker could not assess
    /// (`unknown`) claims nothing, so it is not observed.
    pub async fn observe(
        &self,
        event: Event,
        env: &Envelope,
        scope: Vec<String>,
        event_key: String,
    ) {
        if env.status == Status::Unknown || scope.is_empty() {
            return;
        }
        let Some(reader) = self.reader.clone() else {
            self.count(|c| c.rejected += 1);
            return;
        };
        // Reserved in one step, so concurrent answers never go past the cap.
        let max = self.config.max_in_flight;
        let reserved = self
            .in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < max).then_some(n + 1)
            });
        if reserved.is_err() {
            self.count(|c| c.unconfirmed += 1);
            return;
        }
        let slot = Slot(self.in_flight.clone());
        let draft = Draft {
            event_key,
            event,
            outcome: match env.status {
                Status::AttentionRequired => Outcome::AttentionRequired,
                _ => Outcome::AnalysisCompleted,
            },
            tests: Tests::Unknown,
            scope,
            evidence: env
                .provenance
                .upstream_tools
                .iter()
                .map(|v| v.to_string())
                .collect(),
        };
        let stamp = Stamp {
            observed_at_ms: SystemClock.now_ms(),
            ingest_seq: 0,
            generation: 0,
            retention_ms: self.config.retention_ms,
        };
        if let Some(a) = &self.config.activity {
            a.mem_stores.record(crate::hook::now());
        }
        let (spool, workspace_id) = (self.config.spool.clone(), self.config.workspace_id.clone());
        let write = tokio::task::spawn_blocking(move || {
            let _slot = slot;
            admission::admit(&reader, &workspace_id, &draft, stamp)
                .map_err(|_| ())
                .and_then(|record| spool.enqueue(&record).map_err(|_| ()))
        });
        match tokio::time::timeout(self.config.wait, write).await {
            Ok(Ok(Ok(()))) => self.count(|c| c.confirmed += 1),
            Ok(_) => self.count(|c| c.rejected += 1),
            Err(_) => {
                // Still running: a late write keeps the same node id and is reconciled on
                // ingestion; it is never counted as durable here.
                self.count(|c| c.unconfirmed += 1)
            }
        }
    }

    fn count(&self, f: impl FnOnce(&mut Counts)) {
        f(&mut self.counts.lock().unwrap());
    }
}

/// A write's place under `max_in_flight`, given back when the write ends, even by a panic.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
