//! The memory of one workspace on disk (PRD jev-mem §6): `<state-dir>/memory/<workspace_id>/`,
//! outside the repository, 0700 and 0600. A generation is published whole, through a private
//! temporary, `sync_all`, `rename` and a sync of the directory, so a reader sees the previous
//! generation or the new one and a power loss keeps one of them. What cannot be read is
//! unavailable and is never overwritten: it may be a newer schema or evidence of a fault.
//!
//! Observations arrive in a spool of immutable files, one per node, written without a lock so a
//! hook never waits. One writer at a time incorporates them into a new generation and only then
//! removes them: a crash in between replays the spool, and a replay is the same node.

use super::model::Record;
use super::time::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
const SNAPSHOT: &str = "snapshot.json";
const SPOOL: &str = "spool";
const LOCK: &str = "lock";
/// Written by `forget --all`; only `memory resume` removes it (PD-4).
const REVOKED: &str = "revoked";

/// The caps of PRD jev-mem §6; injectable so a test can reach them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_nodes: usize,
    pub spool_entries: usize,
    pub spool_bytes: u64,
    pub snapshot_bytes: u64,
    /// Spool and snapshot together.
    pub total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 2_000,
            spool_entries: 1_000,
            spool_bytes: 16 * 1024 * 1024,
            snapshot_bytes: 64 * 1024 * 1024,
            total_bytes: 96 * 1024 * 1024,
        }
    }
}

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

/// Which cap a write ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Full {
    Nodes,
    SpoolEntries,
    SpoolBytes,
    Snapshot,
    Total,
}

/// Why a write did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Unavailable(Unavailable),
    Full(Full),
    /// Another writer holds the store; nobody waits for it.
    Locked,
    /// A node id that cannot name a spool file.
    InvalidId,
    /// Stopped at a [`Step`] by [`Store::ingest_crashing_at`].
    Crashed,
    /// Collection was revoked by `forget --all` and not resumed.
    Revoked,
}

impl From<Unavailable> for Refusal {
    fn from(u: Unavailable) -> Self {
        Self::Unavailable(u)
    }
}

/// The points of an ingestion a crash can fall between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    BeforePublish,
    AfterPublish,
    /// After the first spool entry is removed.
    MidRemoval,
}

/// What one ingestion did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ingested {
    pub added: usize,
    /// Already in the store: removed from the spool, nothing counted.
    pub duplicates: usize,
    /// Spool entries that did not parse; removed.
    pub rejected: usize,
    /// Spool entries of forgotten nodes; removed, never incorporated.
    pub forgotten: usize,
    /// The cap that stopped it; what did not fit stays pending.
    pub refused: Option<Full>,
}

/// One generation of a workspace's memory.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub generation: u64,
    pub sequence: Sequence,
    /// By `node_id`.
    pub nodes: BTreeMap<String, Record>,
    /// The latest wall clock a sweep trusted; an earlier reading means the clock went back.
    pub trusted_ms: u64,
    /// Forgotten node ids and until when they stay blocked from coming back.
    pub tombstones: BTreeMap<String, u64>,
}

/// What one retention sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    pub removed: usize,
    /// The clock is behind the trusted reading: nothing expires by age until it catches up.
    pub suspended: bool,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    schema_version: u32,
    #[serde(flatten)]
    state: State,
}

fn on_disk(state: &State) -> Result<Vec<u8>, Unavailable> {
    serde_json::to_vec(&OnDisk {
        schema_version: SCHEMA_VERSION,
        state: state.clone(),
    })
    .map_err(|_| Unavailable::Io)
}

pub struct Store {
    dir: PathBuf,
    limits: Limits,
}

impl Store {
    pub fn new(state_dir: &Path, workspace_id: &str) -> Self {
        Self::with_limits(state_dir, workspace_id, Limits::default())
    }

    pub fn with_limits(state_dir: &Path, workspace_id: &str, limits: Limits) -> Self {
        Self {
            dir: state_dir.join("memory").join(workspace_id),
            limits,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The current generation; an empty state when there is none yet. Never creates anything
    /// and never waits for the writer.
    pub fn load(&self) -> Result<State, Unavailable> {
        Ok(self.read()?.unwrap_or_default())
    }

    /// Publishes `state` as the new generation, unless the current one cannot be read.
    pub fn publish(&self, state: &State) -> Result<(), Unavailable> {
        self.read()?;
        self.write_snapshot(&on_disk(state)?)
    }

    fn write_snapshot(&self, bytes: &[u8]) -> Result<(), Unavailable> {
        crate::state::write_private(&self.dir, &self.dir.join(SNAPSHOT), bytes)
            .and_then(|()| fs::File::open(&self.dir)?.sync_all())
            .map_err(|_| Unavailable::Io)
    }

    /// The exclusive writer, held until dropped. Never waits: a held store is [`Refusal::Locked`].
    pub fn writer(&self) -> Result<fs::File, Refusal> {
        self.check_dir()?;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)
            .map_err(|_| Unavailable::Io)?;
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.dir.join(LOCK))
            .map_err(|_| Unavailable::Io)?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(_) => Err(Refusal::Locked),
        }
    }

    /// Publishes an admitted observation to the spool. Durable once it returns; no lock, no
    /// snapshot read. A replay of a pending node replaces its file and takes no room.
    pub fn enqueue(&self, record: &Record) -> Result<(), Refusal> {
        let name = spool_name(&record.node_id)?;
        self.check_dir()?;
        if self.is_revoked() {
            return Err(Refusal::Revoked);
        }
        let spool = self.dir.join(SPOOL);
        let bytes = serde_json::to_vec(record).map_err(|_| Unavailable::Io)?;
        let target = spool.join(&name);
        if fs::symlink_metadata(&target).is_err() {
            let (entries, used) = self.spool_usage()?;
            if entries >= self.limits.spool_entries {
                return Err(Refusal::Full(Full::SpoolEntries));
            }
            if used + bytes.len() as u64 > self.limits.spool_bytes {
                return Err(Refusal::Full(Full::SpoolBytes));
            }
            let snapshot = fs::symlink_metadata(self.dir.join(SNAPSHOT)).map_or(0, |m| m.len());
            if snapshot + used + bytes.len() as u64 > self.limits.total_bytes {
                return Err(Refusal::Full(Full::Total));
            }
        }
        crate::state::write_private(&spool, &target, &bytes)
            .and_then(|()| fs::File::open(&spool)?.sync_all())
            .map_err(|_| Refusal::Unavailable(Unavailable::Io))
    }

    /// Observations waiting in the spool.
    pub fn pending(&self) -> Result<usize, Unavailable> {
        Ok(self.spool_usage()?.0)
    }

    /// Incorporates the spool into a new generation under the writer lock.
    pub fn ingest(&self) -> Result<Ingested, Refusal> {
        self.ingest_until(None)
    }

    /// [`Store::ingest`] stopped at `step`, as a crash there would: for fault-injection tests.
    pub fn ingest_crashing_at(&self, step: Step) -> Result<Ingested, Refusal> {
        self.ingest_until(Some(step))
    }

    fn ingest_until(&self, crash: Option<Step>) -> Result<Ingested, Refusal> {
        let _writer = self.writer()?;
        if self.is_revoked() {
            return Err(Refusal::Revoked);
        }
        let mut state = self.load()?;
        let mut done = Ingested::default();
        let mut consumed = Vec::new();
        let mut size = on_disk(&state)?.len() as u64;
        for path in self.spool_files()? {
            let Some(bytes) = read_checked(&path, self.limits.spool_bytes)? else {
                continue;
            };
            let Ok(mut record) = Record::parse(&bytes) else {
                done.rejected += 1;
                consumed.push(path);
                continue;
            };
            if state.tombstones.contains_key(&record.node_id) {
                done.forgotten += 1;
                consumed.push(path);
                continue;
            }
            if state.nodes.contains_key(&record.node_id) {
                done.duplicates += 1;
                consumed.push(path);
                continue;
            }
            if state.nodes.len() >= self.limits.max_nodes {
                done.refused = Some(Full::Nodes);
                break;
            }
            // The key, its quotes, the colon and the comma around the record.
            let grows = bytes.len() as u64 + record.node_id.len() as u64 + 4;
            if size + grows > self.limits.snapshot_bytes {
                done.refused = Some(Full::Snapshot);
                break;
            }
            record.ingest_seq = state
                .sequence
                .advance()
                .ok_or(Full::Nodes)
                .map_err(Refusal::Full)?;
            record.generation = state.generation + 1;
            size += grows;
            state.nodes.insert(record.node_id.clone(), record);
            done.added += 1;
            consumed.push(path);
        }
        if crash == Some(Step::BeforePublish) {
            return Err(Refusal::Crashed);
        }
        if done.added > 0 {
            state.generation += 1;
            let bytes = on_disk(&state)?;
            if bytes.len() as u64 > self.limits.snapshot_bytes {
                return Err(Refusal::Full(Full::Snapshot));
            }
            self.write_snapshot(&bytes)?;
        }
        if crash == Some(Step::AfterPublish) {
            return Err(Refusal::Crashed);
        }
        for (i, path) in consumed.iter().enumerate() {
            let _ = fs::remove_file(path);
            if i == 0 && crash == Some(Step::MidRemoval) {
                return Err(Refusal::Crashed);
            }
        }
        Ok(done)
    }

    /// Retention (PRD jev-mem §6): removes what expired by `now_ms` and every note derived from
    /// it, at any depth. A derived note never outlives a parent.
    pub fn sweep(&self, now_ms: u64) -> Result<Swept, Refusal> {
        let _writer = self.writer()?;
        let mut state = self.load()?;
        if now_ms < state.trusted_ms {
            return Ok(Swept {
                removed: 0,
                suspended: true,
            });
        }
        let due = state
            .nodes
            .values()
            .filter(|r| r.expires_at_ms <= now_ms)
            .map(|r| r.node_id.clone())
            .collect();
        let gone = with_descendants(&state, due);
        for id in &gone {
            state.nodes.remove(id);
        }
        if !gone.is_empty() {
            state.generation += 1;
        }
        state.tombstones.retain(|_, until| *until > now_ms);
        state.trusted_ms = now_ms;
        self.write_snapshot(&on_disk(&state)?)?;
        Ok(Swept {
            removed: gone.len(),
            suspended: false,
        })
    }

    /// Forgets `node_id` and every note derived from it (PRD jev-mem §6): a new generation
    /// without them, then their spool entries go. A tombstone keeps each id out until `until_ms`,
    /// including an id that never reached the store. Returns how many nodes were removed.
    pub fn forget(&self, node_id: &str, until_ms: u64) -> Result<usize, Refusal> {
        let name = spool_name(node_id)?;
        let _writer = self.writer()?;
        let mut state = self.load()?;
        let gone = with_descendants(&state, BTreeSet::from([node_id.to_string()]));
        let mut removed = 0;
        for id in &gone {
            removed += usize::from(state.nodes.remove(id).is_some());
            state.tombstones.insert(id.clone(), until_ms);
        }
        state.generation += 1;
        self.write_snapshot(&on_disk(&state)?)?;
        let spool = self.dir.join(SPOOL);
        let _ = fs::remove_file(spool.join(name));
        for id in gone.iter().filter(|id| *id != node_id) {
            if let Ok(name) = spool_name(id) {
                let _ = fs::remove_file(spool.join(name));
            }
        }
        Ok(removed)
    }

    /// `forget --all`: revokes collection first, so a crash midway never leaves it on, then
    /// forgets every node and every pending observation. Returns how many nodes were removed.
    pub fn forget_all(&self, until_ms: u64) -> Result<usize, Refusal> {
        let _writer = self.writer()?;
        crate::state::write_private(&self.dir, &self.dir.join(REVOKED), b"")
            .and_then(|()| fs::File::open(&self.dir)?.sync_all())
            .map_err(|_| Unavailable::Io)?;
        let mut state = self.load()?;
        let removed = state.nodes.len();
        for id in std::mem::take(&mut state.nodes).into_keys() {
            state.tombstones.insert(id, until_ms);
        }
        state.generation += 1;
        self.write_snapshot(&on_disk(&state)?)?;
        for path in self.spool_files()? {
            let _ = fs::remove_file(path);
        }
        Ok(removed)
    }

    /// Whether `forget --all` revoked collection; it stays so across restarts until resumed.
    pub fn is_revoked(&self) -> bool {
        fs::symlink_metadata(self.dir.join(REVOKED)).is_ok()
    }

    /// `memory resume`: lifts the revocation. `false` when there was none.
    pub fn resume(&self) -> Result<bool, Refusal> {
        let _writer = self.writer()?;
        if !self.is_revoked() {
            return Ok(false);
        }
        fs::remove_file(self.dir.join(REVOKED))
            .and_then(|()| fs::File::open(&self.dir)?.sync_all())
            .map_err(|_| Unavailable::Io)?;
        Ok(true)
    }

    fn spool_files(&self) -> Result<Vec<PathBuf>, Unavailable> {
        let mut files: Vec<PathBuf> = match fs::read_dir(self.dir.join(SPOOL)) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(_) => return Err(Unavailable::Io),
        };
        files.sort();
        Ok(files)
    }

    fn spool_usage(&self) -> Result<(usize, u64), Unavailable> {
        let files = self.spool_files()?;
        let bytes = files
            .iter()
            .filter_map(|p| fs::symlink_metadata(p).ok())
            .map(|m| m.len())
            .sum();
        Ok((files.len(), bytes))
    }

    /// `false` when there is no store yet.
    fn check_dir(&self) -> Result<bool, Unavailable> {
        let dir = match fs::symlink_metadata(&self.dir) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
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
        Ok(true)
    }

    /// `None` when there is no store yet.
    fn read(&self) -> Result<Option<State>, Unavailable> {
        if !self.check_dir()? {
            return Ok(None);
        }
        let Some(bytes) = read_checked(&self.dir.join(SNAPSHOT), self.limits.snapshot_bytes)?
        else {
            return Ok(None);
        };
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

/// `ids` and every node derived from one of them, transitively.
fn with_descendants(state: &State, mut ids: BTreeSet<String>) -> BTreeSet<String> {
    loop {
        let more: Vec<String> = state
            .nodes
            .values()
            .filter(|r| !ids.contains(&r.node_id))
            .filter(|r| r.derived_from.iter().any(|p| ids.contains(&p.node_id)))
            .map(|r| r.node_id.clone())
            .collect();
        if more.is_empty() {
            return ids;
        }
        ids.extend(more);
    }
}

/// A node id names its spool file, so it must be a plain name.
fn spool_name(node_id: &str) -> Result<String, Refusal> {
    match !node_id.is_empty()
        && node_id.len() <= 128
        && node_id.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        true => Ok(format!("{node_id}.json")),
        false => Err(Refusal::InvalidId),
    }
}

/// `None` when the file does not exist. Opened once, without following a link or blocking on a
/// FIFO, and every check is made on what was opened.
fn read_checked(path: &Path, max: u64) -> Result<Option<Vec<u8>>, Unavailable> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
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
    let dir = path.parent().and_then(|d| fs::metadata(d).ok());
    if dir.is_some_and(|d| d.uid() != meta.uid()) {
        return Err(Unavailable::ForeignOwner);
    }
    if meta.len() > max {
        return Err(Unavailable::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Unavailable::Io)?;
    if bytes.len() as u64 > max {
        return Err(Unavailable::TooLarge);
    }
    Ok(Some(bytes))
}
