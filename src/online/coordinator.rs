//! `SemanticCoordinator` (PRD §23.4, D-061): turns the planner's ranked paths into semantic
//! evidence. Reads eligible snapshots, asks `file_admission` about each preview, then
//! `source_selection` about the units of admitted files, through the cache and the scheduler.
//! It applies thresholds; it never builds a relation or touches the envelope.

use super::cache::{self, KeyParts, SemanticCache};
use super::classifier::Classifier;
use super::decision::{FileDecision, SourceDecision, file_decision, select};
use super::metrics::{Metered, OnlineMetrics};
use super::reader::{LOOKAHEAD_PREVIEW_BYTES, Snapshot, Unit, WorkspaceReader, units};
use super::request::{self, JevRequest, StateItem};
use super::scheduler::{Freshness, Job, Scheduler, SchedulerConfig};
use super::{RankedPath, SemanticStage};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// How the process was started with `--online` (PRD §23.6). No credential: the classifier
/// owns it.
#[derive(Clone)]
pub struct OnlineConfig {
    pub classifier: Arc<dyn Classifier>,
    pub provider: String,
    /// Host of the allowlisted endpoint, for the cache key and the status.
    pub endpoint_host: String,
    pub max_in_flight: usize,
    pub request_limit: usize,
    /// Planner paths the rescore evaluates (D-061).
    pub max_candidates: usize,
    /// Siblings the one-level lookahead may add, in total (D-061).
    pub lookahead_max: usize,
    pub cache: bool,
    /// Capacity of the queue in front of the scheduler.
    pub queue: usize,
    /// Past it the semantic stage stops and the answer goes out `interrupted` (D-063).
    pub deadline: std::time::Duration,
    /// Caps semantic source rendered in the envelope, not what is evaluated; `None`: the
    /// token budget alone decides (PRD §23.6).
    pub max_source_bytes: Option<usize>,
}

impl OnlineConfig {
    /// The Phase 4 defaults (D-059..D-064) around `classifier`.
    pub fn new(classifier: Arc<dyn Classifier>) -> Self {
        Self {
            classifier,
            provider: "typesafe".into(),
            endpoint_host: "api.typesafe.ai".into(),
            max_in_flight: 4,
            request_limit: 24,
            max_candidates: 16,
            lookahead_max: 32,
            cache: true,
            queue: 8,
            deadline: std::time::Duration::from_millis(8_000),
            max_source_bytes: None,
        }
    }
}

impl std::fmt::Debug for OnlineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnlineConfig")
            .field("provider", &self.provider)
            .field("model", &self.classifier.model())
            .field("max_in_flight", &self.max_in_flight)
            .field("request_limit", &self.request_limit)
            .finish_non_exhaustive()
    }
}

/// One answer, as the merger needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    /// `None`: unknown (failed, not sent, invalid). Never zero.
    pub probability: Option<f64>,
    pub request_digest: String,
    pub cache_hit: bool,
}

impl Scored {
    fn unknown() -> Self {
        Self {
            probability: None,
            request_digest: String::new(),
            cache_hit: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnitEvidence {
    pub unit: Unit,
    pub text: String,
    pub scored: Scored,
}

#[derive(Debug, Clone)]
pub struct FileEvidence {
    pub path: String,
    pub content_hash: String,
    pub admission: Scored,
    pub decision: FileDecision,
    pub units: Vec<UnitEvidence>,
    pub location_only: bool,
    /// Found by the lookahead beside a ripwire candidate, not by ripwire.
    pub lookahead: bool,
}

/// What one discovery found, and what it could not do.
#[derive(Debug, Default)]
pub struct Discovery {
    pub files: Vec<FileEvidence>,
    pub requests: usize,
    pub cache_hits: usize,
    /// Candidates never sent, by policy reason (no paths).
    pub not_sent: BTreeMap<&'static str, usize>,
    /// Questions too large to send even alone.
    pub too_large: usize,
    /// Error categories of failed requests, and whether the request limit stopped it.
    pub failures: BTreeMap<&'static str, usize>,
    pub unfinished: usize,
    pub limit_reached: bool,
    /// Batches never sent because a source changed after it was read.
    pub stale_batches: usize,
    /// Files that changed before the output; their evidence was dropped.
    pub changed_files: usize,
    /// The discovery deadline (in ms) cut the semantic stage short.
    pub interrupted: Option<u128>,
    /// Lookahead files the classifier admitted: candidates ripwire did not rank.
    pub semantic_only: usize,
    /// Questions answered without a valid probability (CA-ONLINE-10): never a score.
    pub unknown_answers: usize,
    /// Files the classifier was asked about, planner and lookahead.
    pub candidates: usize,
    pub selected_ranges: usize,
    pub retries: usize,
    pub splits: usize,
    pub rate_limited: usize,
    /// Timings of the admission and selection stages, for the request record.
    pub stages: Vec<crate::metrics::StageSpan>,
}

impl Discovery {
    /// Absences cannot be read as irrelevance (PRD §23.2).
    pub fn incomplete(&self) -> bool {
        !self.not_sent.is_empty()
            || self.too_large > 0
            || !self.failures.is_empty()
            || self.unfinished > 0
            || self.limit_reached
            || self.stale_batches > 0
            || self.changed_files > 0
            || self.interrupted.is_some()
            || self.unknown_answers > 0
    }
}

/// Totals for the status resource: counts and categories only.
#[derive(Debug, Default, Clone)]
pub struct OnlineTotals {
    pub requests: u64,
    pub cache_hits: u64,
    pub last_error: Option<&'static str>,
    /// `semantic_only_candidates_total` (PRD §23.11): the gain beyond ripwire.
    pub semantic_only: u64,
    pub candidates: u64,
    pub selected_ranges: u64,
    pub retries: u64,
    pub splits: u64,
    pub rate_limited: u64,
    pub context_tokens: u64,
}

pub struct OnlineEngine {
    config: OnlineConfig,
    /// The configured classifier, measured; the scheduler only ever sees this one.
    metered: Arc<Metered>,
    reader: Arc<WorkspaceReader>,
    cache: Mutex<SemanticCache>,
    totals: Mutex<OnlineTotals>,
}

/// A question waiting for an answer: which snapshot bytes it shows, and its cache key.
struct Pending {
    item: StateItem,
    key: cache::Key,
    /// The version the item was cut from, rechecked before every attempt.
    snapshot: Arc<Snapshot>,
}

fn digest(req: &JevRequest) -> String {
    let json = serde_json::to_vec(req).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(json))
}

impl OnlineEngine {
    pub fn new(config: OnlineConfig, root: &Path) -> Result<Self, String> {
        Ok(Self {
            reader: Arc::new(WorkspaceReader::new(root)?),
            metered: Arc::new(Metered::new(config.classifier.clone())),
            config,
            cache: Mutex::default(),
            totals: Mutex::default(),
        })
    }

    pub fn config(&self) -> &OnlineConfig {
        &self.config
    }

    pub fn model(&self) -> &str {
        self.config.classifier.model()
    }

    pub fn totals(&self) -> OnlineTotals {
        self.totals.lock().unwrap().clone()
    }

    pub fn cached_decisions(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// The cache entries, rendered as stored.
    pub fn inspect_cache(&self) -> Vec<String> {
        self.cache
            .lock()
            .unwrap()
            .dump()
            .into_iter()
            .map(|(key, value)| format!("{key} {value:?}"))
            .collect()
    }

    /// The §23.11 metrics for the status resource.
    pub fn metrics(&self) -> OnlineMetrics {
        let t = self.totals();
        let mut m = OnlineMetrics {
            jev_cache_hits_total: t.cache_hits,
            jev_rate_limit_total: t.rate_limited,
            jev_retry_total: t.retries,
            jev_split_total: t.splits,
            semantic_candidates_total: t.candidates,
            semantic_selected_ranges_total: t.selected_ranges,
            semantic_only_candidates_total: t.semantic_only,
            online_context_tokens_estimated: t.context_tokens,
            ..OnlineMetrics::default()
        };
        self.metered.fill(&mut m);
        m
    }

    /// Counts the tokens of semantic evidence that reached the agent in `env`: the
    /// `semantic_location` items and the `semantic` annotations, estimated like the budget.
    pub fn delivered(&self, env: &crate::model::Envelope) {
        let bytes: usize = env
            .items
            .iter()
            .map(|i| match (i.kind, &i.semantic) {
                ("semantic_location", _) => serde_json::to_vec(i).map_or(0, |b| b.len()),
                (_, Some(s)) => serde_json::to_vec(s).map_or(0, |b| b.len()),
                _ => 0,
            })
            .sum();
        self.totals.lock().unwrap().context_tokens += bytes.div_ceil(4) as u64;
    }

    fn key(
        &self,
        stage: SemanticStage,
        query: &str,
        snap: &Snapshot,
        range: std::ops::Range<usize>,
    ) -> cache::Key {
        cache::key(&KeyParts {
            provider: &self.config.provider,
            endpoint: &self.config.endpoint_host,
            model: self.model(),
            stage,
            query,
            content_hash: &snap.content_hash,
            range,
        })
    }

    /// Admission of the ranked planner paths, then selection of the admitted files' units.
    pub async fn discover(&self, query: &str, ranked: &[RankedPath]) -> Discovery {
        let mut disc = Discovery::default();
        let mut left = self.config.request_limit;
        let deadline = tokio::time::Instant::now() + self.config.deadline;
        let mut files: Vec<(RankedPath, Arc<Snapshot>)> = vec![];
        let picked: Vec<RankedPath> = ranked
            .iter()
            .take(self.config.max_candidates)
            .cloned()
            .collect();
        let read = self
            .on_disk(move |reader| {
                picked
                    .into_iter()
                    .map(|rp| {
                        let snap = reader.snapshot(&rp.path);
                        (rp, snap)
                    })
                    .collect::<Vec<_>>()
            })
            .await
            .unwrap_or_default();
        for (rp, snap) in read {
            match snap {
                Ok(snap) => files.push((rp, Arc::new(snap))),
                Err(why) => *disc.not_sent.entry(why.as_str()).or_default() += 1,
            }
        }

        let navigation = std::time::Instant::now();
        let before = disc.requests;
        let mut admitted = self
            .ask(
                query,
                SemanticStage::FileAdmission,
                self.admission(query, &files, 0),
                &mut left,
                deadline,
                &mut disc,
            )
            .await;

        // One-level lookahead (D-061): siblings of the admitted planner paths.
        let planner = files.len();
        let dirs: std::collections::BTreeSet<String> = files
            .iter()
            .enumerate()
            .filter(|(n, _)| {
                let p = admitted.get(&format!("f{n}")).and_then(|s| s.probability);
                file_decision(&[p]).0 == FileDecision::Admitted
            })
            .map(|(_, (rp, _))| match rp.path.rsplit_once('/') {
                Some((dir, _)) => dir.to_string(),
                None => String::new(),
            })
            .collect();
        let mut taken: std::collections::HashSet<String> =
            ranked.iter().map(|rp| rp.path.clone()).collect();
        taken.extend(files.iter().map(|(rp, _)| rp.path.clone()));
        let room = self.config.lookahead_max;
        let siblings = self
            .on_disk(move |reader| {
                let mut found = vec![];
                'dirs: for dir in dirs {
                    for path in reader.files_in(&dir) {
                        if found.len() == room {
                            break 'dirs;
                        }
                        if !taken.insert(path.clone()) {
                            continue;
                        }
                        // Ineligible siblings are policy, not candidates ripwire named: skipped
                        // quietly.
                        if let Ok(snap) = reader.snapshot(&path) {
                            found.push((path, snap));
                        }
                    }
                }
                found
            })
            .await
            .unwrap_or_default();
        for (path, snap) in siblings {
            let rank = ranked.len() + files.len();
            let rp = RankedPath {
                path,
                rank,
                priority: crate::normalize::priority::PERIPHERAL,
                origin: super::PathOrigin::Lookahead,
                lines: vec![],
            };
            files.push((rp, Arc::new(snap)));
        }
        if files.len() > planner {
            let extra = self
                .ask(
                    query,
                    SemanticStage::FileAdmission,
                    self.admission(query, &files[planner..], planner),
                    &mut left,
                    deadline,
                    &mut disc,
                )
                .await;
            admitted.extend(extra);
        }
        disc.stages.push(crate::metrics::StageSpan {
            stage: "semantic.navigation.batch",
            us: navigation.elapsed().as_micros() as u64,
            batches: disc.requests - before,
        });
        disc.candidates = files.len();

        let mut selection: Vec<Pending> = vec![];
        let mut owner: HashMap<String, (usize, Unit)> = HashMap::new();
        for (n, (rp, snap)) in files.iter().enumerate() {
            let scored = admitted
                .get(&format!("f{n}"))
                .cloned()
                .unwrap_or_else(Scored::unknown);
            let (decision, _) = file_decision(&[scored.probability]);
            let lookahead = rp.origin == super::PathOrigin::Lookahead;
            if lookahead && decision == FileDecision::Admitted {
                disc.semantic_only += 1;
            }
            disc.files.push(FileEvidence {
                path: snap.path.clone(),
                content_hash: snap.content_hash.clone(),
                admission: scored,
                decision,
                units: vec![],
                location_only: snap.location_only(),
                lookahead,
            });
        }
        // The most likely files are asked about first, so a tight request limit cuts the
        // least likely ones (D-081). The sort is stable: ties keep ripwire's order.
        let mut order: Vec<usize> = (0..files.len())
            .filter(|n| disc.files[*n].decision == FileDecision::Admitted)
            .collect();
        order.sort_by(|a, b| {
            let p = |n: &usize| disc.files[*n].admission.probability.unwrap_or_default();
            p(b).total_cmp(&p(a))
        });
        for n in order {
            let (rp, snap) = &files[n];
            for unit in units(snap, &rp.lines) {
                let id = format!("u{}", owner.len());
                selection.push(Pending {
                    key: self.key(
                        SemanticStage::SourceSelection,
                        query,
                        snap,
                        unit.bytes.clone(),
                    ),
                    item: StateItem {
                        id: id.clone(),
                        path: snap.path.clone(),
                        text: snap.text(&unit).to_string(),
                    },
                    snapshot: snap.clone(),
                });
                owner.insert(id, (n, unit));
            }
        }
        let texts: HashMap<String, String> = selection
            .iter()
            .map(|p| (p.item.id.clone(), p.item.text.clone()))
            .collect();
        let selecting = std::time::Instant::now();
        let before = disc.requests;
        let selected = self
            .ask(
                query,
                SemanticStage::SourceSelection,
                selection,
                &mut left,
                deadline,
                &mut disc,
            )
            .await;
        let mut ids: Vec<&String> = owner.keys().collect();
        ids.sort_by_key(|id| id[1..].parse::<usize>().unwrap_or(usize::MAX));
        for id in ids {
            let (n, unit) = &owner[id];
            disc.files[*n].units.push(UnitEvidence {
                unit: unit.clone(),
                text: texts[id].clone(),
                scored: selected.get(id).cloned().unwrap_or_else(Scored::unknown),
            });
        }
        disc.selected_ranges = disc
            .files
            .iter()
            .flat_map(|f| &f.units)
            .filter(|u| select(u.scored.probability) == SourceDecision::Selected)
            .count();
        disc.stages.push(crate::metrics::StageSpan {
            stage: "semantic.selection.batch",
            us: selecting.elapsed().as_micros() as u64,
            batches: disc.requests - before,
        });

        // Before the output: evidence about a version that no longer exists is dropped
        // (RF-ONLINE-10, CA-ONLINE-11). The file's structural facts stay untouched.
        let before = disc.files.len();
        let snaps: Vec<Arc<Snapshot>> = files.iter().map(|(_, snap)| snap.clone()).collect();
        let fresh: Vec<bool> = self
            .on_disk(move |reader| snaps.iter().map(|s| reader.is_fresh(s)).collect())
            .await
            .unwrap_or_default();
        let mut keep = fresh.iter();
        disc.files.retain(|_| *keep.next().unwrap_or(&false));
        disc.changed_files += before - disc.files.len();

        let mut totals = self.totals.lock().unwrap();
        totals.requests += disc.requests as u64;
        totals.cache_hits += disc.cache_hits as u64;
        totals.semantic_only += disc.semantic_only as u64;
        totals.candidates += disc.candidates as u64;
        totals.selected_ranges += disc.selected_ranges as u64;
        totals.retries += disc.retries as u64;
        totals.splits += disc.splits as u64;
        totals.rate_limited += disc.rate_limited as u64;
        if let Some((category, _)) = disc.failures.iter().next() {
            totals.last_error = Some(category);
        }
        disc
    }

    /// `work` on the reader in the blocking pool: its walks, reads and hashes never hold the async
    /// thread (D-146). `None` if it panicked.
    async fn on_disk<T: Send + 'static>(
        &self,
        work: impl FnOnce(&WorkspaceReader) -> T + Send + 'static,
    ) -> Option<T> {
        let reader = self.reader.clone();
        tokio::task::spawn_blocking(move || work(&reader))
            .await
            .ok()
    }

    /// Admission questions about the previews of `files`, with ids `f{first}..`.
    fn admission(
        &self,
        query: &str,
        files: &[(RankedPath, Arc<Snapshot>)],
        first: usize,
    ) -> Vec<Pending> {
        files
            .iter()
            .enumerate()
            .map(|(k, (rp, snap))| {
                let preview = match rp.origin {
                    super::PathOrigin::Lookahead => snap.preview_at(LOOKAHEAD_PREVIEW_BYTES),
                    super::PathOrigin::Planner => snap.preview(),
                };
                Pending {
                    key: self.key(SemanticStage::FileAdmission, query, snap, 0..preview.len()),
                    item: StateItem {
                        id: format!("f{}", first + k),
                        path: snap.path.clone(),
                        text: preview.to_string(),
                    },
                    snapshot: snap.clone(),
                }
            })
            .collect()
    }

    /// Answers each pending question from the cache or the classifier, by item id.
    async fn ask(
        &self,
        query: &str,
        stage: SemanticStage,
        pending: Vec<Pending>,
        left: &mut usize,
        deadline: tokio::time::Instant,
        disc: &mut Discovery,
    ) -> HashMap<String, Scored> {
        let mut out = HashMap::new();
        let mut keys = HashMap::new();
        let mut snapshots: HashMap<String, Arc<Snapshot>> = HashMap::new();
        let mut send = vec![];
        {
            let cache = self.cache.lock().unwrap();
            for p in pending {
                match cache.get(&p.key).filter(|_| self.config.cache) {
                    Some(hit) => {
                        disc.cache_hits += 1;
                        out.insert(
                            p.item.id.clone(),
                            Scored {
                                probability: Some(hit.probability),
                                request_digest: hit.request_digest.clone(),
                                cache_hit: true,
                            },
                        );
                    }
                    None => {
                        keys.insert(p.item.id.clone(), p.key);
                        snapshots.insert(p.item.id.clone(), p.snapshot);
                        send.push(p.item);
                    }
                }
            }
        }
        if send.is_empty() {
            return out;
        }
        let (requests, too_large) = request::batches(self.model(), query, stage, send);
        disc.too_large += too_large.len();
        if *left == 0 {
            disc.limit_reached = true;
            disc.unfinished += requests.len();
            return out;
        }
        if tokio::time::Instant::now() >= deadline {
            disc.interrupted = Some(self.config.deadline.as_millis());
            disc.unfinished += requests.len();
            return out;
        }
        let scheduler = Scheduler::new(
            self.metered.clone(),
            SchedulerConfig {
                max_in_flight: self.config.max_in_flight,
                request_limit: *left,
                queue: self.config.queue,
            },
        );
        let (tx, rx) = scheduler.queue();
        let produce = async {
            for (id, request) in requests.iter().cloned().enumerate() {
                let mut sources: Vec<Arc<Snapshot>> = vec![];
                for item in &request.state.items {
                    let snap = &snapshots[&item.id];
                    if !sources.iter().any(|s| Arc::ptr_eq(s, snap)) {
                        sources.push(snap.clone());
                    }
                }
                let reader = self.reader.clone();
                let fresh = Freshness(Arc::new(move || sources.iter().all(|s| reader.is_fresh(s))));
                let job = Job {
                    id,
                    stage,
                    request,
                    fresh: Some(fresh),
                };
                if tx.send(job).await.is_err() {
                    break;
                }
            }
            drop(tx);
        };
        // The deadline cancels the run instead of dropping it, so what already came back
        // stays usable (v0.1 §11.10).
        let stop = CancellationToken::new();
        let run = async { tokio::join!(produce, scheduler.run(rx, stop.clone())).1 };
        tokio::pin!(run);
        let report = tokio::select! {
            report = &mut run => report,
            _ = tokio::time::sleep_until(deadline) => {
                stop.cancel();
                disc.interrupted = Some(self.config.deadline.as_millis());
                run.await
            }
        };
        *left = left.saturating_sub(report.requests);
        disc.requests += report.requests;
        disc.unfinished += report.unfinished.len();
        disc.stale_batches += report.stale.len();
        disc.retries += report.retries;
        disc.splits += report.splits;
        disc.rate_limited += report.rate_limited;
        if report.stop == Some(super::scheduler::Stop::RequestLimit) {
            disc.limit_reached = true;
        }
        let mut cache = self.cache.lock().unwrap();
        for done in report.results {
            let req = &done.request;
            match done.result {
                Ok(answers) => {
                    let request_digest = digest(req);
                    disc.unknown_answers += answers.iter().filter(|p| p.is_none()).count();
                    for (item, p) in req.state.items.iter().zip(answers) {
                        if let (Some(p), true) = (p, self.config.cache) {
                            cache.insert(keys[&item.id], p, request_digest.clone());
                        }
                        out.insert(
                            item.id.clone(),
                            Scored {
                                probability: p,
                                request_digest: request_digest.clone(),
                                cache_hit: false,
                            },
                        );
                    }
                }
                Err(e) => *disc.failures.entry(e.category()).or_default() += 1,
            }
        }
        out
    }
}
