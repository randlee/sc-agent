//! Pool mechanics with plain closures: no Jev, no network.
//!
//! No test asserts on elapsed time. Every wait that could hang has a
//! generous failsafe deadline that turns a deadlock into a failure.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// Failsafe for waits that must finish; never a measurement.
const FAILSAFE: Duration = Duration::from_secs(60);

/// A barrier that gives up instead of hanging: `arrive` is true only when
/// `parties` threads were inside it at the same moment.
struct Rendezvous {
    parties: usize,
    arrived: Mutex<usize>,
    all_here: Condvar,
}

impl Rendezvous {
    fn new(parties: usize) -> Self {
        Self {
            parties,
            arrived: Mutex::new(0),
            all_here: Condvar::new(),
        }
    }

    fn arrive(&self) -> bool {
        let mut arrived = self.arrived.lock().unwrap_or_else(PoisonError::into_inner);
        *arrived += 1;
        if *arrived >= self.parties {
            self.all_here.notify_all();
            return true;
        }
        let (arrived, _) = self
            .all_here
            .wait_timeout_while(arrived, FAILSAFE, |count| *count < self.parties)
            .unwrap_or_else(PoisonError::into_inner);
        *arrived >= self.parties
    }
}

fn pool_of<J: Send + 'static, A: Default + Send + 'static>(
    workers: usize,
    queue: usize,
    cancel: &CancelToken,
    run: impl Fn(&J) -> A + Send + Sync + 'static,
    skipped: impl Fn(&J, Skipped) -> A + Send + Sync + 'static,
) -> WorkerPool<J, A> {
    WorkerPool::start(
        workers,
        queue,
        cancel.clone(),
        Arc::new(run),
        Arc::new(skipped),
        Arc::new(|_| A::default()),
    )
}

/// The next answer, failing clearly if none arrives within `FAILSAFE`.
fn next<J: Send + 'static, A: Send + 'static>(pool: &WorkerPool<J, A>) -> Option<A> {
    match pool.recv_within(Some(FAILSAFE)) {
        Popped::Item(answer) => Some(answer),
        Popped::Finished => None,
        Popped::Empty => panic!("no answer within the failsafe"),
    }
}

fn drain<J: Send + 'static, A: Send + 'static>(pool: &WorkerPool<J, A>) -> Vec<A> {
    let mut all = Vec::new();
    while let Some(answer) = next(pool) {
        all.push(answer);
    }
    all
}
/// N jobs finish only if N workers are inside the run function at once.
#[test]
fn workers_run_jobs_in_parallel_not_in_turn() {
    for workers in [2_usize, 8] {
        let rendezvous = Arc::new(Rendezvous::new(workers));
        let gate = Arc::clone(&rendezvous);
        let pool = pool_of(
            workers,
            workers,
            &CancelToken::new(),
            move |job: &usize| (*job, gate.arrive()),
            |job, _| (*job, false),
        );
        for job in 0..workers {
            assert!(pool.submit(job, Some(FAILSAFE)).is_ok());
        }
        pool.close_input();
        let mut answers = drain(&pool);
        join(&pool);
        answers.sort_unstable();
        let expected: Vec<(usize, bool)> = (0..workers).map(|job| (job, true)).collect();
        assert_eq!(answers, expected, "{workers} workers must overlap");
        assert_eq!(pool.alive(), 0);
    }
}

/// Memory is bounded: with a consumer that is not reading, the producer is
/// Cancel stops new work: the running job finishes, queued jobs are
/// answered as `Cancelled` without running, and later submits are refused.
#[test]
fn cancel_skips_queued_jobs_and_refuses_new_ones() {
    let cancel = CancelToken::new();
    let hold = Arc::new(Rendezvous::new(2));
    let gate = Arc::clone(&hold);
    let ran = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&ran);
    let pool = pool_of(
        1,
        8,
        &cancel,
        move |job: &usize| {
            counted.fetch_add(1, Ordering::AcqRel);
            gate.arrive();
            (*job, None)
        },
        |job, why| (*job, Some(why)),
    );
    for job in 0..4 {
        assert!(pool.submit(job, Some(FAILSAFE)).is_ok());
    }
    wait_until("a worker to start", || pool.running() > 0);
    pool.cancel();
    assert!(cancel.is_cancelled(), "the engine's token is the pool's");
    assert!(matches!(
        pool.submit(9, Some(FAILSAFE)),
        Err(PushError::Closed(9))
    ));
    assert!(hold.arrive());
    let mut answers = drain(&pool);
    join(&pool);
    answers.sort_by_key(|(job, _)| *job);
    assert_eq!(
        answers,
        vec![
            (0, None),
            (1, Some(Skipped::Cancelled)),
            (2, Some(Skipped::Cancelled)),
            (3, Some(Skipped::Cancelled)),
        ]
    );
    assert_eq!(ran.load(Ordering::Acquire), 1, "only the running job ran");
}

/// A panicking job gets an answer and does not take its worker down.
#[test]
fn a_panicking_job_is_answered_and_the_worker_survives() {
    let pool = pool_of(
        1,
        4,
        &CancelToken::new(),
        |job: &usize| {
            assert_ne!(*job, 1, "boom");
            (*job, None)
        },
        |job, why| (*job, Some(why)),
    );
    for job in 0..3 {
        assert!(pool.submit(job, Some(FAILSAFE)).is_ok());
    }
    pool.close_input();
    let answers = drain(&pool);
    join(&pool);
    assert_eq!(
        answers,
        vec![(0, None), (1, Some(Skipped::Panicked)), (2, None)]
    );
}

