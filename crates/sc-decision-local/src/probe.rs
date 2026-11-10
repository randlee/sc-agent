//! [`LocalProbe`]: the I/O driver of a [`LocalBackend`]'s daemon probe (discovery and the
//! load wait), over the caller's transport and clock.

use std::fmt;
use std::time::{Duration, Instant};

use sc_agent_http::{HttpError, LogContext, Transport};
use sc_agent_log::{GeneralLogger, Level};
use sc_agent_types::{CancelToken, Clock, ErrorId, Wait};
use sc_decision_schema::DecisionError;
use serde_json::{Map, Value};

use crate::backend::{LOAD_POLL_INTERVAL, LocalBackend, MAX_LOAD_WAIT};
use crate::daemon::{Availability, ModelState};
use crate::error::LocalError;

/// The log target of every record this crate writes.
const TARGET: &str = "sc_decision_local";

/// The `DecisionError::Backend` name of this crate.
const BACKEND_NAME: &str = "local";

/// Sends a [`LocalBackend`]'s daemon probe: once ([`Self::discover`]) or until the model is
/// loaded ([`Self::wait_until_loaded`]).
///
/// The backend stays sans-IO (ADR-SAG-0001): it builds the probe
/// ([`LocalBackend::probe_request`]) and reads the reply ([`LocalBackend::read_probe`]); this
/// type owns the transport, the clock and the log. `transport` is the caller's probe transport
/// (`UreqTransport::new(Timeouts::probe())`), so a probe is never waited on for the decision
/// timeout; `clock` is the real clock in production and a manual one in tests, so no load wait
/// depends on real time in a test.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU16;
///
/// use sc_agent_config::RunnerHost;
/// use sc_agent_http::testing::ScriptedTransport;
/// use sc_agent_http::{HttpResponse, LogContext};
/// use sc_agent_types::Clock;
/// use sc_decision_local::{DaemonKind, LocalBackend, LocalProbe, ModelState};
///
/// let host = RunnerHost::try_from("127.0.0.1".to_owned())?;
/// let port = NonZeroU16::new(9000).ok_or("port")?;
/// let backend = LocalBackend::new(DaemonKind::Mapika, &host, port, "jev-latest".parse()?, None)?;
/// let transport = ScriptedTransport::responses(vec![HttpResponse::new(
///     200,
///     vec![],
///     br#"{"ok":true,"model":"jev-latest"}"#.to_vec(),
/// )]);
/// let probe = LocalProbe::new(&backend, &transport, Clock::system());
/// assert_eq!(probe.discover(LogContext::NONE)?.model, ModelState::Loaded);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LocalProbe<'a> {
    backend: &'a LocalBackend,
    transport: &'a dyn Transport,
    clock: Clock,
    log: Option<GeneralLogger>,
}

impl fmt::Debug for LocalProbe<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalProbe")
            .field("backend", self.backend)
            .field("clock", &self.clock)
            .field("log", &self.log.is_some())
            .finish_non_exhaustive()
    }
}

impl<'a> LocalProbe<'a> {
    /// A probe of `backend`'s daemon, sent on `transport`, timed by `clock`; no logger until
    /// [`Self::with_log`].
    #[must_use]
    pub fn new(backend: &'a LocalBackend, transport: &'a dyn Transport, clock: Clock) -> Self {
        Self {
            backend,
            transport,
            clock,
            log: None,
        }
    }

    /// This probe writing its `local.*` records to the caller's `log` (REQ-SAG-0008).
    #[must_use]
    pub fn with_log(self, log: GeneralLogger) -> Self {
        Self {
            log: Some(log),
            ..self
        }
    }

    /// Whether the daemon is up and where the configured model stands.
    ///
    /// Sends the daemon kind's probe once (no resend: this is not a decision) and, with a
    /// logger set, writes one `local.discover` record carrying `ctx`: `daemon`, `model`, and
    /// `loaded` and `state` on Ok or `error` (the [`LocalError`] id) on Err. A log write error
    /// is ignored; it never changes the result.
    ///
    /// # Errors
    ///
    /// As [`LocalBackend::probe_request`] and [`LocalBackend::read_probe`]: a daemon that is
    /// not listening is [`LocalError::Http`] with cause `http.network`.
    pub fn discover(&self, ctx: LogContext) -> Result<Availability, LocalError> {
        let result = self.probe_once();
        if let Some(log) = &self.log {
            let mut fields = self.base_fields();
            match &result {
                Ok(availability) => {
                    fields.insert("loaded".into(), loaded_field(availability));
                    fields.insert("state".into(), availability.model.as_str().into());
                }
                Err(error) => {
                    fields.insert("error".into(), error.id().into());
                }
            }
            // A failed write is counted and reported by sc-agent-log's flush; discovery
            // still returns its own result.
            let _ = log.write(Level::Info, TARGET, "local.discover", ctx, fields);
        }
        result
    }

    /// Waits for the configured model to load before the first decision (REQ-SAG-0015).
    ///
    /// Probes the daemon every [`LOAD_POLL_INTERVAL`] of the probe's clock, waiting between
    /// probes with [`Clock::wait_until`]. Returns Ok when a probe reports
    /// [`ModelState::Loaded`], and at once for [`ModelState::LoadsOnDemand`] (the first request
    /// loads it, see [`LocalBackend::with_timeouts`]) and [`ModelState::Unlisted`] (the daemon
    /// cannot tell). A transport failure (Strands Decider listens only after loading) or
    /// [`ModelState::NotReady`] means not loaded yet: polling goes on. With a logger set,
    /// writes one `local.load_wait` record per probe (`phase: "poll"`, `poll`, `waited_ms`,
    /// `state` or `error`) and one at the end (`phase: "end"`, `outcome`, `polls`,
    /// `waited_ms`).
    ///
    /// `load_timeout` is clamped to [`MAX_LOAD_WAIT`]. The deadline is checked after each
    /// probe, and the last probe is sent at the deadline, so the wait can end later than
    /// `load_timeout` by at most one probe's duration (bounded by the probe transport's
    /// request timeout, 10 s for `Timeouts::probe()`).
    ///
    /// # Errors
    ///
    /// - [`DecisionError::Backend`] (`backend: "local"`) with the
    ///   [`LocalError::LoadTimeout`] detail once `load_timeout` has passed without the model
    ///   loaded;
    /// - [`DecisionError::Cancelled`] (`sends: 0`) when `cancel` fires; no probe follows;
    /// - [`DecisionError::Backend`] with the error's detail at once for any other probe error
    ///   ([`LocalError::ModelNotServed`], [`LocalError::ProbeStatus`],
    ///   [`LocalError::ProbeParse`], an invalid probe request).
    pub fn wait_until_loaded(
        &self,
        cancel: &CancelToken,
        load_timeout: Duration,
        ctx: LogContext,
    ) -> Result<(), DecisionError> {
        let start = self.clock.now();
        let deadline = load_deadline(start, load_timeout);
        let mut polls: u64 = 0;
        let outcome = loop {
            if cancel.is_cancelled() {
                break LoadOutcome::Cancelled;
            }
            let probe = self.probe_once();
            polls += 1;
            self.log_poll(ctx, polls, self.waited(start), &probe);
            match probe {
                Ok(availability) => match availability.model {
                    ModelState::Loaded | ModelState::LoadsOnDemand | ModelState::Unlisted => {
                        break LoadOutcome::Ready;
                    }
                    ModelState::NotReady => {}
                },
                Err(LocalError::Http(HttpError::Network { .. })) => {}
                Err(error) => break LoadOutcome::Failed(error),
            }
            let now = self.clock.now();
            if now >= deadline {
                break LoadOutcome::Failed(LocalError::LoadTimeout {
                    runner: self.backend.base().to_owned(),
                    waited: now.saturating_duration_since(start),
                });
            }
            let next = now
                .checked_add(LOAD_POLL_INTERVAL)
                .map_or(deadline, |next| next.min(deadline));
            if self.clock.wait_until(cancel, next) == Wait::Cancelled {
                break LoadOutcome::Cancelled;
            }
        };
        self.log_load_end(ctx, polls, self.waited(start), &outcome);
        match outcome {
            LoadOutcome::Ready => Ok(()),
            LoadOutcome::Cancelled => Err(DecisionError::Cancelled { sends: 0 }),
            LoadOutcome::Failed(error) => Err(DecisionError::Backend {
                backend: BACKEND_NAME,
                detail: error.detail(),
            }),
        }
    }

    /// Builds the probe, sends it once and reads the reply (both halves sans-IO on the
    /// backend; the transport error is redacted there).
    fn probe_once(&self) -> Result<Availability, LocalError> {
        let request = self.backend.probe_request()?;
        let reply = self.transport.send(&request);
        self.backend.read_probe(&request, reply)
    }

    /// How long the wait started at `start` has lasted, on the probe's clock.
    fn waited(&self, start: Instant) -> Duration {
        self.clock.now().saturating_duration_since(start)
    }

    /// `daemon` and `model`, the first fields of every record.
    fn base_fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("daemon".into(), self.backend.kind().as_str().into());
        fields.insert("model".into(), self.backend.model().as_str().into());
        fields
    }

    fn log_poll(
        &self,
        ctx: LogContext,
        poll: u64,
        waited: Duration,
        probe: &Result<Availability, LocalError>,
    ) {
        let Some(log) = &self.log else { return };
        let mut fields = self.base_fields();
        fields.insert("phase".into(), "poll".into());
        fields.insert("poll".into(), poll.into());
        fields.insert("waited_ms".into(), millis(waited).into());
        match probe {
            Ok(availability) => {
                fields.insert("state".into(), availability.model.as_str().into());
            }
            Err(error) => {
                fields.insert("error".into(), error.id().into());
            }
        }
        // Ignored as in `discover`: the wait's result does not depend on the log.
        let _ = log.write(Level::Info, TARGET, "local.load_wait", ctx, fields);
    }

    fn log_load_end(&self, ctx: LogContext, polls: u64, waited: Duration, outcome: &LoadOutcome) {
        let Some(log) = &self.log else { return };
        let mut fields = self.base_fields();
        let outcome = match outcome {
            LoadOutcome::Ready => "loaded",
            LoadOutcome::Cancelled => "cancelled",
            LoadOutcome::Failed(error) => error.id(),
        };
        fields.insert("phase".into(), "end".into());
        fields.insert("outcome".into(), outcome.into());
        fields.insert("polls".into(), polls.into());
        fields.insert("waited_ms".into(), millis(waited).into());
        let _ = log.write(Level::Info, TARGET, "local.load_wait", ctx, fields);
    }
}

/// The deadline of a load wait started at `start`: `load_timeout` clamped to
/// [`MAX_LOAD_WAIT`], so it is always finite.
fn load_deadline(start: Instant, load_timeout: Duration) -> Instant {
    let bound = load_timeout.min(MAX_LOAD_WAIT);
    // `start + MAX_LOAD_WAIT` fits on every supported platform; if it ever did not, the wait
    // ends after one probe rather than never.
    start.checked_add(bound).unwrap_or(start)
}

/// How a load wait ended.
enum LoadOutcome {
    Ready,
    Cancelled,
    Failed(LocalError),
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn loaded_field(availability: &Availability) -> Value {
    Value::Array(
        availability
            .loaded
            .iter()
            .map(|name| Value::String(name.clone()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use sc_agent_types::testing::ManualClock;

    use super::*;

    #[test]
    fn test_load_deadline_is_finite_for_huge_timeouts() {
        let start = ManualClock::new().now();
        assert_eq!(load_deadline(start, Duration::MAX) - start, MAX_LOAD_WAIT);
        assert_eq!(
            load_deadline(start, MAX_LOAD_WAIT + Duration::from_secs(1)) - start,
            MAX_LOAD_WAIT
        );
        assert_eq!(
            load_deadline(start, Duration::from_millis(1500)) - start,
            Duration::from_millis(1500)
        );
    }
}
