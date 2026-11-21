#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Client, Codec, Policy, holdoff, http, retry_after};
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
fn busy_responses_are_resent() {
    for status in [429, 500, 501, 502, 503, 529, 599] {
        let server = Server::start(vec![
            (status, vec![("retry-after", "0".into())], "down".into()),
            ok(json!({ "n": 7 })),
        ]);
        let client = Client::new(Echo(format!("{}/x", server.base)), policy());
        assert_eq!(
            client.send(&7, &Cancel::default()).unwrap(),
            7,
            "status {status}"
        );
        assert_eq!(server.finish().len(), 2);
    }
}

#[test]
fn invalid_and_malformed_replies_are_resent() {
    for malformed in [false, true] {
        let replies = if malformed {
            vec![(200, vec![], "not json".into()), ok(json!({ "n": 7 }))]
        } else {
            vec![ok(json!({})), ok(json!({})), ok(json!({}))]
        };
        let server = Server::start(replies);
        let client = Client::new(Echo(format!("{}/x", server.base)), policy());
        let result = client.send(&7, &Cancel::default());
        if malformed {
            assert!(matches!(result, Ok(7)));
        } else {
            assert!(matches!(result, Err(Error::Invalid(_))));
        }
        assert_eq!(server.finish().len(), if malformed { 2 } else { 3 });
    }
}

#[test]
fn quota_and_client_errors_are_not_resent() {
    for status in [400, 401, 429] {
        let body = if status == 429 {
            json!({ "error": { "type": "insufficient_quota" } }).to_string()
        } else {
            "bad".into()
        };
        let server = Server::start(vec![(status, vec![], body)]);
        let client = Client::new(Echo(format!("{}/x", server.base)), policy());
        let result = client.send(&1, &Cancel::default());
        if status == 429 {
            assert!(matches!(
                result,
                Err(Error::QuotaExhausted { status: 429, .. })
            ));
        } else {
            assert!(matches!(result, Err(Error::ApiStatus { status: got, .. }) if got == status));
        }
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn batch_matches_every_result_to_its_input() {
    let server = Server::start((0..6).map(|_| ok(json!({ "n": 0 }))).collect());
    // Each reply says 0; the item still carries its own request and position.
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    let mut got = Vec::new();
    client.batch((10..16).collect(), 3, &Cancel::default(), |item| {
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
fn retry_after_and_backoff_are_unclamped() {
    let seconds = Duration::from_secs;
    assert_eq!(retry_after("3"), Some(seconds(3)));
    assert_eq!(retry_after("2.5"), Some(Duration::from_millis(2500)));
    assert_eq!(retry_after("-1"), None);
    assert_eq!(retry_after("1e999"), Some(Duration::MAX));
    assert_eq!(
        holdoff(Duration::from_nanos(1), 35, None),
        Duration::from_nanos(1_u64 << 34)
    );
    assert_eq!(retry_after("not-a-number"), None);
    assert_eq!(holdoff(seconds(1), 1, Some(seconds(99))), seconds(99));
    assert_eq!(holdoff(seconds(1), 1, Some(Duration::ZERO)), Duration::ZERO);
    assert_eq!(
        (1..=4)
            .map(|n| holdoff(seconds(1), n, None))
            .collect::<Vec<_>>(),
        [seconds(1), seconds(2), seconds(4), seconds(8)]
    );
}

#[test]
fn busy_exhaustion_reports_attempts_and_last_retry_after() {
    for (limit, last_header, seconds) in [(2, "2.5", 3), (5, "99", 99)] {
        let mut replies = vec![(503, vec![("retry-after", "0".into())], "down".into()); limit - 1];
        replies.push((
            503,
            vec![("retry-after", last_header.into())],
            "down".into(),
        ));
        let server = Server::start(replies);
        let client = Client::new(
            Echo(format!("{}/x", server.base)),
            Policy {
                max_sends: limit as u32,
                ..policy()
            },
        );
        assert!(
            matches!(client.send(&1, &Cancel::default()), Err(Error::ProviderBusy { attempts, retry_after_seconds: Some(last) }) if attempts == limit as u32 && last == seconds)
        );
        assert_eq!(server.finish().len(), limit);
    }
}
