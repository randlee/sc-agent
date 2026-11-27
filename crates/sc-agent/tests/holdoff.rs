pub mod support;
use sc_agent::host_holdoff;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::*;

#[test]
fn req_sag_0601_holdoff_lock_is_shared_only_by_host_and_held_during_wait() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![http(429, json!({})), http(200, answer())])?;
    let host = host_holdoff("127.0.0.1")?;
    assert!(Arc::ptr_eq(&host, &host_holdoff("127.0.0.1")?));
    let other = host_holdoff("independent.invalid")?;
    assert!(!Arc::ptr_eq(&host, &other));
    let observations = Arc::new(Mutex::new(vec![]));
    let mut run = run(options(&root, &stub.url), &logger)?;
    let recorded = observations.clone();
    run.wait = Box::new(move |_| {
        let blocked = matches!(host.try_lock(), Err(std::sync::TryLockError::WouldBlock));
        let independent = other.try_lock().is_ok();
        if let Ok(mut seen) = recorded.lock() {
            seen.push((blocked, independent));
        }
        Ok(())
    });
    run.send(&request())?;
    assert_eq!(
        observations
            .lock()
            .map(|v| v.clone())
            .map_err(|_| "holdoff recorder poisoned")?,
        vec![(true, true)]
    );
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}
