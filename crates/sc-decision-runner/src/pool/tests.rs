//! Pool mechanics with plain closures: no backend, no network.
//!
//! No test asserts on elapsed time and nothing sleeps. Every wait that could hang has a
//! failsafe deadline that turns a deadlock into a failure.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::*;

/// Failsafe for waits that must finish; never a measurement.
const FAILSAFE: Duration = Duration::from_secs(2);

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

type RunFn<A> = dyn Fn(usize) -> Step<usize, A> + Send + Sync;
type SkipFn<A> = dyn Fn(usize, Skipped) -> A + Send + Sync;

/// A [`Work`] of closures over `usize` jobs.
struct Closures<A> {
    run: Box<RunFn<A>>,
    skipped: Box<SkipFn<A>>,
}

impl<A: Send + 'static> Work for Closures<A> {
    type Job = usize;
    type Answer = A;
    type InFlight = ();
    type Receipt = usize;

    fn receipt(&self, job: &usize) -> usize {
        *job
    }

    fn run(&self, job: usize) -> Step<usize, A> {
        (self.run)(job)
    }

    fn skipped(&self, receipt: usize, why: Skipped) -> Option<A> {
        Some((self.skipped)(receipt, why))
    }
}

fn pool_of<A: Send + 'static>(
    workers: usize,
    queue: usize,
    run: impl Fn(usize) -> Step<usize, A> + Send + Sync + 'static,
    skipped: impl Fn(usize, Skipped) -> A + Send + Sync + 'static,
) -> Pool<Closures<A>> {
    let workers = NonZeroUsize::new(workers).expect("workers");
    let jobs = Arc::new(JobQueue::new(queue, Arc::new(Instant::now)));
    let results = Arc::new(BoundedQueue::new(queue, workers.get()));
    let work = Arc::new(Closures {
        run: Box::new(run),
        skipped: Box::new(skipped),
    });
    Pool::start(workers, jobs, results, work).expect("workers start")
}

/// Every answer until the pool finishes; the failsafe turns a hang into a failure.
fn drain<A: Send + 'static>(pool: Pool<Closures<A>>) -> Vec<A> {
    let (send, receive) = mpsc::channel();
    let collector = thread::Builder::new()
        .spawn(move || {
            let mut all = Vec::new();
            while let Some(answer) = pool.recv() {
                all.push(answer);
            }
            pool.join();
            let _ = send.send(all);
        })
        .expect("collector");
    let all = receive
        .recv_timeout(FAILSAFE)
        .expect("the pool finished within the failsafe");
    collector.join().expect("collector joined");
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
            move |job| Step::Done((job, gate.arrive())),
            |job, _| (job, false),
        );
        for job in 0..workers {
            assert!(pool.submit(job).is_ok());
        }
        pool.close_input();
        let mut answers = drain(pool);
        answers.sort_unstable();
        let expected: Vec<(usize, bool)> = (0..workers).map(|job| (job, true)).collect();
        assert_eq!(answers, expected, "{workers} workers must overlap");
    }
}

/// Cancel stops new work: the running job finishes, queued jobs are answered as
/// `Cancelled` without running, and later submits are refused.
#[test]
fn cancel_skips_queued_jobs_and_refuses_new_ones() {
    let hold = Arc::new(Rendezvous::new(2));
    let gate = Arc::clone(&hold);
    let ran = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&ran);
    let (started, on_start) = mpsc::channel();
    let started = Mutex::new(started);
    let pool = pool_of(
        1,
        8,
        move |job| {
            counted.fetch_add(1, Ordering::AcqRel);
            let _ = started
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .send(job);
            gate.arrive();
            Step::Done((job, None))
        },
        |job, why| (job, Some(why)),
    );
    for job in 0..4 {
        assert!(pool.submit(job).is_ok());
    }
    assert_eq!(on_start.recv_timeout(FAILSAFE), Ok(0), "a worker started");
    pool.cancel();
    assert_eq!(pool.submit(9), Err(9), "submits are refused after cancel");
    assert!(hold.arrive());
    let mut answers = drain(pool);
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
        |job| {
            assert_ne!(job, 1, "boom");
            Step::Done((job, None))
        },
        |job, why| (job, Some(why)),
    );
    for job in 0..3 {
        assert!(pool.submit(job).is_ok());
    }
    pool.close_input();
    let answers = drain(pool);
    assert_eq!(answers.len(), 3);
    assert_eq!(answers.first(), Some(&(0, None)));
    assert!(matches!(
        answers.get(1),
        Some((1, Some(Skipped::Panicked(text)))) if text.contains("boom")
    ));
    assert_eq!(answers.get(2), Some(&(2, None)));
}

/// A held job waits in the queue, not in the worker: the worker runs the next job first.
#[test]
fn a_held_job_frees_its_worker() {
    let (release, on_release) = mpsc::channel::<()>();
    let on_release = Mutex::new(on_release);
    let pool = pool_of(
        1,
        4,
        move |job| match job {
            // Held until a far instant; answered only after job 1 ran.
            0 => Step::HoldOff {
                until: Instant::now() + Duration::from_secs(3600),
                job: 100,
            },
            100 => Step::Done(job),
            other => {
                let _ = on_release
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .recv_timeout(FAILSAFE);
                Step::Done(other)
            }
        },
        |job, _| job + 1000,
    );
    assert!(pool.submit(0).is_ok());
    assert!(pool.submit(1).is_ok());
    release.send(()).expect("release");
    let first = pool.recv();
    assert_eq!(first, Some(1), "job 1 ran while job 0 was held");
    pool.cancel();
    let rest = drain(pool);
    assert_eq!(rest, vec![1100], "the held job is answered as cancelled");
}
