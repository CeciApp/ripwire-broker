//! Hook session state on disk (D-032): one private file per host session, outside the
//! workspace, holding fingerprints and flags only.

use crate::hook::SessionState;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

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

    /// Forgets a session; missing files are fine.
    pub fn remove(&self, session_id: &str) {
        let _ = fs::remove_file(self.path(session_id));
        let _ = fs::remove_file(self.path(session_id).with_extension("lock"));
    }

    /// Written to a private temporary file and renamed: readers never see half a state.
    /// Concurrent hooks of one session: the last writer wins.
    pub fn save(&self, session_id: &str, state: &SessionState) -> std::io::Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;
        let path = self.path(session_id);
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(serde_json::to_string(state)?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, &path)
    }
}
