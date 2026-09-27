//! `OnlineMetrics` (PRD §23.11): counts and times only, never query, path, source or
//! credential. Per-request figures come from `Metered`, a wrapper around the classifier;
//! per-discovery figures from the coordinator; delivered tokens from the envelope.

use super::classifier::{Classifier, ClassifyError};
use super::request::JevRequest;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Latencies kept for the percentiles.
pub const LATENCY_WINDOW: usize = 256;

#[derive(Debug, Default, Clone, Serialize)]
pub struct Sized {
    pub total: u64,
    pub max: u64,
}

impl Sized {
    pub fn add(&mut self, n: u64) {
        self.total += n;
        self.max = self.max.max(n);
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// The status view of the metrics, with the names of §23.11.
#[derive(Debug, Default, Clone, Serialize)]
pub struct OnlineMetrics {
    pub jev_requests_total: u64,
    pub jev_questions_total: u64,
    pub jev_in_flight: u64,
    /// Questions per request.
    pub jev_batch_items: Sized,
    pub jev_request_bytes: Sized,
    pub jev_response_bytes: Sized,
    pub jev_latency_ms: Percentiles,
    pub jev_cache_hits_total: u64,
    pub jev_rate_limit_total: u64,
    pub jev_retry_total: u64,
    pub jev_split_total: u64,
    /// Files the classifier was asked about (planner and lookahead).
    pub semantic_candidates_total: u64,
    pub semantic_selected_ranges_total: u64,
    /// Lookahead files admitted: the gain beyond ripwire.
    pub semantic_only_candidates_total: u64,
    /// Tokens of semantic evidence that reached the agent.
    pub online_context_tokens_estimated: u64,
}

/// What `Metered` measures on every request.
#[derive(Debug, Default)]
struct Wire {
    requests: u64,
    questions: u64,
    batch_items: Sized,
    request_bytes: Sized,
    latency_ms: VecDeque<f64>,
}

/// The classifier as the scheduler sees it: the real one, measured.
pub struct Metered {
    inner: Arc<dyn Classifier>,
    wire: Mutex<Wire>,
    in_flight: AtomicU64,
}

impl Metered {
    pub fn new(inner: Arc<dyn Classifier>) -> Self {
        Self {
            inner,
            wire: Mutex::default(),
            in_flight: AtomicU64::new(0),
        }
    }

    /// Fills the per-request fields of `m`.
    pub fn fill(&self, m: &mut OnlineMetrics) {
        let wire = self.wire.lock().unwrap();
        m.jev_requests_total = wire.requests;
        m.jev_questions_total = wire.questions;
        m.jev_in_flight = self.in_flight.load(Relaxed);
        m.jev_batch_items = wire.batch_items.clone();
        m.jev_request_bytes = wire.request_bytes.clone();
        m.jev_response_bytes = self.inner.response_bytes();
        let mut sorted: Vec<f64> = wire.latency_ms.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| match sorted.len() {
            0 => 0.0,
            n => sorted[((n as f64 * q).ceil() as usize).clamp(1, n) - 1],
        };
        m.jev_latency_ms = Percentiles {
            p50: at(0.50),
            p95: at(0.95),
            p99: at(0.99),
        };
    }
}

/// Decrements the in-flight gauge even when the request is dropped (cancelled).
struct Flying<'a>(&'a AtomicU64);

impl Drop for Flying<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Relaxed);
    }
}

#[async_trait]
impl Classifier for Metered {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn response_bytes(&self) -> Sized {
        self.inner.response_bytes()
    }

    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError> {
        {
            let mut wire = self.wire.lock().unwrap();
            wire.requests += 1;
            wire.questions += req.questions.0.len() as u64;
            wire.batch_items.add(req.state.items.len() as u64);
            let bytes = serde_json::to_vec(req).map_or(0, |b| b.len());
            wire.request_bytes.add(bytes as u64);
        }
        self.in_flight.fetch_add(1, Relaxed);
        let _flying = Flying(&self.in_flight);
        let started = Instant::now();
        let result = self.inner.classify(req).await;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut wire = self.wire.lock().unwrap();
        if wire.latency_ms.len() == LATENCY_WINDOW {
            wire.latency_ms.pop_front();
        }
        wire.latency_ms.push_back(ms);
        result
    }
}
