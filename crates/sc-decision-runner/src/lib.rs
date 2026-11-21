#![forbid(unsafe_code)]
//! Decisions: one request form, a typed answer per question, any backend.

use sc_agent_http::{Cancel, Client, Policy};
use sc_agent_log::Session;
use sc_agent_types::Error;
use sc_decision_schema::{Backend, DecisionRequest, DecisionResponse};
use serde_json::{Map, Value, json};

/// The shared client's result, with decision request and response types.
pub type Outcome = sc_agent_http::Item<DecisionRequest, DecisionResponse>;

pub fn failed(position: usize, request: DecisionRequest, error: Error) -> Outcome {
    Outcome {
        position,
        session: ulid::Ulid::new(),
        request,
        result: Err(error),
        sends: 0,
    }
}

/// What callers use, whatever is behind it.
/// This is an open trait; downstream crates may implement it.
pub trait Decision: Send + Sync {
    fn decide(&self, request: DecisionRequest, cancel: &Cancel) -> Outcome;

    /// Results reach `on_result` in any order, each with its input position.
    fn decide_batch(
        &self,
        requests: Vec<DecisionRequest>,
        workers: usize,
        cancel: &Cancel,
        on_result: &mut dyn FnMut(Outcome),
    );
}

/// A backend on the shared client, writing one session row per result.
pub struct Decider<B> {
    client: Client<B>,
    session: Option<Session>,
}

impl<B: Backend> Decider<B> {
    pub fn new(backend: B, policy: Policy, session: Option<Session>) -> Self {
        Self {
            client: Client::new(backend, policy),
            session,
        }
    }

    fn record(&self, outcome: &Outcome) {
        if let Some(session) = &self.session {
            session.row(row(outcome));
        }
    }
}

impl<B: Backend> Decision for Decider<B> {
    fn decide(&self, request: DecisionRequest, cancel: &Cancel) -> Outcome {
        let (result, sends) = self.client.send_counted(&request, cancel);
        let outcome = Outcome {
            position: 0,
            session: ulid::Ulid::new(),
            request,
            result,
            sends,
        };
        self.record(&outcome);
        outcome
    }

    fn decide_batch(
        &self,
        requests: Vec<DecisionRequest>,
        workers: usize,
        cancel: &Cancel,
        on_result: &mut dyn FnMut(Outcome),
    ) {
        self.client.batch(requests, workers, cancel, |outcome| {
            self.record(&outcome);
            on_result(outcome);
        });
    }
}

/// The same question digests are carried by session rows and public replies.
pub fn digests(request: &DecisionRequest) -> Map<String, Value> {
    request
        .questions
        .iter()
        .map(|(id, q)| {
            (
                id.clone(),
                DecisionRequest::digest(id, q)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
            )
        })
        .collect()
}

/// The session row of one outcome: the request text, per-question digests, and the response or error.
fn row(outcome: &Outcome) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("session_id".into(), outcome.session.to_string().into());
    fields.insert("position".into(), outcome.position.into());
    fields.insert(
        "request".into(),
        serde_json::to_string(&outcome.request)
            .unwrap_or_default()
            .into(),
    );
    fields.insert("digests".into(), digests(&outcome.request).into());
    match &outcome.result {
        Ok(response) => fields.insert(
            "response".into(),
            serde_json::to_value(response).unwrap_or_default(),
        ),
        Err(error) => fields.insert(
            "error".into(),
            json!({ "id": error.id(), "message": error.to_string(), "attempts": outcome.sends }),
        ),
    };
    fields
}
