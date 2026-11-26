pub mod support;
use sc_agent::{
    Call, Environment, KeyringError, Run, Secret,
    providers::{self, Base, KeySources},
};
use serde_json::json;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use support::*;

#[test]
fn table_order_and_resolution_with_configured_overrides() -> Test {
    let cases = [
        (
            "haiku",
            json!({}),
            vec!["ANTHROPIC_API_KEY"],
            "anthropic",
            "claude-haiku-5-5",
        ),
        (
            "haiku",
            json!({}),
            vec!["NOUS_API_KEY"],
            "nous",
            "anthropic/claude-haiku-5.5",
        ),
        (
            "luna",
            json!({}),
            vec!["NOUS_API_KEY"],
            "nous",
            "openai/gpt-6-luna",
        ),
        (
            "anthropic/claude-haiku-5.5",
            json!({}),
            vec!["OPENROUTER_API_KEY"],
            "openrouter",
            "anthropic/claude-haiku-5.5",
        ),
        (
            "claude-haiku-5-5",
            json!({}),
            vec!["ANTHROPIC_API_KEY"],
            "anthropic",
            "claude-haiku-5-5",
        ),
        (
            "luna",
            json!({"luna":{"provider":"nous","id":"openai/gpt-6.1-luna","released":"2026-11-20"}}),
            vec!["NOUS_API_KEY"],
            "nous",
            "openai/gpt-6.1-luna",
        ),
        (
            "haiku",
            json!({"haiku":{"provider":"anthropic","id":"claude-haiku-5-6"}}),
            vec!["ANTHROPIC_API_KEY", "NOUS_API_KEY"],
            "anthropic",
            "claude-haiku-5-6",
        ),
        (
            "jev",
            json!({"jev":{"provider":"typesafe","id":"jev-1.14.0"}}),
            vec!["TYPESAFE_API_KEY"],
            "typesafe",
            "jev-1.14.0",
        ),
    ];
    for (selector, models, keys, provider, id) in cases {
        let root = Temp::new()?;
        let stub = Stub::new(vec![])?;
        fs::write(
            root.0.join("sc-agent.json"),
            json!({"models":models}).to_string(),
        )?;
        let mut config = run_config(&root, &stub)?;
        let mut env = keys
            .into_iter()
            .map(|k| (k.into(), "dummy-key".into()))
            .collect::<Vec<_>>();
        env.push(("DECIDR_LOG_ROOT".into(), root.0.as_os_str().to_owned()));
        config.seams.environment = Some(Environment(Secret::new(env)));
        config.seams.keyring = Arc::new(|_| Ok(String::new()));
        config.model = Some(selector.into());
        let run = Run::new(config)?;
        assert_eq!(
            (run.model().provider.name, run.model().id.as_str()),
            (provider, id)
        );
        run.close()?;
        assert!(stub.finish()?.is_empty());
    }
    assert!(
        providers::MODELS
            .windows(2)
            .all(|rows| rows[0].released >= rows[1].released)
    );
    let mut identities = std::collections::BTreeSet::new();
    for row in providers::MODELS {
        assert!(
            providers::PROVIDERS
                .iter()
                .any(|p| std::ptr::eq(*p, row.provider))
        );
        assert!(identities.insert((row.provider.name, row.id)));
        assert!(!row.released.is_empty());
    }
    Ok(())
}
#[test]
fn local_fixed_ids_and_echo_mismatch() -> Test {
    for (selector, endpoint, id, mismatch) in [
        ("decider", "decider_4b", "decider-4b-v2.1", false),
        ("decider-2b-v11", "decider_2b", "decider-2b-v11", false),
        ("laya", "laya", "laya:en", false),
        ("decider", "decider_4b", "decider-4b-v2.1", true),
    ] {
        let root = Temp::new()?;
        let mut response = answer();
        response["model"] = json!(if mismatch { "decider-2b-v11" } else { id });
        let stub = Stub::new(vec![http(200, response)])?;
        let port: u16 = stub
            .url
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches("/v1")
            .parse()?;
        fs::write(
            root.0.join("sc-agent.json"),
            json!({"project":"phase-b",endpoint:{"host":"127.0.0.1","port":port}}).to_string(),
        )?;
        let mut cfg = run_config(&root, &stub)?;
        cfg.model = Some(selector.into());
        cfg.seams.base_url = None;
        let run = Run::new(cfg)?;
        let outcome = sc_agent::result(run.send(&Call::default(), &request()));
        if mismatch {
            assert_eq!(outcome["error"]["id"], "decision.model_mismatch");
            assert_eq!(outcome["error"]["category"], "response_invalid");
        } else {
            assert_eq!(outcome["value"]["model"], id);
        }
        let seen = stub.finish()?;
        let sent = seen.first().ok_or("missing send")?;
        assert_eq!(sent.path, "/v1/systemone");
        assert_eq!(sent.body["model"], id);
        assert!(
            !sent
                .headers
                .iter()
                .any(|(k, _)| k == "authorization" || k == "x-api-key")
        );
        run.close()?;
    }
    Ok(())
}
#[test]
fn credentials_are_snapshotted_and_keyring_is_memoized() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let mut cfg = run_config(&root, &stub)?;
    cfg.seams.environment = Some(Environment(Secret::new(vec![(
        "DECIDR_LOG_ROOT".into(),
        root.0.as_os_str().to_owned(),
    )])));
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    cfg.seams.keyring = Arc::new(move |_| {
        called.fetch_add(1, Ordering::SeqCst);
        Ok("ring-key".into())
    });
    let run = Run::new(cfg)?;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    fs::write(root.0.join(".env"), "TYPESAFE_API_KEY=changed-key")?;
    for _ in 0..2 {
        run.send(&Call::default(), &request())?;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    for sent in stub.finish()? {
        assert!(
            sent.headers
                .iter()
                .any(|(k, v)| k == "authorization" && v == "Bearer ring-key")
        );
    }
    run.close()?;
    Ok(())
}
#[test]
fn credential_order_malformed_values_and_redacting_debug() -> Test {
    let root = Temp::new()?;
    fs::write(root.0.join(".env"), "TYPESAFE_API_KEY=dotenv-key")?;
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let ring: Arc<providers::KeyringLookup> = Arc::new(move |_| {
        called.fetch_add(1, Ordering::SeqCst);
        Ok("ring-key".into())
    });
    let env = Environment(Secret::new(vec![(
        "TYPESAFE_API_KEY".into(),
        "env-key".into(),
    )]));
    let source = KeySources::new(env.clone(), &root.0, ring.clone())?;
    let credential = source.find("TYPESAFE_API_KEY")?.ok_or("missing env key")?;
    assert_eq!(credential.value.expose(), "env-key");
    let debug = format!("{env:?} {credential:?}");
    for key in ["env-key", "dotenv-key", "ring-key"] {
        assert!(!debug.contains(key));
    }
    let source = KeySources::new(Environment::default(), &root.0, ring.clone())?;
    assert_eq!(
        source
            .find("TYPESAFE_API_KEY")?
            .ok_or("missing dotenv key")?
            .value
            .expose(),
        "dotenv-key"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    fs::remove_file(root.0.join(".env"))?;
    let source = KeySources::new(Environment::default(), &root.0, ring.clone())?;
    assert_eq!(
        source
            .find("TYPESAFE_API_KEY")?
            .ok_or("missing ring key")?
            .value
            .expose(),
        "ring-key"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    for value in ["", "with space", "line\nend", "control\u{7f}", "nonascii-é"] {
        let source = KeySources::new(
            Environment(Secret::new(vec![("TYPESAFE_API_KEY".into(), value.into())])),
            &root.0,
            ring.clone(),
        )?;
        let error = source
            .find("TYPESAFE_API_KEY")
            .err()
            .ok_or("invalid key accepted")?;
        assert_eq!(
            error.id(),
            if value.is_empty() {
                "secrets.key_missing"
            } else {
                "secrets.key_malformed"
            }
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    for (ring, id) in [
        (
            Arc::new(|_: &str| Err(KeyringError::Timeout)) as Arc<providers::KeyringLookup>,
            "secrets.keyring_timeout",
        ),
        (
            Arc::new(|_: &str| Err(KeyringError::Read("controlled refusal".into())))
                as Arc<providers::KeyringLookup>,
            "secrets.keyring_read",
        ),
    ] {
        let error = KeySources::new(Environment::default(), &root.0, ring)?
            .find("TYPESAFE_API_KEY")
            .err()
            .ok_or("failure lost")?;
        assert_eq!(error.id(), id);
        assert!(!error.to_string().contains("controlled refusal"));
    }
    Ok(())
}
#[test]
fn configured_missing_key_never_falls_back_and_doctor_lists_source_errors() -> Test {
    let root = Temp::new()?;
    let stub = Stub::new(vec![])?;
    fs::write(
        root.0.join("sc-agent.json"),
        json!({"models":{"haiku":{"provider":"anthropic","id":"claude-haiku-5-6"}}}).to_string(),
    )?;
    let mut cfg = run_config(&root, &stub)?;
    cfg.model = Some("haiku".into());
    cfg.seams.environment = Some(Environment(Secret::new(vec![(
        "NOUS_API_KEY".into(),
        "dummy-nous".into(),
    )])));
    cfg.seams.keyring = Arc::new(|_| Ok(String::new()));
    let failure = Run::new(cfg).err().ok_or("fallback accepted")?;
    assert_eq!(failure.id(), "secrets.key_missing");
    assert!(failure.to_string().contains("ANTHROPIC_API_KEY"));
    let env = Environment(Secret::new(vec![(
        "ANTHROPIC_API_KEY".into(),
        "dummy-anthropic".into(),
    )]));
    let keys = KeySources::new(env, &root.0, Arc::new(keyring))?;
    let rows = providers::doctor_key_rows(&keys);
    assert_eq!(
        rows.iter()
            .map(|r| r["name"].as_str().unwrap_or(""))
            .collect::<Vec<_>>(),
        providers::provider_keys()
    );
    assert_eq!(rows[0]["error"]["id"], "secrets.keyring_read");
    assert_eq!(rows[1]["source"], "Environment");
    for p in providers::PROVIDERS {
        if let Base::Local(name) = p.base {
            assert!(p.key.is_none());
            assert!(!name.is_empty());
        }
    }
    Ok(())
}
