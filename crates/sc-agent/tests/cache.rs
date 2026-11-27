pub mod support;
use serde_json::Value;
use support::*;

#[test]
fn j3_cache_hit_one_model_call() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let first = run.send(&request(), None)?;
    let second = run.send(&request(), None)?;
    assert_eq!(first["cached"], false);
    assert_eq!(second["cached"], true);
    assert_eq!(first["session_id"], second["session_id"]);
    assert_eq!(first["answers"], second["answers"]);
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(rows(&root)?.len(), 1);
    let links = std::fs::read_to_string(
        root.0
            .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/runs.jsonl"),
    )?
    .lines()
    .map(serde_json::from_str::<Value>)
    .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(links.len(), 2);
    assert_eq!(links[0]["fields"]["cached"], false);
    assert_eq!(links[1]["fields"]["cached"], true);
    assert_eq!(links[1]["fields"]["session_id"], first["session_id"]);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_per_call_cache_overrides_the_run() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.cache = false;
    let run = run(opts, &logger)?;
    let first = run.send(&request(), None)?;
    let second = run.send(&request(), Some(false))?;
    let cached = run.send(&request(), Some(true))?;
    let expected_id = std::cmp::max(
        first["session_id"].as_str().ok_or("missing id")?,
        second["session_id"].as_str().ok_or("missing id")?,
    );
    assert_eq!(cached["session_id"], expected_id);
    assert_eq!(cached["cached"], true);
    assert_eq!(stub.finish()?.len(), 2);
    assert_eq!(rows(&root)?.len(), 2);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_provider_error_is_not_cached() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        http(401, serde_json::json!({"error":"denied"})),
        http(200, answer()),
    ])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let first = sc_agent::result(run.send(&request(), Some(true)));
    assert_eq!(first["error"]["id"], "http.auth");
    assert!(!root.0.join("sessions").exists());
    let second = run.send(&request(), Some(true))?;
    assert_eq!(second["cached"], false);
    let third = run.send(&request(), Some(true))?;
    assert_eq!(third["cached"], true);
    assert_eq!(stub.finish()?.len(), 2);
    let rows = rows(&root)?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["result"]["ok"], true);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_override_can_force_a_fresh_send() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    assert_eq!(run.send(&request(), None)?["cached"], false);
    assert_eq!(run.send(&request(), Some(false))?["cached"], false);
    assert_eq!(run.send(&request(), Some(true))?["cached"], true);
    assert_eq!(stub.finish()?.len(), 2);
    assert_eq!(rows(&root)?.len(), 2);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_hit_skips_holdoff_and_wait() -> Test {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        http(200, answer()),
        Reply::Http(529, vec![("Retry-After", "3600")], "busy".into()),
    ])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 1;
    let mut run = run(opts, &logger)?;
    let waits = Arc::new(AtomicUsize::new(0));
    let observed = waits.clone();
    run.wait = Box::new(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let first = run.send(&request(), None)?;
    let second = run.send(&request(), None)?;
    assert_eq!(first["cached"], false);
    assert_eq!(second["cached"], true);
    assert_eq!(waits.load(Ordering::SeqCst), 0);
    assert_eq!(stub.finish()?.len(), 1);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_batch_per_item() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new((0..12).map(|_| http(200, answer())).collect())?;
    let run = run(options(&root, &stub.url), &logger)?;
    let mut requests: Vec<Value> = (0..10)
        .map(|index| {
            let mut value = request();
            value["state"] = serde_json::json!(format!("item-{index}"));
            value
        })
        .collect();
    let first = run.send_batch(&requests, None, &|_, _| {})?;
    assert_eq!(first.as_array().ok_or("batch missing")?.len(), 10);
    requests.get_mut(2).ok_or("missing item 2")?["state"] = serde_json::json!("changed-2");
    requests.get_mut(7).ok_or("missing item 7")?["state"] = serde_json::json!("changed-7");
    let second = run.send_batch(&requests, None, &|_, _| {})?;
    let second = second.as_array().ok_or("batch missing")?;
    assert_eq!(second.len(), 10);
    for (index, row) in second.iter().enumerate() {
        assert_eq!(row["index"], index);
        assert_eq!(row["result"]["value"]["cached"], ![2, 7].contains(&index));
    }
    assert_eq!(stub.finish()?.len(), 12);
    assert_eq!(rows(&root)?.len(), 12);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_unreadable_cache_stops_before_model_send() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    std::fs::create_dir_all(&path)?;
    let stub = Stub::new(vec![])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let failure = sc_agent::result(run.send(&request(), Some(true)));
    assert_eq!(failure["error"]["id"], "session.unreadable");
    assert_eq!(stub.finish()?.len(), 0);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_misses_on_model_project_role_and_override() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new((0..5).map(|_| http(200, answer())).collect())?;
    let base = options(&root, &stub.url);
    let first = run(base.clone(), &logger)?;
    assert_eq!(first.send(&request(), None)?["cached"], false);
    assert_eq!(first.send(&request(), Some(false))?["cached"], false);
    first.close()?;
    let mut model = base.clone();
    model.settings.models.insert(
        "jev".into(),
        sc_agent::ModelEntry {
            provider: "typesafe".into(),
            id: "different-model".into(),
            released: None,
        },
    );
    let model = run(model, &logger)?;
    assert_eq!(model.send(&request(), None)?["cached"], false);
    model.close()?;
    let mut project = base.clone();
    project.settings.project = Some("other".into());
    let project = run(project, &logger)?;
    assert_eq!(project.send(&request(), None)?["cached"], false);
    project.close()?;
    let mut role = base;
    role.agent_role = "other".into();
    let role = run(role, &logger)?;
    assert_eq!(role.send(&request(), None)?["cached"], false);
    role.close()?;
    assert_eq!(stub.finish()?.len(), 5);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_uses_the_selected_row_session_id() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    let original = run.send(&request(), Some(false))?;
    let id = original["session_id"]
        .as_str()
        .ok_or("id missing")?
        .to_owned();
    let mut row = rows(&root)?.into_iter().next().ok_or("row missing")?;
    row["result"]["value"]["session_id"] = serde_json::json!("incorrect-nested-id");
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    std::fs::write(&path, format!("{}\n", serde_json::json!({"fields":row})))?;
    let cached = run.send(&request(), Some(true))?;
    assert_eq!(cached["session_id"], id);
    assert_eq!(cached["cached"], true);
    let links = std::fs::read_to_string(path.with_file_name("runs.jsonl"))?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        links.last().ok_or("link missing")?["fields"]["session_id"],
        id
    );
    assert_eq!(stub.finish()?.len(), 1);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_cache_newest_success_only() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(200, answer()), http(200, answer())])?;
    let run = run(options(&root, &stub.url), &logger)?;
    run.send(&request(), Some(false))?;
    let original = rows(&root)?.into_iter().next().ok_or("seed row missing")?;
    let mut older = original.clone();
    older["session_id"] = serde_json::json!("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    older["result"]["value"]["marker"] = serde_json::json!("older");
    let mut newer = original.clone();
    newer["session_id"] = serde_json::json!("01ARZ3NDEKTSV4RRFFQ69G5FAW");
    newer["result"]["value"]["marker"] = serde_json::json!("newer");
    let mut error = original;
    error["session_id"] = serde_json::json!("01ARZ3NDEKTSV4RRFFQ69G5FAX");
    error["result"] = serde_json::json!({"ok":false,"error":{"id":"http.auth"}});
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    std::fs::write(
        &path,
        [older, error, newer]
            .iter()
            .map(|row| format!("{}\n", serde_json::json!({"fields":row})))
            .collect::<String>(),
    )?;
    let cached = run.send(&request(), Some(true))?;
    assert_eq!(cached["session_id"], "01ARZ3NDEKTSV4RRFFQ69G5FAW");
    assert_eq!(cached["marker"], "newer");
    assert_eq!(cached["cached"], true);
    assert_eq!(stub.finish()?.len(), 1);
    assert_eq!(rows(&root)?.len(), 3);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}

#[test]
fn j3_batch_cache_override_wins_over_run_setting() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new((0..3).map(|_| http(200, answer())).collect())?;
    let run = run(options(&root, &stub.url), &logger)?;
    let fresh = run.send_batch(&vec![request(); 2], Some(false), &|_, _| {})?;
    for item in fresh.as_array().ok_or("batch missing")? {
        assert_eq!(item["result"]["value"]["cached"], false);
    }
    let cached = run.send_batch(&vec![request(); 2], Some(true), &|_, _| {})?;
    for item in cached.as_array().ok_or("batch missing")? {
        assert_eq!(item["result"]["value"]["cached"], true);
    }
    assert_eq!(stub.finish()?.len(), 2);
    assert_eq!(rows(&root)?.len(), 2);
    run.close()?;
    logger.shutdown()?;
    Ok(())
}
