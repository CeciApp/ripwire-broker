//! The memory of one workspace on disk (PRD jev-mem §6): `<state-dir>/memory/<workspace_id>/`,
//! outside the repository, 0700 and 0600. A generation is published whole, through a private
//! temporary, `sync_all`, `rename` and a sync of the directory, so a reader sees the previous
//! generation or the new one and a power loss keeps one of them. What cannot be read is
//! unavailable and is never overwritten: it may be a newer schema or evidence of a fault.

use super::model::Record;
use super::time::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024 * 1024;
const SNAPSHOT: &str = "snapshot.json";

/// Why a store cannot be used. Carries no path or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    Symlink,
    /// The directory is open to the group or others.
    NotPrivate,
    /// The snapshot belongs to another user than its directory.
    ForeignOwner,
    NotRegular,
    TooLarge,
    Corrupt,
    UnknownSchema,
    Io,
}

/// One generation of a workspace's memory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub generation: u64,
    pub sequence: Sequence,
    /// By `node_id`.
    pub nodes: BTreeMap<String, Record>,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    schema_version: u32,
    #[serde(flatten)]
    state: State,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(state_dir: &Path, workspace_id: &str) -> Self {
        Self {
            dir: state_dir.join("memory").join(workspace_id),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The current generation; an empty state when there is none yet. Never creates anything.
    pub fn load(&self) -> Result<State, Unavailable> {
        Ok(self.read()?.unwrap_or_default())
    }

    /// Publishes `state` as the new generation, unless the current one cannot be read.
    pub fn publish(&self, state: &State) -> Result<(), Unavailable> {
        self.read()?;
        let bytes = serde_json::to_vec(&OnDisk {
            schema_version: SCHEMA_VERSION,
            state: state.clone(),
        })
        .map_err(|_| Unavailable::Io)?;
        crate::state::write_private(&self.dir, &self.dir.join(SNAPSHOT), &bytes)
            .and_then(|()| fs::File::open(&self.dir)?.sync_all())
            .map_err(|_| Unavailable::Io)
    }

    /// `None` when there is no store yet. The snapshot is opened once, without following a link
    /// or blocking on a FIFO, and every check is made on what was opened.
    fn read(&self) -> Result<Option<State>, Unavailable> {
        let dir = match fs::symlink_metadata(&self.dir) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Unavailable::Io),
        };
        if dir.file_type().is_symlink() {
            return Err(Unavailable::Symlink);
        }
        if !dir.is_dir() {
            return Err(Unavailable::NotRegular);
        }
        if dir.mode() & 0o077 != 0 {
            return Err(Unavailable::NotPrivate);
        }
        let file = match fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.dir.join(SNAPSHOT))
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(Unavailable::Symlink),
            Err(_) => return Err(Unavailable::Io),
        };
        let meta = file.metadata().map_err(|_| Unavailable::Io)?;
        if !meta.is_file() {
            return Err(Unavailable::NotRegular);
        }
        // Not reachable in a test without root: kept because the PRD asks for it (§6).
        if meta.uid() != dir.uid() {
            return Err(Unavailable::ForeignOwner);
        }
        if meta.len() > MAX_SNAPSHOT_BYTES {
            return Err(Unavailable::TooLarge);
        }
        let mut bytes = Vec::new();
        file.take(MAX_SNAPSHOT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Unavailable::Io)?;
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(Unavailable::TooLarge);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Unavailable::Corrupt)?;
        let schema = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64);
        if schema != Some(u64::from(SCHEMA_VERSION)) {
            return Err(Unavailable::UnknownSchema);
        }
        let on_disk: OnDisk = serde_json::from_value(value).map_err(|_| Unavailable::Corrupt)?;
        Ok(Some(on_disk.state))
    }
}
