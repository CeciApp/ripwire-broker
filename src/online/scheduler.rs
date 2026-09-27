//! `JevScheduler` (PRD §23.5): runs classifier requests from a bounded queue with at most
//! `max_in_flight` in flight and at most `request_limit` per MCP call. It owns every task in a
//! `JoinSet`, so an auth failure or a cancellation aborts the siblings (dropping their HTTP
//! requests) and closes the queue, which stops the producer. It never interprets
//! probabilities, and it never restarts a search.

use super::classifier::{Classifier, ClassifyError};
use super::request::JevRequest;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerConfig {
    /// RF-ONLINE-08: never more requests in flight per query.
    pub max_in_flight: usize,
    /// Requests one MCP call may send.
    pub request_limit: usize,
    /// Capacity of the queue between the producer and the scheduler (RF-ONLINE-09).
    pub queue: usize,
}

/// One request, with the producer's id for matching the answer back.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    pub request: JevRequest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JobResult {
    pub id: usize,
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
    /// In completion order; match them to jobs by `id`.
    pub results: Vec<JobResult>,
    /// Jobs admitted but never answered: aborted in flight, or queued and never sent. Sorted.
    pub unfinished: Vec<usize>,
    /// Requests sent (started).
    pub requests: usize,
    pub stop: Option<Stop>,
}

impl Report {
    /// Absences in this run cannot be read as irrelevance (PRD §23.2).
    pub fn incomplete(&self) -> bool {
        self.stop.is_some()
            || !self.unfinished.is_empty()
            || self.results.iter().any(|r| r.result.is_err())
    }
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
        let mut tasks: JoinSet<JobResult> = JoinSet::new();
        let mut in_flight: Vec<usize> = vec![];
        let mut admitting = true;
        loop {
            // The cancellation branch never goes idle, so the end is checked here.
            if !admitting && tasks.is_empty() {
                break;
            }
            let room = in_flight.len() < self.config.max_in_flight.max(1);
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    report.stop = Some(Stop::Cancelled);
                    break;
                }
                Some(done) = tasks.join_next(), if !tasks.is_empty() => {
                    let Ok(done) = done else { continue };
                    in_flight.retain(|i| *i != done.id);
                    let auth = matches!(done.result, Err(ClassifyError::Auth(_)));
                    report.results.push(done);
                    if auth {
                        report.stop = Some(Stop::Auth);
                        break;
                    }
                }
                job = jobs.recv(), if admitting && room => match job {
                    None => admitting = false,
                    Some(job) if report.requests == self.config.request_limit => {
                        report.unfinished.push(job.id);
                        report.stop = Some(Stop::RequestLimit);
                        admitting = false;
                        jobs.close();
                    }
                    Some(Job { id, request }) => {
                        report.requests += 1;
                        in_flight.push(id);
                        let classifier = self.classifier.clone();
                        tasks.spawn(async move {
                            let result = classifier.classify(&request).await;
                            JobResult { id, result }
                        });
                    }
                },
            }
        }
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        report.unfinished.extend(in_flight);
        jobs.close();
        while let Ok(job) = jobs.try_recv() {
            report.unfinished.push(job.id);
        }
        report.unfinished.sort_unstable();
        report
    }
}
