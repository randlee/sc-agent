//! [`Repo`]: the repository a session ran in, from its git origin or the folder (REQ-SAG-0004).
//!
//! git runs as an argv-only child (`git --no-pager -C <dir> ...`) with no terminal prompt, the C
//! locale, the repository-redirect variables removed and stdin closed. It is waited for with
//! [`Command::output`]: no timer and no sleep (REQ-SAG-0010).

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use crate::LogError;

/// `<host>/<owner>/<repo>` from the git origin URL, else `local/<host>/<folder-name>` (REQ-SAG-0004).
///
/// Every `/`-separated segment is non-empty, is not `.` or `..`, and has no `\` or control
/// character, so the value is also a relative path under the log root (REQ-SAG-0005).
///
/// # Examples
///
/// ```
/// use sc_agent_log::Repo;
///
/// let repo = Repo::from_origin_url("git@github.com:randlee/sc-agent.git")?;
/// assert_eq!(repo.as_str(), "github.com/randlee/sc-agent");
/// assert_eq!(Repo::local("m5", "work")?.as_str(), "local/m5/work");
/// # Ok::<(), sc_agent_log::LogError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repo(Arc<str>);

/// The form of an origin URL.
enum Origin<'a> {
    /// `scheme://authority/path`.
    Url { authority: &'a str, path: &'a str },
    /// scp form `[user@]host:path`.
    Scp { authority: &'a str, path: &'a str },
    /// A local path or a `file://` URL.
    Local,
}

impl Repo {
    /// The repo of the folder `dir`.
    ///
    /// In a git work tree whose `remote.origin.url` is a remote URL, the normalized origin
    /// ([`Self::from_origin_url`]). Outside a work tree, in one with no origin, or with a local-path
    /// or `file://` origin: `local/<host>/<folder>`, where host is the machine's host name before
    /// its first `.` (`m5.local` gives `m5`) and folder is the work tree's (or else `dir`'s) name.
    ///
    /// # Errors
    ///
    /// [`LogError::GitUnavailable`] when git cannot be started (not installed),
    /// [`LogError::GitOutput`] for non-UTF-8 git output, [`LogError::InvalidRepo`] for an origin
    /// that does not normalize, [`LogError::HostUnknown`] and [`LogError::FolderUnknown`] for the
    /// local form.
    pub fn detect(dir: &Path) -> Result<Self, LogError> {
        let Some(root) = Self::work_tree_root(dir)? else {
            return local_for(dir);
        };
        let origin = git_text(&root, &["config", "--get", "remote.origin.url"], "config")?;
        match origin {
            Some(url) if !matches!(classify(&url), Origin::Local) => Self::from_origin_url(&url),
            _ => local_for(&root),
        }
    }

    /// The top folder of the git work tree containing `dir` (`git rev-parse --show-toplevel`).
    ///
    /// `None` outside git. This is the repo root whose `.env` A-4 reads.
    ///
    /// # Errors
    ///
    /// [`LogError::GitUnavailable`] when git cannot be started, [`LogError::GitOutput`] for
    /// output that is not UTF-8.
    pub fn work_tree_root(dir: &Path) -> Result<Option<PathBuf>, LogError> {
        Ok(git_text(dir, &["rev-parse", "--show-toplevel"], "rev-parse")?.map(PathBuf::from))
    }

    /// Normalizes a remote origin URL to `<host>/<path>`. Pure: runs nothing.
    ///
    /// Accepts `scheme://[user[:pass]@]host[:port]/path` and scp `[user@]host:path`. Drops the
    /// scheme, userinfo and port; strips one trailing `/`, then one `.git`. The host's case is
    /// kept. At least one path segment is required.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidRepo`] for a local path or `file://` URL, a missing host or path, or a
    /// segment that is empty, `.`, `..`, or has a `\` or control character. The reported value
    /// never contains the userinfo.
    pub fn from_origin_url(url: &str) -> Result<Self, LogError> {
        let (authority, path) = match classify(url) {
            Origin::Url { authority, path } | Origin::Scp { authority, path } => (authority, path),
            Origin::Local => {
                return Err(LogError::InvalidRepo {
                    value: url.to_owned(),
                    reason: "the origin is a local path, not a remote URL",
                });
            }
        };
        let host_port = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = strip_port(host_port);
        let path = path.strip_suffix('/').unwrap_or(path);
        let path = path.strip_suffix(".git").unwrap_or(path);
        let value = format!("{host}/{path}");
        if host.is_empty() || path.is_empty() {
            return Err(LogError::InvalidRepo {
                value,
                reason: "the origin has no host or no path",
            });
        }
        if !value.split('/').all(valid_segment) {
            return Err(LogError::InvalidRepo {
                value,
                reason: "a segment is empty, '.', '..', or has a '\\' or control character",
            });
        }
        Ok(Self(Arc::from(value)))
    }

    /// `local/<host>/<folder>`, for a folder outside git or with no remote origin.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidRepo`] when `host` or `folder` is empty, `.`, `..`, or has a `/`, `\`
    /// or control character.
    pub fn local(host: &str, folder: &str) -> Result<Self, LogError> {
        let value = format!("local/{host}/{folder}");
        if !(valid_segment(host) && valid_segment(folder))
            || host.contains('/')
            || folder.contains('/')
        {
            return Err(LogError::InvalidRepo {
                value,
                reason: "the host and folder must each be one non-empty name",
            });
        }
        Ok(Self(Arc::from(value)))
    }

    /// The repo text, e.g. `github.com/randlee/sc-agent`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for Repo {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

fn classify(url: &str) -> Origin<'_> {
    if let Some((scheme, rest)) = url.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            return Origin::Local;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        return Origin::Url { authority, path };
    }
    // git's rule: scp form has a ':' before any '/'. A one-letter prefix is a Windows drive.
    match url.split_once(':') {
        Some((authority, path)) if !authority.contains('/') && authority.len() > 1 => {
            Origin::Scp { authority, path }
        }
        _ => Origin::Local,
    }
}

/// `host:port` -> `host`; a bracketed IPv6 host keeps its brackets.
fn strip_port(host_port: &str) -> &str {
    if host_port.starts_with('[') {
        return host_port
            .find(']')
            .and_then(|end| host_port.get(..=end))
            .unwrap_or(host_port);
    }
    host_port
        .split_once(':')
        .map_or(host_port, |(host, _)| host)
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.chars().any(|ch| ch == '\\' || ch.is_control())
}

/// The machine's host name before its first `.` (`m5.local` -> `m5`).
pub(crate) fn short_host(name: &str) -> &str {
    name.split_once('.').map_or(name, |(host, _)| host)
}

fn local_for(folder: &Path) -> Result<Repo, LogError> {
    let name = hostname::get().map_err(|source| LogError::HostUnknown {
        source: Some(source),
    })?;
    let name = name
        .into_string()
        .map_err(|_| LogError::HostUnknown { source: None })?;
    Repo::local(short_host(&name), &folder_name(folder)?)
}

/// The final name of `folder`; `.` and similar are resolved against the current directory.
fn folder_name(folder: &Path) -> Result<String, LogError> {
    let unknown = || LogError::FolderUnknown {
        path: folder.to_path_buf(),
    };
    if let Some(name) = folder.file_name().and_then(OsStr::to_str) {
        return Ok(name.to_owned());
    }
    let absolute = std::path::absolute(folder).map_err(|_| unknown())?;
    absolute
        .file_name()
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .ok_or_else(unknown)
}

/// Runs `git <args>` in `dir`: `None` when git exits non-zero or prints nothing, else its
/// stdout without the trailing line break.
fn git_text(dir: &Path, args: &[&str], command: &'static str) -> Result<Option<String>, LogError> {
    let output = git(dir)
        .args(args)
        .output()
        .map_err(|source| LogError::GitUnavailable { source })?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(output.stdout).map_err(|_| LogError::GitOutput { command })?;
    let text = text.trim_end_matches(['\n', '\r']);
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// The one git spawn of sc-agent-log: argv only, no prompt, no repository redirect.
fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .arg("-C")
        .arg(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        // `-C dir` must decide the repository; inherited overrides would
        // point git at another one.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .stdin(Stdio::null());
    command
}

#[cfg(test)]
mod tests {
    use super::short_host;

    /// A8-06: `m5.local` is the host `m5`.
    #[test]
    fn short_host_drops_the_domain() {
        assert_eq!(short_host("m5.local"), "m5");
        assert_eq!(short_host("m5"), "m5");
        assert_eq!(short_host("build-01.ci.example.com"), "build-01");
    }
}
