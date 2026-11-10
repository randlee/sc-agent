//! JobQueue and BoundedQueue unit tests: forced order, bounded waits, no clock reads
//! asserted, nothing sleeps (QA-R2-001, QA-R2-005).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::*;

/// Failsafe for waits that must finish; never a measurement.
const FAILSAFE: Duration = Duration::from_secs(60);

fn queue(capacity: usize) -> JobQueue<u32, ()> {
    JobQueue::new(capacity, Arc::new(Instant::now))
}

/// QA-R2-001 (a): a queue whose input was closed and drained finishes not cancelled.
#[test]
fn a_closed_drained_queue_finishes_not_cancelled() {
    let jobs = queue(4);
    assert!(jobs.submit(1).is_ok());
    jobs.close();
    assert!(matches!(jobs.take(), Taken::Run(1)));
    jobs.done();
    assert!(matches!(jobs.take(), Taken::Finished { cancelled: false }));
    assert!(!jobs.is_cancelled());
}

/// QA-R2-001 (a): a cancelled queue finishes cancelled, its waiting jobs taken as cancelled.
#[test]
fn a_cancelled_queue_finishes_cancelled() {
    let jobs = queue(4);
    assert!(jobs.submit(1).is_ok());
    jobs.cancel();
    assert!(matches!(jobs.take(), Taken::Cancelled(1)));
    jobs.done();
    assert!(matches!(jobs.take(), Taken::Finished { cancelled: true }));
    assert!(jobs.submit(2).is_err(), "a cancelled queue refuses submits");
}

/// QA-R2-005: a submit at capacity blocks (seen as a waiting submitter under the lock) until
/// a slot frees; then it completes.
#[test]
fn a_submit_at_capacity_blocks_until_a_slot_frees() {
    let jobs = Arc::new(queue(1));
    assert!(jobs.submit(1).is_ok());
    let (finished, done) = mpsc::channel();
    let submitter = {
        let jobs = Arc::clone(&jobs);
        thread::spawn(move || {
            let outcome = jobs.submit(2);
            let _ = finished.send(outcome.is_ok());
        })
    };
    assert_eq!(
        jobs.wait_for_waiting_submitters(1, FAILSAFE),
        1,
        "the submit blocked"
    );
    assert!(
        done.try_recv().is_err(),
        "it has not returned while blocked"
    );
    assert!(matches!(jobs.take(), Taken::Run(1)));
    jobs.done();
    assert_eq!(
        done.recv_timeout(FAILSAFE),
        Ok(true),
        "the freed slot let it finish"
    );
    submitter.join().expect("submitter");
    assert!(matches!(jobs.take(), Taken::Run(2)));
}

/// QA-R2-005: a push into a full result queue blocks (a waiting pusher) until the consumer
/// pops; then it completes.
#[test]
fn a_push_into_a_full_result_queue_blocks_until_a_pop() {
    let results = Arc::new(BoundedQueue::new(1, 1));
    assert!(results.push(1_u32).is_ok());
    let (finished, done) = mpsc::channel();
    let pusher = {
        let results = Arc::clone(&results);
        thread::spawn(move || {
            let outcome = results.push(2);
            let _ = finished.send(outcome.is_ok());
        })
    };
    assert_eq!(
        results.wait_for_waiting_pushers(1, FAILSAFE),
        1,
        "the push blocked"
    );
    assert!(
        done.try_recv().is_err(),
        "it has not returned while blocked"
    );
    let seen = results.generation();
    assert!(matches!(results.pop(seen), Popped::Item(1)));
    assert_eq!(
        done.recv_timeout(FAILSAFE),
        Ok(true),
        "the pop let it finish"
    );
    pusher.join().expect("pusher");
    assert!(matches!(results.pop(seen), Popped::Item(2)));
}

/// QA-R2-007: a held job does not fall due while the clock does not move; a cancel ends the
/// hold at once (the job is taken as cancelled without the clock advancing).
#[test]
fn a_cancel_ends_a_hold_without_the_clock_moving() {
    let start = Instant::now();
    let jobs: JobQueue<u32, ()> = JobQueue::new(4, Arc::new(move || start));
    jobs.hold(7, start + Duration::from_secs(3600));
    let jobs = Arc::new(jobs);
    let (taken, receiver) = mpsc::channel();
    let worker = {
        let jobs = Arc::clone(&jobs);
        thread::spawn(move || {
            let _ = taken.send(matches!(jobs.take(), Taken::Cancelled(7)));
        })
    };
    assert!(
        receiver.try_recv().is_err(),
        "the frozen clock never makes the hold due"
    );
    jobs.cancel();
    assert_eq!(receiver.recv_timeout(FAILSAFE), Ok(true));
    worker.join().expect("worker");
}
