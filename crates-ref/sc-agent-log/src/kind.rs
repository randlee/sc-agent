//! [`LogKind`]: which sc-agent-log logger an error or report is about.

use std::fmt;

/// Which logger an error or last-resort report is about (RBP-F001).
///
/// # Examples
///
/// ```
/// use sc_agent_log::LogKind;
///
/// assert_eq!(LogKind::Session.as_str(), "session");
/// assert_eq!(LogKind::General.to_string(), "general");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LogKind {
    /// The general (diagnostic) log, [`GeneralLogger`](crate::GeneralLogger).
    General,
    /// The session log, [`SessionLogger`](crate::SessionLogger).
    Session,
    /// The in-memory test log (`testing::MemoryLog`).
    Memory,
}

impl LogKind {
    /// The lowercase name: `"general"`, `"session"` or `"memory"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Session => "session",
            Self::Memory => "memory",
        }
    }
}

impl fmt::Display for LogKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
