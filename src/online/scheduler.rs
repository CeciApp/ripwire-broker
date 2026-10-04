//! `JevScheduler` (PRD §23.5): runs classifier requests from a bounded queue with at most
//! `max_in_flight` in flight and at most `request_limit` per MCP call. It owns every task in a
//! `JoinSet`, so an auth failure or a cancellation aborts the siblings (dropping their HTTP
//! requests) and closes the queue, which stops the producer. It retries and splits failed
//! batches by stage (v0.1 §11.9), rechecks freshness before every attempt, never interprets
//! probabilities, and never restarts a search.

use super::SemanticStage;
use super::classifier::{Classifier, ClassifyError};
use super::request::{JevRequest, build};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerConfig {
    /// RF-ONLINE-08: never more requests in flight per query.
    pub max_in_flight: usize,
    /// Requests one MCP call may send; every attempt counts.
    pub request_limit: usize,
    /// Capacity of the queue between the producer and the scheduler (RF-ONLINE-09).
    pub queue: usize,
}

/// One request, with the producer's id for matching the answer back.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    /// Decides the retry policy and how a split half is rebuilt.
    pub stage: SemanticStage,
    pub request: JevRequest,
    /// Checked right before every attempt; `false` means the source changed and the request
    /// is never sent (RF-ONLINE-10).
    pub fresh: Option<Freshness>,
}

/// Whether the sources behind a request are still the versions it was built from.
#[derive(Clone)]
pub struct Freshness(pub Arc<dyn Fn() -> bool + Send + Sync>);

impl std::fmt::Debug for Freshness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Freshness")
    }
}

/// The final outcome of a job or of one half of it: the request that was answered (a split
/// half has its own items and question ids) and its answers.
#[derive(Debug, Clone, PartialEq)]
pub struct JobResult {
    pub id: usize,
    pub request: JevRequest,
    pub result: Result<Vec<Option<f64>>, ClassifyError>,
}

/// Why the scheduler stopped before running every job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    RequestLimit,
    /// `401`/`403`: no other request can succeed.
    Auth,
    Cancelled,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Final outcomes, in completion order; match them to jobs by `id`.
    pub results: Vec<JobResult>,
    /// Jobs admitted but never answered: aborted in flight, or queued and never sent. Sorted.
    pub unfinished: Vec<usize>,
    /// Jobs never sent because their source changed first. Sorted.
    pub stale: Vec<usize>,
    /// Requests sent (started), retries included.
    pub requests: usize,
    pub retries: usize,
    pub splits: usize,
    /// `429` answers received.
    pub rate_limited: usize,
    pub stop: Option<Stop>,
}

impl Report {
    /// Absences in this run cannot be read as irrelevance (PRD §23.2).
    pub fn incomplete(&self) -> bool {
        self.stop.is_some()
            || !self.unfinished.is_empty()
            || !self.stale.is_empty()
            || self.results.iter().any(|r| r.result.is_err())
    }
}

/// A finished attempt and what it got.
type Done = (Attempt, Result<Vec<Option<f64>>, ClassifyError>);

/// A job and the attempt it is on.
struct Attempt {
    job: Job,
    number: u32,
    /// `429`s this job already waited out; it gets one retry after the cooldown.
    rate_limited: u32,
}

/// Wait after a `429` without a readable `Retry-After`.
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(1);
/// A longer `Retry-After` is not waited for: the answer goes out incomplete instead.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(30);

/// Attempts a batch gets before it is split or given up (v0.1 §11.9): evidence batches and
/// singletons two, multi-item admission batches one.
fn max_attempts(stage: SemanticStage, items: usize) -> u32 {
    match (stage, items) {
        (_, 1) | (SemanticStage::SourceSelection, _) => 2,
        (SemanticStage::FileAdmission, _) => 1,
    }
}

/// Worth retrying or splitting. A `429` waits for the shared cooldown instead.
fn retryable(e: &ClassifyError) -> bool {
    e.is_transient() && !matches!(e, ClassifyError::RateLimited { .. })
}

/// `job` cut into two halves with the same id, stage and freshness check.
fn halves(job: &Job) -> [Job; 2] {
    let items = &job.request.state.items;
    let (a, b) = items.split_at(items.len() / 2);
    let half = |part: &[super::request::StateItem]| Job {
        id: job.id,
        stage: job.stage,
        request: build(
            &job.request.model,
            &job.request.state.query,
            job.stage,
            part.to_vec(),
        ),
        fresh: job.fresh.clone(),
    };
    [half(a), half(b)]
}

pub struct Scheduler {
    classifier: Arc<dyn Classifier>,
    config: SchedulerConfig,
}

impl Scheduler {
    pub fn new(classifier: Arc<dyn Classifier>, config: SchedulerConfig) -> Self {
        Self { classifier, config }
    }

    /// The bounded queue a producer feeds; `send` waits while it is full.
    pub fn queue(&self) -> (mpsc::Sender<Job>, mpsc::Receiver<Job>) {
        mpsc::channel(self.config.queue.max(1))
    }

    pub async fn run(&self, mut jobs: mpsc::Receiver<Job>, cancel: CancellationToken) -> Report {
        let mut report = Report::default();
        let mut tasks: JoinSet<Done> = JoinSet::new();
        // Each running task, by its id, with the job it carries: a task that panics still frees
        // its slot, and its job is unfinished.
        let mut in_flight: Vec<(tokio::task::Id, usize)> = vec![];
        // Retries and split halves; they go before new jobs from the producer.
        let mut again: VecDeque<Attempt> = VecDeque::new();
        let mut admitting = true;
        // Shared by every sibling: nothing starts before it (v0.1 §11.8). Waiting happens here,
        // outside the tasks, so it holds no in-flight slot.
        let mut cooldown: Option<Instant> = None;
        let max = self.config.max_in_flight.max(1);
        loop {
            if report.stop.is_some() && report.stop != Some(Stop::RequestLimit) {
                break;
            }
            // The cancellation branch never goes idle, so the end is checked here.
            if !admitting && tasks.is_empty() && again.is_empty() {
                break;
            }
            let cooling = cooldown.filter(|t| *t > Instant::now());
            if in_flight.len() < max
                && cooling.is_none()
                && let Some(next) = again.pop_front()
            {
                self.launch(next, &mut tasks, &mut in_flight, &mut report, &mut again);
                if report.stop == Some(Stop::RequestLimit) {
                    admitting = false;
                    jobs.close();
                }
                continue;
            }
            let room = in_flight.len() < max && again.is_empty() && cooling.is_none();
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    report.stop = Some(Stop::Cancelled);
                    break;
                }
                Some(done) = tasks.join_next_with_id(), if !tasks.is_empty() => {
                    let task = match &done {
                        Ok((task, _)) => *task,
                        Err(e) => e.id(),
                    };
                    let job = in_flight
                        .iter()
                        .position(|(t, _)| *t == task)
                        .map(|i| in_flight.swap_remove(i).1);
                    match done {
                        Ok((_, (attempt, result))) => {
                            self.settle(attempt, result, &mut report, &mut again, &mut cooldown)
                        }
                        Err(_) => report.unfinished.extend(job),
                    }
                }
                _ = tokio::time::sleep_until(cooling.unwrap_or_else(Instant::now)), if cooling.is_some() => {}
                job = jobs.recv(), if admitting && room => match job {
                    None => admitting = false,
                    Some(job) => {
                        let first = Attempt { job, number: 1, rate_limited: 0 };
                        self.launch(first, &mut tasks, &mut in_flight, &mut report, &mut again);
                        if report.stop == Some(Stop::RequestLimit) {
                            admitting = false;
                            jobs.close();
                        }
                    }
                },
            }
        }
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        report
            .unfinished
            .extend(in_flight.iter().map(|(_, job)| *job));
        report.unfinished.extend(again.iter().map(|a| a.job.id));
        jobs.close();
        while let Ok(job) = jobs.try_recv() {
            report.unfinished.push(job.id);
        }
        report.unfinished.sort_unstable();
        report.unfinished.dedup();
        report.stale.sort_unstable();
        report
    }

    /// Starts an attempt unless its source changed or the request limit is spent.
    fn launch(
        &self,
        attempt: Attempt,
        tasks: &mut JoinSet<Done>,
        in_flight: &mut Vec<(tokio::task::Id, usize)>,
        report: &mut Report,
        again: &mut VecDeque<Attempt>,
    ) {
        if let Some(fresh) = &attempt.job.fresh
            && !(fresh.0)()
        {
            report.stale.push(attempt.job.id);
            return;
        }
        if report.requests == self.config.request_limit {
            report.unfinished.push(attempt.job.id);
            report.stop = Some(Stop::RequestLimit);
            report.unfinished.extend(again.drain(..).map(|a| a.job.id));
            return;
        }
        report.requests += 1;
        if attempt.number > 1 {
            report.retries += 1;
        }
        let job = attempt.job.id;
        let classifier = self.classifier.clone();
        let task = tasks.spawn(async move {
            let result = classifier.classify(&attempt.job.request).await;
            (attempt, result)
        });
        in_flight.push((task.id(), job));
    }

    /// Records a finished attempt, or queues its retry or its two halves.
    fn settle(
        &self,
        attempt: Attempt,
        result: Result<Vec<Option<f64>>, ClassifyError>,
        report: &mut Report,
        again: &mut VecDeque<Attempt>,
        cooldown: &mut Option<Instant>,
    ) {
        let Attempt {
            job,
            number,
            rate_limited,
        } = attempt;
        let items = job.request.state.items.len();
        match &result {
            Err(ClassifyError::Auth(_)) => report.stop = Some(Stop::Auth),
            Err(ClassifyError::RateLimited { retry_after }) => {
                report.rate_limited += 1;
                let wait = retry_after
                    .as_deref()
                    .and_then(|v| super::retry_after::parse(v, SystemTime::now()))
                    .unwrap_or(DEFAULT_COOLDOWN);
                if rate_limited == 0 && wait <= MAX_COOLDOWN {
                    let until = Instant::now() + wait;
                    *cooldown = Some(cooldown.map_or(until, |c| c.max(until)));
                    again.push_back(Attempt {
                        job,
                        number: number + 1,
                        rate_limited: 1,
                    });
                    return;
                }
            }
            Err(e) if retryable(e) && number < max_attempts(job.stage, items) => {
                again.push_back(Attempt {
                    job,
                    number: number + 1,
                    rate_limited,
                });
                return;
            }
            Err(e) if retryable(e) && items > 1 => {
                report.splits += 1;
                for half in halves(&job) {
                    again.push_back(Attempt {
                        job: half,
                        number: 1,
                        rate_limited: 0,
                    });
                }
                return;
            }
            _ => {}
        }
        report.results.push(JobResult {
            id: job.id,
            request: job.request,
            result,
        });
    }
}
