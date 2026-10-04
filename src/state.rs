//! Hook session state on disk (D-032): one private file per host session, outside the
//! workspace, holding fingerprints and flags only.

use crate::hook::SessionState;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A directory named by an environment variable: only a non-empty absolute path. An empty or
/// relative value would resolve against the current directory, which for a hook is the
/// workspace; it counts as unset (the XDG specification says to ignore a relative one).
pub fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

pub struct StateStore {
    dir: PathBuf,
}

impl StateStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// `$XDG_STATE_HOME/ripwire-broker`, else `~/.local/state/ripwire-broker`.
    pub fn default_dir() -> Option<PathBuf> {
        let base = env_dir("XDG_STATE_HOME")
            .or_else(|| env_dir("HOME").map(|h| h.join(".local/state")))?;
        Some(base.join("ripwire-broker"))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The session id is named by its hash, so the file name reveals nothing.
    fn path(&self, session_id: &str) -> PathBuf {
        let name = format!("{:x}", Sha256::digest(session_id.as_bytes()));
        self.dir.join(format!("{name}.json"))
    }

    /// An exclusive lock on this session until the returned file is dropped. Hooks of one
    /// session hold it across load → handle → save, so parallel ones take turns instead of
    /// reusing request ids and overwriting each other's memory (D-052).
    pub fn lock(&self, session_id: &str) -> std::io::Result<fs::File> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path(session_id).with_extension("lock"))?;
        file.lock()?;
        Ok(file)
    }

    /// A missing or unreadable file is a fresh session; so is one that is not a regular file.
    pub fn load(&self, session_id: &str) -> SessionState {
        read_regular(&self.path(session_id), MAX_STATE_BYTES)
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Every saved session in the directory. Lock files, temporaries and files that do not
    /// parse are not sessions; a missing directory has none.
    pub fn sessions(&self) -> Vec<SessionState> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return vec![];
        };
        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| read_regular(&p, MAX_STATE_BYTES))
            .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
            .collect()
    }

    /// Whether `session_id` has a saved state.
    pub fn has(&self, session_id: &str) -> bool {
        self.path(session_id).exists()
    }

    /// Removes what the sessions last touched before `cutoff` left behind: their state, their lock
    /// and their status line projection (D-147). A session whose lock is held is skipped: it may
    /// have just been resumed, and its state is about to be read (D-148). Temporaries and anything
    /// else are left alone.
    pub fn prune(&self, cutoff: std::time::SystemTime) {
        let stale = |path: &Path| {
            fs::symlink_metadata(path)
                .and_then(|m| m.modified())
                .is_ok_and(|at| at < cutoff)
        };
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for path in entries.filter_map(Result::ok).map(|e| e.path()) {
                // A session is its state and its lock; either one, alone and stale, is enough.
                let state = path.with_extension("json");
                let lock = path.with_extension("lock");
                let ours = path.extension().is_some_and(|x| x == "json" || x == "lock");
                let alone = !state.exists() || !lock.exists();
                if !ours || !stale(&state) && !(alone && stale(&path)) {
                    continue;
                }
                let held = fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&lock)
                    .map(|f| f.try_lock().map(|()| f));
                match held {
                    // Locked by a hook of that session: it is not stale after all.
                    Ok(Err(_)) => continue,
                    // Removed while holding the lock, so no hook of the session loads in between.
                    Ok(Ok(_guard)) => {
                        let _ = fs::remove_file(&state);
                        let _ = fs::remove_file(&lock);
                    }
                    Err(_) => {
                        let _ = fs::remove_file(&state);
                    }
                }
            }
        }
        if let Ok(entries) = fs::read_dir(self.dir.join("statusline")) {
            for path in entries.filter_map(Result::ok).map(|e| e.path()) {
                if path.extension().is_some_and(|x| x == "json") && stale(&path) {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }

    /// Forgets a session; missing files are fine.
    pub fn remove(&self, session_id: &str) {
        let _ = fs::remove_file(self.path(session_id));
        let _ = fs::remove_file(self.path(session_id).with_extension("lock"));
    }

    /// Written to a private temporary file and renamed: readers never see half a state.
    /// Concurrent hooks of one session: the last writer wins.
    pub fn save(&self, session_id: &str, state: &SessionState) -> std::io::Result<()> {
        write_private(
            &self.dir,
            &self.path(session_id),
            serde_json::to_string(state)?.as_bytes(),
        )
    }
}

/// A session's state stays far below this: about 5,000 fingerprints and a short log.
const MAX_STATE_BYTES: u64 = 16 * 1024 * 1024;

/// The bytes of a private file, opened once without following a link or blocking on a FIFO, and
/// every check made on what was opened (D-147). `None` when it is missing, unreadable, larger than
/// `max`, or not a regular file.
fn read_regular(path: &Path, max: u64) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > max {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= max).then_some(bytes)
}

/// Process-wide: threads writing the same file never share a temporary.
static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// A new file that did not exist and is not a link: a planted name is skipped, never written
/// through.
fn create_temp(path: &Path) -> std::io::Result<(PathBuf, fs::File)> {
    let mut last = None;
    for _ in 0..8 {
        let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp{}-{n}", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)
        {
            Ok(file) => return Ok((tmp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.expect("at least one attempt"))
}

/// Writes `bytes` to `path` through a private temporary file in `dir` and a rename: readers see
/// the old file or the new one, never half of it. A `dir` that is missing is created as 0700 (with
/// the missing directories above it); one that exists is left exactly as it is, since it may be a
/// directory the user owns for other reasons. A directory owned by another uid is refused. The
/// file is 0600. A failed write leaves no temporary behind.
pub(crate) fn write_private(dir: &Path, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let (tmp, mut file) = create_temp(path)?;
    let written = file
        .metadata()
        .and_then(|m| same_owner(fs::metadata(dir)?.uid(), m.uid()))
        .and_then(|()| file.write_all(bytes))
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// `file_uid` is that of a file this process just created, which is how it learns its own uid
/// without `unsafe`: state kept in another user's directory is not ours to write.
fn same_owner(dir_uid: u32, file_uid: u32) -> std::io::Result<()> {
    if dir_uid == file_uid {
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "the state directory belongs to another user",
    ))
}
