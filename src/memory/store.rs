//! The memory of one workspace on disk (PRD jev-mem §6): `<state-dir>/memory/<workspace_id>/`,
//! outside the repository, 0700 and 0600. A generation is published whole, through a private
//! temporary, `sync_all`, `rename` and a sync of the directory, so a reader sees the previous
//! generation or the new one and a power loss keeps one of them. What cannot be read is
//! unavailable and is never overwritten: it may be a newer schema or evidence of a fault.
//!
//! Observations arrive in a spool of immutable files, one per node, written without a lock so a
//! hook never waits. One writer at a time incorporates them into a new generation and only then
//! removes them: a crash in between replays the spool, and a replay is the same node.

use super::consolidate::{self, Consolidation, Decided};
use super::model::{Edge, EnrichmentState, Record, Rejected, Types};
use super::queue::{Job, JobState, Lease, Ledger, MAX_RUNS, Outcome};
use super::time::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
const SNAPSHOT: &str = "snapshot.json";
/// The generation of the last snapshot written, beside it: what tells a kept copy of the
/// memories is still current without reading the snapshot.
const GENERATION: &str = "generation";
const SPOOL: &str = "spool";
const LOCK: &str = "lock";
/// The 24-hour quota, apart from the snapshot: charging it rewrites a few entries, never the
/// memories, and never waits on their writer.
const QUOTA: &str = "quota.json";
const QUOTA_LOCK: &str = "quota.lock";
/// Written by `forget --all`; only `memory resume` removes it (PD-4).
const REVOKED: &str = "revoked";
/// Lease lock files, one per job; a held lock is a live run.
const LEASES: &str = "leases";
/// Held while a job talks to the provider: one remote job per workspace (PRD jev-mem §8.2).
const REMOTE: &str = "remote.lock";
/// How long the worker's bookkeeping waits for another writer.
const WRITER_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
/// Older than this, a spool temporary is a dead writer's.
const DEAD_TEMPORARY: std::time::Duration = std::time::Duration::from_secs(60);

/// The caps of PRD jev-mem §6; injectable so a test can reach them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_nodes: usize,
    pub spool_entries: usize,
    pub spool_bytes: u64,
    pub snapshot_bytes: u64,
    /// Spool and snapshot together.
    pub total_bytes: u64,
    /// Edges in all, deterministic and inferred.
    pub max_edges: usize,
    /// Classifier attempts and questions in a moving 24-hour window, for every process.
    pub attempts_per_day: u32,
    pub questions_per_day: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_nodes: 2_000,
            spool_entries: 1_000,
            spool_bytes: 16 * 1024 * 1024,
            snapshot_bytes: 64 * 1024 * 1024,
            total_bytes: 96 * 1024 * 1024,
            max_edges: 32_000,
            attempts_per_day: 1_000,
            questions_per_day: 20_000,
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

impl Unavailable {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Symlink => "symlink",
            Self::NotPrivate => "not_private",
            Self::ForeignOwner => "foreign_owner",
            Self::NotRegular => "not_regular",
            Self::TooLarge => "too_large",
            Self::Corrupt => "corrupt",
            Self::UnknownSchema => "unknown_schema",
            Self::Io => "io",
        }
    }
}

/// How much a store holds on disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub pending: usize,
    pub spool_bytes: u64,
    pub snapshot_bytes: u64,
}

/// Which cap a write ran into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Full {
    Nodes,
    SpoolEntries,
    SpoolBytes,
    Snapshot,
    Total,
    /// The 24-hour classifier quota.
    Quota,
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
    /// After the snapshot is renamed into place, before its generation is recorded.
    MidPublish,
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
    /// Edges by [`Edge::key`].
    pub edges: BTreeMap<String, Edge>,
    /// Nodes whose planned enrichment finished: what the consolidation cadence counts.
    pub enriched: u64,
    /// Enrichment jobs, by `node_id`.
    pub jobs: BTreeMap<String, Job>,
    /// Where the quota was before it had a file of its own; read only for such a store.
    pub ledger: Ledger,
    /// The consolidation cadence, cursor and decisions (PRD jev-mem §9).
    pub consolidation: Consolidation,
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

/// What identifies the memories on disk, for a copy kept in memory to be compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    Generation(u64),
    File {
        len: u64,
        modified: std::time::SystemTime,
        inode: u64,
    },
}

/// A store is its directory and its limits: a copy is the same store.
#[derive(Clone)]
pub struct Store {
    dir: PathBuf,
    limits: Limits,
}

impl Store {
    /// `work` on this store in the blocking pool. The store's locks wait with
    /// `std::thread::sleep` for up to [`WRITER_WAIT`] and its writes sync files and directories,
    /// so an async caller (the worker, which shares the server's runtime) runs them here (D-150).
    pub async fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Store) -> T + Send + 'static,
    ) -> T {
        let store = self.clone();
        match tokio::task::spawn_blocking(move || work(&store)).await {
            Ok(done) => done,
            Err(e) => std::panic::resume_unwind(e.into_panic()),
        }
    }

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

    /// What identifies the memories on disk now, without reading the snapshot; `None` without
    /// one. The generation beside it changes only when a memory or a relation does, so writes
    /// that change neither (a quota charge, a lease, a finished job) leave it alone. Without that
    /// record, the snapshot file itself: length, modification time and inode, which a rename to
    /// a new file always changes.
    pub fn snapshot_version(&self) -> Option<Version> {
        let meta = fs::symlink_metadata(self.dir.join(SNAPSHOT)).ok()?;
        let generation = fs::read_to_string(self.dir.join(GENERATION))
            .ok()
            .and_then(|g| g.trim().parse().ok());
        Some(match generation {
            Some(g) => Version::Generation(g),
            None => Version::File {
                len: meta.len(),
                modified: meta.modified().ok()?,
                inode: std::os::unix::fs::MetadataExt::ino(&meta),
            },
        })
    }

    /// Publishes `state` as the new generation, unless the current one cannot be read.
    pub fn publish(&self, state: &State) -> Result<(), Unavailable> {
        self.read()?;
        self.write_snapshot(state.generation, &on_disk(state)?)
    }

    /// Writes the snapshot of `generation`, then the record of it: a reader that sees the new
    /// generation finds the new snapshot.
    fn write_snapshot(&self, generation: u64, bytes: &[u8]) -> Result<(), Unavailable> {
        self.write_snapshot_until(generation, bytes, false)
            .map_err(|_| Unavailable::Io)
    }

    /// [`Store::write_snapshot`], stopping as a crash would between the snapshot and its
    /// generation when `crash_mid` is set (a test of the store's crash safety).
    fn write_snapshot_until(
        &self,
        generation: u64,
        bytes: &[u8],
        crash_mid: bool,
    ) -> Result<(), Refusal> {
        let path = self.dir.join(GENERATION);
        let recorded = fs::read_to_string(&path)
            .ok()
            .and_then(|g| g.trim().parse::<u64>().ok());
        let changes = recorded != Some(generation);
        // The old record goes first: a crash before the new one is written leaves no record,
        // and readers fall back to the file's own identity, never to the old generation, which
        // would keep a warm copy (forgotten memories included) valid.
        if changes && recorded.is_some() {
            fs::remove_file(&path)
                .and_then(|()| fs::File::open(&self.dir)?.sync_all())
                .map_err(|_| Unavailable::Io)?;
        }
        crate::state::write_private(&self.dir, &self.dir.join(SNAPSHOT), bytes)
            .map_err(|_| Unavailable::Io)?;
        if crash_mid {
            return Err(Refusal::Crashed);
        }
        if changes {
            crate::state::write_private(&self.dir, &path, generation.to_string().as_bytes())
                .map_err(|_| Unavailable::Io)?;
        }
        fs::File::open(&self.dir)
            .and_then(|d| d.sync_all())
            .map_err(|_| Unavailable::Io)?;
        Ok(())
    }

    /// The exclusive writer, held until dropped. Never waits: a held store is [`Refusal::Locked`].
    pub fn writer(&self) -> Result<fs::File, Refusal> {
        self.lock(LOCK)
    }

    fn lock(&self, name: &str) -> Result<fs::File, Refusal> {
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
            .open(self.dir.join(name))
            .map_err(|_| Unavailable::Io)?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(_) => Err(Refusal::Locked),
        }
    }

    /// The writer for the worker's bookkeeping (quota, commits, job ends), which must not be
    /// lost to a brief ingestion elsewhere: waits up to [`WRITER_WAIT`], then [`Refusal::Locked`].
    /// A hook never takes it.
    pub fn writer_waiting(&self) -> Result<fs::File, Refusal> {
        let until = std::time::Instant::now() + WRITER_WAIT;
        loop {
            match self.writer() {
                Err(Refusal::Locked) if std::time::Instant::now() < until => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other,
            }
        }
    }

    /// Publishes an admitted observation to the spool. Durable once it returns; no lock, no
    /// snapshot read. A replay of a pending node replaces its file and takes no room.
    pub fn enqueue(&self, record: &Record) -> Result<(), Refusal> {
        self.enqueue_with(record, || {})
    }

    /// [`Store::enqueue`] with `between` run after the revocation check and before the file
    /// appears: where a concurrent `forget --all` can land. For race tests.
    pub fn enqueue_with(&self, record: &Record, between: impl FnOnce()) -> Result<(), Refusal> {
        let name = spool_name(&record.node_id)?;
        self.check_dir()?;
        if self.is_revoked() {
            return Err(Refusal::Revoked);
        }
        let spool = self.dir.join(SPOOL);
        if let Ok(m) = fs::symlink_metadata(&spool) {
            if m.file_type().is_symlink() {
                return Err(Unavailable::Symlink.into());
            }
            if m.mode() & 0o077 != 0 {
                return Err(Unavailable::NotPrivate.into());
            }
        }
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
        between();
        crate::state::write_private(&spool, &target, &bytes)
            .and_then(|()| fs::File::open(&spool)?.sync_all())
            .map_err(|_| Refusal::Unavailable(Unavailable::Io))?;
        // A `forget --all` that landed after the check above has already listed the spool: take
        // back what it could not see, so nothing the user erased comes back.
        if self.is_revoked() {
            let _ = fs::remove_file(&target);
            return Err(Refusal::Revoked);
        }
        Ok(())
    }

    /// Sizes on disk, without reading the snapshot.
    pub fn usage(&self) -> Result<Usage, Unavailable> {
        let (pending, spool_bytes) = self.spool_usage()?;
        let snapshot_bytes = fs::symlink_metadata(self.dir.join(SNAPSHOT)).map_or(0, |m| m.len());
        Ok(Usage {
            pending,
            spool_bytes,
            snapshot_bytes,
        })
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
        // The worker comes by every few seconds: with nothing in the spool (not even a temporary
        // to clean up) there is nothing to lock, load or rewrite.
        if self.is_revoked() {
            return Err(Refusal::Revoked);
        }
        if self.spool_entries()?.is_empty() {
            return Ok(Ingested::default());
        }
        let _writer = self.writer()?;
        if self.is_revoked() {
            return Err(Refusal::Revoked);
        }
        let mut state = self.load()?;
        let mut done = Ingested::default();
        let mut consumed = Vec::new();
        let mut size = on_disk(&state)?.len() as u64;
        self.remove_dead_temporaries()?;
        for path in self.spool_files()? {
            // One entry that cannot be read (a link, a directory) is dropped, never followed,
            // and never stops the others.
            let bytes = match read_checked(&path, self.limits.spool_bytes) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => continue,
                Err(_) => {
                    done.rejected += 1;
                    let _ = fs::remove_file(&path);
                    continue;
                }
            };
            let mut record = match Record::parse(&bytes) {
                Ok(record) => record,
                // A newer schema may be a newer broker's: not ours to destroy.
                Err(Rejected::UnknownSchema) => continue,
                Err(_) => {
                    done.rejected += 1;
                    consumed.push(path);
                    continue;
                }
            };
            // The id names the spool file and later the lease: one that cannot, or that names
            // another file, came from outside the store and is refused (D-150).
            let names_this_file = spool_name(&record.node_id)
                .is_ok_and(|name| path.file_name().is_some_and(|f| f == name.as_str()));
            if !names_this_file {
                done.rejected += 1;
                consumed.push(path);
                continue;
            }
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
            state.jobs.insert(record.node_id.clone(), Job::default());
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
            self.write_snapshot_until(state.generation, &bytes, crash == Some(Step::MidPublish))?;
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
            state.jobs.remove(id);
            self.drop_lease(id);
        }
        self.remove_dead_snapshot_temporaries();
        state.consolidation.drop_nodes(&gone);
        state
            .edges
            .retain(|_, e| !gone.contains(&e.source) && !gone.contains(&e.target));
        if !gone.is_empty() {
            state.generation += 1;
        }
        state.tombstones.retain(|_, until| *until > now_ms);
        state.trusted_ms = now_ms;
        self.remove_orphan_leases(&state);
        self.write_snapshot(state.generation, &on_disk(&state)?)?;
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
            state.jobs.remove(id);
            state.tombstones.insert(id.clone(), until_ms);
            self.drop_lease(id);
        }
        self.remove_dead_snapshot_temporaries();
        state.consolidation.drop_nodes(&gone);
        state
            .edges
            .retain(|_, e| !gone.contains(&e.source) && !gone.contains(&e.target));
        state.generation += 1;
        self.write_snapshot(state.generation, &on_disk(&state)?)?;
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
        // Pending observations and every temporary go before the snapshot is read: a store that
        // cannot be read keeps its snapshot, never the text the user asked to erase.
        for path in self.spool_entries()? {
            let _ = fs::remove_file(path);
        }
        self.remove_dead_snapshot_temporaries();
        let mut state = self.load()?;
        let removed = state.nodes.len();
        for id in state.jobs.keys() {
            self.drop_lease(id);
        }
        state.jobs.clear();
        state.edges.clear();
        state.consolidation = Consolidation::default();
        for id in std::mem::take(&mut state.nodes).into_keys() {
            state.tombstones.insert(id, until_ms);
        }
        state.generation += 1;
        self.write_snapshot(state.generation, &on_disk(&state)?)?;
        Ok(removed)
    }

    /// Removes `node_id`'s lease file (D-150), for a job that cannot be leased again (done, failed,
    /// or gone from the state). Only once its lock is taken here: a worker still running holds it,
    /// and unlinking that file would let a job back under the same id take a second lock beside
    /// it (D-151). A held one stays, and goes later as an orphan.
    fn drop_lease(&self, node_id: &str) {
        if let Ok(name) = spool_name(node_id) {
            remove_free_lease(&self.dir.join(LEASES).join(name.replace(".json", ".lock")));
        }
    }

    /// Lease files of jobs no longer in `state` whose lock nobody holds: what [`Store::drop_lease`]
    /// had to leave because a worker was still running.
    fn remove_orphan_leases(&self, state: &State) {
        let Ok(entries) = fs::read_dir(self.dir.join(LEASES)) else {
            return;
        };
        for path in entries.filter_map(Result::ok).map(|e| e.path()) {
            let id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if !state.jobs.contains_key(id) {
                remove_free_lease(&path);
            }
        }
    }

    /// The temporary snapshots and generations a writer that died left behind: they hold memory
    /// text, and `forget` must not leave it on disk (D-150). Called under the writer lock, which
    /// every writer of those two holds, so none of them belongs to a live writer. A temporary
    /// quota is written under its own lock, so only an old one goes.
    fn remove_dead_snapshot_temporaries(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let name = e.file_name().to_string_lossy().into_owned();
            let ours = name.starts_with("snapshot.tmp") || name.starts_with("generation.tmp");
            let old_quota = name.starts_with("quota.tmp")
                && fs::symlink_metadata(e.path())
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|at| at.elapsed().ok())
                    .is_some_and(|age| age > DEAD_TEMPORARY);
            if ours || old_quota {
                let _ = fs::remove_file(e.path());
            }
        }
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

    /// Takes the next job that may run at `now_ms`: a pending one past its time, or a leased
    /// one whose holder is gone (its lease lock can be taken). A job out of runs fails instead.
    pub fn lease_next(&self, now_ms: u64) -> Result<Option<Lease>, Refusal> {
        let _writer = self.writer_waiting()?;
        let mut state = self.load()?;
        let leases = self.dir.join(LEASES);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&leases)
            .map_err(|_| Unavailable::Io)?;
        let mut taken = None;
        let mut changed = false;
        for (id, job) in state.jobs.iter_mut() {
            let ready = match job.state {
                JobState::Pending => job.not_before_ms <= now_ms,
                JobState::Leased => true,
                JobState::Failed | JobState::Done => false,
            };
            if !ready {
                continue;
            }
            // An id that cannot name a file (a state from before ingest checked it) is skipped:
            // it must not stop every other job.
            let Ok(name) = spool_name(id).map(|n| n.replace(".json", ".lock")) else {
                continue;
            };
            let lock = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(leases.join(&name))
                .map_err(|_| Unavailable::Io)?;
            // Held: a live process is running it. Free: pending, or its holder died.
            if lock.try_lock().is_err() {
                continue;
            }
            changed = true;
            if job.runs >= MAX_RUNS {
                job.state = JobState::Failed;
                // It cannot be leased again: its lease file goes while its lock is held here.
                let _ = fs::remove_file(leases.join(&name));
                continue;
            }
            job.runs += 1;
            job.state = JobState::Leased;
            taken = Some(Lease {
                node_id: id.clone(),
                run: job.runs,
                at_ms: now_ms,
                _lock: lock,
            });
            break;
        }
        if changed {
            self.write_snapshot(state.generation, &on_disk(&state)?)?;
        }
        Ok(taken)
    }

    /// The workspace's single remote slot, held until dropped; `None` while another worker,
    /// in this process or another, holds it.
    pub fn remote_slot(&self) -> Result<Option<fs::File>, Refusal> {
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
            .open(self.dir.join(REMOTE))
            .map_err(|_| Unavailable::Io)?;
        Ok(file.try_lock().is_ok().then_some(file))
    }

    /// Ends a run. A retry with no run left fails the job.
    pub fn finish(&self, lease: Lease, outcome: Outcome) -> Result<(), Refusal> {
        let _writer = self.writer_waiting()?;
        let mut state = self.load()?;
        let Some(job) = state.jobs.get_mut(&lease.node_id) else {
            return Ok(());
        };
        // A lease from before the job left and came back under the same id settles nothing: only
        // the run that holds the job now may (D-151).
        if job.state != JobState::Leased || job.runs != lease.run {
            return Ok(());
        }
        // The cadence of PRD jev-mem §9 counts a node once, when its planned stages finish:
        // `Done` is terminal, so this happens once per job.
        let newly_counted = outcome == Outcome::Done;
        job.state = match outcome {
            Outcome::Done => JobState::Done,
            Outcome::Failed => JobState::Failed,
            Outcome::Retry { not_before_ms } => {
                job.not_before_ms = not_before_ms;
                JobState::Pending
            }
            Outcome::Defer { not_before_ms } => {
                job.runs = job.runs.saturating_sub(1);
                job.not_before_ms = not_before_ms;
                JobState::Pending
            }
        };
        // A job that cannot run again keeps no lease file: removed while its lock is held.
        // Removed directly: the lock is this lease's own.
        if matches!(outcome, Outcome::Done | Outcome::Failed)
            && let Ok(name) = spool_name(&lease.node_id)
        {
            let _ = fs::remove_file(self.dir.join(LEASES).join(name.replace(".json", ".lock")));
        }
        state.enriched += u64::from(newly_counted);
        if newly_counted {
            // Its pairs wait from now; an older wait is kept.
            state
                .consolidation
                .pending_since_ms
                .get_or_insert(lease.at_ms);
        }
        self.write_snapshot(state.generation, &on_disk(&state)?)?;
        drop(lease);
        Ok(())
    }

    /// `memory retry`: every failed job back to pending, with its runs. Returns how many.
    pub fn retry_all_failed(&self) -> Result<usize, Refusal> {
        let _writer = self.writer()?;
        let mut state = self.load()?;
        let mut n = 0;
        for job in state
            .jobs
            .values_mut()
            .filter(|j| j.state == JobState::Failed)
        {
            (job.state, job.runs, job.not_before_ms) = (JobState::Pending, 0, 0);
            n += 1;
        }
        if n > 0 {
            self.write_snapshot(state.generation, &on_disk(&state)?)?;
        }
        Ok(n)
    }

    /// Applies what the classifier decided about `node_id`, under the writer lock and never
    /// while waiting on the network. A node forgotten meanwhile stays forgotten: nothing is
    /// recreated. Edges to nodes that are gone are dropped.
    pub fn commit_enrichment(
        &self,
        node_id: &str,
        seen_generation: u64,
        types: Option<Types>,
        enrichment: EnrichmentState,
        edges: Vec<Edge>,
        neighbours: &[String],
    ) -> Result<bool, Refusal> {
        let _writer = self.writer_waiting()?;
        let mut state = self.load()?;
        let generation = state.generation + 1;
        // Answers about a node that is gone, or was replaced since they were asked, are late.
        let Some(node) = state
            .nodes
            .get_mut(node_id)
            .filter(|n| n.generation == seen_generation)
        else {
            return Ok(false);
        };
        if let Some(types) = types {
            node.types = types;
        }
        node.enrichment.state = enrichment;
        node.enrichment.prompt_version = Some(super::prompts::VERSION.into());
        // What consolidation will pair it with (derived notes are left out there).
        for n in neighbours.iter().filter(|n| *n != node_id) {
            if state.nodes.contains_key(n) {
                state
                    .consolidation
                    .pairs
                    .insert(consolidate::Pair::of(node_id, n));
            }
        }
        let mut added = Vec::new();
        for mut edge in edges {
            if !state.nodes.contains_key(&edge.source) || !state.nodes.contains_key(&edge.target) {
                continue;
            }
            // Past the cap a new edge is left out; one already there is only refreshed.
            let key = edge.key();
            if state.edges.len() >= self.limits.max_edges && !state.edges.contains_key(&key) {
                continue;
            }
            edge.generation = generation;
            if state.edges.insert(key.clone(), edge).is_none() {
                added.push(key);
            }
        }
        state.generation = generation;
        let mut bytes = on_disk(&state)?;
        if bytes.len() as u64 > self.limits.snapshot_bytes {
            // The new edges would overflow the snapshot: keep the rest of the answer without them.
            for key in &added {
                state.edges.remove(key);
            }
            bytes = on_disk(&state)?;
            if bytes.len() as u64 > self.limits.snapshot_bytes {
                return Err(Refusal::Full(Full::Snapshot));
            }
        }
        self.write_snapshot(state.generation, &bytes)?;
        Ok(true)
    }

    /// Ends a consolidation round in one new snapshot: the decisions about pairs whose
    /// memories are still there unchanged, with their links and derived notes, the counter at `seen_enriched`,
    /// the cursor when the pairs were asked, and the wait restarted while pairs are still
    /// pending. Returns how many pairs were decided.
    pub fn commit_round(
        &self,
        seen_enriched: u64,
        cursor: Option<String>,
        decided: Vec<Decided>,
        model: &str,
        now_ms: u64,
    ) -> Result<usize, Refusal> {
        let _writer = self.writer_waiting()?;
        let mut state = self.load()?;
        let generation = state.generation + 1;
        let intact = |state: &State, id: &str, hash: &str| {
            state.nodes.get(id).is_some_and(|r| r.content_hash == hash)
        };
        let mut count = 0;
        let mut added = Vec::new();
        let mut added_notes: Vec<String> = Vec::new();
        let before = state.consolidation.decisions.clone();
        for d in decided {
            let p = &d.decision;
            if !intact(&state, &p.pair.first, &p.hashes.0)
                || !intact(&state, &p.pair.second, &p.hashes.1)
            {
                continue;
            }
            if let Some(mut link) = d.link {
                let key = link.key();
                if state.edges.len() < self.limits.max_edges || state.edges.contains_key(&key) {
                    link.generation = generation;
                    if state.edges.insert(key.clone(), link).is_none() {
                        added.push(key);
                    }
                }
            }
            let mut decision = d.decision;
            if let Some(mut note) = d.note {
                let id = note.node_id.clone();
                let blocked = state.tombstones.contains_key(&id);
                if !blocked
                    && !state.nodes.contains_key(&id)
                    && state.nodes.len() < self.limits.max_nodes
                {
                    note.ingest_seq = state
                        .sequence
                        .advance()
                        .ok_or(Full::Nodes)
                        .map_err(Refusal::Full)?;
                    note.generation = generation;
                    state.nodes.insert(id.clone(), note);
                    added_notes.push(id.clone());
                }
                // Cached only when the note is there to be reused.
                if let Some(key) = d.cache.filter(|_| state.nodes.contains_key(&id)) {
                    state.consolidation.notes.insert(key, id);
                }
            }
            // A note that could not be added, or was forgotten meanwhile, is not pointed at.
            if decision
                .note
                .as_ref()
                .is_some_and(|n| !state.nodes.contains_key(n))
            {
                decision.note = None;
            }
            state.consolidation.decisions.insert(d.key, decision);
            count += 1;
        }
        let still = !consolidate::pending(&state, model).is_empty();
        let c = &mut state.consolidation;
        c.counted = c.counted.max(seen_enriched);
        if cursor.is_some() {
            c.cursor = cursor;
        }
        c.pending_since_ms = still.then_some(now_ms);
        if !added.is_empty() || !added_notes.is_empty() {
            state.generation = generation;
        }
        let mut bytes = on_disk(&state)?;
        if bytes.len() as u64 > self.limits.snapshot_bytes {
            // The links and notes would overflow the snapshot: keep the decisions without them.
            for key in &added {
                state.edges.remove(key);
            }
            let gone: BTreeSet<String> = added_notes.into_iter().collect();
            for id in &gone {
                state.nodes.remove(id);
            }
            state.consolidation.drop_nodes(&gone);
            bytes = on_disk(&state)?;
        }
        if bytes.len() as u64 > self.limits.snapshot_bytes {
            // Not even the decisions fit: the cadence alone is written, so the round is not
            // bought again at the next tick.
            state.consolidation.decisions = before;
            count = 0;
            bytes = on_disk(&state)?;
            if bytes.len() as u64 > self.limits.snapshot_bytes {
                return Err(Refusal::Full(Full::Snapshot));
            }
        }
        self.write_snapshot(state.generation, &bytes)?;
        Ok(count)
    }

    /// The explicit action that gives a failed job its runs back. `false` when it is not failed.
    pub fn retry_failed(&self, node_id: &str) -> Result<bool, Refusal> {
        let _writer = self.writer()?;
        let mut state = self.load()?;
        let Some(job) = state
            .jobs
            .get_mut(node_id)
            .filter(|j| j.state == JobState::Failed)
        else {
            return Ok(false);
        };
        (job.state, job.runs, job.not_before_ms) = (JobState::Pending, 0, 0);
        self.write_snapshot(state.generation, &on_disk(&state)?)?;
        Ok(true)
    }

    /// The 24-hour quota spent so far, by every process. A store from before the quota had its
    /// own file still has it in the snapshot.
    pub fn ledger(&self) -> Result<Ledger, Unavailable> {
        match self.quota_file()? {
            Some(ledger) => Ok(ledger),
            None => Ok(self.load()?.ledger),
        }
    }

    /// The quota's own file, without the snapshot; `None` before anything was charged to it.
    pub fn quota_file(&self) -> Result<Option<Ledger>, Unavailable> {
        if !self.check_dir()? {
            return Ok(None);
        }
        read_checked(&self.dir.join(QUOTA), self.limits.snapshot_bytes)?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| Unavailable::Corrupt))
            .transpose()
    }

    /// `change` applied to the quota under its own lock, waiting up to [`WRITER_WAIT`] for it.
    fn quota<T>(&self, change: impl FnOnce(&mut Ledger) -> T) -> Result<T, Refusal> {
        let until = std::time::Instant::now() + WRITER_WAIT;
        let _lock = loop {
            match self.lock(QUOTA_LOCK) {
                Err(Refusal::Locked) if std::time::Instant::now() < until => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => break other?,
            }
        };
        let mut ledger = self.ledger()?;
        let out = change(&mut ledger);
        let bytes = serde_json::to_vec(&ledger).map_err(|_| Unavailable::Io)?;
        crate::state::write_private(&self.dir, &self.dir.join(QUOTA), &bytes)
            .and_then(|()| fs::File::open(&self.dir)?.sync_all())
            .map_err(|_| Unavailable::Io)?;
        Ok(out)
    }

    /// Charges the 24-hour quota before a request is sent; refused, nothing is sent.
    pub fn charge(&self, now_ms: u64, attempts: u32, questions: u32) -> Result<(), Refusal> {
        let (max_attempts, max_questions) =
            (self.limits.attempts_per_day, self.limits.questions_per_day);
        match self.quota(|l| l.charge(now_ms, attempts, questions, max_attempts, max_questions))? {
            true => Ok(()),
            false => Err(Refusal::Full(Full::Quota)),
        }
    }

    /// Records requests a read already sent (PRD jev-mem §8.2): nothing is refused.
    pub fn spend(&self, now_ms: u64, attempts: u32, questions: u32) -> Result<(), Refusal> {
        self.quota(|l| l.record(now_ms, attempts, questions))
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// A temporary older than [`DEAD_TEMPORARY`] belongs to a writer that died between create and
    /// rename; a younger one may still be renamed.
    fn remove_dead_temporaries(&self) -> Result<(), Unavailable> {
        for path in self.spool_entries()? {
            let is_temporary = path
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.starts_with("tmp"));
            let dead = fs::symlink_metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > DEAD_TEMPORARY);
            if is_temporary && dead {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    /// Everything in the spool, observations and temporaries alike, in name order.
    fn spool_entries(&self) -> Result<Vec<PathBuf>, Unavailable> {
        let mut files: Vec<PathBuf> = match fs::read_dir(self.dir.join(SPOOL)) {
            Ok(entries) => entries.filter_map(Result::ok).map(|e| e.path()).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(_) => return Err(Unavailable::Io),
        };
        files.sort();
        Ok(files)
    }

    /// The observations waiting: `*.json` entries.
    fn spool_files(&self) -> Result<Vec<PathBuf>, Unavailable> {
        let mut files = self.spool_entries()?;
        files.retain(|p| p.extension().is_some_and(|x| x == "json"));
        Ok(files)
    }

    /// Observations waiting, and the bytes of everything in the spool: a temporary takes room.
    fn spool_usage(&self) -> Result<(usize, u64), Unavailable> {
        let pending = self.spool_files()?.len();
        let bytes = self
            .spool_entries()?
            .iter()
            .filter_map(|p| fs::symlink_metadata(p).ok())
            .map(|m| m.len())
            .sum();
        Ok((pending, bytes))
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

/// Removes the lease file at `path` if its lock can be taken, while holding it.
fn remove_free_lease(path: &Path) {
    let Ok(file) = fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    else {
        return;
    };
    if file.try_lock().is_ok() {
        let _ = fs::remove_file(path);
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

/// Where admitted observations go: the store's spool, or a stand-in a test makes slow.
pub trait Spool: Send + Sync {
    fn enqueue(&self, record: &Record) -> Result<(), Refusal>;
}

impl Spool for Store {
    fn enqueue(&self, record: &Record) -> Result<(), Refusal> {
        Store::enqueue(self, record)
    }
}
