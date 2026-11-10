//! JobQueue and BoundedQueue unit tests: forced order, bounded waits, no clock reads
//! asserted, nothing sleeps (QA-R2-001, QA-R2-005).

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use sc_agent_testkit::{HANG_GUARD, join_within, recv_within};
use sc_agent_types::testing::ManualClock;

use super::*;

fn queue(capacity: usize) -> JobQueue<u32, ()> {
    JobQueue::new(capacity, ManualClock::new().clock())
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
        jobs.wait_for_waiting_submitters(1, HANG_GUARD),
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
        done.recv_timeout(HANG_GUARD),
        Ok(true),
        "the freed slot let it finish"
    );
    join_within(submitter, HANG_GUARD).expect("submitter");
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
        results.wait_for_waiting_pushers(1, HANG_GUARD),
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
        done.recv_timeout(HANG_GUARD),
        Ok(true),
        "the pop let it finish"
    );
    join_within(pusher, HANG_GUARD).expect("pusher");
    assert!(matches!(results.pop(seen), Popped::Item(2)));
}

/// QA-R2-007: a held job does not fall due while the clock does not move; a cancel ends the
/// hold at once (the job is taken as cancelled without the clock advancing). The worker is
/// known to be parked on the hold (registered with the manual clock) before the test looks.
#[test]
fn a_cancel_ends_a_hold_without_the_clock_moving() {
    let manual = ManualClock::new();
    let start = manual.now();
    let jobs: JobQueue<u32, ()> = JobQueue::new(4, manual.clock());
    jobs.hold(7, start + Duration::from_secs(3600));
    let jobs = Arc::new(jobs);
    let (taken, receiver) = mpsc::channel();
    let worker = {
        let jobs = Arc::clone(&jobs);
        thread::spawn(move || {
            let _ = taken.send(matches!(jobs.take(), Taken::Cancelled(7)));
        })
    };
    assert!(manual.wait_for_waiters(1), "the worker parked on the hold");
    assert!(
        receiver.try_recv().is_err(),
        "the frozen clock never makes the hold due"
    );
    jobs.cancel();
    assert!(recv_within(&receiver, HANG_GUARD));
    join_within(worker, HANG_GUARD).expect("worker");
}

/// A held job falls due exactly when the run's clock reaches its holdoff end: an advance 1 ms
/// short leaves the parked worker waiting, the advance to the end hands it the job.
#[test]
fn a_hold_falls_due_when_the_clock_reaches_it() {
    let manual = ManualClock::new();
    let until = manual.now() + Duration::from_secs(30);
    let jobs: JobQueue<u32, ()> = JobQueue::new(4, manual.clock());
    jobs.hold(7, until);
    let jobs = Arc::new(jobs);
    let (taken, receiver) = mpsc::channel();
    let worker = {
        let jobs = Arc::clone(&jobs);
        thread::spawn(move || {
            let _ = taken.send(matches!(jobs.take(), Taken::Run(7)));
        })
    };
    assert!(manual.wait_for_waiters(1), "the worker parked on the hold");
    manual.advance(Duration::from_secs(30) - Duration::from_millis(1));
    assert!(manual.wait_for_waiters(1), "still parked 1 ms short");
    assert!(receiver.try_recv().is_err(), "not due 1 ms short");
    manual.advance_to(until);
    assert!(recv_within(&receiver, HANG_GUARD), "due at its end");
    join_within(worker, HANG_GUARD).expect("worker");
}
