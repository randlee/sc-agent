//! The one session row per result, and the proof tokens that make it exactly one.
//!
//! Every job carries an `Arc<Claim>`. [`Claim::win`] hands out the one [`Finish`]; only
//! [`write_row`] consumes a `Finish`, and it returns the one [`RowWritten`]; only
//! [`Finished::new`] consumes a `RowWritten`, and a `Finished` is the only thing a result
//! queue or the result iterator yields. None of the three is `Clone`, and none can be
//! built outside this module (their one field is a private `()`), so no path writes a row
//! or a result without winning the claim, or twice.
//!
//! `runner::finish` is the only caller of [`Claim::win`], [`write_row`] and
//! [`Finished::new`]: it writes the row and builds the result from the same identity
//! (session id, position, digests) and the same outcome, so a row and its result always
//! describe the same request.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use sc_agent_log::{GeneralLogger, Level, LogContext, SessionLogger};
use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_decision_schema::{
    DecisionError, DecisionResult, OrderedMap, QuestionDigest, SystemOneResponse,
};
use serde_json::{Map, Value};

/// The `target` of every general event this crate writes.
pub(crate) const LOG_TARGET: &str = "sc_decision_runner";

/// Who finishes a request: won once.
#[derive(Debug, Default)]
pub(crate) struct Claim(AtomicBool);

impl Claim {
    /// An unclaimed request.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The one [`Finish`] of this request: `Some` exactly once.
    pub(crate) fn win(&self) -> Option<Finish> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            .then_some(Finish(()))
    }
}

/// Proof that this caller alone finishes the request. Not `Clone`, not `Copy`; only
/// [`Claim::win`] makes one.
#[derive(Debug)]
#[must_use]
pub(crate) struct Finish(());

/// Proof that the one row was written (or its failure logged). Not `Clone`, not `Copy`;
/// only [`write_row`] makes one.
#[derive(Debug)]
#[must_use]
pub(crate) struct RowWritten(());

/// A result whose row is written: the only item a result queue carries.
#[derive(Debug)]
pub(crate) struct Finished {
    result: DecisionResult,
}

impl Finished {
    /// `result`, after its row: consumes the [`RowWritten`] proof.
    pub(crate) fn new(proof: RowWritten, result: DecisionResult) -> Self {
        let RowWritten(_) = proof;
        Self { result }
    }

    /// The result.
    pub(crate) fn into_result(self) -> DecisionResult {
        self.result
    }
}

/// Sends made and schema resends of one request, as recorded in its row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    /// Sends made, the first included (`Attempts::sends`).
    pub(crate) sends: u32,
    /// Resends after a schema mismatch.
    pub(crate) schema_resends: u32,
}

/// The live [`Tally`] of one request, shared by every clone of its identity, so any path
/// that answers it (worker, panic, cancel, orphan) reads the sends made so far.
#[derive(Debug, Default)]
pub(crate) struct Progress {
    sends: AtomicU32,
    schema_resends: AtomicU32,
}

impl Progress {
    /// The tally now.
    pub(crate) fn tally(&self) -> Tally {
        Tally {
            sends: self.sends.load(Ordering::Acquire),
            schema_resends: self.schema_resends.load(Ordering::Acquire),
        }
    }

    /// A send started; `sends` counts it.
    pub(crate) fn send_started(&self, sends: u32, schema_resend: bool) {
        self.sends.store(sends, Ordering::Release);
        if schema_resend {
            self.schema_resends.fetch_add(1, Ordering::AcqRel);
        }
    }
}

/// Whether a row write may block on a full log queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The worker path: `write`.
    Blocking,
    /// The cancel, orphan and abandon paths: `try_write`, never blocks.
    NonBlocking,
}

/// One session row.
#[derive(Debug)]
pub(crate) struct Row<'a> {
    /// Input position of the request.
    pub(crate) position: u64,
    /// `request.body().model`.
    pub(crate) model: &'a str,
    /// Sends made and schema resends.
    pub(crate) tally: Tally,
    /// One digest per question.
    pub(crate) digests: &'a OrderedMap<String, QuestionDigest>,
    /// The result's outcome.
    pub(crate) outcome: &'a Result<SystemOneResponse, DecisionError>,
}

impl Row<'_> {
    /// The row payload (the `SessionLogger` adds the identity fields first).
    fn payload(&self) -> Result<Map<String, Value>, serde_json::Error> {
        let digests: Map<String, Value> = self
            .digests
            .iter()
            .map(|(question, digest)| (question.clone(), Value::from(digest.as_str())))
            .collect();
        let mut payload = Map::new();
        payload.insert("position".to_owned(), self.position.into());
        payload.insert("model".to_owned(), self.model.into());
        payload.insert("sends".to_owned(), self.tally.sends.into());
        payload.insert(
            "schema_resends".to_owned(),
            self.tally.schema_resends.into(),
        );
        match self.outcome {
            Ok(response) => {
                payload.insert("outcome".to_owned(), "ok".into());
                payload.insert("digests".to_owned(), Value::Object(digests));
                payload.insert("response".to_owned(), serde_json::to_value(response)?);
            }
            Err(error) => {
                payload.insert("outcome".to_owned(), "error".into());
                payload.insert("digests".to_owned(), Value::Object(digests));
                payload.insert("error".to_owned(), serde_json::to_value(error.detail())?);
            }
        }
        Ok(payload)
    }
}

/// Writes the one row of `session_id`, consuming `proof`.
///
/// A write failure never changes the result. A write the `SessionLogger` refuses is
/// counted by it (reported once by its flush, so by
/// [`DecisionRunner::flush_session_log`](crate::DecisionRunner::flush_session_log)) and
/// logged as `runner.session_log_failed` with the request's `LogContext`. A payload that
/// cannot be serialized never reaches the `SessionLogger`, so it is not counted there; it
/// is only logged as `runner.session_log_failed` (in practice unreachable: `serde_json`
/// turns non-finite floats into `null` rather than failing).
pub(crate) fn write_row(
    session: &SessionLogger,
    general: &GeneralLogger,
    proof: Finish,
    session_id: SessionId,
    row: &Row<'_>,
    mode: Mode,
) -> RowWritten {
    let Finish(_) = proof;
    let written = match row.payload() {
        Ok(payload) => match mode {
            Mode::Blocking => session.write(session_id, payload),
            Mode::NonBlocking => session.try_write(session_id, payload),
        }
        .map_err(|error| error.detail()),
        Err(error) => Err(ErrorDetail::foreign("serde_json", &error)),
    };
    if let Err(detail) = written {
        let ctx = LogContext::request(session_id, Some(row.position));
        let mut fields = Map::new();
        fields.insert(
            "error".to_owned(),
            serde_json::to_value(&detail).unwrap_or(Value::Null),
        );
        let _counted_by_the_logger = match mode {
            Mode::Blocking => general.write(
                Level::Warn,
                LOG_TARGET,
                "runner.session_log_failed",
                ctx,
                fields,
            ),
            Mode::NonBlocking => general.try_write(
                Level::Warn,
                LOG_TARGET,
                "runner.session_log_failed",
                ctx,
                fields,
            ),
        };
    }
    RowWritten(())
}
