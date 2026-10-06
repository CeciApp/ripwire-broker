//! The provider boundary (PRD §23.9, §23.12): one internal trait, even with one provider.
//! The client knows the protocol, never the repository; errors carry categories and HTTP
//! statuses only, never remote text or the credential.

use super::request::JevRequest;
use super::response::InvalidResponse;
use async_trait::async_trait;

#[async_trait]
pub trait Classifier: Send + Sync {
    /// The pinned model; part of every cache key.
    fn model(&self) -> &str;
    /// Response body bytes received so far, for `jev_response_bytes`; `0` when unknown.
    fn response_bytes(&self) -> super::metrics::Sized {
        super::metrics::Sized::default()
    }

    /// One attempt: the probability of each question, in request order (`None`: unknown).
    /// Retries, cooldown and splitting belong to the scheduler, never to the client.
    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError>;
}

/// The memory controller's view of the provider (PRD jev-mem §7): typed decisions about a
/// structured state. Same client, credential, endpoint and limits as [`Classifier`].
#[async_trait]
pub trait MemoryClassifier: Send + Sync {
    /// One attempt: a decision per question, in request order; retries are the caller's.
    async fn decide(
        &self,
        req: &super::request::StateRequest,
    ) -> Result<Vec<super::response::Decision>, ClassifyError>;
}

/// A failed attempt, classified (PRD §23.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifyError {
    /// `401`/`403`: cancel siblings; never retried.
    Auth(u16),
    /// `429`; the raw `Retry-After` value, parsed by the scheduler.
    RateLimited {
        retry_after: Option<String>,
    },
    /// `408` or no answer within the attempt's timeout.
    Timeout,
    /// `5xx`.
    Server(u16),
    /// Any other non-success status, including a refused redirect.
    Rejected(u16),
    Invalid(InvalidResponse),
    /// A response body above the client's limit.
    TooLarge,
    /// Connection refused, reset or closed.
    Network,
    /// No usable key (D-155): nothing was sent. Handled like a refused key, never retried.
    NoKey,
}

impl ClassifyError {
    /// A category for status and logs (PRD §23.9: never the remote message).
    pub fn category(&self) -> &'static str {
        match self {
            Self::Auth(_) => "auth",
            Self::RateLimited { .. } => "rate_limited",
            Self::Timeout => "timeout",
            Self::Server(_) => "server",
            Self::Rejected(_) => "rejected",
            Self::Invalid(_) => "invalid_response",
            Self::TooLarge => "response_too_large",
            Self::Network => "network",
            Self::NoKey => "no_key",
        }
    }

    /// Worth another attempt under the stage's retry policy (Phase 5).
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::Timeout | Self::Server(_) | Self::Network
        )
    }
}

impl std::fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(s) | Self::Server(s) | Self::Rejected(s) => {
                write!(f, "classifier {}: HTTP {s}", self.category())
            }
            Self::Invalid(why) => write!(f, "classifier {}: {why}", self.category()),
            _ => write!(f, "classifier {}", self.category()),
        }
    }
}

impl std::error::Error for ClassifyError {}

/// One client shared by discovery and memory, under one ceiling of requests in flight for the
/// whole process (PRD jev-mem §4, §8.2): memory never gets a second quota of concurrency.
pub struct Shared<T> {
    inner: std::sync::Arc<T>,
    permits: tokio::sync::Semaphore,
}

impl<T> Shared<T> {
    pub fn new(inner: std::sync::Arc<T>, max_in_flight: usize) -> Self {
        Self {
            inner,
            permits: tokio::sync::Semaphore::new(max_in_flight.max(1)),
        }
    }
}

#[async_trait]
impl<T: Classifier + 'static> Classifier for Shared<T> {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn response_bytes(&self) -> super::metrics::Sized {
        self.inner.response_bytes()
    }

    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| ClassifyError::Network)?;
        self.inner.classify(req).await
    }
}

#[async_trait]
impl<T: MemoryClassifier + 'static> MemoryClassifier for Shared<T> {
    async fn decide(
        &self,
        req: &super::request::StateRequest,
    ) -> Result<Vec<super::response::Decision>, ClassifyError> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| ClassifyError::Network)?;
        self.inner.decide(req).await
    }
}

/// The classifiers of a process. Without memory, discovery gets `client` exactly as before, under
/// its per-query ceiling only (PRD jev-mem §4: no change without `--memory`). With memory, both
/// get one [`Shared`] client under one process-wide ceiling.
#[allow(clippy::type_complexity)]
pub fn for_process<T: Classifier + MemoryClassifier + 'static>(
    client: std::sync::Arc<T>,
    max_in_flight: usize,
    memory: bool,
) -> (
    std::sync::Arc<dyn Classifier>,
    Option<std::sync::Arc<dyn MemoryClassifier>>,
) {
    if !memory {
        return (client, None);
    }
    let shared = std::sync::Arc::new(Shared::new(client, max_in_flight));
    (shared.clone(), Some(shared))
}
