//! [`Repo`]: the repository a session ran in, from its git origin or the folder (REQ-SAG-0004).
//!
//! git runs as an argv-only child (`git --no-pager -C <dir> ...`) with no terminal prompt, the C
//! locale, the repository-redirect variables removed and stdin closed. Each call has a
//! [`GIT_DEADLINE`] (RSH-001): the child is polled with `try_wait` between deadline waits on
//! the [`Clock`] (no sleep, REQ-SAG-0010), and a git still running at the deadline is killed,
//! reaped and reported as [`LogError::GitUnavailable`], so a hung git (a stalled network
//! filesystem, lock contention) cannot hold an interrupted decide.

use std::ffi::OsStr;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use sc_agent_types::{CancelToken, Clock};

use crate::LogError;
use crate::name::is_path_safe;

/// Longest one git call may run before it is killed (RSH-001).
pub const GIT_DEADLINE: Duration = Duration::from_secs(10);

/// How often a running git is looked at: a deadline wait between two `try_wait` calls.
const GIT_POLL: Duration = Duration::from_millis(10);

/// `<host>/<owner>/<repo>` from the git origin URL, else `local/<host>/<folder-name>` (REQ-SAG-0004).
///
/// Every `/`-separated segment is a path-safe directory name: non-empty, at most
/// [`MAX_SEGMENT_BYTES`](crate::MAX_SEGMENT_BYTES) bytes, not `.` or `..`, no `\`, `:`, `<`,
/// `>`, `"`, `|`, `?`, `*` or control character, no trailing `.` or space, not a Windows device
/// name. The value is a relative path under the log root on every platform (REQ-SAG-0005).
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
    /// [`LogError::GitUnavailable`] when git cannot be started or has not exited within
    /// [`GIT_DEADLINE`] (it is then killed), [`LogError::GitOutput`] for output that is not
    /// UTF-8.
    pub fn work_tree_root(dir: &Path) -> Result<Option<PathBuf>, LogError> {
        Ok(git_text(dir, &["rev-parse", "--show-toplevel"], "rev-parse")?.map(PathBuf::from))
    }

    /// Normalizes a remote origin URL to `<host>/<path>`. Pure: runs nothing.
    ///
    /// Accepts `scheme://[user[:pass]@]host[:port]/path` and scp `[user@]host:path`. Drops the
    /// scheme, userinfo and port; strips one trailing `/`, then one `.git`. The host's case is
    /// kept. A bracketed IPv6 host becomes path-safe: brackets dropped, each `:` written as `-`
    /// (`[2001:db8::1]` -> `2001-db8--1`). At least one path segment is required.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidRepo`] for a local path or `file://` URL, a missing host or path, or a
    /// segment that is not path-safe (see [`Repo`]). The reported value never contains the
    /// userinfo.
    pub fn from_origin_url(url: &str) -> Result<Self, LogError> {
        let (authority, path) = match classify(url) {
            Origin::Url { authority, path } | Origin::Scp { authority, path } => (authority, path),
            Origin::Local => {
                return Err(LogError::InvalidRepo {
                    value: without_userinfo(url),
                    reason: "the origin is a local path, not a remote URL",
                });
            }
        };
        let host_port = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = path_safe_host(host_port);
        let path = path.strip_suffix('/').unwrap_or(path);
        let path = path.strip_suffix(".git").unwrap_or(path);
        let value = format!("{host}/{path}");
        if host.is_empty() || path.is_empty() {
            return Err(LogError::InvalidRepo {
                value,
                reason: "the origin has no host or no path",
            });
        }
        if !value.split('/').all(is_path_safe) {
            return Err(LogError::InvalidRepo {
                value,
                reason: "a segment is not a path-safe directory name",
            });
        }
        Ok(Self(Arc::from(value)))
    }

    /// `local/<host>/<folder>`, for a folder outside git or with no remote origin.
    ///
    /// # Errors
    ///
    /// [`LogError::InvalidRepo`] when `host` or `folder` is not one path-safe directory name
    /// (see [`Repo`]).
    pub fn local(host: &str, folder: &str) -> Result<Self, LogError> {
        let value = format!("local/{host}/{folder}");
        if !(is_path_safe(host) && is_path_safe(folder)) {
            return Err(LogError::InvalidRepo {
                value,
                reason: "the host and folder must each be one path-safe directory name",
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
    // scp form with a bracketed IPv6 host: `[user@][address]:path`.
    let host_and_path = match url.split_once('@') {
        Some((user, rest)) if !user.contains(['/', ':', '[']) => rest,
        _ => url,
    };
    if let Some(rest) = host_and_path.strip_prefix('[')
        && let Some((_, path)) = rest.split_once("]:")
        && let Some(authority) = url.get(..url.len() - path.len() - 1)
    {
        return Origin::Scp { authority, path };
    }
    // git's rule: scp form has a ':' before any '/'. A one-letter prefix is a Windows drive.
    match url.split_once(':') {
        Some((authority, path)) if !authority.contains('/') && authority.len() > 1 => {
            Origin::Scp { authority, path }
        }
        _ => Origin::Local,
    }
}

/// `host:port` -> `host`; a bracketed IPv6 host `[a:b::c]:port` -> `a-b--c`.
fn path_safe_host(host_port: &str) -> String {
    if let Some(rest) = host_port.strip_prefix('[') {
        let address = rest.split_once(']').map_or(rest, |(address, _)| address);
        return address.replace(':', "-");
    }
    host_port
        .split_once(':')
        .map_or(host_port, |(host, _)| host)
        .to_owned()
}

/// `url` with the userinfo of a `scheme://user[:pass]@host/...` URL removed.
fn without_userinfo(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match path {
        Some(path) => format!("{scheme}://{host}/{path}"),
        None => format!("{scheme}://{host}"),
    }
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
    let mut git = git(dir);
    git.args(args);
    let (status, stdout) = run_within(git, &Clock::system(), GIT_DEADLINE)
        .map_err(|source| LogError::GitUnavailable { source })?;
    if !status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(stdout).map_err(|_| LogError::GitOutput { command })?;
    let text = text.trim_end_matches(['\n', '\r']);
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// Runs `command` with its stdout piped and stderr discarded, waiting at most `deadline` on
/// `clock`: its exit status and stdout.
///
/// The child is polled with `try_wait` between deadline waits of [`GIT_POLL`]; stdout is read
/// after it exits (the git calls here print one short line, far below a pipe buffer). A child
/// still running at the deadline is killed and reaped.
///
/// # Errors
///
/// The spawn or wait error, or `TimedOut` when the deadline passed (the child was killed).
fn run_within(
    mut command: Command,
    clock: &Clock,
    deadline: Duration,
) -> std::io::Result<(ExitStatus, Vec<u8>)> {
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn()?;
    let start = clock.now();
    let deadline_at = start.checked_add(deadline).unwrap_or(start);
    let never = CancelToken::new();
    loop {
        if let Some(status) = child.try_wait()? {
            let mut stdout = Vec::new();
            if let Some(mut pipe) = child.stdout.take() {
                pipe.read_to_end(&mut stdout)?;
            }
            return Ok((status, stdout));
        }
        let now = clock.now();
        if now >= deadline_at {
            // A killed child exits at once; reaping it leaves no zombie.
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "git did not exit within {} s and was killed",
                    deadline.as_secs()
                ),
            ));
        }
        let next = now
            .checked_add(GIT_POLL)
            .map_or(deadline_at, |next| next.min(deadline_at));
        let _ = clock.wait_until(&never, next);
    }
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
    use std::process::Stdio;

    use sc_agent_testkit::{HANG_GUARD, join_within};
    use sc_agent_types::testing::ManualClock;

    use super::{GIT_DEADLINE, git, run_within, short_host};

    /// RSH-001: a git that does not exit is killed at the deadline and reported as a timeout.
    /// `git hash-object --stdin` with a stdin the child keeps open blocks forever (no repository
    /// is needed); the clock is a ManualClock moved to the deadline once the wait is parked, so
    /// the test waits no real time.
    #[test]
    fn a_git_that_does_not_exit_is_killed_at_the_deadline() {
        let clock = ManualClock::new();
        let mut hung = git(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        hung.args(["hash-object", "--stdin"]).stdin(Stdio::piped());
        let waiting = clock.clock();
        let run = std::thread::spawn(move || run_within(hung, &waiting, GIT_DEADLINE));
        assert!(clock.wait_for_waiters(1), "the poll is parked on the clock");
        clock.advance(GIT_DEADLINE);
        let error = join_within(run, HANG_GUARD)
            .expect("runner thread")
            .expect_err("killed at the deadline");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("10 s"), "{error}");
    }

    /// A git that exits gives its status and stdout through the same bounded path. The poll
    /// runs on the real clock (a manual one would never move between polls) with the hang
    /// guard as the deadline, so only a hang can fail it; no time is asserted.
    #[test]
    fn a_git_that_exits_gives_its_output() {
        let mut version = git(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        version.arg("--version");
        let (status, stdout) =
            run_within(version, &sc_agent_types::Clock::system(), HANG_GUARD).expect("git runs");
        assert!(status.success());
        assert!(String::from_utf8_lossy(&stdout).starts_with("git version"));
    }

    /// A8-06: `m5.local` is the host `m5`.
    #[test]
    fn short_host_drops_the_domain() {
        assert_eq!(short_host("m5.local"), "m5");
        assert_eq!(short_host("m5"), "m5");
        assert_eq!(short_host("build-01.ci.example.com"), "build-01");
    }
}
