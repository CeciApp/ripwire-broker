//! The broker core: routes a request to ripwire verbs, normalizes, and shapes the envelope.

use crate::budget;
use crate::memory::admission::Event as MemoryEvent;
use crate::memory::publish::{MemoryConfig, MemoryStatus, Publisher};
use crate::memory::recall::{Recall, Recalled};
use crate::memory::retrieve;
use crate::metrics::{Metrics, RequestRecord, StageSpan, UpstreamSpan};
use crate::model::*;
use crate::normalize::{self, Entry};
use crate::notes::{self as note_engine, NoteEngine, SummarizerStatus};
use crate::online::{self, OnlineConfig, OnlineEngine, merge as online_merge};
use crate::router;
use crate::session::{self, SessionMemory};
use crate::summarizer::Summarizer;
use crate::upstream::{Upstream, UpstreamError};
use crate::workspace::Workspace;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The tool call being served: its id and the upstream calls made for it. Task-local, so
/// concurrent tool calls never mix their upstream calls (PRD 14.2).
struct RequestCtx {
    id: u64,
    /// Shared with the call's `Unfinished` guard, which reports them if the call is dropped.
    spans: Arc<Mutex<Vec<UpstreamSpan>>>,
    /// `--online` stages of this call (PRD §23.11).
    stages: Arc<Mutex<Vec<StageSpan>>>,
}

/// Records an `--online` stage under the current tool call.
fn stage(name: &'static str, took: Duration, batches: usize) {
    let _ = REQUEST.try_with(|c| {
        c.stages.lock().unwrap().push(StageSpan {
            stage: name,
            us: took.as_micros() as u64,
            batches,
        })
    });
}

tokio::task_local! {
    static REQUEST: RequestCtx;
}

/// Records a tool call that was dropped before it finished (client cancellation, RF-14).
struct Unfinished<'a> {
    broker: &'a Broker,
    tool: &'static str,
    id: u64,
    started: Instant,
    spans: Arc<Mutex<Vec<UpstreamSpan>>>,
    stages: Arc<Mutex<Vec<StageSpan>>>,
    finished: bool,
}

impl Drop for Unfinished<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let took = self.started.elapsed();
        let mut metrics = self.broker.metrics.lock().unwrap();
        metrics.cancelled(self.tool);
        metrics.request(RequestRecord {
            request_id: self.id,
            tool: self.tool,
            outcome: "cancelled",
            total_us: took.as_micros() as u64,
            upstream: std::mem::take(&mut *self.spans.lock().unwrap()),
            stages: std::mem::take(&mut *self.stages.lock().unwrap()),
        });
    }
}

/// Risks that decide a gate's status; never suppressed, so the status keeps its evidence (CA-05).
const GATE_RISKS: &[&str] = &["cochange_missing", "contract_change"];

/// Longest the status resource waits for ripwire to answer its availability probe.
pub const STATUS_PROBE: Duration = Duration::from_secs(1);
/// How long the availability probe answers later reads of the status resource (D-098). Tied to
/// `STATUS_PROBE` on purpose: the answer can never be staler than the time one probe is already
/// allowed to take, so the cache adds no uncertainty the probe did not already carry.
pub const STATUS_CACHE: Duration = STATUS_PROBE;

/// Smallest budget that still fits the envelope skeleton plus a few limitations.
pub const MIN_BUDGET_TOKENS: u32 = 256;

/// Largest budget the tools accept. The number is not new: the MCP input schema has declared
/// `"maximum": 100000` since the schemas were written, but nothing enforced it, so a client
/// calling outside the schema — or the CLI and the hooks, which never see it — could ask for
/// any `u32`. Shaping an envelope costs bytes proportional to the entries times the envelope,
/// and the entries follow the budget, so an unenforced ceiling made that cost caller-controlled
/// (D-099). `src/mcp.rs` now publishes this constant instead of repeating the literal.
pub const MAX_BUDGET_TOKENS: u32 = 100_000;

/// Oldest ripwire whose verbs and payload formats the fixtures were recorded from (PRD 15.3).
pub const MIN_RIPWIRE_VERSION: (u64, u64, u64) = (0, 6, 4);

/// `"0.6.4"` → `(0, 6, 4)`; `None` when the version could not be read.
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut parts = v.trim().trim_start_matches('v').split('.');
    let mut next = || {
        parts
            .next()?
            .split(|c: char| !c.is_ascii_digit())
            .next()?
            .parse()
            .ok()
    };
    Some((next()?, next()?, next()?))
}

/// Refuses a ripwire older than `MIN_RIPWIRE_VERSION`; an unreadable version passes.
pub fn check_version(version: &str) -> Result<(), BrokerError> {
    match parse_version(version) {
        Some(v) if v < MIN_RIPWIRE_VERSION => {
            let (a, b, c) = MIN_RIPWIRE_VERSION;
            Err(BrokerError {
                error: "incompatible_upstream",
                message: format!("ripwire {version} is older than the minimum {a}.{b}.{c}"),
            })
        }
        _ => Ok(()),
    }
}

/// `context_for_task` floor in a process started with `--online`: the envelope skeleton,
/// which is never cut, also carries `provenance.online` and the online limitations (D-072).
pub const MIN_ONLINE_BUDGET_TOKENS: u32 = 512;

fn check_budget(budget: u32) -> Result<(), BrokerError> {
    check_budget_at(budget, MIN_BUDGET_TOKENS)
}

fn check_budget_at(budget: u32, min: u32) -> Result<(), BrokerError> {
    if budget < min {
        return Err(BrokerError {
            error: "invalid_input",
            message: format!("budget_tokens must be at least {min}"),
        });
    }
    if budget > MAX_BUDGET_TOKENS {
        return Err(BrokerError {
            error: "invalid_input",
            message: format!("budget_tokens must be at most {MAX_BUDGET_TOKENS}"),
        });
    }
    Ok(())
}

/// The read-only verbs the broker calls; also its allowlist (PRD 15.3). The upstream edit
/// verbs and `quality_baseline` are deliberately absent.
pub const REQUIRED_VERBS: &[&str] = &[
    "explore",
    "from_trace",
    "find_symbol",
    "fetch_body",
    "impact",
    "memory_recall",
    "situational_awareness",
    "edit_check",
    "affected",
    "quality_delta",
];

#[derive(Debug, Clone)]
pub struct BrokerConfig {
    pub workspace: PathBuf,
    pub ripwire_version: String,
    /// Upper bound on `edit_check` calls per `context_after_edit`.
    pub max_edit_checks: usize,
    /// Hide the workspace path in the status resource.
    pub redact_workspace: bool,
    /// Ceiling for a single item's content, so one body cannot take the whole budget.
    pub max_item_tokens: u32,
    /// Send unchanged items only once per session (PRD 11.1, D-029).
    pub incremental: bool,
    /// Local model for architectural notes (PRD 10.3); `None` keeps Phase 3 off.
    pub summarizer: Option<Arc<dyn Summarizer>>,
    /// Longest a response waits for a note before moving on (D-036).
    pub summarizer_wait: Duration,
    /// The remote classifier (PRD §23); `None` keeps the broker offline (RF-ONLINE-01).
    pub online: Option<OnlineConfig>,
    /// Persistent memory (PRD jev-mem); `None` keeps no history.
    pub memory: Option<MemoryConfig>,
}

impl BrokerConfig {
    pub fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            ripwire_version: "unknown".into(),
            max_edit_checks: 5,
            redact_workspace: false,
            max_item_tokens: 800,
            incremental: false,
            summarizer: None,
            summarizer_wait: Duration::from_millis(1500),
            online: None,
            memory: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BrokerError {
    pub error: &'static str,
    pub message: String,
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error, self.message)
    }
}

impl From<UpstreamError> for BrokerError {
    fn from(e: UpstreamError) -> Self {
        let error = match e {
            UpstreamError::Refused(_) => "upstream_refused",
            UpstreamError::Timeout => "upstream_timeout",
            UpstreamError::Unavailable(_) => "upstream_unavailable",
        };
        Self {
            error,
            message: e.to_string(),
        }
    }
}

/// `context_for_task` mode: `auto` lets the router decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Auto,
    Orient,
    Debug,
    Change,
    Review,
}

#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub task: String,
    pub budget_tokens: u32,
    pub mode: Mode,
    pub include_docs: bool,
    pub include_bodies: bool,
    /// Ignore the session memory and send everything again (D-029).
    pub include_seen: bool,
}

impl TaskRequest {
    pub fn new(task: &str) -> Self {
        Self {
            task: task.into(),
            budget_tokens: 2500,
            mode: Mode::Auto,
            include_docs: true,
            include_bodies: true,
            include_seen: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditRequest {
    /// Changed files known to the host; empty means the working tree.
    pub files: Vec<String>,
    /// Symbols deliberately modified.
    pub symbols: Vec<String>,
    pub budget_tokens: u32,
    /// Ignore the session memory and send everything again (D-029).
    pub include_seen: bool,
}

impl Default for EditRequest {
    fn default() -> Self {
        Self {
            files: vec![],
            symbols: vec![],
            budget_tokens: 1500,
            include_seen: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FinishRequest {
    pub budget_tokens: u32,
    pub include_test_commands: bool,
    /// Also treat minor findings as blocking.
    pub strict: bool,
}

impl Default for FinishRequest {
    fn default() -> Self {
        Self {
            budget_tokens: 1800,
            include_test_commands: true,
            strict: false,
        }
    }
}

pub struct Broker {
    upstream: Arc<dyn Upstream>,
    workspace: Workspace,
    ripwire_version: String,
    max_edit_checks: usize,
    redact_workspace: bool,
    max_item_tokens: u32,
    incremental: bool,
    session: Mutex<SessionMemory>,
    notes: Option<NoteEngine>,
    online: Option<OnlineEngine>,
    memory: Option<Publisher>,
    /// The memory read of `context_for_task`, with `--memory`.
    recall: Option<Recall>,
    metrics: Mutex<Metrics>,
    last_error: Mutex<Option<&'static str>>,
    /// The last availability probe and when it finished, so back-to-back reads of the status
    /// resource do not each pay an upstream round trip. `busy` costs a whole `STATUS_PROBE` to
    /// learn, which is exactly the answer worth not asking twice.
    probe: Mutex<Option<(tokio::time::Instant, bool, bool)>>,
    next_request: AtomicU64,
}

/// The `ripwire-broker://status` payload (PRD 9.4): operational data only.
#[derive(Debug, Serialize)]
pub struct BrokerStatus {
    pub broker_version: &'static str,
    pub schema_version: &'static str,
    pub mcp_protocol: String,
    pub workspace: String,
    pub offline: bool,
    pub telemetry: &'static str,
    pub upstream: UpstreamStatus,
    pub budget_defaults: Value,
    pub summarizer: SummarizerStatus,
    /// Only for a process started with `--online` (RF-ONLINE-15).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<OnlineStatus>,
    /// Only for a process started with `--memory`: counts, never content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryStatus>,
    pub metrics: Metrics,
}

/// The classifier's health for the status resource: no query, path, code or credential.
#[derive(Debug, Serialize)]
pub struct OnlineStatus {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub endpoint_host: String,
    pub max_in_flight: usize,
    pub request_limit: usize,
    pub requests: u64,
    pub cache_hits: u64,
    pub cached_decisions: usize,
    /// Error category only.
    pub last_error: Option<&'static str>,
    /// Lookahead files admitted: candidates beyond ripwire (`semantic_only_candidates_total`).
    pub semantic_only_candidates: u64,
    /// The §23.11 metrics: counts and times only.
    pub metrics: crate::online::metrics::OnlineMetrics,
}

#[derive(Debug, Serialize)]
pub struct UpstreamStatus {
    pub ripwire_version: String,
    pub available: bool,
    pub restarts: u32,
    /// Did not answer the availability probe within `STATUS_PROBE`: alive but occupied,
    /// e.g. with a long or abandoned call.
    pub busy: bool,
    /// Error kind only; messages can quote symbols or paths.
    pub last_error: Option<&'static str>,
}

impl Broker {
    pub async fn connect(
        upstream: Arc<dyn Upstream>,
        mut config: BrokerConfig,
    ) -> Result<Self, BrokerError> {
        // One `--jev-request-limit` per query for discovery and the memory read together: memory
        // takes at most four of it, discovery keeps the rest (PRD jev-mem §8.2).
        if let (Some(online), Some(read)) = (
            config.online.as_mut(),
            config.memory.as_mut().and_then(|m| m.read.as_mut()),
        ) {
            let (memory, discovery) = retrieve::slots(read.cfg.request_limit, online.request_limit);
            read.cfg.request_limit = memory;
            online.request_limit = discovery;
        }
        check_version(&config.ripwire_version)?;
        let tools = upstream.list_tools().await?;
        let missing: Vec<&str> = REQUIRED_VERBS
            .iter()
            .copied()
            .filter(|v| !tools.iter().any(|t| t == v))
            .collect();
        if !missing.is_empty() {
            return Err(BrokerError {
                error: "incompatible_upstream",
                message: format!(
                    "the installed ripwire lacks required verbs: {}",
                    missing.join(", ")
                ),
            });
        }
        let workspace = Workspace::new(&config.workspace).map_err(|message| BrokerError {
            error: "workspace_violation",
            message,
        })?;
        let online = config
            .online
            .map(|o| OnlineEngine::new(o, workspace.root()))
            .transpose()
            .map_err(|message| BrokerError {
                error: "workspace_violation",
                message,
            })?;
        let (memory, recall) = match config.memory {
            Some(mut m) => {
                let read = m.read.take();
                let recall = read
                    .map(|r| Recall::new(r, workspace.root()))
                    .transpose()
                    .map_err(|message| BrokerError {
                        error: "workspace_violation",
                        message,
                    })?;
                (Some(Publisher::new(m, workspace.root())), recall)
            }
            None => (None, None),
        };
        Ok(Self {
            upstream,
            workspace,
            ripwire_version: config.ripwire_version,
            max_edit_checks: config.max_edit_checks,
            redact_workspace: config.redact_workspace,
            max_item_tokens: config.max_item_tokens,
            incremental: config.incremental,
            session: Mutex::new(SessionMemory::default()),
            notes: config
                .summarizer
                .map(|m| NoteEngine::new(m, config.summarizer_wait)),
            online,
            memory,
            recall,
            metrics: Mutex::new(Metrics::default()),
            last_error: Mutex::new(None),
            probe: Mutex::new(None),
            next_request: AtomicU64::new(1),
        })
    }

    /// Whether the upstream answers, reusing the last probe while it is inside `STATUS_CACHE`.
    /// The lock is never held across the probe: one reader waiting on another reader's round
    /// trip would be the very stall the status resource must not have (RF-13, D-052).
    async fn availability(&self) -> (bool, bool) {
        let cached = *self.probe.lock().unwrap();
        if let Some((taken, available, busy)) = cached
            && tokio::time::Instant::now().saturating_duration_since(taken) < STATUS_CACHE
        {
            return (available, busy);
        }
        // Never let a busy ripwire hang the status resource (RF-13).
        let probe = tokio::time::timeout(STATUS_PROBE, self.upstream.list_tools()).await;
        let busy = probe.is_err();
        let available = matches!(probe, Ok(Ok(_)));
        // Timed from when the answer was known, not from when it was asked: a probe that took
        // the whole `STATUS_PROBE` would otherwise land already expired, and the expensive
        // case is the one that most needs not to be repeated.
        *self.probe.lock().unwrap() = Some((tokio::time::Instant::now(), available, busy));
        (available, busy)
    }

    pub async fn status(&self) -> BrokerStatus {
        let (available, busy) = self.availability().await;
        BrokerStatus {
            broker_version: env!("CARGO_PKG_VERSION"),
            schema_version: SCHEMA_VERSION,
            mcp_protocol: rust_mcp_sdk::schema::ProtocolVersion::latest().to_string(),
            workspace: if self.redact_workspace {
                "<redacted>".into()
            } else {
                self.workspace.root().display().to_string()
            },
            offline: self.online.is_none(),
            telemetry: "none",
            upstream: UpstreamStatus {
                ripwire_version: self.ripwire_version.clone(),
                available,
                busy,
                restarts: self.upstream.restarts(),
                last_error: *self.last_error.lock().unwrap(),
            },
            budget_defaults: json!({
                "context_for_task": TaskRequest::new("").budget_tokens,
                "context_after_edit": EditRequest::default().budget_tokens,
                "context_before_finish": FinishRequest::default().budget_tokens,
            }),
            summarizer: match &self.notes {
                Some(engine) => engine.status(),
                None => SummarizerStatus {
                    enabled: false,
                    program: None,
                    generated: 0,
                    cache_hits: 0,
                    pending: 0,
                    failures: 0,
                    cached_notes: 0,
                },
            },
            online: self.online.as_ref().map(|engine| {
                let (config, totals) = (engine.config(), engine.totals());
                OnlineStatus {
                    enabled: true,
                    provider: config.provider.clone(),
                    model: engine.model().into(),
                    endpoint_host: config.endpoint_host.clone(),
                    max_in_flight: config.max_in_flight,
                    request_limit: config.request_limit,
                    requests: totals.requests,
                    cache_hits: totals.cache_hits,
                    cached_decisions: engine.cached_decisions(),
                    last_error: totals.last_error,
                    semantic_only_candidates: totals.semantic_only,
                    metrics: engine.metrics(),
                }
            }),
            memory: self.memory.as_ref().map(Publisher::status),
            metrics: {
                let mut m = self.metrics.lock().unwrap().clone();
                m.session.remembered = self.session.lock().unwrap().len();
                m
            },
        }
    }

    /// `path` (absolute, or relative to the root) as workspace-relative, or `None` if it
    /// resolves outside (CA-08).
    /// Every entry of the semantic cache as stored (CA-ONLINE-13): hex digest keys and
    /// validated probabilities only. Empty without `--online`.
    pub fn inspect_semantic_cache(&self) -> Vec<String> {
        self.online
            .as_ref()
            .map_or_else(Vec::new, OnlineEngine::inspect_cache)
    }

    /// The smallest `budget_tokens` `context_for_task` accepts in this process.
    pub fn min_task_budget(&self) -> u32 {
        match self.online {
            Some(_) => MIN_ONLINE_BUDGET_TOKENS,
            None => MIN_BUDGET_TOKENS,
        }
    }

    pub fn in_workspace(&self, path: &str) -> Option<String> {
        self.workspace.relative(path).ok()
    }

    /// Waits for a note still being written in the background, if any: for tests, which observe
    /// a note settle. The server never waits for one.
    pub async fn wait_background(&self) {
        if let Some(engine) = &self.notes {
            engine.wait_background().await;
        }
    }

    /// The id the next tool call will get.
    pub fn next_request_id(&self) -> u64 {
        self.next_request.load(Ordering::Relaxed)
    }

    /// Continues request ids from an earlier process of the same session (hooks), so
    /// `provenance.request_id` stays unique within it. Never moves backwards.
    pub fn resume_request_ids(&self, next: u64) {
        self.next_request.fetch_max(next, Ordering::Relaxed);
    }

    /// How many items, tests, risks and notes this process left out or reduced to a reference
    /// because the session already had them. Hooks add it to the saved session, which outlives
    /// the process (§21.3).
    pub fn session_hits(&self) -> u64 {
        self.metrics.lock().unwrap().session_hits
    }

    /// What this session was already shown, to persist between processes (hooks).
    pub fn session_snapshot(&self) -> SessionMemory {
        self.session.lock().unwrap().clone()
    }

    /// Continues a session saved by `session_snapshot`. Trimmed at the door, because the file
    /// may have been written by a longer session or before the ceiling existed (D-097).
    pub fn restore_session(&self, mut memory: SessionMemory) {
        memory.trim();
        *self.session.lock().unwrap() = memory;
    }

    /// Runs one tool call under a fresh request id and records it with its upstream calls.
    async fn traced(
        &self,
        tool: &'static str,
        inner: impl Future<Output = Result<Envelope, BrokerError>>,
    ) -> Result<Envelope, BrokerError> {
        let id = self.next_request.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        let spans = Arc::new(Mutex::new(Vec::new()));
        let stages = Arc::new(Mutex::new(Vec::new()));
        let mut guard = Unfinished {
            broker: self,
            tool,
            id,
            started,
            spans: spans.clone(),
            stages: stages.clone(),
            finished: false,
        };
        let ctx = RequestCtx {
            id,
            spans: spans.clone(),
            stages: stages.clone(),
        };
        let result = REQUEST.scope(ctx, inner).await;
        guard.finished = true;
        let spans = std::mem::take(&mut *spans.lock().unwrap());
        let took = started.elapsed();
        let mut metrics = self.metrics.lock().unwrap();
        metrics.tool(tool, took, &result);
        metrics.request(RequestRecord {
            request_id: id,
            tool,
            outcome: match &result {
                Ok(env) => env.status.as_str(),
                Err(e) => e.error,
            },
            total_us: took.as_micros() as u64,
            upstream: spans,
            stages: std::mem::take(&mut *stages.lock().unwrap()),
        });
        result
    }

    /// With `--memory`, publishes what this answer observed, after it is built and without
    /// changing it (PRD jev-mem §8.1).
    async fn observe(&self, event: MemoryEvent, env: &Envelope, scope: Vec<String>) {
        if let Some(memory) = &self.memory {
            let id = REQUEST.try_with(|r| r.id).unwrap_or(0);
            memory
                .observe(event, env, scope, format!("mcp/{}/{id}", env.tool))
                .await;
        }
    }

    pub async fn context_after_edit(&self, req: EditRequest) -> Result<Envelope, BrokerError> {
        self.traced("context_after_edit", self.context_after_edit_inner(req))
            .await
    }

    async fn context_after_edit_inner(&self, req: EditRequest) -> Result<Envelope, BrokerError> {
        check_budget(req.budget_tokens)?;
        // Refuse before any upstream call (CA-08).
        let violation = |message| BrokerError {
            error: "workspace_violation",
            message,
        };
        let files = req
            .files
            .iter()
            .map(|f| self.workspace.relative(f))
            .collect::<Result<Vec<_>, _>>()
            .map_err(violation)?;
        for symbol in &req.symbols {
            self.workspace.check_symbol(symbol).map_err(violation)?;
        }
        let mut verbs = vec!["situational_awareness"];
        let args = if files.is_empty() {
            json!({})
        } else {
            json!({"files": files.join(",")})
        };
        let situation = self.call("situational_awareness", args).await?;
        let mut entries = normalize::situation(&situation);
        // What the observation is about: the files named, else those ripwire saw change.
        let scope = match files.is_empty() {
            true => normalize::changed_files(&situation),
            false => files.clone(),
        };
        // The checks are independent of each other, so they go out together: at most
        // `max_edit_checks` of them, the same bound the sequential loop had. RF-14 (D-049) is
        // that a cancellation is respected -- the future is dropped, the tool records
        // `cancelled`, and nothing further is asked upstream -- not that only one call may be in
        // flight, so this keeps it. What it does change is the waste when a cancellation lands
        // mid-flight: up to `max_edit_checks` answers nobody reads instead of one. They are
        // ripwire calls on the same machine, so the waste is local CPU, never a remote request
        // (D-104). The results are collected in the order asked, so `impact_needed` and the
        // entries do not depend on which check answered first.
        let symbols: Vec<&String> = req.symbols.iter().take(self.max_edit_checks).collect();
        if !symbols.is_empty() {
            push_once(&mut verbs, "edit_check");
        }
        // The rest is named, never dropped in silence (D-148).
        if let Some(rest) = req.symbols.get(symbols.len()..).filter(|r| !r.is_empty()) {
            entries.push(normalize::limitation(
                "edit_check",
                "symbols_truncated",
                format!(
                    "checked the first {} of {} symbols; call again with the rest: {}",
                    symbols.len(),
                    req.symbols.len(),
                    rest.join(", ")
                ),
            ));
        }
        // A symbol ripwire refuses (the agent renamed or deleted it) is missing evidence, not a
        // failed call: the situation already fetched still answers. Only an unavailable
        // upstream fails the tool.
        let checks = futures_util::future::join_all(symbols.iter().map(|symbol| {
            self.evidence_or(
                "edit_check",
                json!({"symbol": symbol}),
                normalize::unchecked,
            )
        }))
        .await;
        let mut impact_needed = Vec::new();
        for (symbol, answer) in symbols.iter().zip(checks) {
            match answer? {
                Ok(payload) => {
                    let (found, changed) = normalize::edit_check(&payload);
                    entries.extend(found);
                    if changed {
                        impact_needed.push(*symbol);
                    }
                }
                Err(missing) => entries.push(missing),
            }
        }
        // `impact` only to clarify a high-risk change: a contract that actually changed. Also
        // independent of each other, and bounded by the same `max_edit_checks`, since a symbol
        // only reaches here after its own check came back changed. Fanning out the checks alone
        // left this loop as the cost: with 5 symbols the call was 11 round trips and became 7,
        // where doing both makes it 3 (D-104).
        if !impact_needed.is_empty() {
            push_once(&mut verbs, "impact");
        }
        let impacts = futures_util::future::join_all(impact_needed.iter().map(|symbol| {
            self.evidence_or("impact", json!({"symbol": symbol}), normalize::unchecked)
        }))
        .await;
        for answer in impacts {
            match answer? {
                Ok(payload) => entries.extend(normalize::impact(&payload)),
                Err(missing) => entries.push(missing),
            }
        }
        let attention = entries
            .iter()
            .any(|e| matches!(e, Entry::Risk(_, r) if GATE_RISKS.contains(&r.kind)));
        let status = if attention {
            Status::AttentionRequired
        } else {
            Status::Ready
        };
        let env = self.envelope(
            Shape {
                tool: "context_after_edit",
                intent: None,
                status,
                verbs,
                budget: req.budget_tokens,
                suppress_seen: !req.include_seen,
                online: None,
            },
            entries,
        );
        self.remember(&env);
        self.observe(MemoryEvent::AfterEdit, &env, scope).await;
        Ok(env)
    }

    async fn call(&self, verb: &'static str, args: Value) -> Result<String, BrokerError> {
        Ok(self.guarded_call(verb, args).await?)
    }

    /// The single path to ripwire: only allowlisted, read-only verbs get through.
    async fn guarded_call(&self, verb: &'static str, args: Value) -> Result<String, UpstreamError> {
        if !REQUIRED_VERBS.contains(&verb) {
            return Err(UpstreamError::Refused(format!(
                "{verb} is not an allowlisted read-only verb"
            )));
        }
        let started = Instant::now();
        let result = self.upstream.call(verb, args).await;
        let took = started.elapsed();
        self.metrics.lock().unwrap().upstream(verb, took);
        let outcome = match &result {
            Ok(_) => "ok",
            Err(e) => {
                let kind = BrokerError::from(e.clone()).error;
                *self.last_error.lock().unwrap() = Some(kind);
                kind
            }
        };
        let _ = REQUEST.try_with(|c| {
            c.spans.lock().unwrap().push(UpstreamSpan {
                verb,
                us: took.as_micros() as u64,
                outcome,
            })
        });
        result
    }

    /// Like `call`, but a refusal or timeout becomes missing evidence instead of an error.
    /// An unavailable upstream is still an error: the broker never fabricates context (RF-12).
    async fn evidence(
        &self,
        verb: &'static str,
        args: Value,
    ) -> Result<Result<String, Entry>, BrokerError> {
        self.evidence_or(verb, args, normalize::missing_evidence)
            .await
    }

    /// [`Broker::evidence`], with the limitation a refusal becomes chosen by the caller.
    async fn evidence_or(
        &self,
        verb: &'static str,
        args: Value,
        missing: fn(&'static str, &str) -> Entry,
    ) -> Result<Result<String, Entry>, BrokerError> {
        match self.guarded_call(verb, args).await {
            Ok(p) => Ok(Ok(p)),
            Err(e @ UpstreamError::Unavailable(_)) => Err(e.into()),
            Err(e) => Ok(Err(missing(verb, &e.to_string()))),
        }
    }

    pub async fn context_before_finish(&self, req: FinishRequest) -> Result<Envelope, BrokerError> {
        self.traced(
            "context_before_finish",
            self.context_before_finish_inner(req),
        )
        .await
    }

    async fn context_before_finish_inner(
        &self,
        req: FinishRequest,
    ) -> Result<Envelope, BrokerError> {
        check_budget(req.budget_tokens)?;
        let mut verbs = vec!["situational_awareness", "quality_delta"];
        let mut entries = Vec::new();
        let mut unknown = false;
        let mut changed = Vec::new();
        // Independentes entre si; o `affected` abaixo é que depende da situação (D-096).
        let (situation, quality) = tokio::join!(
            self.evidence("situational_awareness", json!({})),
            self.evidence("quality_delta", json!({}))
        );
        match situation? {
            Ok(p) => {
                changed = normalize::changed_files(&p);
                entries.extend(normalize::situation(&p));
            }
            Err(missing) => {
                unknown = true;
                entries.push(missing);
            }
        }
        let (mut regressions, mut minor) = (0, 0);
        match quality? {
            Ok(p) => match normalize::quality_delta(&p) {
                Some(q) => {
                    (regressions, minor) = (q.regressions, q.minor);
                    entries.extend(q.entries);
                }
                None => {
                    unknown = true;
                    entries.push(normalize::missing_evidence(
                        "quality_delta",
                        "unreadable answer",
                    ));
                }
            },
            Err(missing) => {
                unknown = true;
                entries.push(missing);
            }
        }
        if !changed.is_empty() {
            verbs.push("affected");
            match self
                .evidence("affected", json!({"files": changed.join(",")}))
                .await?
            {
                Ok(p) => entries.extend(normalize::affected(&p)),
                Err(missing) => entries.push(missing),
            }
        }
        if !req.include_test_commands {
            for e in &mut entries {
                if let Entry::Test(_, t) = e {
                    t.run = None;
                }
            }
        }
        let open_obligation = entries
            .iter()
            .any(|e| matches!(e, Entry::Risk(_, r) if GATE_RISKS.contains(&r.kind)));
        let status = if regressions > 0 || (req.strict && minor > 0) || open_obligation {
            Status::AttentionRequired
        } else if unknown {
            Status::Unknown
        } else {
            Status::Ready
        };
        let env = self.envelope(
            Shape {
                tool: "context_before_finish",
                intent: None,
                status,
                verbs,
                budget: req.budget_tokens,
                // The gate's evidence must always show (CA-05).
                suppress_seen: false,
                online: None,
            },
            entries,
        );
        self.remember(&env);
        self.observe(MemoryEvent::BeforeFinish, &env, changed).await;
        Ok(env)
    }

    pub async fn context_for_task(&self, req: TaskRequest) -> Result<Envelope, BrokerError> {
        self.traced("context_for_task", self.context_for_task_inner(req))
            .await
    }

    /// The structural context and, with `--memory`, the memory read, side by side (PRD jev-mem
    /// §10). Memory only adds to the answer: it never fails it and never changes its status.
    async fn context_for_task_inner(&self, req: TaskRequest) -> Result<Envelope, BrokerError> {
        check_budget_at(req.budget_tokens, self.min_task_budget())?;
        let recall = async {
            match &self.recall {
                Some(r) => Some(r.read(&req.task).await),
                None => None,
            }
        };
        let (env, recalled) = tokio::join!(self.structural_task(&req), recall);
        let mut env = env?;
        if let Some(recalled) = recalled {
            self.attach_memory(&mut env, recalled, !req.include_seen);
        }
        self.remember(&env);
        Ok(env)
    }

    /// `memories` and `provenance.memory`, and what kept the read short as limitations.
    fn attach_memory(&self, env: &mut Envelope, recalled: Recalled, skip_seen: bool) {
        env.limitations.extend(recalled.limitations);
        if let Some(read) = recalled.read {
            let session = self.session.lock().unwrap();
            let seen = |id: &str| skip_seen && session.has(&session::memory_fingerprint(id));
            retrieve::attach(env, &read, &seen);
        }
        env.budget.estimated_tokens = budget::estimate_tokens(env);
    }

    async fn structural_task(&self, req: &TaskRequest) -> Result<Envelope, BrokerError> {
        let route = router::route(&req.task, req.mode);
        let mut verbs = Vec::new();
        let entries = match (route.intent, route.symbol.as_deref()) {
            (Intent::Debug, _) => {
                verbs.push("from_trace");
                let payload = self
                    .call(
                        "from_trace",
                        json!({"trace": req.task, "budget_tokens": req.budget_tokens}),
                    )
                    .await?;
                normalize::ctx("from_trace", &payload)
            }
            (Intent::Review, _) => {
                verbs.push("situational_awareness");
                let payload = self.call("situational_awareness", json!({})).await?;
                normalize::situation(&payload)
            }
            (Intent::Docs, _) => {
                verbs.push("memory_recall");
                let payload = self
                    .call(
                        "memory_recall",
                        json!({"task": req.task, "budget_tokens": req.budget_tokens}),
                    )
                    .await?;
                normalize::recall(&payload)
            }
            (intent @ (Intent::Symbol | Intent::Change), Some(symbol)) => {
                verbs.push("find_symbol");
                let found = match self
                    .guarded_call("find_symbol", json!({"symbol": symbol}))
                    .await
                {
                    Ok(payload) => payload,
                    // Not in the repository (often a word the router misread): explore
                    // instead of failing the whole request.
                    Err(UpstreamError::Refused(_)) => {
                        verbs.push("explore");
                        let payload = self
                            .call(
                                "explore",
                                json!({"task": req.task, "budget_tokens": req.budget_tokens}),
                            )
                            .await?;
                        let mut entries = normalize::ctx("explore", &payload);
                        entries.push(normalize::symbol_not_found(symbol));
                        return Ok(self.complete_task(req, route.intent, verbs, entries).await);
                    }
                    Err(e) => return Err(e.into()),
                };
                let mut entries = self.symbol_context(&found, req, &mut verbs).await?;
                if intent == Intent::Change {
                    verbs.push("impact");
                    let payload = self.call("impact", json!({"symbol": symbol})).await?;
                    entries.extend(normalize::impact(&payload));
                }
                entries
            }
            _ => {
                verbs.push("explore");
                // In auto mode `orient` is the fallback, i.e. the router saw no signal.
                let uncertain = req.mode == Mode::Auto && route.intent == Intent::Orient;
                let asked = if uncertain {
                    (req.budget_tokens / 2).max(MIN_BUDGET_TOKENS)
                } else {
                    req.budget_tokens
                };
                let payload = self
                    .call("explore", json!({"task": req.task, "budget_tokens": asked}))
                    .await?;
                let mut entries = normalize::ctx("explore", &payload);
                if uncertain {
                    entries.push(normalize::route_uncertain(
                        "explore",
                        asked,
                        req.budget_tokens,
                    ));
                }
                entries
            }
        };
        Ok(self.complete_task(req, route.intent, verbs, entries).await)
    }

    /// The task's envelope, with notes when a local model is configured (PRD 10.3).
    async fn complete_task(
        &self,
        req: &TaskRequest,
        intent: Intent,
        verbs: Vec<&'static str>,
        entries: Vec<Entry>,
    ) -> Envelope {
        let (entries, online) = self.semantic_step(req, intent, &verbs, entries).await;
        let shaping = Instant::now();
        let (mut env, included) = self.finish_task(req, intent, verbs, entries, online);
        if let Some(engine) = &self.online {
            stage("context.budget", shaping.elapsed(), 0);
            engine.delivered(&env);
        }
        if let Some(engine) = &self.notes {
            self.attach_notes(engine, &mut env, &included, !req.include_seen)
                .await;
        }
        env
    }

    /// The `--online` step (PRD §23.4, D-060, D-061): on routes that ended in `explore`, the
    /// classifier rescores the planner's paths and the evidence is merged additively; on the
    /// others it is skipped, and the envelope says so.
    async fn semantic_step(
        &self,
        req: &TaskRequest,
        intent: Intent,
        verbs: &[&'static str],
        mut entries: Vec<Entry>,
    ) -> (Vec<Entry>, Option<OnlineProvenance>) {
        let Some(engine) = &self.online else {
            return (entries, None);
        };
        let provider = engine.config().provider.clone();
        if !verbs.contains(&"explore") {
            let route = serde_json::to_value(intent)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            entries.push(online_merge::skipped(&route));
            return (
                entries,
                Some(online_merge::provenance(&provider, engine.model(), None)),
            );
        }
        let ranked = online::ranked_paths(entries.iter().filter_map(|e| match e {
            Entry::Item(p, i) => Some((*p, i)),
            _ => None,
        }));
        let started = Instant::now();
        let disc = engine.discover(&req.task, &ranked).await;
        for s in &disc.stages {
            stage(s.stage, Duration::from_micros(s.us), s.batches);
        }
        stage("semantic.discovery", started.elapsed(), disc.requests);
        let merging = Instant::now();
        let online = online_merge::provenance(&provider, engine.model(), Some(&disc));
        let cap = engine.config().max_source_bytes;
        let merged = online_merge::merge(entries, &disc, engine.model(), cap);
        stage("context.merge", merging.elapsed(), 0);
        (merged, Some(online))
    }

    /// Records what `env` delivers in the session memory. The last step of every tool, on
    /// the final envelope: nothing a later budget cut removed, and nothing from a call that
    /// was dropped (cancelled) before it finished, is ever marked as delivered (D-052).
    fn remember(&self, env: &Envelope) {
        if !self.incremental {
            return;
        }
        let mut memory = self.session.lock().unwrap();
        for i in env
            .items
            .iter()
            .filter(|i| i.why_included != session::SEEN_REFERENCE)
        {
            memory.remember(session::item_fingerprint(i));
        }
        for t in &env.tests {
            memory.remember(session::test_fingerprint(t));
        }
        for r in &env.risks {
            memory.remember(session::risk_fingerprint(r));
        }
        for n in &env.notes {
            memory.remember(session::note_fingerprint(n));
        }
        for m in &env.memories {
            memory.remember(session::memory_fingerprint(&m.id));
        }
    }

    /// Notes from the items this envelope includes, in full even when the session sent
    /// them as references, so a repeated task still hits the note cache.
    async fn attach_notes(
        &self,
        engine: &NoteEngine,
        env: &mut Envelope,
        included: &[Item],
        suppress_seen: bool,
    ) {
        let model = engine.model_id();
        let mut fresh = vec![];
        let mut limitations = vec![];
        for (scope, items) in note_engine::groups(included) {
            let evidence = note_engine::evidence(&items);
            let key = note_engine::key(&model, &scope, &evidence);
            match engine
                .note(key, note_engine::prompt(&scope, &evidence))
                .await
            {
                note_engine::Outcome::Ready { text, cached } => fresh.push(note_engine::note(
                    scope,
                    text,
                    model.clone(),
                    &items,
                    cached,
                )),
                note_engine::Outcome::Pending => limitations.push(note_engine::pending(&scope)),
                note_engine::Outcome::Failed(why) => {
                    limitations.push(note_engine::unavailable(&scope, &why))
                }
            }
        }
        if self.incremental && suppress_seen {
            let memory = self.session.lock().unwrap();
            let before = fresh.len();
            fresh.retain(|n| !memory.has(&session::note_fingerprint(n)));
            let repeated = before - fresh.len();
            env.budget.already_delivered += repeated;
            self.metrics.lock().unwrap().session_hits += repeated as u64;
        }
        // What a memory read writes comes after the notes: they leave its room alone.
        let reserve = match self.recall.is_some() {
            true => budget::memory_reserve(),
            false => 0,
        };
        budget::add_notes(env, fresh, limitations, reserve);
    }

    /// Applies the task's switches (docs, bodies) and shapes the envelope.
    fn finish_task(
        &self,
        req: &TaskRequest,
        intent: Intent,
        verbs: Vec<&'static str>,
        entries: Vec<Entry>,
        online: Option<OnlineProvenance>,
    ) -> (Envelope, Vec<Item>) {
        let entries: Vec<Entry> = entries
            .into_iter()
            .filter(|e| req.include_docs || !matches!(e, Entry::Item(_, i) if i.role == Role::Doc))
            .map(|mut e| {
                if let (false, Entry::Item(_, i)) = (req.include_bodies, &mut e) {
                    i.content = None;
                }
                e
            })
            .collect();
        let status = if entries.iter().any(|e| !matches!(e, Entry::Limitation(_))) {
            Status::Ready
        } else {
            Status::Unknown
        };
        self.envelope_full(
            Shape {
                tool: "context_for_task",
                intent: Some(intent),
                status,
                verbs,
                budget: req.budget_tokens,
                suppress_seen: !req.include_seen,
                online,
            },
            entries,
        )
    }

    /// Bodies and relations of a symbol `find_symbol` located.
    async fn symbol_context(
        &self,
        find_symbol: &str,
        req: &TaskRequest,
        verbs: &mut Vec<&'static str>,
    ) -> Result<Vec<Entry>, BrokerError> {
        let (mut entries, handle) = normalize::find_symbol(find_symbol);
        if let (true, Some(handle)) = (req.include_bodies, handle) {
            verbs.push("fetch_body");
            let body = self.call("fetch_body", json!({"handle": handle})).await?;
            let extra = normalize::attach_body(&mut entries, &body);
            entries.extend(extra);
        }
        Ok(entries)
    }

    fn envelope(&self, shape: Shape, entries: Vec<Entry>) -> Envelope {
        self.envelope_full(shape, entries).0
    }

    /// The envelope plus the full version of every item it includes (a session reference
    /// resolved to the item it points to): the evidence notes may use.
    fn envelope_full(&self, shape: Shape, entries: Vec<Entry>) -> (Envelope, Vec<Item>) {
        // Counted once each: the summary describes what was found, not its repetitions.
        let entries = crate::dedup::dedup(entries);
        let Shape {
            tool,
            intent,
            status,
            verbs,
            budget,
            suppress_seen,
            online,
        } = shape;
        let lead = match intent {
            Some(i) => serde_json::to_value(i)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            None => serde_json::to_value(status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
        };
        let mut env = Envelope {
            schema_version: SCHEMA_VERSION,
            tool,
            status,
            intent,
            summary: normalize::summary(&lead, &entries),
            items: vec![],
            tests: vec![],
            risks: vec![],
            limitations: vec![],
            notes: vec![],
            memories: vec![],
            provenance: Provenance {
                request_id: REQUEST.try_with(|c| c.id).unwrap_or(0),
                upstream_tools: verbs,
                workspace: self.workspace.root().display().to_string(),
                ripwire_version: self.ripwire_version.clone(),
                broker_version: env!("CARGO_PKG_VERSION"),
                online,
                memory: None,
            },
            budget: Budget {
                requested_tokens: budget,
                estimated_tokens: 0,
                truncated: false,
                shown: 0,
                omitted: 0,
                next_step: None,
                already_delivered: 0,
            },
        };
        let entries = normalize::cap_items(entries, self.max_item_tokens as usize * 4);
        // With a summarizer, `add_notes` runs after this and always writes at least the record
        // that notes did not fit. Holding that room back here is what keeps it from evicting an
        // item whose body `attach_notes` has already sent to the local model (D-103).
        let reserve = match self.notes.is_some() {
            true => budget::notes_reserve(),
            false => 0,
        };
        // The same for what a memory read writes after the entries are fitted.
        let reserve = match self.recall.is_some() {
            true => reserve + budget::memory_reserve(),
            false => reserve,
        };
        if !self.incremental {
            budget::fill(&mut env, entries, reserve);
            let included = env.items.clone();
            return (env, included);
        }
        let originals: Vec<Item> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Item(_, i) => Some(i.clone()),
                _ => None,
            })
            .collect();
        let memory = self.session.lock().unwrap();
        let mut entries = entries;
        if suppress_seen {
            let before = entries.len();
            entries = entries
                .into_iter()
                .filter_map(|e| match e {
                    Entry::Item(p, i) if memory.has(&session::item_fingerprint(&i)) => {
                        Some(Entry::Item(p, session::reference(&i)))
                    }
                    Entry::Test(_, t) if memory.has(&session::test_fingerprint(&t)) => None,
                    Entry::Risk(_, r)
                        if !GATE_RISKS.contains(&r.kind)
                            && memory.has(&session::risk_fingerprint(&r)) =>
                    {
                        None
                    }
                    other => Some(other),
                })
                .collect();
            env.budget.already_delivered = before - entries.len();
            let references = entries
                .iter()
                .filter(
                    |e| matches!(e, Entry::Item(_, i) if i.why_included == session::SEEN_REFERENCE),
                )
                .count();
            self.metrics.lock().unwrap().session_hits +=
                (env.budget.already_delivered + references) as u64;
        }
        drop(memory);
        budget::fill(&mut env, entries, reserve);
        let included = env
            .items
            .iter()
            .map(|i| {
                originals
                    .iter()
                    .find(|o| {
                        i.why_included == session::SEEN_REFERENCE
                            && (&o.path, o.line, &o.symbol) == (&i.path, i.line, &i.symbol)
                    })
                    .unwrap_or(i)
                    .clone()
            })
            .collect();
        (env, included)
    }
}

/// What a tool decided about its answer, before the entries are shaped into an envelope.
struct Shape {
    tool: &'static str,
    intent: Option<Intent>,
    status: Status,
    verbs: Vec<&'static str>,
    budget: u32,
    /// False for the finish gate, whose evidence must always show (CA-05).
    suppress_seen: bool,
    /// `provenance.online`; set before the budget so its bytes are counted.
    online: Option<OnlineProvenance>,
}

fn push_once(verbs: &mut Vec<&'static str>, verb: &'static str) {
    if !verbs.contains(&verb) {
        verbs.push(verb);
    }
}
