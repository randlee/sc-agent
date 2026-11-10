//! [`Pool`]: a fixed set of named worker threads draining one [`JobQueue`] into one
//! result [`BoundedQueue`].
//!
//! A worker takes a job, runs one [`Step`] of it and either pushes the answer, holds the
//! job back in the queue until an instant, or drops it (its result was given elsewhere).
//! A worker never waits on a holdoff itself, and the pool never joins its threads on
//! drop: a worker blocked in a send exits on its own once the send returns.

use std::io;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::queue::{BoundedQueue, JobQueue, Skipped, Taken};

/// What one run of a job gave.
#[derive(Debug)]
pub(crate) enum Step<J, A> {
    /// The job is finished with this answer.
    Done(A),
    /// Hold the job until `until`, then run it again (a schema resend is `until: now`).
    HoldOff {
        /// Not before this instant.
        until: Instant,
        /// The job, with its state.
        job: J,
    },
    /// The job's result was already given elsewhere (a cancelled in-flight send): no answer.
    Discarded,
}

/// Why a worker stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stopped {
    /// The input ended and the queue drained.
    InputEnded,
    /// The queue was cancelled.
    Cancelled,
}

impl Stopped {
    /// The `reason` field of `runner.worker.stopped`.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InputEnded => "input_ended",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The work a pool does: how to run, skip and answer a job.
pub(crate) trait Work: Send + Sync + 'static {
    /// A queued job.
    type Job: Send + 'static;
    /// What a finished job gives.
    type Answer: Send + 'static;
    /// What a job's send registers while it is in flight.
    type InFlight: Send + 'static;
    /// What is kept of a job while it runs, to answer it if the run panics.
    type Receipt;

    /// The receipt of `job`, taken before it runs or is skipped.
    fn receipt(&self, job: &Self::Job) -> Self::Receipt;

    /// Runs one step of `job`.
    fn run(&self, job: Self::Job) -> Step<Self::Job, Self::Answer>;

    /// The answer for a job that is not run to the end; `None` when it was given elsewhere.
    fn skipped(&self, receipt: Self::Receipt, why: Skipped) -> Option<Self::Answer>;

    /// Worker `worker` (0-based) is about to exit.
    fn stopped(&self, _worker: usize, _why: Stopped) {}
}

/// The job queue type of a [`Work`].
pub(crate) type Jobs<W> = JobQueue<<W as Work>::Job, <W as Work>::InFlight>;

/// A fixed set of worker threads named `sc-decision-worker`.
pub(crate) struct Pool<W: Work> {
    jobs: Arc<Jobs<W>>,
    results: Arc<BoundedQueue<W::Answer>>,
    /// Detached on drop (never joined there); only the tests join.
    handles: Vec<JoinHandle<()>>,
}

impl<W: Work> Pool<W> {
    /// Starts `workers` threads draining `jobs` into `results`.
    ///
    /// Each worker is one producer of `results` (counted in its `new`) and reports
    /// [`BoundedQueue::producer_done`] when it exits. `live` counts the pool's threads
    /// that are alive: up while a worker runs, down when it exits, a panic included.
    ///
    /// # Errors
    ///
    /// The spawn error when a thread cannot start. The queue is then cancelled, the
    /// threads already started exit, and every worker that did not start is reported done.
    pub(crate) fn start(
        workers: NonZeroUsize,
        jobs: Arc<Jobs<W>>,
        results: Arc<BoundedQueue<W::Answer>>,
        work: Arc<W>,
        live: Arc<AtomicUsize>,
    ) -> Result<Self, io::Error> {
        let mut handles = Vec::with_capacity(workers.get());
        for index in 0..workers.get() {
            let worker = Worker {
                index,
                jobs: Arc::clone(&jobs),
                results: Arc::clone(&results),
                work: Arc::clone(&work),
                live: Arc::clone(&live),
            };
            live.fetch_add(1, Ordering::AcqRel);
            let spawned = thread::Builder::new()
                .name("sc-decision-worker".to_owned())
                .spawn(move || worker.work());
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    // The closure (and its worker) was dropped unrun.
                    live.fetch_sub(1, Ordering::AcqRel);
                    jobs.cancel();
                    for _ in index..workers.get() {
                        results.producer_done();
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            jobs,
            results,
            handles,
        })
    }

    /// Submits `job`, blocking while the job queue is full.
    ///
    /// # Errors
    ///
    /// The job back once the queue is closed or cancelled.
    #[cfg(test)]
    pub(crate) fn submit(&self, job: W::Job) -> Result<(), W::Job> {
        self.jobs.submit(job)
    }

    /// No more jobs; the workers exit when the queue drains.
    #[cfg(test)]
    pub(crate) fn close_input(&self) {
        self.jobs.close();
    }

    /// Cancels the job queue (never a caller's token).
    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.jobs.cancel();
    }

    /// The next answer; `None` once every worker exited and the results drained.
    #[cfg(test)]
    pub(crate) fn recv(&self) -> Option<W::Answer> {
        loop {
            let seen = self.results.generation();
            match self.results.pop(seen) {
                crate::queue::Popped::Item(answer) => return Some(answer),
                crate::queue::Popped::Interrupted => {}
                crate::queue::Popped::Finished => return None,
            }
        }
    }

    /// Joins the worker threads. Call after [`Self::recv`] returned `None`.
    #[cfg(test)]
    pub(crate) fn join(&mut self) {
        for handle in std::mem::take(&mut self.handles) {
            // A worker catches its job's panic, so a join error is a bug in the worker
            // loop itself; the test then sees missing answers.
            let _ = handle.join();
        }
    }

    /// Stops everything without joining: results are discarded, the queue is cancelled.
    pub(crate) fn abandon(&self) {
        self.results.discard();
        self.jobs.cancel();
    }
}

impl<W: Work> Drop for Pool<W> {
    /// Ends input and discards unread answers; never cancels the queue, so a pool whose
    /// input already drained stops with `input_ended`. A run that must stop early calls
    /// [`Pool::abandon`] first. Not joined: a running job may be an in-flight send.
    fn drop(&mut self) {
        self.jobs.close();
        self.results.discard();
        self.handles.clear();
    }
}

struct Worker<W: Work> {
    index: usize,
    jobs: Arc<Jobs<W>>,
    results: Arc<BoundedQueue<W::Answer>>,
    work: Arc<W>,
    live: Arc<AtomicUsize>,
}

/// Runs on every exit of a worker, a panic included: the last producer out closes the
/// result queue, so the consumer cannot hang on a dead worker; the live count goes down.
struct Exit<A> {
    results: Arc<BoundedQueue<A>>,
    live: Arc<AtomicUsize>,
}

impl<A> Drop for Exit<A> {
    fn drop(&mut self) {
        // Down before `producer_done`: once the last producer closes the result queue, every
        // exited thread is already uncounted, so `runner.stream.end` sees 0 (QA-R2-006).
        self.live.fetch_sub(1, Ordering::AcqRel);
        self.results.producer_done();
    }
}

/// Marks a taken job done on every exit from its turn, unwinding included, so a panic
/// outside the run function never leaves `running` counted (the others would wait).
struct Turn<'a, J, F> {
    jobs: &'a JobQueue<J, F>,
}

impl<J, F> Drop for Turn<'_, J, F> {
    fn drop(&mut self) {
        self.jobs.done();
    }
}

impl<W: Work> Worker<W> {
    fn work(self) {
        let _exit = Exit {
            results: Arc::clone(&self.results),
            live: Arc::clone(&self.live),
        };
        let why = loop {
            let taken = self.jobs.take();
            let _turn = match &taken {
                Taken::Finished { .. } => None,
                Taken::Run(_) | Taken::Cancelled(_) => Some(Turn { jobs: &*self.jobs }),
            };
            match taken {
                Taken::Finished { cancelled } => {
                    break if cancelled {
                        Stopped::Cancelled
                    } else {
                        Stopped::InputEnded
                    };
                }
                Taken::Cancelled(job) => {
                    let receipt = self.work.receipt(&job);
                    drop(job);
                    self.answer(self.work.skipped(receipt, Skipped::Cancelled));
                }
                Taken::Run(job) => {
                    let receipt = self.work.receipt(&job);
                    match catch_unwind(AssertUnwindSafe(|| self.work.run(job))) {
                        Ok(Step::Done(answer)) => self.answer(Some(answer)),
                        // Held before `_turn` marks the job done.
                        Ok(Step::HoldOff { until, job }) => self.jobs.hold(job, until),
                        Ok(Step::Discarded) => {}
                        Err(payload) => {
                            let text = panic_text(payload.as_ref());
                            self.answer(self.work.skipped(receipt, Skipped::Panicked(text)));
                        }
                    }
                }
            }
        };
        self.work.stopped(self.index, why);
    }

    /// Pushes `answer`, blocking while the result queue is full. A refused push means the
    /// consumer is gone: the answer is dropped (its row is already written).
    fn answer(&self, answer: Option<W::Answer>) {
        if let Some(answer) = answer {
            let _consumer_gone = self.results.push(answer);
        }
    }
}

/// The text of a panic payload (`&str` or `String`), else a fixed note.
pub(crate) fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic with a non-text payload".to_owned())
}

#[cfg(test)]
mod tests;
