//! The optional `--online` adapter (PRD §23): a remote semantic classifier that only
//! enriches `context_for_task`. Everything here is off unless the process starts with
//! `--online`; only the HTTP client needs the `online` Cargo feature (D-059).

pub mod cache;
pub mod classifier;
mod coordinator;
#[cfg(feature = "online")]
pub mod credential;
pub mod decision;
#[cfg(feature = "online")]
pub mod jev;
#[cfg(feature = "online")]
pub mod log;
pub(crate) mod merge;
pub mod metrics;
pub mod prompt;
pub mod reader;
pub mod redact;
pub mod request;
pub mod response;
pub mod retry_after;
pub mod scheduler;

pub use coordinator::{OnlineConfig, OnlineEngine};

/// The pinned model when no `--jev-model` is given.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";
/// How long one request to the provider may take when no `--jev-timeout-ms` is given.
pub const DEFAULT_TIMEOUT_MS: u64 = 15_000;
/// Requests in flight at once when no `--jev-max-in-flight` is given (RF-ONLINE-08).
pub const DEFAULT_MAX_IN_FLIGHT: usize = 4;

/// Who answers the `systemone` protocol (PRD §23.9, D-166). The request and the answers are the
/// same on both sides; only the transport differs: the URL, the token and the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JevProvider {
    #[default]
    TypeSafe,
    Cloudflare,
}

/// What Workers AI accepts as `model`, in the body and in the URL: any other name is refused
/// there (error 5006), a Jev name included.
const CLOUDFLARE_MODELS: [&str; 2] = ["clef", "clef-flash"];

impl JevProvider {
    /// The value of `--jev-provider`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::TypeSafe, Self::Cloudflare]
            .into_iter()
            .find(|p| p.name() == name)
    }

    /// For the status, the provenance and the cache key.
    pub fn name(self) -> &'static str {
        match self {
            Self::TypeSafe => "typesafe",
            Self::Cloudflare => "cloudflare",
        }
    }

    /// The allowlisted host: no URL comes from configuration, only this choice.
    pub fn host(self) -> &'static str {
        match self {
            Self::TypeSafe => "api.typesafe.ai",
            Self::Cloudflare => "api.cloudflare.com",
        }
    }

    /// The model when no `--jev-model` is given. Pinned for TypeSafe; Cloudflare has no versions
    /// in its names, and the smaller model is the one closer to Jev's price.
    pub fn default_model(self) -> &'static str {
        match self {
            Self::TypeSafe => DEFAULT_MODEL,
            Self::Cloudflare => "clef-flash",
        }
    }

    /// Questions one request may carry. Workers AI refuses more than 64 (a 422); TypeSafe takes
    /// the protocol's own ceiling.
    pub fn max_questions(self) -> usize {
        match self {
            Self::TypeSafe => request::MAX_QUESTIONS,
            Self::Cloudflare => 64,
        }
    }

    /// The path of the endpoint. Cloudflare's carries the account and the model, so both are
    /// checked here, once: an account is letters and digits only, and a model is one of
    /// Cloudflare's. The errors name the option to fix.
    fn path(self, account: Option<&str>, model: &str) -> Result<String, String> {
        let account = match (self, account) {
            (Self::TypeSafe, None) => return Ok("/v1/systemone".into()),
            (Self::TypeSafe, Some(_)) => {
                return Err("--jev-account-id is only for --jev-provider cloudflare".into());
            }
            (Self::Cloudflare, None) => {
                return Err("--jev-provider cloudflare needs --jev-account-id".into());
            }
            (Self::Cloudflare, Some(account)) => account,
        };
        let plain = account.bytes().all(|b| b.is_ascii_alphanumeric());
        if account.is_empty() || account.len() > 64 || !plain {
            return Err("--jev-account-id takes letters and digits only".into());
        }
        if !CLOUDFLARE_MODELS.contains(&model) {
            return Err("--jev-provider cloudflare: --jev-model takes clef or clef-flash".into());
        }
        Ok(format!(
            "/client/v4/accounts/{account}/ai/run/@cf/cloudflare/{model}"
        ))
    }

    /// Where the requests go: HTTPS to [`Self::host`], and nowhere else.
    pub fn endpoint(self, account: Option<&str>, model: &str) -> Result<String, String> {
        let path = self.path(account, model)?;
        Ok(format!("https://{}{path}", self.host()))
    }

    /// Test fixtures only: the same path over plain HTTP on `127.0.0.1`.
    #[doc(hidden)]
    pub fn loopback(self, port: u16, account: Option<&str>, model: &str) -> Result<String, String> {
        let path = self.path(account, model)?;
        Ok(format!("http://127.0.0.1:{port}{path}"))
    }
}

/// The variable that carries the provider key. Defined outside the `online` feature: every build
/// keeps it out of the processes it starts, since the variable can be set either way (D-146).
pub const KEY_VAR: &str = "RIPWIRE_BROKER_JEV_API_KEY";

/// `no_jev_api_key` (D-155): set by `serve` when [`KEY_VAR`] is not set. While it is on, every call
/// to Jev is skipped before anything is sent, whatever client makes it.
static NO_JEV_API_KEY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn no_jev_api_key() -> bool {
    NO_JEV_API_KEY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Only `serve` sets it, once, at startup: a test process never does, so its clients are not
/// affected by one another.
pub fn set_no_jev_api_key(on: bool) {
    NO_JEV_API_KEY.store(on, std::sync::atomic::Ordering::Relaxed);
}

use crate::model::{Item, Role};
use serde::Serialize;

/// The two classifier questions this adapter asks (PRD §23.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStage {
    /// Is the file worth reading at all? `p > 0.25` admits it.
    FileAdmission,
    /// Is this block evidence? `p > 0.50` selects it; `(0.25, 0.50]` makes a reading lead.
    SourceSelection,
}

/// Where a candidate path came from (D-061).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOrigin {
    /// A path the ripwire route already chose (Phase 4 rescore).
    Planner,
    /// A sibling found by the one-level lookahead (Phase 5).
    Lookahead,
}

/// A file the classifier may evaluate, ranked before the budgeter sees it (D-061).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedPath {
    pub path: String,
    /// Position of the path's first item among the items that reach here — docs are filtered
    /// out before the count, so this is not an index into ripwire's raw output. Only the
    /// relative order matters, and filtering preserves it (D-110).
    pub rank: usize,
    /// Best (lowest) priority among the path's items.
    pub priority: u8,
    pub origin: PathOrigin,
    /// Lines of ripwire symbols in the file, sorted and distinct; links chunks to symbols.
    pub lines: Vec<u64>,
}

/// The planner paths of a route's items: best priority first, then ripwire's order. Docs are
/// left out, since the classifier's questions are about code (D-061).
pub fn ranked_paths<'a>(items: impl IntoIterator<Item = (u8, &'a Item)>) -> Vec<RankedPath> {
    let mut paths: Vec<RankedPath> = Vec::new();
    for (rank, (priority, item)) in items
        .into_iter()
        .filter(|(_, i)| i.role != Role::Doc)
        .enumerate()
    {
        let at = match paths.iter().position(|p| p.path == item.path) {
            Some(at) => at,
            None => {
                paths.push(RankedPath {
                    path: item.path.clone(),
                    rank,
                    priority,
                    origin: PathOrigin::Planner,
                    lines: vec![],
                });
                paths.len() - 1
            }
        };
        let path = &mut paths[at];
        path.priority = path.priority.min(priority);
        if let Some(line) = item.line {
            path.lines.push(line);
        }
    }
    for p in &mut paths {
        p.lines.sort_unstable();
        p.lines.dedup();
    }
    paths.sort_by_key(|p| (p.priority, p.rank));
    paths
}
