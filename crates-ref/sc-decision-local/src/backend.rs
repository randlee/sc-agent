//! [`LocalBackend`]: a [`SystemOneBackend`] at a configured runner, and the sans-IO halves of
//! its probe (build the request, read the reply). The probe I/O is [`LocalProbe`]'s.
//!
//! [`LocalProbe`]: crate::LocalProbe

use std::fmt;
use std::net::Ipv6Addr;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sc_agent_config::RunnerHost;
use sc_agent_http::{HttpError, HttpRequest, HttpResponse, LogContext};
use sc_agent_secrets::ApiKey;
use sc_agent_types::ModelId;
use sc_decision_schema::{Backend, DecisionError, DecisionRequest, SystemOneResponse};
use sc_decision_systemone::{BaseUrl, SystemOneBackend};

use crate::daemon::{Availability, DaemonKind, LocalDaemon};
use crate::error::{LocalError, redact_string};

/// How often [`LocalProbe::wait_until_loaded`](crate::LocalProbe::wait_until_loaded) probes
/// the daemon.
pub const LOAD_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The longest load wait: a larger `load_timeout` (up to `Duration::MAX`) is clamped to it, so
/// the wait always has a finite deadline. One day, far above the largest configurable load
/// timeout (7200 s).
pub const MAX_LOAD_WAIT: Duration = Duration::from_secs(86_400);

/// A runner's own timeouts (REQ-SAG-0015): local runners get longer ones than providers, and
/// model loading does not count toward the request timeout.
///
/// Values come from the caller (decidr, from the runner's config entry); this crate has no
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
/// `Arc<dyn Backend>`. It does no I/O and reads no clock (ADR-SAG-0001): the daemon probe is
/// built by [`Self::probe_request`] and read by [`Self::read_probe`], and
/// [`LocalProbe`](crate::LocalProbe) sends it on the caller's transport and clock.
///
/// `Debug` prints the daemon, base, model, timeouts and whether a key is set, never the key.
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
    /// decidr passes `DaemonKind::from(runner.kind)`, `&runner.host`, `runner.port` and
    /// `runner.model` of a `LocalRunner`; host and port are checked at config load, so no
    /// check is repeated here. An IPv6 host is written in brackets. No timeouts until
    /// [`Self::with_timeouts`].
    ///
    /// A key on a plain-http host that is not loopback is accepted here and refused in
    /// [`Backend::encode`] and [`Self::probe_request`], before any send (REQ-SAG-0013).
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
            timeouts: None,
            served_once: AtomicBool::new(false),
        })
    }

    /// This backend with the runner's REQ-SAG-0015 `timeouts`.
    ///
    /// For a kind that loads its model on the first request (Laya), requests encoded before
    /// the first 2xx reply carry the deadline `load + request`. Without timeouts no deadline
    /// is set. [`LocalProbe::wait_until_loaded`](crate::LocalProbe::wait_until_loaded) takes
    /// its load timeout as an argument.
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

    /// The configured model.
    #[must_use]
    pub fn model(&self) -> &ModelId {
        &self.model
    }

    /// The daemon kind's probe request (sans-IO): the one GET that tells whether the daemon is
    /// up and what it has loaded, with the key attached as Bearer by kinds that check it.
    ///
    /// # Errors
    ///
    /// [`LocalError::Http`] when the probe URL is invalid, or when a key would go to a
    /// plain-http host that is not loopback (`http.key_on_plain_http`, REQ-SAG-0013).
    pub fn probe_request(&self) -> Result<HttpRequest, LocalError> {
        self.daemon.probe(&self.base, self.key.as_ref())
    }

    /// What the transport's `reply` to `request` (from [`Self::probe_request`]) says about the
    /// daemon and the configured model (sans-IO).
    ///
    /// A transport error is redacted of the probe's key exactly as
    /// `HttpClient::send_once` redacts its errors
    /// ([`HttpError::redacted_for`]). When the probe carried the key, every copy of it in the
    /// reply text the result carries (error bodies and messages, loaded and served model names)
    /// is redacted too.
    ///
    /// # Errors
    ///
    /// [`LocalError::Http`] for a transport error (a daemon that is not listening is cause
    /// `http.network`); otherwise as [`LocalDaemon::availability`].
    pub fn read_probe(
        &self,
        request: &HttpRequest,
        reply: Result<HttpResponse, HttpError>,
    ) -> Result<Availability, LocalError> {
        let response = reply.map_err(|error| LocalError::Http(error.redacted_for(request)))?;
        let result = self.daemon.availability(&response, &self.model);
        match self.key.as_ref().filter(|_| request.secret().is_some()) {
            Some(key) => match result {
                Ok(availability) => Ok(Availability {
                    loaded: availability
                        .loaded
                        .into_iter()
                        .map(|name| redact_string(key, name))
                        .collect(),
                    ..availability
                }),
                Err(error) => Err(error.redact(key)),
            },
            None => result,
        }
    }
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
