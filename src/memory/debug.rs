//! `--memory-debug-log` (D-164): one line per memory event, in `debug.log` beside the store, for a
//! person following it with `tail -F` in another terminal. Off by default.
//!
//! Every process of the workspace writes to the same file: the server, each hook, the `memory`
//! commands. A line goes out in one `write` on an `O_APPEND` descriptor, so lines of different
//! processes never interleave. A line carries ids, counts, reasons and durations, never a memory's
//! text, a task, a file's body or a prompt: what goes to the provider is `jev.log`'s (`--log`).
//!
//! Past [`MAX_BYTES`] the file is renamed to `debug.log.1` (replacing the previous one) and a new
//! one is started; a writer that finds the path no longer names its file reopens it. A failed
//! write is ignored: the log never fails what it describes.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const FILE: &str = "debug.log";
/// About 10 MiB of lines before the file is set aside.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
/// Width of the stage column, so the details line up.
const STAGE: usize = 9;

pub struct DebugLog {
    path: PathBuf,
    /// `serve`, `hook`, `drain` or `cli`, then `#pid`.
    who: String,
    max_bytes: u64,
    file: Mutex<File>,
}

impl std::fmt::Debug for DebugLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebugLog")
            .field("path", &self.path)
            .finish()
    }
}

fn open_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

impl DebugLog {
    /// Appends to `dir/debug.log`, creating it 0600 (and a missing `dir` 0700). A link is refused.
    pub fn open(dir: &Path, process: &str) -> std::io::Result<Self> {
        Self::open_with_limit(dir, process, MAX_BYTES)
    }

    /// [`DebugLog::open`] with another rotation size, for tests.
    pub fn open_with_limit(dir: &Path, process: &str, max_bytes: u64) -> std::io::Result<Self> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        let path = dir.join(FILE);
        let file = open_file(&path)?;
        Ok(Self {
            path,
            who: format!("{process}#{}", std::process::id()),
            max_bytes,
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line: UTC time, process, stage and `detail`, control characters replaced by spaces.
    pub fn event(&self, stage: &str, detail: &str) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let detail: String = detail
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let line = format!(
            "{} {} {stage:<STAGE$} {detail}\n",
            super::time::utc(secs),
            self.who
        );
        let mut file = self.file.lock().unwrap_or_else(|p| p.into_inner());
        self.follow(&mut file, line.len() as u64);
        let _ = file.write_all(line.as_bytes());
    }

    /// Before a write: a file another process set aside is left for the new one at the path, and
    /// a file this line would take past the limit is set aside.
    fn follow(&self, file: &mut File, adding: u64) {
        let current = file.metadata().ok();
        let at_path = std::fs::symlink_metadata(&self.path).ok();
        let moved = match (&current, &at_path) {
            (Some(c), Some(p)) => c.ino() != p.ino() || c.dev() != p.dev(),
            _ => true,
        };
        let full = current.is_some_and(|c| c.len() > 0 && c.len() + adding > self.max_bytes);
        if full && !moved {
            let _ = std::fs::rename(&self.path, self.path.with_extension("log.1"));
        }
        if (full || moved)
            && let Ok(f) = open_file(&self.path)
        {
            *file = f;
        }
    }
}

/// The first 12 characters of an id: enough to tell memories apart in a log, and to grep for.
pub fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// A serde enum's name as it is written (`snake_case`), for a line.
pub fn name(v: &impl serde::Serialize) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        _ => "?".into(),
    }
}
