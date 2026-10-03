//! A broker reading persistent memory (PRD jev-mem §10): a workspace with one remembered edit of
//! `src/cache.rs`, and a classifier that agrees with every question.
use super::fake::FakeUpstream;
use ripwire_broker::broker::{Broker, BrokerConfig};
use ripwire_broker::memory::admission::{self, Draft, Event, Outcome, Stamp, Tests};
use ripwire_broker::memory::identity;
use ripwire_broker::memory::publish::MemoryConfig;
use ripwire_broker::memory::retrieve::{ReadConfig, ReadSetup};
use ripwire_broker::memory::store::Store;
use ripwire_broker::online::classifier::{ClassifyError, MemoryClassifier};
use ripwire_broker::online::request::StateRequest;
use ripwire_broker::online::response::Decision;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Says yes to everything, and that the evidence is sufficient; counts the requests.
#[derive(Default)]
pub struct Agreeable {
    pub requests: AtomicUsize,
}

#[async_trait::async_trait]
impl MemoryClassifier for Agreeable {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        Ok(req
            .questions
            .0
            .iter()
            .map(|(_, q)| {
                let t = &q.instructions;
                let p = match () {
                    _ if t.starts_with("Does evidence support every factual part") => 0.99,
                    _ if t.starts_with("Is a fact needed to answer query absent") => 0.0,
                    _ if t.starts_with("Do items of evidence contradict") => 0.0,
                    _ => 0.9,
                };
                Decision::Noul { probability: p }
            })
            .collect())
    }
}

pub struct Remembering {
    pub broker: Broker,
    pub ws: tempfile::TempDir,
    pub st: tempfile::TempDir,
    pub store: Arc<Store>,
}

/// The broker over `fake`, with `--incremental` and memory read through `classifier`.
pub async fn remembering_from(
    fake: FakeUpstream,
    classifier: impl FnOnce(&Store) -> Arc<dyn MemoryClassifier>,
    cfg: ReadConfig,
) -> Remembering {
    let (ws, st) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    super::write(ws.path(), "src/cache.rs", "fn get() {}\n");
    let id = identity::workspace_id(ws.path()).unwrap();
    let store = Arc::new(Store::new(st.path(), &id));
    let reader = ripwire_broker::online::reader::WorkspaceReader::new(ws.path()).unwrap();
    let draft = Draft {
        event_key: "e".into(),
        event: Event::AfterEdit,
        outcome: Outcome::AnalysisCompleted,
        tests: Tests::Unknown,
        scope: vec!["src/cache.rs".into()],
        evidence: vec!["quality_delta".into()],
    };
    let stamp = Stamp {
        observed_at_ms: 1,
        ingest_seq: 0,
        generation: 0,
        retention_ms: u64::MAX / 2,
    };
    store
        .enqueue(&admission::admit(&reader, &id, &draft, stamp).unwrap())
        .unwrap();
    store.ingest().unwrap();
    let mut config = BrokerConfig::new(ws.path());
    config.incremental = true;
    let mut memory = MemoryConfig::new(store.clone(), id, 1_000_000);
    memory.read = Some(ReadSetup {
        store: store.clone(),
        classifier: classifier(&store),
        cfg,
    });
    config.memory = Some(memory);
    let broker = Broker::connect(Arc::new(fake), config).await.unwrap();
    Remembering {
        broker,
        ws,
        st,
        store,
    }
}
