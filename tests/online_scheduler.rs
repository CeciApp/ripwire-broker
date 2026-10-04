//! Seam 3: the scheduler with a scripted classifier and paused Tokio time (PRD §23.5).
mod common;

use common::classifier::FakeClassifier;
use ripwire_broker::online::SemanticStage;
use ripwire_broker::online::classifier::ClassifyError;
use ripwire_broker::online::request::{StateItem, build};
use ripwire_broker::online::scheduler::{Job, Scheduler, SchedulerConfig, Stop};
use std::sync::Arc;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn job(n: usize) -> Job {
    batch(n, SemanticStage::FileAdmission, &[n])
}

/// Job `id` asking `stage`'s question about items `j{k}` for each `k` in `items`.
fn batch(id: usize, stage: SemanticStage, items: &[usize]) -> Job {
    let items = items
        .iter()
        .map(|k| StateItem {
            id: format!("j{k}"),
            path: format!("src/f{k}.py"),
            text: "x".into(),
        })
        .collect();
    Job {
        id,
        stage,
        request: build("jev-1.13.0", "q", stage, items),
        fresh: None,
    }
}

/// Runs `jobs` to completion.
async fn run_jobs(
    fake: Arc<FakeClassifier>,
    cfg: SchedulerConfig,
    jobs: Vec<Job>,
) -> ripwire_broker::online::scheduler::Report {
    let scheduler = Scheduler::new(fake, cfg);
    let (tx, rx) = scheduler.queue();
    tokio::spawn(async move {
        for j in jobs {
            tx.send(j).await.unwrap();
        }
    });
    scheduler.run(rx, CancellationToken::new()).await
}

/// Item ids answered, in id order.
fn answered(report: &ripwire_broker::online::scheduler::Report) -> Vec<String> {
    let mut ids: Vec<String> = report
        .results
        .iter()
        .filter(|r| r.result.is_ok())
        .flat_map(|r| r.request.state.items.iter().map(|i| i.id.clone()))
        .collect();
    ids.sort();
    ids
}

fn config(max_in_flight: usize, request_limit: usize) -> SchedulerConfig {
    SchedulerConfig {
        max_in_flight,
        request_limit,
        queue: 8,
    }
}

/// Feeds `n` jobs from a separate producer task, like the coordinator will.
fn produce(
    scheduler: &Scheduler,
    n: usize,
) -> (
    tokio::sync::mpsc::Receiver<Job>,
    tokio::task::JoinHandle<usize>,
) {
    let (tx, rx) = scheduler.queue();
    let producer = tokio::spawn(async move {
        let mut sent = 0;
        for i in 0..n {
            if tx.send(job(i)).await.is_err() {
                break;
            }
            sent += 1;
        }
        sent
    });
    (rx, producer)
}

#[tokio::test(start_paused = true)]
async fn no_more_than_l_requests_are_in_flight() {
    let mut fake = FakeClassifier::new();
    for i in 0..100 {
        fake = fake.delay(&format!("j{i}"), Duration::from_millis(10 + (i % 7) as u64));
    }
    let fake = Arc::new(fake);
    let scheduler = Scheduler::new(fake.clone(), config(4, 1000));
    let (rx, _producer) = produce(&scheduler, 100);

    let report = scheduler.run(rx, CancellationToken::new()).await;

    assert_eq!(
        fake.counters.max_in_flight.load(SeqCst),
        4,
        "CA-ONLINE-04 with L = 4"
    );
    assert_eq!(report.results.len(), 100);
    assert_eq!(report.requests, 100);
    assert_eq!(report.stop, None);
}

#[tokio::test(start_paused = true)]
async fn a_slow_provider_bounds_memory_and_blocks_producers() {
    let (fake, gate) = FakeClassifier::new().held();
    let fake = Arc::new(fake);
    let scheduler = Arc::new(Scheduler::new(fake.clone(), config(4, 1000)));
    let (rx, producer) = produce(&scheduler, 100);
    let run = tokio::spawn({
        let s = scheduler.clone();
        async move { s.run(rx, CancellationToken::new()).await }
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(fake.calls(), 4, "only L requests start");
    assert!(
        !producer.is_finished(),
        "the producer waits on the full queue (CA-ONLINE-05)"
    );

    gate.add_permits(100);
    assert_eq!(producer.await.unwrap(), 100);
    assert_eq!(run.await.unwrap().results.len(), 100);
}

#[tokio::test(start_paused = true)]
async fn out_of_order_answers_keep_their_ids() {
    let mut fake = FakeClassifier::new();
    for i in 0..8 {
        fake = fake
            .delay(&format!("j{i}"), Duration::from_millis(80 - 10 * i as u64))
            .answer(&format!("j{i}"), i as f64 / 10.0);
    }
    let scheduler = Scheduler::new(Arc::new(fake), config(8, 1000));
    let (rx, _p) = produce(&scheduler, 8);

    let report = scheduler.run(rx, CancellationToken::new()).await;

    let order: Vec<usize> = report.results.iter().map(|r| r.id).collect();
    assert_eq!(
        order,
        (0..8).rev().collect::<Vec<_>>(),
        "finished in reverse"
    );
    for r in &report.results {
        assert_eq!(r.result, Ok(vec![Some(r.id as f64 / 10.0)]), "job {}", r.id);
    }
}

#[tokio::test(start_paused = true)]
async fn an_auth_failure_cancels_siblings() {
    // j0 fails after 5 ms; its three siblings are held in flight.
    let (fake, _gate) = FakeClassifier::new()
        .fail("j0", ClassifyError::Auth(401))
        .delay("j0", Duration::from_millis(5))
        .held();
    let fake = Arc::new(fake);
    let scheduler = Scheduler::new(fake.clone(), config(4, 1000));
    let (rx, producer) = produce(&scheduler, 20);

    let report = scheduler.run(rx, CancellationToken::new()).await;

    assert_eq!(report.stop, Some(Stop::Auth));
    assert_eq!(fake.calls(), 4, "no request starts after the auth failure");
    assert_eq!(fake.counters.dropped.load(SeqCst), 3, "siblings aborted");
    assert_eq!(report.results.len(), 1);
    assert!(report.unfinished.contains(&1) && report.unfinished.contains(&3));
    assert!(producer.await.unwrap() < 20, "admission closed");
}

#[tokio::test(start_paused = true)]
async fn the_request_limit_stops_admission_and_marks_incomplete() {
    let fake = Arc::new(FakeClassifier::new());
    let scheduler = Scheduler::new(fake.clone(), config(4, 24));
    let (rx, producer) = produce(&scheduler, 100);

    let report = scheduler.run(rx, CancellationToken::new()).await;

    assert_eq!(fake.calls(), 24);
    assert_eq!(report.requests, 24);
    assert_eq!(report.results.len(), 24);
    assert_eq!(report.stop, Some(Stop::RequestLimit));
    assert!(
        !report.unfinished.is_empty(),
        "admitted work that was never sent is reported"
    );
    assert!(report.incomplete());
    let sent = producer.await.unwrap();
    assert!(
        sent <= 24 + 1 + 8,
        "the producer stops once the queue closes: {sent}"
    );
    assert_eq!(
        report.unfinished.len(),
        sent - 24,
        "every admitted job is answered or reported"
    );
}

#[tokio::test(start_paused = true)]
async fn cancellation_aborts_requests_in_flight_and_closes_the_queue() {
    let (fake, _gate) = FakeClassifier::new().held();
    let fake = Arc::new(fake);
    let scheduler = Arc::new(Scheduler::new(fake.clone(), config(4, 1000)));
    let (rx, producer) = produce(&scheduler, 50);
    let cancel = CancellationToken::new();
    let run = tokio::spawn({
        let (s, c) = (scheduler.clone(), cancel.clone());
        async move { s.run(rx, c).await }
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    cancel.cancel();
    let report = run.await.unwrap();

    assert_eq!(report.stop, Some(Stop::Cancelled));
    assert_eq!(fake.counters.dropped.load(SeqCst), 4);
    assert_eq!(fake.calls(), 4);
    assert!(producer.await.unwrap() < 50);
}

#[tokio::test(start_paused = true)]
async fn a_job_whose_source_changed_is_never_sent() {
    use ripwire_broker::online::scheduler::Freshness;
    let fake = Arc::new(FakeClassifier::new());
    let scheduler = Scheduler::new(fake.clone(), config(4, 1000));
    let (tx, rx) = scheduler.queue();
    let mut stale = job(1);
    stale.fresh = Some(Freshness(Arc::new(|| false)));
    let mut fresh = job(2);
    fresh.fresh = Some(Freshness(Arc::new(|| true)));
    tokio::spawn(async move {
        for j in [job(0), stale, fresh] {
            tx.send(j).await.unwrap();
        }
    });

    let report = scheduler.run(rx, CancellationToken::new()).await;

    assert_eq!(report.stale, vec![1]);
    assert_eq!(report.requests, 2, "a stale job is not a request");
    assert_eq!(fake.calls(), 2);
    assert!(report.incomplete());
}

// --- S5.2: retry and split (v0.1 §11.9) ---

const DOWN: ClassifyError = ClassifyError::Server(503);

#[tokio::test(start_paused = true)]
async fn retries_follow_the_stage_policy() {
    // A source-selection batch is retried once as it is.
    let fake = Arc::new(FakeClassifier::new().fail_times("j0", DOWN, 1));
    let r = run_jobs(
        fake.clone(),
        config(4, 1000),
        vec![batch(0, SemanticStage::SourceSelection, &[0, 1])],
    )
    .await;
    assert_eq!((fake.calls(), r.retries, r.splits), (2, 1, 0));
    assert_eq!(answered(&r), vec!["j0", "j1"]);

    // A multi-item admission batch gets one attempt, then splits in half.
    let fake = Arc::new(FakeClassifier::new().fail_times("j0", DOWN, 1));
    let r = run_jobs(
        fake.clone(),
        config(4, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0, 1])],
    )
    .await;
    assert_eq!((fake.calls(), r.retries, r.splits), (3, 0, 1));
    assert_eq!(answered(&r), vec!["j0", "j1"]);

    // A singleton is retried once.
    let fake = Arc::new(FakeClassifier::new().fail_times("j0", DOWN, 1));
    let r = run_jobs(
        fake.clone(),
        config(4, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0])],
    )
    .await;
    assert_eq!((fake.calls(), r.retries), (2, 1));
    assert_eq!(answered(&r), vec!["j0"]);

    // ...and only once.
    let fake = Arc::new(FakeClassifier::new().fail("j0", DOWN));
    let r = run_jobs(
        fake.clone(),
        config(4, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0])],
    )
    .await;
    assert_eq!(fake.calls(), 2);
    assert_eq!(r.results.len(), 1);
    assert_eq!(r.results[0].result, Err(DOWN));
    assert!(r.incomplete());
}

#[tokio::test(start_paused = true)]
async fn non_transient_errors_do_not_retry_or_split() {
    for e in [
        ClassifyError::Rejected(409),
        ClassifyError::Invalid(ripwire_broker::online::response::InvalidResponse::Malformed),
        ClassifyError::TooLarge,
    ] {
        let fake = Arc::new(FakeClassifier::new().fail("j0", e.clone()));
        let r = run_jobs(
            fake.clone(),
            config(4, 1000),
            vec![batch(0, SemanticStage::SourceSelection, &[0, 1, 2])],
        )
        .await;
        assert_eq!((fake.calls(), r.retries, r.splits), (1, 0, 0), "{e:?}");
        assert_eq!(r.results[0].result, Err(e));
    }
}

#[tokio::test(start_paused = true)]
async fn a_persistent_failure_is_bounded_and_never_restarts_the_search() {
    // Every request of [j0, j1, j2] fails. Admission: the batch once (1), split into [j0] and
    // [j1, j2]; [j0] twice (2); [j1, j2] once (1), split into [j1] and [j2], twice each (4).
    let fake = Arc::new(
        FakeClassifier::new()
            .fail("j0", DOWN)
            .fail("j1", DOWN)
            .fail("j2", DOWN),
    );
    let r = run_jobs(
        fake.clone(),
        config(4, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0, 1, 2])],
    )
    .await;

    assert_eq!(fake.calls(), 8);
    assert_eq!((r.retries, r.splits), (3, 2));
    assert_eq!(r.requests, 8);
    assert!(answered(&r).is_empty());
    let failed: Vec<usize> = r
        .results
        .iter()
        .map(|x| x.request.state.items.len())
        .collect();
    assert_eq!(
        failed,
        vec![1, 1, 1],
        "one final failure per item, each unknown"
    );
}

#[tokio::test(start_paused = true)]
async fn retries_count_against_the_request_limit() {
    let fake = Arc::new(FakeClassifier::new().fail("j0", DOWN));
    let r = run_jobs(
        fake.clone(),
        config(4, 1),
        vec![batch(0, SemanticStage::FileAdmission, &[0])],
    )
    .await;

    assert_eq!(fake.calls(), 1);
    assert_eq!(r.stop, Some(Stop::RequestLimit));
    assert_eq!(r.unfinished, vec![0], "the retry was never sent");
}

#[tokio::test(start_paused = true)]
async fn a_retry_is_not_sent_over_changed_source() {
    use ripwire_broker::online::scheduler::Freshness;
    use std::sync::atomic::AtomicBool;
    let changed = Arc::new(AtomicBool::new(false));
    let flag = changed.clone();
    let fake = Arc::new(
        FakeClassifier::new()
            .fail("j0", DOWN)
            .on_call(move |_| flag.store(true, SeqCst)),
    );
    let mut j = batch(0, SemanticStage::FileAdmission, &[0]);
    let seen = changed.clone();
    j.fresh = Some(Freshness(Arc::new(move || !seen.load(SeqCst))));

    let r = run_jobs(fake.clone(), config(4, 1000), vec![j]).await;

    assert_eq!(
        fake.calls(),
        1,
        "the source changed during the first attempt"
    );
    assert_eq!(r.stale, vec![0]);
}

#[tokio::test(start_paused = true)]
async fn split_never_deadlocks() {
    let mut fake = FakeClassifier::new();
    for k in 0..64 {
        fake = fake.fail_times(&format!("j{k}"), DOWN, 1);
    }
    let fake = Arc::new(fake);
    let jobs = (0..8)
        .map(|b| {
            batch(
                b,
                SemanticStage::FileAdmission,
                &(b * 8..b * 8 + 8).collect::<Vec<_>>(),
            )
        })
        .collect();
    let small = SchedulerConfig {
        max_in_flight: 1,
        request_limit: 1000,
        queue: 1,
    };

    let r = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        run_jobs(fake, small, jobs),
    )
    .await
    .expect("no deadlock");

    assert_eq!(answered(&r).len(), 64);
}

// --- S5.3: 429 and the shared cooldown (CA-ONLINE-09) ---

fn limited(secs: &str) -> ClassifyError {
    ClassifyError::RateLimited {
        retry_after: Some(secs.into()),
    }
}

#[tokio::test(start_paused = true)]
async fn a_429_sets_a_shared_cooldown_for_all_siblings() {
    let t0 = tokio::time::Instant::now();
    let fake = Arc::new(
        FakeClassifier::new()
            .fail_times("j0", limited("5"), 1)
            .delay("j1", std::time::Duration::from_millis(10)),
    );
    let probe = fake.clone();
    let watcher = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        probe.counters.in_flight.load(SeqCst)
    });
    let jobs = (0..6)
        .map(|n| batch(n, SemanticStage::FileAdmission, &[n]))
        .collect();

    let r = run_jobs(fake.clone(), config(2, 1000), jobs).await;

    assert_eq!(
        watcher.await.unwrap(),
        0,
        "waiting for the cooldown holds no slot"
    );
    let started = fake.started.lock().unwrap().clone();
    let late: Vec<&(String, tokio::time::Instant)> = started.iter().skip(2).collect();
    assert!(
        late.iter()
            .all(|(_, at)| *at >= t0 + std::time::Duration::from_secs(5)),
        "no request starts before the Retry-After: {started:?}"
    );
    assert_eq!(
        started.iter().filter(|(id, _)| id == "j0").count(),
        2,
        "j0 gets a second attempt"
    );
    assert_eq!(answered(&r).len(), 6);
    assert_eq!(r.rate_limited, 1);
    assert_eq!(r.splits, 0, "a 429 is not the batch's fault");
}

#[tokio::test(start_paused = true)]
async fn the_cooldown_keeps_the_longest_retry_after() {
    let t0 = tokio::time::Instant::now();
    let fake = Arc::new(
        FakeClassifier::new()
            .fail_times("j0", limited("3"), 1)
            .fail_times("j1", limited("8"), 1),
    );
    let jobs = (0..4)
        .map(|n| batch(n, SemanticStage::FileAdmission, &[n]))
        .collect();

    let r = run_jobs(fake.clone(), config(2, 1000), jobs).await;

    let started = fake.started.lock().unwrap().clone();
    assert!(
        started
            .iter()
            .skip(2)
            .all(|(_, at)| *at >= t0 + std::time::Duration::from_secs(8)),
        "{started:?}"
    );
    assert_eq!(answered(&r).len(), 4);
}

#[tokio::test(start_paused = true)]
async fn cancellation_works_during_cooldown() {
    let t0 = tokio::time::Instant::now();
    // j1 answers after j0's 429, so the cooldown is already set when a slot frees up.
    let fake = Arc::new(
        FakeClassifier::new()
            .fail("j0", limited("20"))
            .delay("j1", std::time::Duration::from_millis(100)),
    );
    let scheduler = Arc::new(Scheduler::new(fake.clone(), config(2, 1000)));
    let (rx, _producer) = produce(&scheduler, 5);
    let cancel = CancellationToken::new();
    let run = tokio::spawn({
        let (s, c) = (scheduler.clone(), cancel.clone());
        async move { s.run(rx, c).await }
    });

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let at_cancel = fake.calls();
    cancel.cancel();
    let r = run.await.unwrap();
    let returned = tokio::time::Instant::now();
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    assert!(
        returned < t0 + std::time::Duration::from_secs(2),
        "no waiting for the cooldown"
    );
    assert_eq!(r.stop, Some(Stop::Cancelled));
    assert_eq!(
        at_cancel, 2,
        "only j0 and j1: nothing starts during the cooldown"
    );
    assert_eq!(fake.calls(), at_cancel, "nor after it, once cancelled");
}

#[tokio::test(start_paused = true)]
async fn a_second_429_or_an_excessive_retry_after_is_final() {
    let fake = Arc::new(FakeClassifier::new().fail("j0", limited("1")));
    let r = run_jobs(
        fake.clone(),
        config(2, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0])],
    )
    .await;
    assert_eq!(fake.calls(), 2, "one retry after the cooldown, then final");
    assert!(matches!(
        r.results[0].result,
        Err(ClassifyError::RateLimited { .. })
    ));

    let t0 = tokio::time::Instant::now();
    let fake = Arc::new(FakeClassifier::new().fail("j0", limited("3600")));
    let r = run_jobs(
        fake.clone(),
        config(2, 1000),
        vec![batch(0, SemanticStage::FileAdmission, &[0])],
    )
    .await;
    assert_eq!(fake.calls(), 1, "an hour is not worth waiting for");
    assert!(tokio::time::Instant::now() < t0 + std::time::Duration::from_secs(1));
    assert!(r.incomplete());
}

// --- S5.4: cancellation reaches every task (RF-ONLINE-14) ---

#[tokio::test(start_paused = true)]
async fn dropping_the_run_aborts_requests_in_flight_and_queued_retries() {
    // How an MCP cancel arrives: the tool call's future is dropped (RF-14). j0 gets a 429, so
    // its retry waits in the scheduler's queue for the cooldown; j1..j3 are held in flight.
    let (fake, _gate) = FakeClassifier::new().fail("j0", limited("5")).held();
    let fake = Arc::new(fake);
    let scheduler = Arc::new(Scheduler::new(fake.clone(), config(4, 1000)));
    let (rx, producer) = produce(&scheduler, 50);
    let run = tokio::spawn({
        let s = scheduler.clone();
        async move { s.run(rx, CancellationToken::new()).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(fake.calls(), 4);

    run.abort();
    let _ = run.await;
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    assert_eq!(
        fake.counters.dropped.load(SeqCst),
        3,
        "every request in flight was dropped"
    );
    assert_eq!(
        fake.calls(),
        4,
        "the queued retry never went out, even past the cooldown"
    );
    assert!(
        producer.await.unwrap() < 50,
        "the queue closed with the run"
    );
}

// --- D-113: P1.3, the scheduler's limits as properties ---

/// Every invariant the scheduler owes its caller, over generated job mixes and configurations:
/// the in-flight ceiling, the request budget counting every attempt, every admitted job accounted
/// for exactly once, and — the one that costs most to get right — an answer never migrating to
/// another job's question when responses come back out of order.
///
/// `proptest!` cannot wrap an async body, so the runner is driven directly and each case blocks on
/// its own scheduler under paused time. Few cases, because each one runs a scheduler to
/// completion; the test finishing at all is the no-deadlock assertion.
#[test]
fn the_scheduler_keeps_its_limits_and_never_mixes_up_an_answer() {
    use proptest::prelude::*;
    use proptest::test_runner::{Config as PropConfig, TestRunner};

    /// Item `j{k}` is scripted with its own probability, so a misrouted answer is visible rather
    /// than plausible: every item would otherwise share one value and any mix-up would pass.
    fn probability_of(k: usize) -> f64 {
        0.1 + (k % 800) as f64 / 1000.0
    }

    TestRunner::new(PropConfig {
        cases: 40,
        failure_persistence: None,
        ..PropConfig::default()
    })
    .run(
        &(
            // Items per job, so jobs differ in size.
            prop::collection::vec(1usize..4, 1..10),
            1usize..5, // max_in_flight
            // Weighted toward a budget that actually runs out, because that is the only region
            // where the unaccounted-job clause has anything to check. Measured over the generated
            // cases: 8 jobs with a limit of 1 leaves 6 jobs never admitted, while a wide uniform
            // draw mostly finishes everything and the clause never fires (D-113).
            prop_oneof![
                3 => 1usize..4,
                1 => 4usize..25,
            ],
            1usize..6, // queue
            // Which jobs fail once, to put retries in the accounting.
            prop::collection::vec(any::<bool>(), 0..10),
            // Which jobs answer late, to force completion out of submission order.
            prop::collection::vec(0u64..40, 0..10),
        ),
        |(shapes, max_in_flight, request_limit, queue, fails, delays)| {
            // A fresh runtime per case, with time already paused: `tokio::time::pause()` panics
            // if called twice on one runtime, and a shared runtime would also keep each case's
            // producer task alive into the next one.
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .start_paused(true)
                .build()
                .unwrap();
            rt.block_on(async {
                let mut fake = FakeClassifier::new();
                let mut jobs = Vec::new();
                let mut items_of: Vec<Vec<usize>> = Vec::new();
                let mut next = 0usize;
                for (id, n) in shapes.iter().enumerate() {
                    let ks: Vec<usize> = (next..next + n).collect();
                    next += n;
                    for k in &ks {
                        fake = fake.answer(&format!("j{k}"), probability_of(*k));
                    }
                    if fails.get(id).copied().unwrap_or(false) {
                        fake =
                            fake.fail_times(&format!("j{}", ks[0]), ClassifyError::Server(503), 1);
                    }
                    if let Some(d) = delays.get(id).copied().filter(|d| *d > 0) {
                        fake = fake.delay(&format!("j{}", ks[0]), Duration::from_millis(d));
                    }
                    jobs.push(batch(id, SemanticStage::FileAdmission, &ks));
                    items_of.push(ks);
                }
                let fake = Arc::new(fake);
                let cfg = SchedulerConfig {
                    max_in_flight,
                    request_limit,
                    queue,
                };

                let report = run_jobs(fake.clone(), cfg, jobs).await;

                // RF-ONLINE-08: the ceiling is on concurrent requests, observed by the classifier
                // itself rather than inferred from the report.
                let peak = fake.counters.max_in_flight.load(SeqCst);
                prop_assert!(
                    peak <= max_in_flight,
                    "{peak} requests in flight against a ceiling of {max_in_flight}"
                );
                // Every attempt counts, retries included — and the count is checked against what
                // the classifier actually saw, not only against the ceiling. A `<=` assertion
                // alone cannot see **under**-counting: making the counter stop incrementing keeps
                // it under any limit forever, and the limit then never stops anything (D-113).
                // With `fresh: None` every admitted job is really sent, so the two must be equal.
                prop_assert_eq!(
                    report.requests,
                    fake.calls(),
                    "the report counts {} requests, the classifier saw {}",
                    report.requests,
                    fake.calls()
                );
                prop_assert!(
                    report.requests <= request_limit,
                    "{} requests sent against a limit of {request_limit}",
                    report.requests
                );
                if report.stop == Some(Stop::RequestLimit) {
                    prop_assert_eq!(
                        report.requests,
                        request_limit,
                        "stopped for the limit without reaching it"
                    );
                }

                // Accounting. `unfinished` holds jobs that were **admitted** and never answered,
                // so a job still in the queue when the scheduler stops at its request limit is in
                // none of the three lists — it was never admitted. Asserting that every submitted
                // job appears somewhere was my property claiming a promise the report never made
                // (D-113).
                //
                // What must hold is the safety direction: a job missing from the accounting is
                // only acceptable while the report **says it is incomplete**, because absence must
                // never be readable as an answer (PRD §23.2).
                let mut seen: std::collections::HashSet<usize> =
                    report.results.iter().map(|r| r.id).collect();
                seen.extend(report.unfinished.iter().copied());
                seen.extend(report.stale.iter().copied());
                let missing: Vec<usize> = (0..shapes.len()).filter(|i| !seen.contains(i)).collect();
                if !missing.is_empty() {
                    prop_assert!(
                        report.incomplete(),
                        "jobs {missing:?} are unaccounted for and the report calls itself complete"
                    );
                }
                // A complete run leaves nothing out.
                if !report.incomplete() {
                    prop_assert!(
                        missing.is_empty(),
                        "a complete report is missing jobs {missing:?}"
                    );
                }
                // And no id was invented.
                for id in &seen {
                    prop_assert!(
                        *id < shapes.len(),
                        "the report names job {id}, never submitted"
                    );
                }
                prop_assert!(
                    report.unfinished.windows(2).all(|w| w[0] < w[1]),
                    "unfinished is not sorted"
                );

                // The one that matters most: an answer belongs to the item it was asked about,
                // whatever order the responses arrived in.
                for r in &report.results {
                    let Ok(answers) = &r.result else { continue };
                    prop_assert_eq!(
                        answers.len(),
                        r.request.state.items.len(),
                        "job {}: {} answers for {} items",
                        r.id,
                        answers.len(),
                        r.request.state.items.len()
                    );
                    for (item, got) in r.request.state.items.iter().zip(answers) {
                        let k: usize = item.id.trim_start_matches('j').parse().unwrap();
                        prop_assert_eq!(
                            *got,
                            Some(probability_of(k)),
                            "job {}: item {} came back with another item's answer",
                            r.id,
                            item.id
                        );
                        // And a split half only ever carries items of its own job.
                        prop_assert!(
                            items_of[r.id].contains(&k),
                            "job {} carries item {}, which belongs to another job",
                            r.id,
                            item.id
                        );
                    }
                }
                Ok(())
            })
        },
    )
    .unwrap();
}

// --- jev-mem T2.10: one ceiling for the process (PRD jev-mem §4, §8.2) ---

use ripwire_broker::online::classifier::{Classifier, MemoryClassifier, Shared};
use ripwire_broker::online::request::{JevQuestion, JevRequest, StateRequest};
use ripwire_broker::online::response::Decision;
use std::sync::atomic::AtomicUsize;

/// Answers after a pause, counting how many calls are inside at once, of both kinds.
#[derive(Default)]
struct Probe {
    now: AtomicUsize,
    peak: AtomicUsize,
    calls: AtomicUsize,
}

impl Probe {
    async fn enter(&self) {
        let n = self.now.fetch_add(1, SeqCst) + 1;
        self.peak.fetch_max(n, SeqCst);
        self.calls.fetch_add(1, SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        self.now.fetch_sub(1, SeqCst);
    }
}

#[async_trait::async_trait]
impl Classifier for Probe {
    fn model(&self) -> &str {
        "jev-1.13.0"
    }
    async fn classify(&self, req: &JevRequest) -> Result<Vec<Option<f64>>, ClassifyError> {
        self.enter().await;
        Ok(vec![Some(0.5); req.questions.0.len()])
    }
}

#[async_trait::async_trait]
impl MemoryClassifier for Probe {
    async fn decide(&self, req: &StateRequest) -> Result<Vec<Decision>, ClassifyError> {
        self.enter().await;
        Ok(vec![
            Decision::Noul { probability: 0.5 };
            req.questions.0.len()
        ])
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn memory_and_discovery_share_four_requests_and_one_client() {
    let probe = Arc::new(Probe::default());
    let shared = Arc::new(Shared::new(probe.clone(), 4));
    let discovery: Arc<dyn Classifier> = shared.clone();
    let memory: Arc<dyn MemoryClassifier> = shared.clone();
    assert_eq!(
        discovery.model(),
        "jev-1.13.0",
        "the same client behind both"
    );

    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..8 {
        let d = discovery.clone();
        tasks.spawn(async move {
            d.classify(&build(
                "jev-1.13.0",
                "q",
                SemanticStage::FileAdmission,
                vec![StateItem {
                    id: format!("i{n}"),
                    path: "a.rs".into(),
                    text: "x".into(),
                }],
            ))
            .await
            .map(|_| ())
        });
        let m = memory.clone();
        tasks.spawn(async move {
            let req = StateRequest::new(
                "jev-1.13.0",
                serde_json::json!({}),
                vec![JevQuestion::noul("q")],
            );
            m.decide(&req).await.map(|_| ())
        });
    }
    while let Some(done) = tasks.join_next().await {
        done.unwrap().unwrap();
    }
    assert_eq!(probe.calls.load(SeqCst), 16);
    assert!(
        probe.peak.load(SeqCst) <= 4,
        "peak {}",
        probe.peak.load(SeqCst)
    );
    assert!(probe.peak.load(SeqCst) >= 2, "they did run together");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_memory_discovery_keeps_its_client_and_its_per_query_ceiling() {
    use ripwire_broker::online::classifier::for_process;
    let probe = Arc::new(Probe::default());
    let (discovery, memory) = for_process(probe.clone(), 4, false);
    assert!(memory.is_none(), "no memory, no memory client");
    assert_eq!(
        Arc::as_ptr(&discovery) as *const () as usize,
        Arc::as_ptr(&probe) as *const () as usize,
        "--online alone gets the client as before (PRD jev-mem §4)"
    );
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..8 {
        let d = discovery.clone();
        tasks.spawn(async move {
            let item = StateItem {
                id: format!("i{n}"),
                path: "a.rs".into(),
                text: "x".into(),
            };
            d.classify(&build(
                "jev-1.13.0",
                "q",
                SemanticStage::FileAdmission,
                vec![item],
            ))
            .await
            .map(|_| ())
        });
    }
    while let Some(done) = tasks.join_next().await {
        done.unwrap().unwrap();
    }
    assert!(
        probe.peak.load(SeqCst) > 4,
        "no process-wide ceiling: peak {}",
        probe.peak.load(SeqCst)
    );

    let (_, memory) = for_process(Arc::new(Probe::default()), 4, true);
    assert!(memory.is_some(), "with memory, one shared client for both");
}

/// A classifier whose every call panics (a poisoned lock inside a client, say).
struct Panicking;

#[async_trait::async_trait]
impl ripwire_broker::online::classifier::Classifier for Panicking {
    fn model(&self) -> &str {
        "jev-1.13.0"
    }

    async fn classify(
        &self,
        _: &ripwire_broker::online::request::JevRequest,
    ) -> Result<Vec<Option<f64>>, ClassifyError> {
        panic!("the client's lock was poisoned")
    }
}

#[tokio::test(start_paused = true)]
async fn a_classifier_task_that_panics_frees_its_slot() {
    // One slot: a panicked task that kept it would stall every other job until the deadline.
    let scheduler = Scheduler::new(Arc::new(Panicking), config(1, 24));
    let (tx, rx) = scheduler.queue();
    tokio::spawn(async move {
        for j in [job(1), job(2)] {
            tx.send(j).await.unwrap();
        }
    });
    let report = tokio::time::timeout(
        Duration::from_secs(60),
        scheduler.run(rx, CancellationToken::new()),
    )
    .await
    .expect("the run ends without waiting for a cancellation");
    assert_eq!(report.requests, 2, "the second job was sent too");
    assert_eq!(report.unfinished, vec![1, 2], "neither got an answer");
    assert_ne!(report.stop, Some(Stop::Cancelled));
}
