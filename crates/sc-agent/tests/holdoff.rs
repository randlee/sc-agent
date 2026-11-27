pub mod support;
use std::sync::{Arc, Mutex};
use support::*;

// 2^63 parses exactly through f64 and never expires during a test.
// Callbacks drive completion, not wall time.
const RETRY_AFTER: &str = "9223372036854775808";

#[test]
fn final_busy_reply_holds_off_the_next_send() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        Reply::Http(429, vec![("Retry-After", RETRY_AFTER)], "busy".into()),
        http(200, answer()),
    ])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 1;
    let mut client = run(opts, &logger)?;
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = waits.clone();
    client.wait = Box::new(move |delay| {
        recorded
            .lock()
            .map_err(|e| sc_agent::error("http.network", e))?
            .push(delay);
        Ok(())
    });
    let first = sc_agent::result(client.send(&request()));
    assert_eq!(first["error"]["id"], "http.provider_busy");
    assert!(
        waits
            .lock()
            .map_err(|_| "wait recorder poisoned")?
            .is_empty()
    );
    client.send(&request())?;
    assert_eq!(waits.lock().map_err(|_| "wait recorder poisoned")?.len(), 1);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn callback_failure_does_not_clear_the_host_deadline() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        Reply::Http(
            429,
            vec![("Retry-After", RETRY_AFTER)],
            "busy lower detail".into(),
        ),
        http(200, answer()),
    ])?;
    let mut client = run(options(&root, &stub.url), &logger)?;
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = waits.clone();
    client.wait = Box::new(move |delay| {
        let mut waits = recorded
            .lock()
            .map_err(|e| sc_agent::error("http.network", e))?;
        waits.push(delay);
        if waits.len() == 1 {
            return Err(sc_agent::error(
                "sc_agent_py.callback",
                "controlled callback failure",
            ));
        }
        Ok(())
    });
    let first = sc_agent::result(client.send(&request()));
    assert_eq!(first["error"]["id"], "http.provider_busy");
    let message = first["error"]["message"]
        .as_str()
        .ok_or("missing message")?;
    assert!(message.contains("busy lower detail"));
    assert!(message.contains("controlled callback failure"));
    client.send(&request())?;
    assert_eq!(waits.lock().map_err(|_| "wait recorder poisoned")?.len(), 2);
    assert_eq!(stub.finish()?.len(), 2);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn deadline_is_per_host_within_one_run_and_isolated_between_runs() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        Reply::Http(429, vec![("Retry-After", RETRY_AFTER)], "busy".into()),
        http(200, answer()),
        http(200, answer()),
    ])?;
    let other = Stub::at(vec![http(200, answer())], "[::1]:0".parse()?)?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 1;
    let mut client = run(opts.clone(), &logger)?;
    let waits = Arc::new(Mutex::new(Vec::new()));
    let recorded = waits.clone();
    client.wait = Box::new(move |delay| {
        recorded
            .lock()
            .map_err(|e| sc_agent::error("http.network", e))?
            .push(delay);
        Ok(())
    });
    assert!(client.send(&request()).is_err());
    run(opts, &logger)?.send(&request())?;
    client.options.base_url = Some(other.url.clone());
    client.send(&request())?;
    assert!(
        waits
            .lock()
            .map_err(|_| "wait recorder poisoned")?
            .is_empty()
    );
    client.options.base_url = Some(stub.url.clone());
    client.send(&request())?;
    assert_eq!(waits.lock().map_err(|_| "wait recorder poisoned")?.len(), 1);
    assert_eq!(stub.finish()?.len(), 3);
    assert_eq!(other.finish()?.len(), 1);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn simultaneous_followers_wait_for_one_deadline_without_serializing() -> Test {
    use std::{
        sync::{Condvar, mpsc},
        time::Duration,
    };
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![
        Reply::Http(529, vec![("Retry-After", RETRY_AFTER)], "busy".into()),
        http(200, answer()),
        http(200, answer()),
    ])?;
    let mut opts = options(&root, &stub.url);
    opts.settings.max_sends = 1;
    let mut client = run(opts, &logger)?;
    assert!(client.send(&request()).is_err());
    let released = Arc::new((Mutex::new(false), Condvar::new()));
    let release = released.clone();
    let (entered, observed) = mpsc::channel();
    client.wait = Box::new(move |delay| {
        entered
            .send(delay)
            .map_err(|e| sc_agent::error("http.network", e))?;
        let (lock, ready) = &*release;
        let guard = lock
            .lock()
            .map_err(|e| sc_agent::error("http.network", e))?;
        let (guard, _) = ready
            .wait_timeout_while(guard, Duration::from_secs(10), |released| !*released)
            .map_err(|e| sc_agent::error("http.network", e))?;
        if !*guard {
            return Err(sc_agent::error(
                "http.network",
                "test holdoff was not released",
            ));
        }
        Ok(())
    });
    let client = Arc::new(client);
    let workers = (0..2)
        .map(|_| {
            let client = client.clone();
            std::thread::spawn(move || client.send(&request()))
        })
        .collect::<Vec<_>>();
    let entered = [
        observed.recv_timeout(Duration::from_secs(10)),
        observed.recv_timeout(Duration::from_secs(10)),
    ];
    *released.0.lock().map_err(|_| "release flag poisoned")? = true;
    released.1.notify_all();
    for worker in workers {
        worker.join().map_err(|_| "send worker panicked")??;
    }
    assert_eq!(entered.into_iter().collect::<Result<Vec<_>, _>>()?.len(), 2);
    assert_eq!(stub.finish()?.len(), 3);
    logger.shutdown()?;
    Ok(())
}

#[test]
fn cancelling_from_the_holdoff_wait_prevents_a_resend() -> Test {
    let root = Temp::new()?;
    let logger = logger(&root)?;
    let stub = Stub::new(vec![Reply::Http(
        429,
        vec![("Retry-After", RETRY_AFTER)],
        "busy".into(),
    )])?;
    let mut client = run(options(&root, &stub.url), &logger)?;
    let cancel = client.cancel.clone();
    client.wait = Box::new(move |_| {
        cancel.cancel();
        Ok(())
    });
    let failure = sc_agent::result(client.send(&request()));
    assert_eq!(failure["error"]["id"], "decision.cancelled");
    assert!(client.cancel.is_cancelled());
    assert_eq!(stub.finish()?.len(), 1);
    logger.shutdown()?;
    Ok(())
}
