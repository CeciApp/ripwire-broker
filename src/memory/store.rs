//! The memory of one workspace on disk (PRD jev-mem §6): `<state-dir>/memory/<workspace_id>/`,
//! outside the repository, 0700 and 0600. A generation is published whole, through a private
//! temporary, `sync_all`, `rename` and a sync of the directory, so a reader sees the previous
//! generation or the new one and a power loss keeps one of them. What cannot be read is
//! unavailable and is never overwritten: it may be a newer schema or evidence of a fault.
//!
//! Observations arrive in a spool of immutable files, one per node, written without a lock so a
//! hook never waits. One writer at a time incorporates them into a new generation and only then
//! removes them: a crash in between replays the spool, and a replay is the same node.

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
const SPOOL: &str = "spool";
const LOCK: &str = "lock";
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
    pub ledger: Ledger,
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

    /// The snapshot file's length and modification time, which change with every publication;
    /// `None` without one. A stat, never a read: what tells a kept copy is still current.
    pub fn snapshot_version(&self) -> Option<(u64, std::time::SystemTime)> {
        let meta = fs::symlink_metadata(self.dir.join(SNAPSHOT)).ok()?;
        Some((meta.len(), meta.modified().ok()?))
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
            state.jobs.remove(id);
        }
        state
            .edges
            .retain(|_, e| !gone.contains(&e.source) && !gone.contains(&e.target));
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
            state.jobs.remove(id);
            state.tombstones.insert(id.clone(), until_ms);
        }
        state
            .edges
            .retain(|_, e| !gone.contains(&e.source) && !gone.contains(&e.target));
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
        // Pending observations and every temporary go before the snapshot is read: a store that
        // cannot be read keeps its snapshot, never the text the user asked to erase.
        for path in self.spool_entries()? {
            let _ = fs::remove_file(path);
        }
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for e in entries.filter_map(Result::ok) {
                if e.file_name().to_string_lossy().starts_with("snapshot.tmp") {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
        let mut state = self.load()?;
        let removed = state.nodes.len();
        state.jobs.clear();
        state.edges.clear();
        for id in std::mem::take(&mut state.nodes).into_keys() {
            state.tombstones.insert(id, until_ms);
        }
        state.generation += 1;
        self.write_snapshot(&on_disk(&state)?)?;
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
            let name = spool_name(id)?.replace(".json", ".lock");
            let lock = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(leases.join(name))
                .map_err(|_| Unavailable::Io)?;
            // Held: a live process is running it. Free: pending, or its holder died.
            if lock.try_lock().is_err() {
                continue;
            }
            changed = true;
            if job.runs >= MAX_RUNS {
                job.state = JobState::Failed;
                continue;
            }
            job.runs += 1;
            job.state = JobState::Leased;
            taken = Some(Lease {
                node_id: id.clone(),
                run: job.runs,
                _lock: lock,
            });
            break;
        }
        if changed {
            self.write_snapshot(&on_disk(&state)?)?;
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
        state.enriched += u64::from(newly_counted);
        self.write_snapshot(&on_disk(&state)?)?;
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
            self.write_snapshot(&on_disk(&state)?)?;
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
        self.write_snapshot(&bytes)?;
        Ok(true)
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
        self.write_snapshot(&on_disk(&state)?)?;
        Ok(true)
    }

    /// Charges the 24-hour quota before a request is sent; refused, nothing is sent.
    pub fn charge(&self, now_ms: u64, attempts: u32, questions: u32) -> Result<(), Refusal> {
        let _writer = self.writer_waiting()?;
        let mut state = self.load()?;
        let fits = state.ledger.charge(
            now_ms,
            attempts,
            questions,
            self.limits.attempts_per_day,
            self.limits.questions_per_day,
        );
        self.write_snapshot(&on_disk(&state)?)?;
        match fits {
            true => Ok(()),
            false => Err(Refusal::Full(Full::Quota)),
        }
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
