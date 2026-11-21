//! Path-safe directory names: the rule shared by [`Slug`](crate::Slug) and
//! [`Repo`](crate::Repo) segments (REQ-SAG-0005).
//!
//! Each slug and repo segment becomes one directory name under the log root, on every Tier 1
//! platform, so each must be a name Windows, macOS and Linux all accept and none reinterprets.

/// Longest slug or repo segment, in UTF-8 bytes.
///
/// One directory name of the session log layout. 128 keeps the deepest layout
/// (`<log_root>/logs/<host>/<owner>/<repo>/<agent_role>/<project>/sessions.jsonl`) well under the
/// 255-byte name limit of common file systems and short enough for Windows paths; real host,
/// owner, repo, role and project names are far shorter. Raising it lengthens every possible path.
pub const MAX_SEGMENT_BYTES: usize = 128;

/// Characters Windows refuses in a file name (beyond `/`, `\` and control characters).
const WINDOWS_FORBIDDEN: [char; 7] = [':', '<', '>', '"', '|', '?', '*'];

/// Windows device names: refused as a name, with or without an extension, in any case.
const WINDOWS_DEVICES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether `name` is one path-safe directory name.
///
/// Non-empty, at most [`MAX_SEGMENT_BYTES`], not `.` or `..`, no `/`, `\`, control character or
/// Windows-forbidden character, not a Windows device name, and no trailing `.` or space (Windows
/// drops them, so two names would collide).
pub(crate) fn is_path_safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_SEGMENT_BYTES
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name
            .chars()
            .any(|ch| ch == '/' || ch == '\\' || ch.is_control() || WINDOWS_FORBIDDEN.contains(&ch))
        && !is_windows_device(name)
}

fn is_windows_device(name: &str) -> bool {
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    WINDOWS_DEVICES
        .iter()
        .any(|device| device.eq_ignore_ascii_case(stem))
}
