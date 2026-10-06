//! The server's projection for the status line (D-154): whether `serve` runs online and with
//! memory, and how many Jev requests, memory reads and memory stores it made in the last
//! [`WINDOW_SECS`] seconds. One small private file per server process and workspace, rewritten by
//! `serve` at most once a second, read by `statusline` without a lock. Counts only: no prompt,
//! path, query or response.
//!
//! A file proves nothing by existing (spec §24.6.4): the reader takes only those refreshed within
//! [`STALE_SECS`], and `serve` refreshes its own every [`HEARTBEAT_SECS`] while it runs.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

pub const SCHEMA_VERSION: u32 = 1;
/// The span the status line counts Jev requests over.
pub const WINDOW_SECS: u64 = 5;
/// How often a running server rewrites its file even with nothing new.
pub const HEARTBEAT_SECS: u64 = 10;
/// Older than this, a file is a server that stopped without removing it.
pub const STALE_SECS: u64 = 30;
pub const MAX_BYTES: u64 = 4 * 1024;
/// Files of one workspace the reader opens at most: a few live servers, and crash leftovers until
/// the hooks' prune removes them.
const MAX_FILES: usize = 16;
const PREFIX: &str = "server-";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub schema_version: u32,
    pub workspace_key: String,
    pub pid: u32,
    pub updated_at: u64,
    pub online: bool,
    pub memory: bool,
    /// `(second, count)`, oldest first, only seconds inside the window when written.
    pub jev_calls: Vec<(u64, u32)>,
    pub mem_reads: Vec<(u64, u32)>,
    pub mem_stores: Vec<(u64, u32)>,
    pub jev_key: KeyState,
}

/// Whether the server can talk to Jev (D-155). Ordered from best to worst: the bar shows the worst
/// of the live servers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    #[default]
    Ok,
    /// Jev refused the key (401/403), or it is malformed: calls fail until one succeeds.
    Invalid,
    /// `RIPWIRE_BROKER_JEV_API_KEY` is not set: no call is made.
    Missing,
}

impl KeyState {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Invalid,
            2 => Self::Missing,
            _ => Self::Ok,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Invalid => 1,
            Self::Missing => 2,
        }
    }
}

/// The count of `seconds` inside the [`WINDOW_SECS`] seconds that end at `now`.
fn recent(seconds: &[(u64, u32)], now: u64) -> u32 {
    seconds
        .iter()
        .filter(|(at, _)| *at <= now && now - at < WINDOW_SECS)
        .fold(0u32, |sum, (_, n)| sum.saturating_add(*n))
}

impl Status {
    /// Jev requests in the [`WINDOW_SECS`] seconds that end at `now`.
    pub fn recent_calls(&self, now: u64) -> u32 {
        recent(&self.jev_calls, now)
    }
}

/// What the status line shows: the servers of the workspace that are alive, taken together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct View {
    pub online: bool,
    pub memory: bool,
    pub jev_calls: u32,
    pub mem_reads: u32,
    pub mem_stores: u32,
    pub jev_key: KeyState,
}

/// How the server was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode {
    pub online: bool,
    pub memory: bool,
}

/// What the server counts for the status line. The Jev client records each request it sends,
/// the broker each memory read, and the memory publisher each observation it writes.
#[derive(Debug, Default)]
pub struct Activities {
    pub jev: Activity,
    pub mem_reads: Activity,
    pub mem_stores: Activity,
    key: AtomicU8,
    key_dirty: AtomicBool,
}

impl Activities {
    fn take_dirty(&self) -> bool {
        // Not short-circuited: every flag is cleared.
        self.jev.take_dirty()
            | self.mem_reads.take_dirty()
            | self.mem_stores.take_dirty()
            | self.take_key_dirty()
    }

    pub fn key(&self) -> KeyState {
        KeyState::from_u8(self.key.load(Ordering::Relaxed))
    }

    /// Records the key's state; a change is published like new activity.
    pub fn set_key(&self, state: KeyState) {
        if self.key.swap(state.as_u8(), Ordering::Relaxed) != state.as_u8() {
            self.key_dirty.store(true, Ordering::Relaxed);
        }
    }

    /// Whether the key's state changed since the last call.
    pub fn take_key_dirty(&self) -> bool {
        self.key_dirty.swap(false, Ordering::Relaxed)
    }
}

/// Events per second, kept for the window only.
#[derive(Debug, Default)]
pub struct Activity {
    seconds: Mutex<VecDeque<(u64, u32)>>,
    dirty: AtomicBool,
}

impl Activity {
    pub fn record(&self, now: u64) {
        let mut s = self.seconds.lock().unwrap_or_else(|e| e.into_inner());
        match s.back_mut() {
            Some((at, n)) if *at == now => *n = n.saturating_add(1),
            _ => s.push_back((now, 1)),
        }
        trim(&mut s, now);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// The counts inside the window that ends at `now`, oldest first.
    pub fn calls(&self, now: u64) -> Vec<(u64, u32)> {
        let mut s = self.seconds.lock().unwrap_or_else(|e| e.into_inner());
        trim(&mut s, now);
        s.iter().copied().collect()
    }

    /// Whether an event was recorded since the last call.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}

fn trim(s: &mut VecDeque<(u64, u32)>, now: u64) {
    while s
        .front()
        .is_some_and(|(at, _)| now.saturating_sub(*at) >= WINDOW_SECS)
    {
        s.pop_front();
    }
}

fn prefix(root: &Path) -> String {
    format!("{PREFIX}{}-", crate::statusline_state::workspace_key(root))
}

/// Next to the hooks' projections, so their prune removes what a crashed server left.
pub fn path(state_dir: &Path, root: &Path, pid: u32) -> PathBuf {
    state_dir
        .join("statusline")
        .join(format!("{}{pid}.json", prefix(root)))
}

pub fn publish(state_dir: &Path, root: &Path, status: &Status) -> std::io::Result<()> {
    let file = path(state_dir, root, status.pid);
    let dir = file.parent().expect("statusline dir");
    crate::state::write_private(dir, &file, serde_json::to_string(status)?.as_bytes())
}

/// The live servers of `root` taken together; `None` when there is none. Creates nothing, opens
/// each file once without following a link, and trusts nothing it reads: name, key and schema are
/// checked on the content.
pub fn read(state_dir: &Path, root: &Path, now: u64) -> Option<View> {
    let want = prefix(root);
    let key = crate::statusline_state::workspace_key(root);
    let entries = std::fs::read_dir(state_dir.join("statusline")).ok()?;
    let mut view: Option<View> = None;
    let ours = entries.filter_map(Result::ok).filter(|e| {
        e.file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(&want) && n.ends_with(".json"))
    });
    for entry in ours.take(MAX_FILES) {
        let Some(bytes) = crate::state::read_regular(&entry.path(), MAX_BYTES) else {
            continue;
        };
        let Ok(s) = serde_json::from_slice::<Status>(&bytes) else {
            continue;
        };
        if s.schema_version != SCHEMA_VERSION
            || s.workspace_key != key
            || now.saturating_sub(s.updated_at) > STALE_SECS
        {
            continue;
        }
        let v = view.get_or_insert_with(View::default);
        v.online |= s.online;
        v.memory |= s.memory;
        v.jev_calls = v.jev_calls.saturating_add(s.recent_calls(now));
        v.mem_reads = v.mem_reads.saturating_add(recent(&s.mem_reads, now));
        v.mem_stores = v.mem_stores.saturating_add(recent(&s.mem_stores, now));
        v.jev_key = v.jev_key.max(s.jev_key);
    }
    view
}

/// Keeps this process's file current while it lives: at once, then within a second of a new event,
/// and every [`HEARTBEAT_SECS`] otherwise. Dropping it stops the task and removes the file.
pub struct Publisher {
    task: tokio::task::JoinHandle<()>,
    /// Held across each write and the removal, so a write in progress cannot recreate the file.
    stopped: Arc<Mutex<bool>>,
    file: PathBuf,
}

impl Publisher {
    pub fn start(state_dir: PathBuf, root: PathBuf, mode: Mode, activity: Arc<Activities>) -> Self {
        let pid = std::process::id();
        let file = path(&state_dir, &root, pid);
        let stopped = Arc::new(Mutex::new(false));
        let gate = stopped.clone();
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
            let (mut last, mut pending): (Option<u64>, bool) = (None, false);
            loop {
                tick.tick().await;
                let now = crate::hook::now();
                pending |= activity.take_dirty();
                let due = last.is_none_or(|at| now.saturating_sub(at) >= HEARTBEAT_SECS);
                if !pending && !due {
                    continue;
                }
                let status = Status {
                    schema_version: SCHEMA_VERSION,
                    workspace_key: crate::statusline_state::workspace_key(&root),
                    pid,
                    updated_at: now,
                    online: mode.online,
                    memory: mode.memory,
                    jev_calls: activity.jev.calls(now),
                    mem_reads: activity.mem_reads.calls(now),
                    mem_stores: activity.mem_stores.calls(now),
                    jev_key: activity.key(),
                };
                let stopped = gate.lock().unwrap_or_else(|e| e.into_inner());
                if *stopped {
                    return;
                }
                // A failed write only leaves the bar without the server: the next tick retries.
                if publish(&state_dir, &root, &status).is_ok() {
                    (last, pending) = (Some(now), false);
                }
            }
        });
        Publisher {
            task,
            stopped,
            file,
        }
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        self.task.abort();
        let mut stopped = self.stopped.lock().unwrap_or_else(|e| e.into_inner());
        *stopped = true;
        let _ = std::fs::remove_file(&self.file);
    }
}
