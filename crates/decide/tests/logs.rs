//! General and session logs (REQ-SAG-0003, ADR-SAG-0002, ANC-007) and `decide doctor`
//! (REQ-SAG-0503).

mod common;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::common::send;
    use decide::{Context, DoctorOptions, ModelsOptions, doctor, models};
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_log::Repo;
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::{CancelToken, Provider};
    use serde_json::Value;

    use crate::common::{
        ANTHROPIC_KEY, MapKeyring, NOUL, TYPESAFE_KEY, config_dir, context, environment,
        json_response, ok_noul, options, records_text, request,
    };

    fn find(dir: &Path, name: &str, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                find(&path, name, found);
            } else if path.file_name().is_some_and(|file| file == name) {
                found.push(path);
            }
        }
    }

    /// AC: REQ-SAG-0003, ADR-SAG-0002. With no injected loggers, a send writes
    /// `<log_root>/logs/decide.log.jsonl` and its session row under
    /// `<log_root>/<repo>/<agent_role>/<project>/sessions.jsonl`, identity fields first.
    #[test]
    fn default_logs_are_written_under_the_log_root() {
        let root = tempfile::tempdir().expect("root");
        let dir = config_dir(&serde_json::json!({ "log_root": root.path() }).to_string());
        let transport = ScriptedTransport::responses(vec![ok_noul()]);
        let mut ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport),
            &MemoryLog::new().expect("log"),
        );
        ctx.general = None;
        let log = MemoryLog::new().expect("log");
        let mut options = options("jev", None, ctx, &log);
        options.session = None;
        let result = send(&options, request(NOUL), &CancelToken::new()).expect("send");
        assert!(result.outcome.is_ok());
        drop(options);

        let general = root.path().join("logs/decide.log.jsonl");
        let text = std::fs::read_to_string(&general).expect("general log");
        assert!(text.contains("decide.send"), "{text}");

        let repo = Repo::detect(&std::env::current_dir().expect("cwd")).expect("repo");
        let mut rows = Vec::new();
        find(root.path(), "sessions.jsonl", &mut rows);
        assert_eq!(rows.len(), 1, "{rows:?}");
        let expected = repo
            .as_str()
            .split('/')
            .fold(root.path().to_path_buf(), |path, segment| {
                path.join(segment)
            })
            .join("role/proj/sessions.jsonl");
        assert_eq!(rows[0], expected);
        let line = std::fs::read_to_string(&rows[0]).expect("rows");
        let row: Value = serde_json::from_str(line.lines().next().expect("a row")).expect("json");
        let fields = row["fields"].as_object().expect("fields");
        let keys: Vec<&str> = fields.keys().take(4).map(String::as_str).collect();
        assert_eq!(keys, ["session_id", "repo", "project", "agent_role"]);
        assert_eq!(fields["session_id"], result.session_id.to_string());
        assert_eq!(fields["project"], "proj");
        assert_eq!(fields["agent_role"], "role");
    }

    /// AC: REQ-SAG-0003 (QA-001). With no injected loggers, the batch path has written the
    /// general log and the session rows by the time `send_batch` returns, even while a runner
    /// worker is still alive and holds its logger handles (the CI failure: detached workers
    /// keep the last handle, so a drop never flushes). One send is held in a
    /// bounded `Gate` transport and cancelled, so that worker outlives the call deterministically;
    /// the files are read before the transport is released.
    #[test]
    fn default_logs_are_written_when_a_batch_returns() {
        let root = tempfile::tempdir().expect("root");
        let dir = config_dir(&serde_json::json!({ "log_root": root.path() }).to_string());
        let transport = crate::common::Gate::new(ok_noul());
        let unused = MemoryLog::new().expect("log");
        let mut ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &unused,
        );
        ctx.general = None;
        let mut options = options("jev", None, ctx, &unused);
        options.session = None;
        let cancel = CancelToken::new();
        let canceller = {
            let (transport, cancel) = (transport.clone(), cancel.clone());
            std::thread::spawn(move || {
                transport.wait_started(1);
                cancel.cancel();
            })
        };
        let mut count = 0;
        decide::send_batch(&options, vec![request(NOUL)], &cancel, &mut |_| {
            count += 1;
            Ok(())
        })
        .expect("batch");
        canceller.join().expect("canceller");
        drop(options);
        assert_eq!(count, 1);
        // The blocked worker still holds the runner's logger handles here.
        let general = std::fs::read_to_string(root.path().join("logs/decide.log.jsonl"))
            .expect("the general log exists when send_batch returns");
        assert!(general.contains("decide.send"), "{general}");
        let mut rows = Vec::new();
        find(root.path(), "sessions.jsonl", &mut rows);
        assert_eq!(rows.len(), 1);
        let text = std::fs::read_to_string(&rows[0]).expect("rows");
        assert_eq!(text.lines().count(), 1);
        transport.release();
    }

    /// AC: REQ-SAG-0002 (QA-006). A single send whose session log cannot be flushed still
    /// hands out its completed result, then returns `decide.runner`.
    #[test]
    fn a_session_flush_failure_keeps_the_result() {
        let log = MemoryLog::new().expect("log");
        let dir = config_dir("{}");
        let transport = ScriptedTransport::responses(vec![ok_noul()]);
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        // A refused row (a reserved identity key) is a lost row the next flush reports.
        let session = options.session.clone().expect("session");
        let mut payload = serde_json::Map::new();
        payload.insert("repo".into(), "spoofed".into());
        assert!(
            session
                .write(sc_agent_types::SessionId::new(), payload)
                .is_err()
        );
        let mut delivered = None;
        let error = decide::send(
            &options,
            request(NOUL),
            &CancelToken::new(),
            &mut |result| {
                delivered = Some(result);
                Ok(())
            },
        )
        .expect_err("the flush reports the lost row");
        assert_eq!(sc_agent_types::ErrorId::id(&error), "decide.runner");
        let result = delivered.expect("the result was handed out first");
        assert!(result.outcome.is_ok());
    }

    /// AC: ANC-007. One send, models and doctor write `decide.send`, `decide.models` and
    /// `decide.doctor` to the injected general log, with `http.send` from the shared client;
    /// no record holds a key value.
    #[test]
    fn every_operation_writes_its_record_and_no_key() {
        let log = MemoryLog::new().expect("log");
        let dir = config_dir("{}");
        let vars = [(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)];
        let models_body =
            r#"{"models":[{"name":"jev-1.13.0","description":"d","release_date":"2026-09-15"}]}"#;
        let transport = ScriptedTransport::responses(vec![
            ok_noul(),
            json_response(200, models_body),
            json_response(200, models_body),
        ]);
        let ctx = context(dir.path(), &vars, Arc::new(transport.clone()), &log);
        send(
            &options("jev", None, ctx.clone(), &log),
            request(NOUL),
            &CancelToken::new(),
        )
        .expect("send");
        models(
            &ModelsOptions {
                runner: None,
                context: ctx.clone(),
            },
            &CancelToken::new(),
        )
        .expect("models");
        doctor(&DoctorOptions { context: ctx }, &CancelToken::new()).expect("doctor");

        let records = log.records().expect("records");
        let actions: Vec<&str> = records
            .iter()
            .map(|record| record.action.as_str())
            .collect();
        for action in [
            "decide.send",
            "decide.models",
            "decide.doctor",
            "http.send",
            "session.row",
        ] {
            assert_eq!(
                actions.iter().filter(|seen| **seen == action).count(),
                if action == "http.send" { 3 } else { 1 },
                "{action}: {actions:?}"
            );
        }
        let send_record = records
            .iter()
            .find(|record| record.action == "decide.send")
            .expect("send");
        for field in [
            "service",
            "model",
            "provider",
            "requests",
            "succeeded",
            "failed",
            "cancelled",
        ] {
            assert!(
                send_record.fields.contains_key(field),
                "decide.send lacks {field}"
            );
        }
        assert_eq!(send_record.fields["provider"], "typesafe");
        assert_eq!(send_record.fields["succeeded"], 1);
        let text = records_text(&log);
        assert!(!text.contains(TYPESAFE_KEY), "a key value reached the log");
    }

    fn doctor_context(dir: &Path, transport: ScriptedTransport, log: &MemoryLog) -> Context {
        let mut keyring = HashMap::new();
        keyring.insert(
            sc_agent_secrets::ANTHROPIC_API_KEY.to_owned(),
            ANTHROPIC_KEY.to_owned(),
        );
        Context {
            service: "decide",
            config_dir: Some(dir.to_path_buf()),
            repo_root: Some(dir.to_path_buf()),
            environment: Some(environment(&[(
                sc_agent_secrets::TYPESAFE_API_KEY,
                TYPESAFE_KEY,
            )])),
            keyring: Some(Arc::new(MapKeyring(keyring))),
            transport: Some(Arc::new(transport)),
            general: Some(log.general()),
        }
    }

    /// AC: REQ-SAG-0503. Detected keys in use order with their source (never the value);
    /// one warning per provider model missing from the catalog; one tie per config model on
    /// a built-in's release date; nothing when all are known.
    #[test]
    fn doctor_lists_keys_missing_models_and_ties() {
        let typesafe = r#"{"models":[{"name":"jev-1.13.0","description":"d","release_date":"2026-09-15"},{"name":"jev-latest","description":"d","release_date":"2026-09-15"}]}"#;
        let anthropic = r#"{"data":[{"id":"claude-haiku-5-5"},{"id":"claude-tie-1"},{"id":"claude-mystery-9"}]}"#;
        let transport = ScriptedTransport::responses(vec![
            json_response(200, typesafe),
            json_response(200, anthropic),
        ]);
        let log = MemoryLog::new().expect("log");
        let dir =
            config_dir(r#"{"models":{"claude-tie-1":{"line":"haiku","released":"2026-10-07"}}}"#);
        let report = doctor(
            &DoctorOptions {
                context: doctor_context(dir.path(), transport.clone(), &log),
            },
            &CancelToken::new(),
        )
        .expect("doctor");

        let keys: Vec<(Provider, &str)> = report
            .keys
            .iter()
            .map(|key| (key.provider, key.source.as_str()))
            .collect();
        assert_eq!(
            keys,
            [
                (Provider::TypeSafe, "env"),
                (Provider::Anthropic, "keyring")
            ]
        );
        let missing: Vec<(Provider, &str)> = report
            .missing
            .iter()
            .map(|model| (model.provider, model.model.as_str()))
            .collect();
        assert_eq!(
            missing,
            [
                (Provider::TypeSafe, "jev-latest"),
                (Provider::Anthropic, "claude-mystery-9")
            ]
        );
        assert_eq!(report.ties.len(), 1);
        assert_eq!(report.ties[0].config.as_str(), "claude-tie-1");
        assert_eq!(report.ties[0].builtin.as_str(), "claude-haiku-5-5");
        let printed = serde_json::to_value(&report).expect("json");
        assert_eq!(
            printed["keys"][0],
            serde_json::json!({"provider": "typesafe", "source": "env"})
        );
        assert_eq!(
            printed["missing"][1],
            serde_json::json!({"provider": "anthropic", "model": "claude-mystery-9"})
        );
        assert_eq!(
            printed["ties"][0],
            serde_json::json!({"config": "claude-tie-1", "builtin": "claude-haiku-5-5"})
        );
        let text = serde_json::to_string(&report).expect("json") + &records_text(&log);
        assert!(!text.contains(TYPESAFE_KEY) && !text.contains(ANTHROPIC_KEY));
        let sent = transport.requests();
        assert_eq!(
            sent[0].endpoint().as_str(),
            "https://api.typesafe.ai/v1/models"
        );
        assert_eq!(
            sent[1].endpoint().as_str(),
            "https://api.anthropic.com/v1/models"
        );

        // Everything known, no config model: no warning.
        let transport = ScriptedTransport::responses(vec![
            json_response(
                200,
                r#"{"models":[{"name":"jev-1.14.0","description":"d","release_date":"2026-10-14"}]}"#,
            ),
            json_response(
                200,
                r#"{"data":[{"id":"claude-haiku-5-5"},{"id":"claude-opus-5-5"}]}"#,
            ),
        ]);
        let dir = config_dir("{}");
        let report = doctor(
            &DoctorOptions {
                context: doctor_context(dir.path(), transport, &log),
            },
            &CancelToken::new(),
        )
        .expect("doctor");
        assert!(
            report.missing.is_empty() && report.ties.is_empty(),
            "{report:?}"
        );
        assert_eq!(report.keys.len(), 2);
    }
}
