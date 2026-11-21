//! [`LogContext`]: which request a general log event belongs to (RSH-009).

use sc_agent_types::SessionId;

/// Which request a general event belongs to (RSH-009).
///
/// Passed by value to [`GeneralLogger::write`](crate::GeneralLogger::write) and to
/// `sc_agent_http::HttpClient::send_once`; `local.*`, `llm.*` and `runner.*` events use the same
/// type. Its `session_id` becomes the event's correlation id and the first field `session_id`;
/// its `position` becomes the field `position`. [`LogContext::NONE`] adds neither.
///
/// # Examples
///
/// ```
/// use sc_agent_log::LogContext;
/// use sc_agent_types::SessionId;
///
/// let ctx = LogContext::request(SessionId::new(), Some(3));
/// assert_eq!(ctx.position, Some(3));
/// assert_eq!(LogContext::default(), LogContext::NONE);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LogContext {
    /// The session the request belongs to.
    pub session_id: Option<SessionId>,
    /// 0-based position of the request in its batch or stream; `None` for a single request.
    pub position: Option<u64>,
}

impl LogContext {
    /// No request: the event carries no correlation id, no `session_id` and no `position`.
    pub const NONE: Self = Self {
        session_id: None,
        position: None,
    };

    /// The context of one request of `session_id`, at `position` in its batch if any.
    #[must_use]
    pub const fn request(session_id: SessionId, position: Option<u64>) -> Self {
        Self {
            session_id: Some(session_id),
            position,
        }
    }
}
