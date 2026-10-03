//! Memory in `context_for_task` (PRD jev-mem §10, §13): the snapshot kept warm between calls and
//! loaded off the async threads, one load at a time, and every failure turned into a limitation so
//! the structural answer always goes out.

use super::retrieve::{self, Read, ReadConfig, ReadSetup, StopReason};
use super::store::{State, Unavailable};
use crate::model::{Basis, Limitation, Source};
use crate::online::reader::WorkspaceReader;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use tokio::sync::watch;

type Version = Option<(u64, SystemTime)>;
type Loaded = Option<Result<Arc<State>, Unavailable>>;

#[derive(Default)]
struct Warm {
    /// The last snapshot loaded, with the version of the file it came from.
    state: Option<(Version, Arc<State>)>,
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

pub struct Recall {
    setup: ReadSetup,
    reader: WorkspaceReader,
    warm: Arc<Mutex<Warm>>,
}

impl Recall {
    pub fn new(setup: ReadSetup, workspace: &std::path::Path) -> Result<Self, String> {
        Ok(Self {
            setup,
            reader: WorkspaceReader::new(workspace)?,
            warm: Arc::default(),
        })
    }

    /// Reads memory for `query` inside the read's deadline, the snapshot load included. Never
    /// fails: what goes wrong is a limitation.
    pub async fn read(&self, query: &str) -> Recalled {
        let started = Instant::now();
        let cfg = &self.setup.cfg;
        let state = match self.state(cfg.deadline).await {
            Ok(state) => state,
            Err(miss) => {
                return Recalled {
                    read: None,
                    limitations: vec![missed(miss)],
                };
            }
        };
        let cfg = ReadConfig {
            deadline: cfg.deadline.saturating_sub(started.elapsed()),
            ..cfg.clone()
        };
        let classifier = &*self.setup.classifier;
        let mut read = retrieve::read_fresh(&state, &self.reader, query, classifier, &cfg).await;
        read.pending_writes = self.setup.store.pending().unwrap_or(0);
        let mut limitations = vec![];
        match self
            .state(self.setup.cfg.deadline.saturating_sub(started.elapsed()))
            .await
        {
            Ok(now) => retrieve::revalidate(&mut read, &now, &self.reader),
            // What was read cannot be checked against the current generation: nothing goes out.
            Err(_) => {
                read.memories.clear();
                read.partial = true;
                limitations.push(incomplete("the memories read could not be checked again"));
            }
        }
        if matches!(
            read.stop,
            StopReason::ProviderError | StopReason::Deadline | StopReason::Cancelled
        ) {
            let why = serde_json::json!(read.stop);
            limitations.push(incomplete(&format!(
                "the read stopped early ({})",
                why.as_str().unwrap_or_default()
            )));
        }
        Recalled {
            read: Some(read),
            limitations,
        }
    }

    /// The current snapshot: the warm copy while the file is unchanged, otherwise a load, awaited
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
            match &warm.loading {
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

fn incomplete(why: &str) -> Limitation {
    limitation(
        "memory_incomplete",
        format!("{why}; the rest of the answer is unaffected"),
    )
}
