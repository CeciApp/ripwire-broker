//! A scripted `Classifier`: answers by item id, can fail or be delayed per request (keyed by
//! the request's first item), can be held until the test adds permits, and counts calls,
//! concurrency and requests dropped before they finished. Only Tokio time is used.
use async_trait::async_trait;
use ripwire_broker::online::SemanticStage;
use ripwire_broker::online::classifier::{Classifier, ClassifyError};
use ripwire_broker::online::prompt;
use ripwire_broker::online::request::{JevRequest, StateItem};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

#[derive(Default)]
pub struct Counters {
    pub calls: AtomicUsize,
    pub in_flight: AtomicUsize,
    pub max_in_flight: AtomicUsize,
    pub finished: AtomicUsize,
    /// Calls dropped before they answered (aborted).
    pub dropped: AtomicUsize,
}

type Rule = Box<dyn Fn(SemanticStage, &StateItem) -> Option<f64> + Send + Sync>;
type Hook = Box<dyn Fn(SemanticStage) + Send + Sync>;

pub struct FakeClassifier {
    rule: Option<Rule>,
    on_call: Option<Hook>,
    probability: HashMap<String, f64>,
    default: f64,
    fail: HashMap<String, ClassifyError>,
    /// Fails the first `n` requests whose first item is the key, then answers.
    fail_times: HashMap<String, (ClassifyError, AtomicUsize)>,
    delay: HashMap<String, Duration>,
    hold: Option<Arc<Semaphore>>,
    pub counters: Arc<Counters>,
    pub seen: Mutex<Vec<JevRequest>>,
}

impl FakeClassifier {
    pub fn new() -> Self {
        Self {
            rule: None,
            on_call: None,
            probability: HashMap::new(),
            default: 0.9,
            fail: HashMap::new(),
            fail_times: HashMap::new(),
            delay: HashMap::new(),
            hold: None,
            counters: Arc::default(),
            seen: Mutex::new(vec![]),
        }
    }

    /// The probability answered for every question about item `id`.
    pub fn answer(mut self, id: &str, p: f64) -> Self {
        self.probability.insert(id.into(), p);
        self
    }

    /// Answers every question by stage and item; `None` answers unknown. Overrides `answer`.
    pub fn rule(
        mut self,
        f: impl Fn(SemanticStage, &StateItem) -> Option<f64> + Send + Sync + 'static,
    ) -> Self {
        self.rule = Some(Box::new(f));
        self
    }

    /// Runs `f` on every call, after the request was sent: a side effect such as editing a
    /// file while the provider is answering.
    pub fn on_call(mut self, f: impl Fn(SemanticStage) + Send + Sync + 'static) -> Self {
        self.on_call = Some(Box::new(f));
        self
    }

    /// The stage a request asks about, read from its first question.
    pub fn stage(req: &JevRequest) -> SemanticStage {
        let (q, item) = (&req.questions.0[0].1, &req.state.items[0]);
        match q.instructions == prompt::instructions(SemanticStage::FileAdmission, &item.id) {
            true => SemanticStage::FileAdmission,
            false => SemanticStage::SourceSelection,
        }
    }

    /// Paths asked about in each stage, in the order they were asked.
    pub fn asked(&self, stage: SemanticStage) -> Vec<String> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| Self::stage(r) == stage)
            .flat_map(|r| r.state.items.iter().map(|i| i.path.clone()))
            .collect()
    }

    pub fn otherwise(mut self, p: f64) -> Self {
        self.default = p;
        self
    }

    /// Fails the request whose first item is `id`.
    pub fn fail(mut self, id: &str, e: ClassifyError) -> Self {
        self.fail.insert(id.into(), e);
        self
    }

    /// Fails the first `n` requests whose first item is `id`, then answers normally.
    pub fn fail_times(mut self, id: &str, e: ClassifyError, n: usize) -> Self {
        self.fail_times.insert(id.into(), (e, AtomicUsize::new(n)));
        self
    }

    /// Delays the request whose first item is `id`.
    pub fn delay(mut self, id: &str, d: Duration) -> Self {
        self.delay.insert(id.into(), d);
        self
    }

    /// Every call waits for a permit; the test releases them with `add_permits`.
    pub fn held(mut self) -> (Self, Arc<Semaphore>) {
        let gate = Arc::new(Semaphore::new(0));
        self.hold = Some(gate.clone());
        (self, gate)
    }

    pub fn calls(&self) -> usize {
        self.counters.calls.load(SeqCst)
    }
}

struct Guard(Arc<Counters>, bool);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, SeqCst);
        if !self.1 {
            self.0.dropped.fetch_add(1, SeqCst);
        }
    }
}

#[async_trait]
impl Classifier for FakeClassifier {
    fn model(&self) -> &str {
        "jev-1.13.0"
    }

    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError> {
        let c = &self.counters;
        c.calls.fetch_add(1, SeqCst);
        let now = c.in_flight.fetch_add(1, SeqCst) + 1;
        c.max_in_flight.fetch_max(now, SeqCst);
        let mut guard = Guard(c.clone(), false);
        self.seen.lock().unwrap().push(req.clone());
        if let Some(hook) = &self.on_call {
            hook(Self::stage(req));
        }
        let first = req
            .state
            .items
            .first()
            .map(|i| i.id.clone())
            .unwrap_or_default();
        if let Some(d) = self.delay.get(&first) {
            tokio::time::sleep(*d).await;
        }
        // A scripted failure answers after its delay, without waiting for the gate.
        let scripted = self.fail.get(&first).cloned().or_else(|| {
            let (e, left) = self.fail_times.get(&first)?;
            (left
                .fetch_update(SeqCst, SeqCst, |n| n.checked_sub(1))
                .is_ok())
            .then(|| e.clone())
        });
        if let Some(e) = scripted {
            guard.1 = true;
            c.finished.fetch_add(1, SeqCst);
            return Err(e);
        }
        if let Some(gate) = &self.hold {
            gate.acquire().await.unwrap().forget();
        }
        guard.1 = true;
        c.finished.fetch_add(1, SeqCst);
        let stage = Self::stage(req);
        Ok(req
            .state
            .items
            .iter()
            .map(|i| match &self.rule {
                Some(rule) => rule(stage, i),
                None => Some(*self.probability.get(&i.id).unwrap_or(&self.default)),
            })
            .collect())
    }
}
