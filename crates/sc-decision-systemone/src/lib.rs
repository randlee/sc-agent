#![forbid(unsafe_code)]
//! The Jev API itself: TypeSafe, and local runners that serve it.

use sc_agent_http::{Codec, http};
use sc_agent_types::Error;
use sc_decision_schema::{DecisionRequest, DecisionResponse};

/// The Jev API itself: TypeSafe, and local runners that serve it.
pub struct SystemOne {
    url: String,
    auth: Option<String>,
}

impl SystemOne {
    pub fn new(base: &str, key: Option<&str>) -> Result<Self, Error> {
        let base = sc_agent_http::check_base(base, key.is_some())?;
        Ok(Self { url: format!("{base}/v1/systemone"), auth: key.map(|k| format!("Bearer {k}")) })
    }
}

impl Codec for SystemOne {
    type Request = DecisionRequest;
    type Response = DecisionResponse;

    fn encode(&self, request: &DecisionRequest) -> Result<http::Request<Vec<u8>>, Error> {
        let body = serde_json::to_value(request).map_err(|e| Error::Encode(e.to_string()))?;
        sc_agent_http::post_json(&self.url, self.auth.as_deref().map(|a| ("authorization", a)), &[], &body)
    }

    fn decode(&self, _: &DecisionRequest, status: u16, body: &[u8]) -> Result<DecisionResponse, Error> {
        sc_agent_http::json_reply(status, body)
    }
}
