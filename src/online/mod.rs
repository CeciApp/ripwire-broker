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
pub(crate) mod merge;
pub mod metrics;
pub mod prompt;
pub mod reader;
pub mod redact;
pub mod request;
pub mod response;
pub mod retry_after;
pub mod scheduler;

pub use coordinator::{OnlineConfig, OnlineEngine, OnlineTotals};

/// The variable that carries the provider key. Defined outside the `online` feature: every build
/// keeps it out of the processes it starts, since the variable can be set either way (D-146).
pub const KEY_VAR: &str = "RIPWIRE_BROKER_JEV_API_KEY";

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
