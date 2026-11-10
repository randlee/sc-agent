//! [`LocalBackend`]: a [`SystemOneBackend`] at a configured runner, plus discovery and load wait.

use std::fmt;
use std::net::Ipv6Addr;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sc_agent_config::RunnerHost;
use sc_agent_http::{HttpError, HttpRequest, HttpResponse, LogContext, Transport};
use sc_agent_log::{GeneralLogger, Level};
use sc_agent_secrets::ApiKey;
use sc_agent_types::{CancelToken, ErrorId, ModelId, Wait};
use sc_decision_schema::{Backend, DecisionError, DecisionRequest, SystemOneResponse};
use sc_decision_systemone::{BaseUrl, SystemOneBackend};
use serde_json::{Map, Value};

use crate::daemon::{Availability, DaemonKind, LocalDaemon, ModelState};
use crate::error::LocalError;

/// The log target of every record this crate writes.
const TARGET: &str = "sc_decision_local";

/// The `DecisionError::Backend` name of this crate.
const BACKEND_NAME: &str = "local";

/// How often [`LocalBackend::wait_until_loaded`] probes the daemon.
pub const LOAD_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The longest load wait: a larger `load_timeout` (up to `Duration::MAX`) is clamped to it, so
/// the wait always has a finite deadline. One day, far above the largest configurable load
/// timeout (7200 s).
pub const MAX_LOAD_WAIT: Duration = Duration::from_secs(86_400);

/// A runner's own timeouts (REQ-SAG-0015): local runners get longer ones than providers, and
/// model loading does not count toward the request timeout.
///
/// Values come from the caller (decide, from the runner's config entry); this crate has no
/// default and reads no config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTimeouts {
    /// How long the model may take to load.
    pub load: Duration,
    /// How long one decision request may take once the model is loaded.
    pub request: Duration,
}

/// A System One backend at `http://{host}:{port}` with discovery of its daemon.
///
/// The arguments come from a config `local_runners` entry (REQ-SAG-0801); this crate reads no
/// config and has no built-in host or port. As a [`Backend`] it encodes and decodes exactly as
/// [`SystemOneBackend`] (ADR-SAG-0001) and is used through the runner like any backend, as
/// `Arc<dyn Backend>`. [`Self::discover`] and [`Self::wait_until_loaded`] probe the daemon on
/// the caller's transport.
///
/// `Debug` prints the daemon, base, model, timeouts and whether a key and a logger are set,
/// never the key.
///
/// # Examples
///
/// ```
/// use std::num::NonZeroU16;
///
/// use sc_agent_config::RunnerHost;
/// use sc_decision_local::{DaemonKind, LocalBackend};
///
/// let host = RunnerHost::try_from("decider.internal".to_owned())?;
/// let port = NonZeroU16::new(9000).ok_or("port")?;
/// let backend = LocalBackend::new(DaemonKind::Mapika, &host, port, "jev-latest".parse()?, None)?;
/// assert_eq!(backend.base(), "http://decider.internal:9000");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LocalBackend {
    daemon: Box<dyn LocalDaemon>,
    base: BaseUrl,
    model: ModelId,
    key: Option<ApiKey>,
    backend: SystemOneBackend,
    log: Option<GeneralLogger>,
    timeouts: Option<LocalTimeouts>,
    /// Set by the first 2xx decode and never cleared: a one-way latch. An atomic because
    /// [`Backend`] takes `&self` and the backend is shared as `Arc<dyn Backend>` across the
    /// runner's workers. Acquire/Release is enough: nothing else is published through it, and
    /// a stale `false` only gives one more request the larger load + request deadline, never
    /// a shorter one.
    served_once: AtomicBool,
}

impl fmt::Debug for LocalBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalBackend")
            .field("daemon", &self.daemon.kind())
            .field("base", &self.base)
            .field("model", &self.model)
            .field("key", &set_or_unset(self.key.is_some()))
            .field("log", &set_or_unset(self.log.is_some()))
            .field("timeouts", &self.timeouts)
            .field("served_once", &self.served_once.load(Ordering::Acquire))
            .finish()
    }
}

const fn set_or_unset(set: bool) -> &'static str {
    if set { "<set>" } else { "<unset>" }
}

impl LocalBackend {
    /// A backend for the runner at `http://{host}:{port}` serving `model`.
    ///
    /// decide passes `DaemonKind::from(runner.kind)`, `&runner.host`, `runner.port` and
    /// `runner.model` of a `LocalRunner`; host and port are checked at config load, so no
    /// check is repeated here. An IPv6 host is written in brackets. No logger and no timeouts
    /// until [`Self::with_log`] and [`Self::with_timeouts`].
    ///
    /// A key on a plain-http host that is not loopback is accepted here and refused in
    /// [`Backend::encode`] and [`Self::discover`], before any send (REQ-SAG-0013).
    ///
    /// # Errors
    ///
    /// [`LocalError::Http`] (`http.invalid_endpoint`) when `http://{host}:{port}` is not a
    /// valid base URL ([`BaseUrl::local`]); [`LocalError::Decision`] when
    /// [`SystemOneBackend::at`] cannot join its endpoints onto it.
    pub fn new(
        kind: DaemonKind,
        host: &RunnerHost,
        port: NonZeroU16,
        model: ModelId,
        key: Option<ApiKey>,
    ) -> Result<Self, LocalError> {
        let host = host.as_str();
        let base = BaseUrl::local(&if host.parse::<Ipv6Addr>().is_ok() {
            format!("http://[{host}]:{port}")
        } else {
            format!("http://{host}:{port}")
        })?;
        let backend = SystemOneBackend::at(&base, key.clone())?;
        Ok(Self {
            daemon: kind.daemon(),
            base,
            model,
            key,
            backend,
            log: None,
            timeouts: None,
            served_once: AtomicBool::new(false),
        })
    }

    /// This backend writing its `local.*` records to the caller's `log` (REQ-SAG-0008).
    #[must_use]
    pub fn with_log(self, log: GeneralLogger) -> Self {
        Self {
            log: Some(log),
            ..self
        }
    }

    /// This backend with the runner's REQ-SAG-0015 `timeouts`.
    ///
    /// For a kind that loads its model on the first request (Laya), requests encoded before
    /// the first 2xx reply carry the deadline `load + request`. Without timeouts no deadline
    /// is set. [`Self::wait_until_loaded`] takes its load timeout as an argument.
    #[must_use]
    pub fn with_timeouts(self, timeouts: LocalTimeouts) -> Self {
        Self {
            timeouts: Some(timeouts),
            ..self
        }
    }

    /// The runner's base URL, `http://{host}:{port}`.
    #[must_use]
    pub fn base(&self) -> &str {
        self.base.as_str()
    }

    /// The daemon kind.
    #[must_use]
    pub fn kind(&self) -> DaemonKind {
        self.daemon.kind()
    }

    /// Whether the daemon is up and where the configured model stands.
    ///
    /// Sends the daemon kind's probe once on `transport` (no resend: this is not a decision)
    /// and, with a logger set, writes one `local.discover` record carrying `ctx`: `daemon`,
    /// `model`, and `loaded` and `state` on Ok or `error` (the [`LocalError`] id) on Err. A
    /// log write error is ignored; it never changes the result. `transport` is the caller's
    /// probe transport (`UreqTransport::new(Timeouts::probe())`), so a probe is never waited
    /// on for the decision timeout.
    ///
    /// # Errors
    ///
    /// [`LocalError::Http`] when the probe cannot be built or the daemon is not listening
    /// (cause `http.network`); otherwise as [`LocalDaemon::availability`].
    pub fn discover(
        &self,
        transport: &dyn Transport,
        ctx: LogContext,
    ) -> Result<Availability, LocalError> {
        let result = self.probe_once(transport);
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
    /// Probes the daemon on `transport` (the caller's probe transport) every
    /// [`LOAD_POLL_INTERVAL`], waiting between probes with [`CancelToken::wait_until`]. Returns
    /// Ok when a probe reports [`ModelState::Loaded`], and at once for
    /// [`ModelState::LoadsOnDemand`] (the first request loads it, see [`Self::with_timeouts`])
    /// and [`ModelState::Unlisted`] (the daemon cannot tell). A connect failure (Strands
    /// Decider listens only after loading) or [`ModelState::NotReady`] means not loaded yet:
    /// polling goes on. With a logger set, writes one `local.load_wait` record per probe
    /// (`phase: "poll"`, `poll`, `waited_ms`, `state` or `error`) and one at the end
    /// (`phase: "end"`, `outcome`, `polls`, `waited_ms`).
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
        transport: &dyn Transport,
        cancel: &CancelToken,
        load_timeout: Duration,
        ctx: LogContext,
    ) -> Result<(), DecisionError> {
        let start = Instant::now();
        let deadline = load_deadline(start, load_timeout);
        let mut polls: u64 = 0;
        let outcome = loop {
            if cancel.is_cancelled() {
                break LoadOutcome::Cancelled;
            }
            let probe = self.probe_once(transport);
            polls += 1;
            self.log_poll(ctx, polls, start.elapsed(), &probe);
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
            let now = Instant::now();
            if now >= deadline {
                break LoadOutcome::Failed(LocalError::LoadTimeout {
                    runner: self.base.to_string(),
                    waited: now.duration_since(start),
                });
            }
            let next = now
                .checked_add(LOAD_POLL_INTERVAL)
                .map_or(deadline, |next| next.min(deadline));
            if cancel.wait_until(next) == Wait::Cancelled {
                break LoadOutcome::Cancelled;
            }
        };
        self.log_load_end(ctx, polls, start.elapsed(), &outcome);
        match outcome {
            LoadOutcome::Ready => Ok(()),
            LoadOutcome::Cancelled => Err(DecisionError::Cancelled { sends: 0 }),
            LoadOutcome::Failed(error) => Err(DecisionError::Backend {
                backend: BACKEND_NAME,
                detail: error.detail(),
            }),
        }
    }

    /// Builds the probe, sends it once and reads the reply.
    ///
    /// When the probe carried the key, a [`LocalError::ProbeStatus`] body has it redacted.
    fn probe_once(&self, transport: &dyn Transport) -> Result<Availability, LocalError> {
        let request = self.daemon.probe(&self.base, self.key.as_ref())?;
        let response = transport.send(&request)?;
        let attached = self.key.as_ref().filter(|_| request.secret().is_some());
        self.daemon
            .availability(&response, &self.model)
            .map_err(|error| match (error, attached) {
                (
                    LocalError::ProbeStatus {
                        daemon,
                        path,
                        status,
                        body,
                    },
                    Some(key),
                ) => LocalError::ProbeStatus {
                    daemon,
                    path,
                    status,
                    body: redact_body(key, body),
                },
                (error, _) => error,
            })
    }

    /// `daemon` and `model`, the first fields of every record.
    fn base_fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("daemon".into(), self.daemon.kind().as_str().into());
        fields.insert("model".into(), self.model.as_str().into());
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

/// `body` with every copy of `key` replaced by `[redacted]`; unchanged bytes when it has none.
fn redact_body(key: &ApiKey, body: Vec<u8>) -> Vec<u8> {
    match key.redact_text(&String::from_utf8_lossy(&body)) {
        std::borrow::Cow::Owned(redacted) => redacted.into_bytes(),
        std::borrow::Cow::Borrowed(_) => body,
    }
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

impl Backend for LocalBackend {
    /// [`SystemOneBackend`]'s request for `request`, with the first-request load deadline.
    ///
    /// For a kind that loads on the first request (Laya) with [`LocalTimeouts`] set, every
    /// request encoded before the first 2xx reply carries
    /// [`HttpRequest::with_deadline`]`(load + request)`, so the on-demand load fits inside
    /// its deadline; parallel first requests are all marked. After the first 2xx no deadline
    /// is set.
    ///
    /// # Errors
    ///
    /// As [`SystemOneBackend`]'s encode: [`DecisionError::Encode`], or
    /// [`DecisionError::Http`] (`http.key_on_plain_http`) for a key on a plain-http host that
    /// is not loopback, before any send.
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let http = self.backend.encode(request)?;
        Ok(match self.timeouts {
            Some(timeouts)
                if self.daemon.kind().loads_on_first_request()
                    && !self.served_once.load(Ordering::Acquire) =>
            {
                http.with_deadline(timeouts.load.saturating_add(timeouts.request))
            }
            _ => http,
        })
    }

    /// [`SystemOneBackend`]'s decode; a 2xx reply marks the model served. Writes no record.
    ///
    /// # Errors
    ///
    /// As [`SystemOneBackend`]'s decode.
    fn decode(
        &self,
        response: HttpResponse,
        request: &DecisionRequest,
        ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        let success = matches!(response.status(), 200..=299);
        let decoded = self.backend.decode(response, request, ctx);
        if success {
            self.served_once.store(true, Ordering::Release);
        }
        decoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_deadline_is_finite_for_huge_timeouts() {
        let start = Instant::now();
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
