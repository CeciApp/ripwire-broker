//! `SemanticCoordinator` (PRD §23.4, D-061): turns the planner's ranked paths into semantic
//! evidence. Reads eligible snapshots, asks `file_admission` about each preview, then
//! `source_selection` about the units of admitted files, through the cache and the scheduler.
//! It applies thresholds; it never builds a relation or touches the envelope.

use super::cache::{self, KeyParts, SemanticCache};
use super::classifier::Classifier;
use super::decision::{FileDecision, file_decision};
use super::reader::{Snapshot, Unit, WorkspaceReader, units};
use super::request::{self, JevRequest, StateItem};
use super::scheduler::{Job, Scheduler, SchedulerConfig};
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
    pub cache: bool,
    /// Capacity of the queue in front of the scheduler.
    pub queue: usize,
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
            cache: true,
            queue: 8,
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
}

impl Discovery {
    /// Absences cannot be read as irrelevance (PRD §23.2).
    pub fn incomplete(&self) -> bool {
        !self.not_sent.is_empty()
            || self.too_large > 0
            || !self.failures.is_empty()
            || self.unfinished > 0
            || self.limit_reached
    }
}

/// Totals for the status resource: counts and categories only.
#[derive(Debug, Default, Clone)]
pub struct OnlineTotals {
    pub requests: u64,
    pub cache_hits: u64,
    pub last_error: Option<&'static str>,
}

pub struct OnlineEngine {
    config: OnlineConfig,
    reader: WorkspaceReader,
    cache: Mutex<SemanticCache>,
    totals: Mutex<OnlineTotals>,
}

/// A question waiting for an answer: which snapshot bytes it shows, and its cache key.
struct Pending {
    item: StateItem,
    key: cache::Key,
}

fn digest(req: &JevRequest) -> String {
    let json = serde_json::to_vec(req).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(json))
}

impl OnlineEngine {
    pub fn new(config: OnlineConfig, root: &Path) -> Result<Self, String> {
        Ok(Self {
            reader: WorkspaceReader::new(root)?,
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
        let mut files: Vec<(&RankedPath, Snapshot)> = vec![];
        for rp in ranked.iter().take(self.config.max_candidates) {
            match self.reader.snapshot(&rp.path) {
                Ok(snap) => files.push((rp, snap)),
                Err(why) => *disc.not_sent.entry(why.as_str()).or_default() += 1,
            }
        }

        let admission: Vec<Pending> = files
            .iter()
            .enumerate()
            .map(|(n, (_, snap))| {
                let preview = snap.preview();
                Pending {
                    key: self.key(SemanticStage::FileAdmission, query, snap, 0..preview.len()),
                    item: StateItem {
                        id: format!("f{n}"),
                        path: snap.path.clone(),
                        text: preview.to_string(),
                    },
                }
            })
            .collect();
        let admitted = self
            .ask(
                query,
                SemanticStage::FileAdmission,
                admission,
                &mut left,
                &mut disc,
            )
            .await;

        let mut selection: Vec<Pending> = vec![];
        let mut owner: HashMap<String, (usize, Unit)> = HashMap::new();
        for (n, (rp, snap)) in files.iter().enumerate() {
            let scored = admitted
                .get(&format!("f{n}"))
                .cloned()
                .unwrap_or_else(Scored::unknown);
            let (decision, _) = file_decision(&[scored.probability]);
            disc.files.push(FileEvidence {
                path: snap.path.clone(),
                content_hash: snap.content_hash.clone(),
                admission: scored,
                decision,
                units: vec![],
                location_only: snap.location_only(),
            });
            if decision != FileDecision::Admitted {
                continue;
            }
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
                });
                owner.insert(id, (n, unit));
            }
        }
        let texts: HashMap<String, String> = selection
            .iter()
            .map(|p| (p.item.id.clone(), p.item.text.clone()))
            .collect();
        let selected = self
            .ask(
                query,
                SemanticStage::SourceSelection,
                selection,
                &mut left,
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

        let mut totals = self.totals.lock().unwrap();
        totals.requests += disc.requests as u64;
        totals.cache_hits += disc.cache_hits as u64;
        if let Some((category, _)) = disc.failures.iter().next() {
            totals.last_error = Some(category);
        }
        disc
    }

    /// Answers each pending question from the cache or the classifier, by item id.
    async fn ask(
        &self,
        query: &str,
        stage: SemanticStage,
        pending: Vec<Pending>,
        left: &mut usize,
        disc: &mut Discovery,
    ) -> HashMap<String, Scored> {
        let mut out = HashMap::new();
        let mut keys = HashMap::new();
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
        let scheduler = Scheduler::new(
            self.config.classifier.clone(),
            SchedulerConfig {
                max_in_flight: self.config.max_in_flight,
                request_limit: *left,
                queue: self.config.queue,
            },
        );
        let (tx, rx) = scheduler.queue();
        let produce = async {
            for (id, request) in requests.iter().cloned().enumerate() {
                if tx.send(Job { id, request }).await.is_err() {
                    break;
                }
            }
            drop(tx);
        };
        let (_, report) = tokio::join!(produce, scheduler.run(rx, CancellationToken::new()));
        *left = left.saturating_sub(report.requests);
        disc.requests += report.requests;
        disc.unfinished += report.unfinished.len();
        if report.stop == Some(super::scheduler::Stop::RequestLimit) {
            disc.limit_reached = true;
        }
        let mut cache = self.cache.lock().unwrap();
        for done in report.results {
            let req = &requests[done.id];
            match done.result {
                Ok(answers) => {
                    let request_digest = digest(req);
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
