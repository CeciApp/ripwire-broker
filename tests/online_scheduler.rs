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
    let item = StateItem {
        id: format!("j{n}"),
        path: format!("src/f{n}.py"),
        text: "x".into(),
    };
    Job {
        id: n,
        request: build("jev-1.13.0", "q", SemanticStage::FileAdmission, vec![item]),
    }
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
