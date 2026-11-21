#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use std::time::Duration;

use sc_agent_http::test_server::{Server, ok};
use sc_agent_http::{Cancel, Client, Codec, Policy, http};
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
        v["n"].as_u64().map(|n| n as u32).ok_or_else(|| Error::Invalid("no n".into()))
    }
}

fn policy() -> Policy {
    Policy { max_sends: 3, backoff: Duration::ZERO, timeout: Duration::from_secs(5) }
}

#[test]
fn busy_then_ok_is_resent() {
    let server = Server::start(vec![(429, vec![("retry-after", "0".into())], "{}".into()), ok(json!({ "n": 7 }))]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert_eq!(client.send(&7, &Cancel::default()).unwrap(), 7);
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn invalid_reply_is_resent_then_gives_up() {
    let server = Server::start(vec![ok(json!({})), ok(json!({})), ok(json!({}))]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert!(matches!(client.send(&1, &Cancel::default()), Err(Error::Invalid(_))));
    assert_eq!(server.finish().len(), 3);
}

#[test]
fn quota_is_not_resent() {
    let body = json!({ "error": { "type": "insufficient_quota" } }).to_string();
    let server = Server::start(vec![(429, vec![], body)]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert!(matches!(client.send(&1, &Cancel::default()), Err(Error::QuotaExhausted { status: 429, .. })));
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn api_error_is_not_resent() {
    let server = Server::start(vec![(400, vec![], "bad".into())]);
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    assert!(matches!(client.send(&1, &Cancel::default()), Err(Error::ApiStatus { status: 400, .. })));
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
    let server = Server::start(vec![(429, vec![("retry-after", "3600".into())], "{}".into())]);
    let cancel = Cancel::default();
    let client = Client::new(CancelOnSend(Echo(format!("{}/x", server.base)), cancel.clone()), policy());
    assert!(matches!(client.send(&1, &cancel), Err(Error::Cancelled { sends: 1 })));
    assert_eq!(server.finish().len(), 1);
}

#[test]
fn every_5xx_is_busy_and_resent() {
    for status in [500, 501, 503, 599] {
        let server = Server::start(vec![(status, vec![], "down".into()), ok(json!({ "n": 4 }))]);
        let client = Client::new(Echo(format!("{}/x", server.base)), policy());
        assert_eq!(client.send(&4, &Cancel::default()).unwrap(), 4, "status {status}");
        assert_eq!(server.finish().len(), 2);
    }
}

#[test]
fn busy_reply_pauses_every_sender_on_the_client() {
    let server = Server::start(vec![(503, vec![("retry-after", "3600".into())], "{}".into())]);
    let cancel = Cancel::default();
    let client = Client::new(CancelOnSend(Echo(format!("{}/x", server.base)), cancel.clone()), policy());
    assert!(matches!(client.send(&1, &cancel), Err(Error::Cancelled { sends: 1 })));
    assert_eq!(server.finish().len(), 1);
    // Shared: the pause the first sender hit holds for the client, not just that request.
    assert!(client.paused_for() > Duration::from_secs(3500));
}

#[test]
fn retry_after_zero_still_waits_the_backoff_floor() {
    let server = Server::start(vec![(429, vec![("retry-after", "0".into())], "{}".into())]);
    let cancel = Cancel::default();
    let floor = Policy { backoff: Duration::from_secs(3600), ..policy() };
    let client = Client::new(CancelOnSend(Echo(format!("{}/x", server.base)), cancel.clone()), floor);
    assert!(matches!(client.send(&1, &cancel), Err(Error::Cancelled { sends: 1 })));
    server.finish();
    assert!(client.paused_for() > Duration::from_secs(3500));
}

#[test]
fn batch_matches_every_result_to_its_input() {
    let server = Server::start((0..6).map(|_| ok(json!({ "n": 0 }))).collect());
    // Each reply says 0; the item still carries its own request and position.
    let client = Client::new(Echo(format!("{}/x", server.base)), policy());
    let mut got = Vec::new();
    client.batch((10..16).collect(), 3, &Cancel::default(), |item| got.push((item.position, item.request, item.result.unwrap())));
    got.sort_unstable();
    assert_eq!(got, (0..6).map(|i| (i, 10 + i as u32, 0)).collect::<Vec<_>>());
    assert_eq!(server.finish().len(), 6);
}

#[test]
fn plain_http_with_a_key_is_refused_unless_loopback() {
    assert!(sc_agent_http::check_base("http://example.com", true).is_err());
    assert_eq!(sc_agent_http::check_base("http://127.0.0.1:9/", true).unwrap(), "http://127.0.0.1:9");
    assert!(sc_agent_http::check_base("http://example.com", false).is_ok());
}
