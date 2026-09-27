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
    let fake = Arc::new(FakeClassifier::new().fail("j0", limited("20")));
    let scheduler = Arc::new(Scheduler::new(fake.clone(), config(2, 1000)));
    let (rx, _producer) = produce(&scheduler, 5);
    let cancel = CancellationToken::new();
    let run = tokio::spawn({
        let (s, c) = (scheduler.clone(), cancel.clone());
        async move { s.run(rx, c).await }
    });

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    cancel.cancel();
    let r = run.await.unwrap();

    assert!(
        tokio::time::Instant::now() < t0 + std::time::Duration::from_secs(2),
        "no waiting for the cooldown"
    );
    assert_eq!(r.stop, Some(Stop::Cancelled));
    assert!(fake.calls() <= 2, "nothing starts during the cooldown");
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
