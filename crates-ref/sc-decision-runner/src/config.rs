//! [`RunnerConfig`]: worker count and job capacity of a [`DecisionRunner`](crate::DecisionRunner).

use std::num::NonZeroUsize;

use sc_agent_config::{AgentConfig, DEFAULT_WORKERS};

use crate::error::RunnerError;

/// Most workers one run starts: the bound `AgentConfig.workers` has at load (REQ-SAG-0013).
pub const MAX_WORKERS: usize = sc_agent_config::MAX_WORKERS;

/// Jobs (and waiting results) a run holds per worker: a constant, not a config key.
pub const JOBS_PER_WORKER: usize = 4;

/// How many workers a runner starts and how many jobs it holds at once.
///
/// Valid by construction: 1..=[`MAX_WORKERS`] workers, and a queue capacity of
/// [`JOBS_PER_WORKER`] x workers (at most 256). The fields are private; build one with
/// [`Self::new`], [`Self::from_config`] or [`Default`].
///
/// # Examples
///
/// ```
/// use sc_decision_runner::{RunnerConfig, RunnerError};
///
/// let config = RunnerConfig::default();
/// assert_eq!(config.workers().get(), 8);
/// assert_eq!(config.queue_capacity().get(), 32);
///
/// let two = RunnerConfig::new(2)?;
/// assert_eq!(two.queue_capacity().get(), 8);
/// assert!(matches!(RunnerConfig::new(65), Err(RunnerError::InvalidConfig { .. })));
/// assert!(RunnerConfig::new(0).is_err());
/// # Ok::<(), RunnerError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerConfig {
    workers: NonZeroUsize,
    queue_capacity: NonZeroUsize,
}

impl RunnerConfig {
    /// `workers` workers and [`JOBS_PER_WORKER`] x `workers` capacity.
    ///
    /// # Errors
    ///
    /// [`RunnerError::InvalidConfig`] when `workers` is 0 or over [`MAX_WORKERS`].
    pub fn new(workers: usize) -> Result<Self, RunnerError> {
        let invalid = || RunnerError::InvalidConfig {
            workers,
            max: MAX_WORKERS,
        };
        let count = NonZeroUsize::new(workers)
            .filter(|count| count.get() <= MAX_WORKERS)
            .ok_or_else(invalid)?;
        let queue_capacity = workers
            .checked_mul(JOBS_PER_WORKER)
            .and_then(NonZeroUsize::new)
            .ok_or_else(invalid)?;
        Ok(Self {
            workers: count,
            queue_capacity,
        })
    }

    /// Workers from `config.workers` (REQ-SAG-0701, REQ-SAG-0013), checked again here.
    ///
    /// # Errors
    ///
    /// [`RunnerError::InvalidConfig`] when `config.workers` is over [`MAX_WORKERS`]
    /// (config load refuses that; a hand-built `AgentConfig` may not).
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_config::AgentConfig;
    /// use sc_decision_runner::RunnerConfig;
    ///
    /// let config = RunnerConfig::from_config(&AgentConfig::default())?;
    /// assert_eq!((config.workers().get(), config.queue_capacity().get()), (8, 32));
    /// # Ok::<(), sc_decision_runner::RunnerError>(())
    /// ```
    pub fn from_config(config: &AgentConfig) -> Result<Self, RunnerError> {
        Self::new(config.workers.get())
    }

    /// Worker threads of a `decide_stream` run (`decide` uses one).
    #[must_use]
    pub fn workers(&self) -> NonZeroUsize {
        self.workers
    }

    /// Most jobs in a run at once (queued + held + running; the feeder blocks past it),
    /// and most results waiting for the consumer (a worker blocks past it).
    #[must_use]
    pub fn queue_capacity(&self) -> NonZeroUsize {
        self.queue_capacity
    }
}

impl Default for RunnerConfig {
    /// `sc_agent_config::DEFAULT_WORKERS` (8) workers, capacity 32.
    fn default() -> Self {
        let workers = DEFAULT_WORKERS;
        Self {
            workers,
            queue_capacity: workers
                .saturating_mul(NonZeroUsize::MIN.saturating_add(JOBS_PER_WORKER - 1)),
        }
    }
}
