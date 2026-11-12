//! [`Backend`]: the sans-IO translation between a decision request and one provider's HTTP exchange.

use std::fmt;

use sc_agent_http::{HttpRequest, HttpResponse, LogContext};

use crate::DecisionError;
use crate::decision::DecisionRequest;
use crate::response::SystemOneResponse;

/// Translates a [`DecisionRequest`] to one provider's HTTP request and its reply back.
///
/// Implemented by sc-decision-systemone (Jev and any System One server), sc-decision-local
/// (Strands Decider, Laya, Mapika/decider, generic) and sc-decision-llm. No I/O, no clock: the
/// runner sends through sc-agent-http. Open trait: any crate may implement it (ADR-SAG-0001).
/// `request` is already valid ([`DecisionRequest::new`]).
pub trait Backend: Send + Sync + fmt::Debug {
    /// The HTTP request that sends `request` to this backend's provider.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Encode`] when the body or multipart cannot be built; any other
    /// variant the backend documents.
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError>;

    /// The decided response of `response`, the final reply to `request`.
    ///
    /// `response` has any status; busy and quota replies never reach here (sc-agent-http
    /// handles them). `ctx` is only passed through so the backend's own log records carry
    /// the session id and position; this crate writes no log.
    ///
    /// # Errors
    ///
    /// [`DecisionError::SchemaMismatch`] when the reply does not match the required schema
    /// and must be sent again (REQ-SAG-0901); `ApiBadRequest`, `ApiValidation`, `ApiStatus`
    /// or `ResponseParse` for a refused or unreadable reply.
    fn decode(
        &self,
        response: HttpResponse,
        request: &DecisionRequest,
        ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError>;
}
