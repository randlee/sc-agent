//! ANC-007: LocalBackend::discover writes one `local.discover` record per call.
//!
//! AC n = the nth acceptance line of the a-6 sprint doc.

// Each test binary uses part of the shared helpers.
#[cfg(test)]
#[allow(dead_code)]
mod support;

#[cfg(test)]
mod tests {
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_http::{HttpError, LogContext, NetworkKind};
    use sc_agent_log::Level;
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::SessionId;
    use sc_decision_local::{DaemonKind, LocalBackend, ModelState};
    use sc_decision_schema::Backend;
    use serde_json::json;

    use crate::support::{MODEL, SECRET, decided, decision_request, host, json, key, model, port};

    /// AC 17: one successful (Laya loaded) and one failed (network) discover give exactly two
    /// `local.discover` records with daemon, model, the caller's context and loaded/state or
    /// error, and no key text.
    #[test]
    fn test_discover_writes_one_record_per_call() -> Result<(), Box<dyn std::error::Error>> {
        let log = MemoryLog::new()?;
        let backend = LocalBackend::new(
            DaemonKind::Laya,
            &host("127.0.0.1"),
            port(8000),
            model(),
            Some(key()),
        )?
        .with_log(log.general());
        let transport = ScriptedTransport::new(vec![
            Ok(json(
                200,
                r#"{"status":"ok","loaded":["jev-latest","laya-small"]}"#,
            )),
            Err(HttpError::network(
                NetworkKind::Connect,
                1,
                "connection refused",
            )),
        ]);
        let session_id = SessionId::new();

        let up = backend.discover(&transport, LogContext::request(session_id, Some(0)))?;
        assert_eq!(up.model, ModelState::Loaded);
        let down = backend
            .discover(&transport, LogContext::NONE)
            .expect_err("down");
        assert_eq!(sc_agent_types::ErrorId::id(&down), "local.http");

        // Encode and decode write no local.* record.
        let request = decision_request();
        let _ = backend.encode(&request)?;
        let _ = backend.decode(decided(), &request, &LogContext::NONE)?;

        let records = log.records()?;
        let discover: Vec<_> = records
            .iter()
            .filter(|record| record.target == "sc_decision_local")
            .collect();
        assert_eq!(discover.len(), 2, "{records:?}");
        assert!(
            discover
                .iter()
                .all(|record| record.action == "local.discover")
        );
        assert!(discover.iter().all(|record| record.level == Level::Info));

        let first = &discover[0].fields;
        assert_eq!(
            first.get("session_id"),
            Some(&json!(session_id.to_string()))
        );
        assert_eq!(first.get("position"), Some(&json!(0)));
        assert_eq!(first.get("daemon"), Some(&json!("laya")));
        assert_eq!(first.get("model"), Some(&json!(MODEL)));
        assert_eq!(
            first.get("loaded"),
            Some(&json!(["jev-latest", "laya-small"]))
        );
        assert_eq!(first.get("state"), Some(&json!("loaded")));
        assert!(first.get("error").is_none());

        let second = &discover[1].fields;
        assert!(second.get("session_id").is_none());
        assert!(second.get("position").is_none());
        assert_eq!(second.get("daemon"), Some(&json!("laya")));
        assert_eq!(second.get("model"), Some(&json!(MODEL)));
        assert_eq!(second.get("error"), Some(&json!("local.http")));
        assert!(second.get("state").is_none());

        for record in &records {
            let text = serde_json::to_string(&record.fields)?;
            assert!(!text.contains(SECRET), "key in a record: {text}");
        }
        assert!(!format!("{backend:?}").contains(SECRET));
        assert!(format!("{backend:?}").contains("log: \"<set>\""));
        Ok(())
    }

    /// Without a logger, discover writes nothing and returns the same result.
    #[test]
    fn test_discover_without_logger_writes_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let log = MemoryLog::new()?;
        let backend = LocalBackend::new(
            DaemonKind::Mapika,
            &host("127.0.0.1"),
            port(8000),
            model(),
            None,
        )?;
        let transport =
            ScriptedTransport::responses(vec![json(200, r#"{"ok":true,"model":"jev-latest"}"#)]);
        assert_eq!(
            backend.discover(&transport, LogContext::NONE)?.model,
            ModelState::Loaded
        );
        assert!(log.records()?.is_empty());
        Ok(())
    }
}
