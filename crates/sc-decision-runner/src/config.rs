//! [`RunnerConfig`]: worker count and job capacity of a [`DecisionRunner`](crate::DecisionRunner).

use std::num::NonZeroUsize;

use sc_agent_config::{AgentConfig, DEFAULT_WORKERS};

/// Jobs in the runner per worker: a constant, not a config key.
const JOBS_PER_WORKER: NonZeroUsize = NonZeroUsize::MIN.saturating_add(3);

/// How many workers a runner starts and how many jobs it holds at once.
///
/// # Examples
///
/// ```
/// use sc_decision_runner::RunnerConfig;
///
/// let config = RunnerConfig::default();
/// assert_eq!(config.workers.get(), 8);
/// assert_eq!(config.queue_capacity.get(), 32);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunnerConfig {
    /// Worker threads (REQ-SAG-0701; `AgentConfig.workers`, default 8).
    pub workers: NonZeroUsize,
    /// Most jobs in the runner at once (queued + held + running), and most results
    /// waiting for the consumer; the feeder blocks past it. 4 x workers.
    pub queue_capacity: NonZeroUsize,
}

impl RunnerConfig {
    /// Workers from `config.workers` (1..=64, checked at config load, REQ-SAG-0013);
    /// `queue_capacity` is 4 x workers.
    ///
    /// # Examples
    ///
    /// ```
    /// use sc_agent_config::AgentConfig;
    /// use sc_decision_runner::RunnerConfig;
    ///
    /// let config = RunnerConfig::from_config(&AgentConfig::default());
    /// assert_eq!((config.workers.get(), config.queue_capacity.get()), (8, 32));
    /// ```
    #[must_use]
    pub fn from_config(config: &AgentConfig) -> Self {
        Self::with_workers(config.workers)
    }

    /// `workers` workers and 4 x `workers` capacity (saturating, never a panic).
    fn with_workers(workers: NonZeroUsize) -> Self {
        Self {
            workers,
            queue_capacity: workers
                .checked_mul(JOBS_PER_WORKER)
                .unwrap_or(NonZeroUsize::MAX),
        }
    }
}

impl Default for RunnerConfig {
    /// `sc_agent_config::DEFAULT_WORKERS` (8) workers, capacity 32.
    fn default() -> Self {
        Self::with_workers(DEFAULT_WORKERS)
    }
}
