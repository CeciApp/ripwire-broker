//! Hook session state on disk (D-032): one private file per host session, outside the
//! workspace, holding fingerprints and flags only.

use crate::hook::SessionState;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct StateStore {
    dir: PathBuf,
}

impl StateStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// `$XDG_STATE_HOME/ripwire-broker`, else `~/.local/state/ripwire-broker`.
    pub fn default_dir() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
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
            .open(self.path(session_id).with_extension("lock"))?;
        file.lock()?;
        Ok(file)
    }

    /// A missing or unreadable file is a fresh session.
    pub fn load(&self, session_id: &str) -> SessionState {
        fs::read_to_string(self.path(session_id))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
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
            .filter_map(|p| fs::read_to_string(p).ok())
            .filter_map(|t| serde_json::from_str(&t).ok())
            .collect()
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
            &self.dir,
            &self.path(session_id),
            serde_json::to_string(state)?.as_bytes(),
        )
    }
}

/// Tightens an existing directory that is looser than 0700. Only one we own is touched (`owner` is
/// the uid of a file we just made), and never a sticky one: a shared `/tmp` given as the state
/// directory is not ours to restrict.
fn tighten(dir: &Path, owner: u32) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(dir)?;
    let mode = meta.mode();
    if meta.is_dir() && meta.uid() == owner && mode & 0o1000 == 0 && mode & 0o077 != 0 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
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
/// the old file or the new one, never half of it. `dir` is created as 0700 and, when it already
/// exists looser and is ours, tightened to it; so is `outer`, the directory above it that this
/// store also owns. The file is 0600. A failed write leaves no temporary behind.
pub(crate) fn write_private(
    outer: &Path,
    dir: &Path,
    path: &Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let (tmp, mut file) = create_temp(path)?;
    let written = file
        .metadata()
        .and_then(|m| {
            tighten(outer, m.uid())?;
            tighten(dir, m.uid())
        })
        .and_then(|()| file.write_all(bytes))
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}
