//! Slugs, repo normalization and detection, session identity, error ids (REQ-SAG-0004).

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::events;
use sc_agent_log::{
    LogError, LogKind, MAX_ROW_BYTES, MAX_SEGMENT_BYTES, Repo, SessionIdentity, SessionLogger, Slug,
};
use sc_agent_types::{ErrorDetail, ErrorId, SessionId};
use sc_observability::v2::{EventError, FlushError, InitError};
use sc_observability::{ErrorCode, ErrorContext, Remediation};
use serde_json::Map;
use tempfile::TempDir;

/// A8-08: the REQ-SAG-0004 slug rule; refusals are never rewritten.
#[test]
fn slug_rules() {
    for ok in ["phase-b", "rust-developer", "a.b_c-1", "0"] {
        assert_eq!(Slug::new("project", ok).expect(ok).as_str(), ok);
    }
    for bad in ["Phase B", "", "a/b", "Rust", "a b", ".", "..", "é", "a\\b"] {
        let error = Slug::new("agent_role", bad).expect_err(bad);
        assert_eq!(error.id(), "log.invalid_slug");
        assert!(
            matches!(&error, LogError::InvalidSlug { field: "agent_role", value } if value == bad),
            "{error}"
        );
    }
}

/// RSH-002 / QA-003: a slug is one path-safe directory name: at most MAX_SEGMENT_BYTES bytes,
/// no trailing '.', not a Windows device name.
#[test]
fn slug_path_safety_and_cap() {
    let longest = "a".repeat(MAX_SEGMENT_BYTES);
    assert_eq!(
        Slug::new("project", &longest).expect("128 bytes").as_str(),
        longest
    );
    let over = "a".repeat(MAX_SEGMENT_BYTES + 1);
    assert_eq!(
        Slug::new("project", &over).expect_err("129").id(),
        "log.invalid_slug"
    );
    for bad in ["con", "nul", "aux", "com1", "lpt9", "con.txt", "phase-b."] {
        assert_eq!(
            Slug::new("project", bad).expect_err(bad).id(),
            "log.invalid_slug",
            "{bad}"
        );
    }
    for ok in ["console", "com10", "null", "a.b"] {
        Slug::new("project", ok).expect(ok);
    }
}

/// A8-08: project "Phase B" is log.invalid_slug and no row or file is written.
#[test]
fn invalid_project_writes_nothing() {
    let root = TempDir::new().expect("temp");
    let repo = Repo::from_origin_url("https://github.com/randlee/sc-agent.git").expect("repo");
    let error = SessionIdentity::new(repo, "Phase B", "sanity").expect_err("refused");
    assert_eq!(error.id(), "log.invalid_slug");
    assert!(matches!(
        error,
        LogError::InvalidSlug {
            field: "project",
            ..
        }
    ));
    assert_eq!(std::fs::read_dir(root.path()).expect("root").count(), 0);
}

/// A8-05: https, scp and ssh forms normalize to <host>/<owner>/<repo>; userinfo never appears.
#[test]
fn origin_forms() {
    for url in [
        "https://github.com/randlee/sc-agent.git",
        "https://github.com/randlee/sc-agent",
        "https://github.com/randlee/sc-agent.git/",
        "git@github.com:randlee/sc-agent.git",
        "github.com:randlee/sc-agent",
        "ssh://git@github.com:22/randlee/sc-agent.git",
        "ssh://github.com/randlee/sc-agent.git",
        "https://user:s3cret@github.com/randlee/sc-agent.git",
        "https://token@github.com:443/randlee/sc-agent",
    ] {
        let repo = Repo::from_origin_url(url).expect(url);
        assert_eq!(repo.as_str(), "github.com/randlee/sc-agent", "{url}");
    }
    assert_eq!(
        Repo::from_origin_url("https://GitLab.example.com/group/sub/proj.git")
            .expect("nested")
            .as_str(),
        "GitLab.example.com/group/sub/proj",
        "host case kept, nested groups kept"
    );
    for bad in [
        "https://h/../x",
        "https://h/./x",
        "https://h//x",
        "https://h",
        "https:///owner/repo",
        "git@h:a\\b",
        "/srv/git/repo.git",
        "file:///srv/git/repo.git",
        "../repo",
        "C:\\repos\\x",
        // QA-003: characters Windows refuses, device names, trailing dots.
        "https://h/a:b/x",
        "https://h/a<b/x",
        "https://h/a>b/x",
        "https://h/a\"b/x",
        "https://h/a|b/x",
        "https://h/a?b/x",
        "https://h/a*b/x",
        "https://h/CON/x",
        "https://h/owner/nul.git",
        "https://h/owner./x",
        "git@h:a\u{1}b",
    ] {
        let error = Repo::from_origin_url(bad).expect_err(bad);
        assert_eq!(error.id(), "log.invalid_repo", "{bad}");
    }
    // QA-002: userinfo never appears in a refusal, local (file://) or remote.
    for url in [
        "https://user:s3cret@h/../x",
        "file://user:s3cret@h/srv/x.git",
        "file://s3cret@h",
    ] {
        let error = Repo::from_origin_url(url).expect_err(url);
        assert_eq!(error.id(), "log.invalid_repo");
        assert!(!error.to_string().contains("s3cret"), "{error}");
        assert!(!format!("{error:?}").contains("s3cret"), "{error:?}");
        assert!(!error.detail().message.contains("s3cret"));
    }
    let error = Repo::from_origin_url("file://user:s3cret@h/srv/x.git").expect_err("local");
    assert!(
        matches!(&error, LogError::InvalidRepo { value, .. } if value == "file://h/srv/x.git"),
        "{error:?}"
    );
}

/// QA-003: a bracketed IPv6 host becomes a path-safe segment (brackets dropped, ':' -> '-').
#[test]
fn ipv6_origin_is_path_safe() {
    for url in [
        "ssh://git@[2001:db8::1]:22/owner/repo.git",
        "https://[2001:db8::1]/owner/repo",
        "git@[2001:db8::1]:owner/repo.git",
        "[2001:db8::1]:owner/repo.git",
    ] {
        let repo = Repo::from_origin_url(url).expect(url);
        assert_eq!(repo.as_str(), "2001-db8--1/owner/repo", "{url}");
        assert!(!repo.as_str().contains([':', '[', ']']));
    }
}

/// RSH-002: each repo segment is at most MAX_SEGMENT_BYTES bytes.
#[test]
fn repo_segment_cap() {
    let longest = "r".repeat(MAX_SEGMENT_BYTES);
    let url = format!("https://h/owner/{longest}");
    assert_eq!(
        Repo::from_origin_url(&url).expect("128").as_str(),
        format!("h/owner/{longest}")
    );
    let over = "r".repeat(MAX_SEGMENT_BYTES + 1);
    let url = format!("https://h/owner/{over}");
    assert_eq!(
        Repo::from_origin_url(&url).expect_err("129").id(),
        "log.invalid_repo"
    );
    assert_eq!(
        Repo::local("m5", &over).expect_err("129").id(),
        "log.invalid_repo"
    );
    assert_eq!(
        Repo::local("m5", "a:b").expect_err("colon").id(),
        "log.invalid_repo"
    );
}

/// `git <args>` in `dir`, with the test process's repository overrides removed.
fn git(dir: &Path, args: &[&str]) -> std::io::Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("git {args:?}: {status}")))
    }
}

/// A fresh `work` folder with `git init` in a temp dir.
fn work_tree(root: &TempDir) -> std::io::Result<PathBuf> {
    let work = root.path().join("work");
    std::fs::create_dir(&work)?;
    git(&work, &["init", "-q"])?;
    Ok(work)
}

/// The machine's host name before its first `.`, as Repo::detect reads it.
fn short_host() -> Result<String, Box<dyn std::error::Error>> {
    let name = hostname::get()?
        .into_string()
        .map_err(|_| "host name is not UTF-8")?;
    Ok(name.split('.').next().unwrap_or_default().to_owned())
}

/// A8-05: Repo::detect in a clone whose origin is the sc-agent URL (https, scp, ssh) gives
/// github.com/randlee/sc-agent, from the root or a subfolder, and session rows carry it.
#[test]
fn detect_origin() {
    let root = TempDir::new().expect("temp");
    let work = work_tree(&root).expect("git init");
    git(
        &work,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/randlee/sc-agent.git",
        ],
    )
    .expect("git");
    let sub = work.join("src");
    std::fs::create_dir(&sub).expect("sub");
    for dir in [&work, &sub] {
        assert_eq!(
            Repo::detect(dir).expect("detect").as_str(),
            "github.com/randlee/sc-agent"
        );
    }
    let top = Repo::work_tree_root(&sub)
        .expect("git")
        .expect("in a work tree");
    assert_eq!(
        top.canonicalize().expect("top"),
        work.canonicalize().expect("work")
    );
    for url in [
        "git@github.com:randlee/sc-agent.git",
        "ssh://git@github.com:22/randlee/sc-agent.git",
        "https://user:pw@github.com/randlee/sc-agent.git",
    ] {
        git(&work, &["remote", "set-url", "origin", url]).expect("git");
        assert_eq!(
            Repo::detect(&work).expect(url).as_str(),
            "github.com/randlee/sc-agent"
        );
    }

    let logs = TempDir::new().expect("logs");
    let repo = Repo::detect(&work).expect("detect");
    let identity = SessionIdentity::new(repo, "phase-b", "rust-developer").expect("identity");
    let log = SessionLogger::open_default(logs.path(), identity).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("row");
    log.flush().expect("flush");
    let rows = events(log.path().expect("default path")).expect("rows");
    assert_eq!(rows[0].fields["repo"], "github.com/randlee/sc-agent");
    let text = std::fs::read_to_string(log.path().expect("path")).expect("rows");
    assert!(!text.contains("user") && !text.contains("pw@"), "{text}");
}

/// A8-06: outside git, folder `work` on host m5 is local/m5/work.
#[test]
fn local_repo() {
    assert_eq!(
        Repo::local("m5", "work").expect("local").as_str(),
        "local/m5/work"
    );
    let root = TempDir::new().expect("temp");
    let work = root.path().join("work");
    std::fs::create_dir(&work).expect("work");
    assert_eq!(Repo::work_tree_root(&work).expect("git"), None);
    let repo = Repo::detect(&work).expect("detect");
    assert_eq!(
        repo.as_str(),
        format!("local/{}/work", short_host().expect("host"))
    );
    for (host, folder) in [
        ("", "work"),
        ("m5", ""),
        ("m5", ".."),
        ("a/b", "work"),
        ("m5", "a/b"),
    ] {
        let error = Repo::local(host, folder).expect_err("refused");
        assert_eq!(error.id(), "log.invalid_repo");
    }
}

/// A8-06: a work tree with no origin, or with a local-path origin, is local/<host>/<folder>.
#[test]
fn repo_edge_cases() {
    let expected = format!("local/{}/work", short_host().expect("host"));
    let root = TempDir::new().expect("temp");
    let work = work_tree(&root).expect("git init");
    assert_eq!(Repo::detect(&work).expect("no origin").as_str(), expected);
    let sub = work.join("deep");
    std::fs::create_dir(&sub).expect("sub");
    assert_eq!(
        Repo::detect(&sub)
            .expect("work tree folder, not the subfolder")
            .as_str(),
        expected
    );
    git(&work, &["remote", "add", "origin", "/srv/git/repo.git"]).expect("git");
    for origin in ["/srv/git/repo.git", "../other", "file:///srv/git/repo.git"] {
        git(&work, &["remote", "set-url", "origin", origin]).expect("git");
        assert_eq!(
            Repo::detect(&work).expect(origin).as_str(),
            expected,
            "{origin}"
        );
    }
}

/// A8-07: project and agent_role are SessionIdentity::new's only inputs besides the Repo; no
/// separate agent name exists in sc-agent-log's source or rows.
#[test]
fn identity_inputs() {
    let new: fn(Repo, &str, &str) -> Result<SessionIdentity, LogError> = SessionIdentity::new;
    let repo = Repo::local("m5", "work").expect("repo");
    let identity = new(repo, "phase-b", "judge-jev").expect("identity");
    assert_eq!(identity.agent_role.as_str(), "judge-jev");

    let forbidden = concat!("agent", "_name");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&src).expect("src") {
        let path = entry.expect("entry").path();
        let text = std::fs::read_to_string(&path).expect("source");
        assert!(!text.contains(forbidden), "{}", path.display());
    }
    assert!(!SessionLogger::IDENTITY_KEYS.contains(&forbidden));

    let root = TempDir::new().expect("temp");
    let log = SessionLogger::open_default(root.path(), identity).expect("logger");
    log.write(SessionId::new(), Map::new()).expect("row");
    log.flush().expect("flush");
    let rows = events(log.path().expect("path")).expect("rows");
    assert!(!rows[0].fields.contains_key(forbidden));
}

fn context() -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        ErrorCode::new_static("TEST_CODE"),
        "test failure",
        Remediation::not_recoverable("test"),
    ))
}

/// A8-16: 14 distinct log.* ids (13 + log.row_too_large, RSH-003); Display is "<id>: <message>" only.
#[test]
fn ids() {
    let detail = ErrorDetail::new("x.y", "lower", None);
    let errors = [
        LogError::InvalidSlug {
            field: "project",
            value: "X".into(),
        },
        LogError::InvalidRepo {
            value: "h".into(),
            reason: "r",
        },
        LogError::GitUnavailable {
            source: std::io::Error::other("no git"),
        },
        LogError::GitOutput { command: "config" },
        LogError::HostUnknown { source: None },
        LogError::FolderUnknown {
            path: PathBuf::from("/"),
        },
        LogError::InvalidName {
            what: "target",
            value: "a b".into(),
            cause: detail.clone(),
        },
        LogError::OpenSink {
            path: PathBuf::from("f"),
            source: InitError::Runtime { context: context() },
        },
        LogError::Start {
            log: LogKind::General,
            source: InitError::Configuration { context: context() },
        },
        LogError::ReservedField { key: "repo".into() },
        LogError::Rejected {
            log: LogKind::Session,
            source: EventError::Validation { context: context() },
        },
        LogError::Flush {
            log: LogKind::Session,
            source: FlushError::Drain { context: context() },
        },
        LogError::RowTooLarge {
            log: LogKind::Session,
            bytes: MAX_ROW_BYTES + 1,
            max: MAX_ROW_BYTES,
        },
        LogError::RowsLost {
            log: LogKind::Memory,
            count: 2,
            last: Some(detail),
        },
    ];
    let ids: HashSet<_> = errors.iter().map(ErrorId::id).collect();
    assert_eq!(ids.len(), 14);
    for error in &errors {
        assert!(error.id().starts_with("log."), "{}", error.id());
        assert_eq!(
            error.to_string(),
            format!("{}: {}", error.id(), error.message())
        );
        assert_eq!(error.recovery(), None);
        assert_eq!(error.detail().id, error.id());
    }
    let flush = &errors[11];
    assert_eq!(flush.cause_detail().expect("flush cause").id, "TEST_CODE");
}
