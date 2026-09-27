//! A scripted `Classifier`: answers by stage and item, can fail or be delayed per request,
//! can be held until the test adds permits, and counts calls, concurrency and requests dropped
//! before they finished. Only Tokio time is used.
//!
//! Failures and delays are triggered by what a caller can observe: the stage asked, a path in
//! the request, or, for scheduler tests that build their own requests, the first item's id.
//! Like the real client, it only ever answers probabilities in [0, 1] or unknown.
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

/// Which requests a scripted failure or delay applies to.
enum Trigger {
    /// The request's first item has this id (requests the test built itself).
    FirstItem(String),
    /// The request asks this stage's question.
    Stage(SemanticStage),
    /// Some item of the request has this path.
    Path(String),
}

impl Trigger {
    fn matches(&self, req: &JevRequest) -> bool {
        match self {
            Self::FirstItem(id) => req.state.items.first().is_some_and(|i| &i.id == id),
            Self::Stage(s) => FakeClassifier::stage(req) == *s,
            Self::Path(p) => req.state.items.iter().any(|i| &i.path == p),
        }
    }
}

/// A scripted failure; `left: None` fails every matching request.
struct Failure {
    when: Trigger,
    error: ClassifyError,
    left: Option<AtomicUsize>,
}

pub struct FakeClassifier {
    rule: Option<Rule>,
    on_call: Option<Hook>,
    probability: HashMap<String, f64>,
    default: f64,
    failures: Vec<Failure>,
    delays: Vec<(Trigger, Duration)>,
    hold: Option<Arc<Semaphore>>,
    pub counters: Arc<Counters>,
    pub seen: Mutex<Vec<JevRequest>>,
    /// First item id and Tokio time of every call, in call order.
    pub started: Mutex<Vec<(String, tokio::time::Instant)>>,
}

impl FakeClassifier {
    pub fn new() -> Self {
        Self {
            rule: None,
            on_call: None,
            probability: HashMap::new(),
            default: 0.9,
            failures: vec![],
            delays: vec![],
            hold: None,
            counters: Arc::default(),
            seen: Mutex::new(vec![]),
            started: Mutex::new(vec![]),
        }
    }

    /// The probability answered for every question about item `id` (scheduler tests).
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

    /// The stage a request asks about, read from its first question. A question that is
    /// neither stage's `prompts/v1` text is a bug the fake refuses to hide.
    pub fn stage(req: &JevRequest) -> SemanticStage {
        let (q, item) = (&req.questions.0[0].1, &req.state.items[0]);
        [SemanticStage::FileAdmission, SemanticStage::SourceSelection]
            .into_iter()
            .find(|s| q.instructions == prompt::instructions(*s, &item.id))
            .unwrap_or_else(|| panic!("not a prompts/v1 question: {}", q.instructions))
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

    fn failing(mut self, when: Trigger, error: ClassifyError, times: Option<usize>) -> Self {
        self.failures.push(Failure {
            when,
            error,
            left: times.map(AtomicUsize::new),
        });
        self
    }

    /// Fails every request whose first item is `id` (scheduler tests).
    pub fn fail(self, id: &str, e: ClassifyError) -> Self {
        self.failing(Trigger::FirstItem(id.into()), e, None)
    }

    /// Fails the first `n` requests whose first item is `id`, then answers (scheduler tests).
    pub fn fail_times(self, id: &str, e: ClassifyError, n: usize) -> Self {
        self.failing(Trigger::FirstItem(id.into()), e, Some(n))
    }

    /// Fails every request of `stage`.
    pub fn fail_stage(self, stage: SemanticStage, e: ClassifyError) -> Self {
        self.failing(Trigger::Stage(stage), e, None)
    }

    /// Fails the first `n` requests of `stage`, then answers.
    pub fn fail_stage_times(self, stage: SemanticStage, e: ClassifyError, n: usize) -> Self {
        self.failing(Trigger::Stage(stage), e, Some(n))
    }

    /// Fails the first `n` requests that ask about `path`, then answers.
    pub fn fail_path_times(self, path: &str, e: ClassifyError, n: usize) -> Self {
        self.failing(Trigger::Path(path.into()), e, Some(n))
    }

    /// Delays the request whose first item is `id` (scheduler tests).
    pub fn delay(mut self, id: &str, d: Duration) -> Self {
        self.delays.push((Trigger::FirstItem(id.into()), d));
        self
    }

    /// Delays every request of `stage`.
    pub fn delay_stage(mut self, stage: SemanticStage, d: Duration) -> Self {
        self.delays.push((Trigger::Stage(stage), d));
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

    fn scripted_failure(&self, req: &JevRequest) -> Option<ClassifyError> {
        self.failures.iter().find_map(|f| {
            if !f.when.matches(req) {
                return None;
            }
            match &f.left {
                None => Some(f.error.clone()),
                Some(left) => left
                    .fetch_update(SeqCst, SeqCst, |n| n.checked_sub(1))
                    .is_ok()
                    .then(|| f.error.clone()),
            }
        })
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
        let head = req
            .state
            .items
            .first()
            .map(|i| i.id.clone())
            .unwrap_or_default();
        self.started
            .lock()
            .unwrap()
            .push((head, tokio::time::Instant::now()));
        let stage = Self::stage(req);
        if let Some(hook) = &self.on_call {
            hook(stage);
        }
        if let Some((_, d)) = self.delays.iter().find(|(t, _)| t.matches(req)) {
            tokio::time::sleep(*d).await;
        }
        // A scripted failure answers after its delay, without waiting for the gate.
        if let Some(e) = self.scripted_failure(req) {
            guard.1 = true;
            c.finished.fetch_add(1, SeqCst);
            return Err(e);
        }
        if let Some(gate) = &self.hold {
            gate.acquire().await.unwrap().forget();
        }
        guard.1 = true;
        c.finished.fetch_add(1, SeqCst);
        Ok(req
            .state
            .items
            .iter()
            .map(|i| {
                let p = match &self.rule {
                    Some(rule) => rule(stage, i),
                    None => Some(*self.probability.get(&i.id).unwrap_or(&self.default)),
                };
                // The real client never returns anything else (CA-ONLINE-10).
                assert!(
                    p.is_none_or(|p| (0.0..=1.0).contains(&p)),
                    "a provider never answers {p:?}"
                );
                p
            })
            .collect())
    }
}
