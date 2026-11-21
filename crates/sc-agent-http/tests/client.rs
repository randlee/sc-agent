#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::{Duration, Instant};

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{
    Cancel, Client, Codec, HOLDOFF_MAX, JITTER_MAX, Policy, RETRY_AFTER_MIN, holdoff, http,
    retry_after,
};
use sc_agent_types::Error;
use serde_json::{Value, json};

/// The smallest codec: posts `{"n": n}`, reads `{"n": n}` back.
struct Echo(String);

impl Codec for Echo {
    type Request = u32;
    type Response = u32;
    fn encode(&self, n: &u32) -> Result<http::Request<Vec<u8>>, Error> {
        sc_agent_http::post_json(&self.0, None, &[], &json!({ "n": n }))
    }
    fn decode(&self, _: &u32, status: u16, body: &[u8]) -> Result<u32, Error> {
        let v: Value = sc_agent_http::json_reply(status, body)?;
        v["n"]
            .as_u64()
            .map(|n| n as u32)
            .ok_or_else(|| Error::Invalid("no n".into()))
    }
}

fn policy() -> Policy {
    Policy {
        max_sends: 3,
        backoff: Duration::ZERO,
        timeout: Duration::from_secs(5),
    }
}

#[test]
fn busy_then_ok_is_resent() {
    let server = Server::start(vec![
        (429, vec![("retry-after", "0".into())], "{}".into()),
        ok(json!({ "n": 7 })),
    ]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    let (reply, sends) = client.send_counted(&7, &Cancel::default());
    assert_eq!(reply.unwrap(), 7);
    assert_eq!(sends, 2);
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn invalid_reply_is_resent_then_gives_up() {
    let server = Server::start(vec![ok(json!({})), ok(json!({})), ok(json!({}))]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert!(matches!(
        client.send(&1, &Cancel::default()),
        Err(Error::Invalid(_))
    ));
    assert_eq!(server.finish().len(), 3);
}

#[test]
fn quota_is_not_resent() {
    let body = json!({ "error": { "type": "insufficient_quota" } }).to_string();
    let server = Server::start(vec![(429, vec![], body)]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert!(matches!(
        client.send(&1, &Cancel::default()),
        Err(Error::QuotaExhausted { status: 429, .. })
    ));
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn api_error_is_not_resent() {
    let server = Server::start(vec![(400, vec![], "bad".into())]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    let (reply, sends) = client.send_counted(&1, &Cancel::default());
    assert_eq!(sends, 1);
    assert!(matches!(reply, Err(Error::ApiStatus { status: 400, .. })));
    server.finish();
}

/// Cancels while its request is in flight, so the client is cancelled during the wait.
struct CancelOnSend(Echo, Cancel);

impl Codec for CancelOnSend {
    type Request = u32;
    type Response = u32;
    fn encode(&self, n: &u32) -> Result<http::Request<Vec<u8>>, Error> {
        self.1.cancel();
        self.0.encode(n)
    }
    fn decode(&self, n: &u32, status: u16, body: &[u8]) -> Result<u32, Error> {
        self.0.decode(n, status, body)
    }
}

#[test]
fn cancel_ends_the_wait_between_sends() {
    let server = Server::start(vec![(
        429,
        vec![("retry-after", "3600".into())],
        "{}".into(),
    )]);
    let cancel = Cancel::default();
    let client = Client::new(
        CancelOnSend(Echo(format!("{}/x", server.base)), cancel.clone()),
        policy(),
    );
    assert!(matches!(
        client.send(&1, &cancel),
        Err(Error::Cancelled { sends: 1 })
    ));
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn every_5xx_is_busy_and_resent() {
    for status in [500, 501, 503, 599] {
        let server = Server::start(vec![(status, vec![], "down".into()), ok(json!({ "n": 4 }))]);
        let client = Client::new(Echo(format!("{}/x", server.base)), policy());
        assert_eq!(
            client.send(&4, &Cancel::default()).unwrap(),
            4,
            "status {status}"
        );
        assert_eq!(server.finish().len(), 2);
    }
}

#[test]
fn batch_matches_every_result_to_its_input() {
    let server = Server::start((0..6).map(|_| ok(json!({ "n": 0 }))).collect());
    // Each reply says 0; the item still carries its own request and position.
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    let mut got = Vec::new();
    client.batch((10..16).collect(), 3, &Cancel::default(), |item| {
        assert_eq!(item.sends, 1);
        got.push((item.position, item.request, item.result.unwrap()))
    });
    got.sort_unstable();
    assert_eq!(
        got,
        (0..6).map(|i| (i, 10 + i as u32, 0)).collect::<Vec<_>>()
    );
    assert_eq!(server.finish().len(), 6);
}

#[test]
fn plain_http_with_a_key_is_refused_unless_loopback() {
    assert!(sc_agent_http::check_base("http://example.com", true).is_err());
    assert_eq!(
        sc_agent_http::check_base("http://127.0.0.1:9/", true).unwrap(),
        "http://127.0.0.1:9"
    );
    assert!(sc_agent_http::check_base("http://example.com", false).is_ok());
}

#[test]
fn retry_after_is_valid_only_as_digits_with_an_optional_fraction() {
    assert_eq!(retry_after("3"), Some(Duration::from_secs(3)));
    assert_eq!(retry_after("2.5"), Some(Duration::from_millis(2500)));
    assert_eq!(retry_after("0"), Some(Duration::ZERO));
    assert_eq!(
        retry_after(&u64::MAX.to_string()),
        Some(Duration::from_secs(u64::MAX))
    );
    assert_eq!(
        retry_after(&"9".repeat(32)),
        Some(Duration::from_secs(u64::MAX))
    );
    assert_eq!(retry_after("2.0000000000000001"), Some(Duration::new(2, 1)));
    for bad in ["", "abc", "-1", "1.", ".5", " 3", "1e3", &"9".repeat(33)] {
        assert_eq!(retry_after(bad), None, "{bad:?}");
    }
}

#[test]
fn holdoff_clamps_retry_after_and_doubles_the_backoff() {
    let s = Duration::from_secs;
    assert_eq!(holdoff(s(1), 1, Some(s(3))), s(3));
    assert_eq!(holdoff(s(1), 1, Some(s(99))), HOLDOFF_MAX);
    assert_eq!(holdoff(s(1), 1, Some(Duration::ZERO)), RETRY_AFTER_MIN);
    assert_eq!(
        holdoff(s(1), 1, retry_after(&u64::MAX.to_string())),
        HOLDOFF_MAX
    );
    let backoffs: Vec<_> = (1..=7)
        .map(|attempt| holdoff(s(1), attempt, None))
        .collect();
    assert_eq!(backoffs, [s(1), s(2), s(4), s(8), s(16), s(30), s(30)]);
}

#[test]
fn busy_reply_pauses_the_client_for_its_holdoff() {
    let server = Server::start(vec![(503, vec![("retry-after", "99".into())], "{}".into())]);
    let cancel = Cancel::default();
    let client = Client::new(
        CancelOnSend(Echo(format!("{}/x", server.base)), cancel.clone()),
        policy(),
    );
    let before = Instant::now();
    assert!(matches!(
        client.send(&1, &cancel),
        Err(Error::Cancelled { sends: 1 })
    ));
    let after = Instant::now();
    server.finish();
    // The pause is on the client, so every sender on it waits: 30 s plus at most 250 ms jitter,
    // from a moment between `before` and `after`.
    let until = client.not_before().expect("paused");
    assert!(until >= before + HOLDOFF_MAX && until <= after + HOLDOFF_MAX + JITTER_MAX);
}

#[test]
fn provider_busy_reports_the_retry_after_unclamped_and_rounded_up() {
    let max = u64::MAX.to_string();
    for (header, seconds) in [
        ("2.5", 3),
        ("2.0000000000000001", 3),
        ("99", 99),
        (max.as_str(), u64::MAX),
    ] {
        let server = Server::start(vec![(
            503,
            vec![("retry-after", header.into())],
            "{}".into(),
        )]);
        let client = Client::new(
            Echo(format!("{}/x", server.base)),
            Policy {
                max_sends: 1,
                ..policy()
            },
        );
        let result = client.send(&1, &Cancel::default());
        server.finish();
        assert!(
            matches!(result, Err(Error::ProviderBusy { attempts: 1, retry_after_seconds: Some(s) }) if s == seconds),
            "{header}: {result:?}"
        );
    }
}
