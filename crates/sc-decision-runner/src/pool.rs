/// Counters that are not tied to a queue lock.
struct Counters {
    /// Workers currently running a job.
    running: AtomicUsize,
    /// Workers still alive.
    alive: AtomicUsize,
}

type RunFn<J, A> = dyn Fn(&J) -> A + Send + Sync;
type SkipFn<J, A> = dyn Fn(&J, Skipped) -> A + Send + Sync;
type LostFn<J, A> = dyn Fn(&J) -> A + Send + Sync;

/// A fixed set of worker threads draining a bounded job queue.
pub(crate) struct WorkerPool<J, A> {
    jobs: Arc<BoundedQueue<J>>,
    answers: Arc<BoundedQueue<A>>,
    handles: Mutex<Vec<JoinHandle<()>>>,
    cancel: CancelToken,
    counters: Arc<Counters>,
    workers: usize,
    queue_capacity: usize,
}

impl<J: Send + 'static, A: Send + 'static> WorkerPool<J, A> {
    /// Starts `workers` threads (at least one). Each queue holds
    /// `queue_capacity` items (at least one). `run` turns a job into its
    /// answer; `skipped` does so for a job that is not run. `lost` is the
    /// last resort when `skipped` itself panics: it must not panic, so every
    /// accepted job gets exactly one answer.
    pub(crate) fn start(
        workers: usize,
        queue_capacity: usize,
        cancel: CancelToken,
        run: Arc<RunFn<J, A>>,
        skipped: Arc<SkipFn<J, A>>,
        lost: Arc<LostFn<J, A>>,
    ) -> Self {
        let workers = workers.max(1);
        let queue_capacity = queue_capacity.max(1);
        let jobs = Arc::new(BoundedQueue::new(queue_capacity));
        let answers = Arc::new(BoundedQueue::new(queue_capacity));
        let counters = Arc::new(Counters {
            running: AtomicUsize::new(0),
            alive: AtomicUsize::new(workers),
        });
        let handles = (0..workers)
            .map(|_| {
                let worker = Worker {
                    jobs: Arc::clone(&jobs),
                    answers: Arc::clone(&answers),
                    counters: Arc::clone(&counters),
                    cancel: cancel.clone(),
                    run: Arc::clone(&run),
                    skipped: Arc::clone(&skipped),
                    lost: Arc::clone(&lost),
                };
                thread::spawn(move || worker.work())
            })
            .collect();
        Self {
            jobs,
            answers,
            handles: Mutex::new(handles),
            cancel,
            counters,
            workers,
            queue_capacity,
        }
    }

    /// Submits `job`, waiting at most `wait` (forever when `None`) for room.
    pub(crate) fn submit(&self, job: J, wait: Option<Duration>) -> Result<(), PushError<J>> {
        if self.cancel.is_cancelled() {
            return Err(PushError::Closed(job));
        }
        self.jobs.push(job, wait)
    }

    /// No more jobs will be submitted; the pool finishes when it drains.
    pub(crate) fn close_input(&self) {
        self.jobs.close();
    }

    /// Stops new work: see the module documentation.
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
        self.jobs.close();
    }

    /// Waits for the next answer; `None` once the pool is finished.
    pub(crate) fn recv(&self) -> Option<A> {
        match self.recv_within(None) {
            Popped::Item(answer) => Some(answer),
            Popped::Empty | Popped::Finished => None,
        }
    }
    /// Joins the worker threads. Call after [`Self::recv`] returned `None`,
    /// or after [`Self::abandon`]; the workers have exited or are about to.
    pub(crate) fn join(&self) {
        let handles =
            std::mem::take(&mut *self.handles.lock().unwrap_or_else(PoisonError::into_inner));
        for handle in handles {
            // A worker catches its job's panic, so a join error is a bug in
            // the worker loop itself; there is nobody left to tell.
            let _ = handle.join();
        }
    }

    /// Stops everything and drops unreceived answers, so no worker can stay
    /// blocked on a consumer that is gone. Does not wait for the workers.
    pub(crate) fn abandon(&self) {
        self.cancel();
        self.answers.discard();
    }
}

impl<J, A> Drop for WorkerPool<J, A> {
    fn drop(&mut self) {
        // The workers finish on their own; they are not joined here because
        // a running job may be an in-flight request.
        self.cancel.cancel();
        self.jobs.close();
        self.answers.discard();
    }
}

struct Worker<J, A> {
    jobs: Arc<BoundedQueue<J>>,
    answers: Arc<BoundedQueue<A>>,
    counters: Arc<Counters>,
    cancel: CancelToken,
    run: Arc<RunFn<J, A>>,
    skipped: Arc<SkipFn<J, A>>,
    lost: Arc<LostFn<J, A>>,
}

/// Runs on every exit of a worker, a panic included: the last worker out
/// closes the answer queue, so the stream cannot hang on a dead worker.
struct Exit<A> {
    counters: Arc<Counters>,
    answers: Arc<BoundedQueue<A>>,
}

impl<A> Drop for Exit<A> {
    fn drop(&mut self) {
        if self.counters.alive.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.answers.close();
        }
    }
}

impl<J, A> Worker<J, A> {
    fn work(self) {
        let _exit = Exit {
            counters: Arc::clone(&self.counters),
            answers: Arc::clone(&self.answers),
        };
        while let Popped::Item(job) = self.jobs.pop(None) {
            let answer = if self.cancel.is_cancelled() {
                self.skip(&job, Skipped::Cancelled)
            } else {
                self.counters.running.fetch_add(1, Ordering::AcqRel);
                let outcome = catch_unwind(AssertUnwindSafe(|| (self.run)(&job)));
                self.counters.running.fetch_sub(1, Ordering::AcqRel);
                outcome.unwrap_or_else(|_| self.skip(&job, Skipped::Panicked))
            };
            // A refused push means the consumer is gone: drop the answer.
            let _ = self.answers.push(answer, None);
        }
    }

    /// The answer for a job that was not run; `skipped` may panic (it logs).
    fn skip(&self, job: &J, why: Skipped) -> A {
        catch_unwind(AssertUnwindSafe(|| (self.skipped)(job, why)))
            .unwrap_or_else(|_| (self.lost)(job))
    }
}
