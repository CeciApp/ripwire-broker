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
