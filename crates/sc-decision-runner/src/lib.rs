#![forbid(unsafe_code)]
//! Decisions: one request form, a typed answer per question, any backend.

use sc_agent_http::{Cancel, Client, Policy};
use sc_agent_log::Session;
use sc_agent_types::Error;
use sc_decision_schema::{Backend, DecisionRequest, DecisionResponse};
use serde_json::{Map, Value, json};

/// One decided request, matched to its input.
#[derive(Debug)]
pub struct Outcome {
    pub position: usize,
    pub session: ulid::Ulid,
    pub request: DecisionRequest,
    pub result: Result<DecisionResponse, Error>,
}

impl Outcome {
    pub fn failed(position: usize, request: DecisionRequest, error: Error) -> Self {
        Self {
            position,
            session: ulid::Ulid::new(),
            request,
            result: Err(error),
        }
    }
}

/// What callers use, whatever is behind it.
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
        let result = self.client.send(&request, cancel);
        let outcome = Outcome {
            position: 0,
            session: ulid::Ulid::new(),
            request,
            result,
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
        self.client.batch(requests, workers, cancel, |item| {
            let outcome = Outcome {
                position: item.position,
                session: item.session,
                request: item.request,
                result: item.result,
            };
            self.record(&outcome);
            on_result(outcome);
        });
    }
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
    let digests: Map<String, Value> = outcome
        .request
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
        .collect();
    fields.insert("digests".into(), digests.into());
    match &outcome.result {
        Ok(response) => fields.insert(
            "response".into(),
            serde_json::to_value(response).unwrap_or_default(),
        ),
        Err(error) => fields.insert(
            "error".into(),
            json!({ "id": error.id(), "message": error.to_string() }),
        ),
    };
    fields
}
