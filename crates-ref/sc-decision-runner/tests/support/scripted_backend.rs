//! [`ScriptedBackend`]: a test `Backend` with replaceable encode and decode (a-9 tests only).

use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

use sc_agent_http::{Endpoint, HttpRequest, HttpResponse, LogContext, Method};
use sc_decision_schema::{Backend, DecisionError, DecisionRequest, SystemOneResponse};

type EncodeFn = Box<dyn Fn(&DecisionRequest) -> Result<HttpRequest, DecisionError> + Send + Sync>;
type DecodeFn = Box<
    dyn Fn(
            u32,
            HttpResponse,
            &DecisionRequest,
            &LogContext,
        ) -> Result<SystemOneResponse, DecisionError>
        + Send
        + Sync,
>;

/// A backend whose encode and decode are closures; counts its calls.
pub struct ScriptedBackend {
    encode: EncodeFn,
    decode: DecodeFn,
    encodes: AtomicU32,
    decodes: AtomicU32,
}

/// POST https://scripted.test/v1/systemone with the JSON body.
pub fn default_encode(request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
    encode_to("https://scripted.test/v1/systemone", request)
}

/// POST `url` with the JSON body.
pub fn encode_to(url: &str, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
    Ok(HttpRequest::new(
        Method::Post,
        Endpoint::parse(url)?,
        request.body().to_json()?.into_bytes(),
    ))
}

impl ScriptedBackend {
    /// Encode: [`default_encode`]; decode: `SystemOneResponse::from_json`.
    pub fn new() -> Self {
        Self {
            encode: Box::new(default_encode),
            decode: Box::new(|_, response, _, _| {
                SystemOneResponse::from_json(response.status(), response.body())
            }),
            encodes: AtomicU32::new(0),
            decodes: AtomicU32::new(0),
        }
    }

    /// Replaces encode.
    pub fn with_encode(
        mut self,
        encode: impl Fn(&DecisionRequest) -> Result<HttpRequest, DecisionError> + Send + Sync + 'static,
    ) -> Self {
        self.encode = Box::new(encode);
        self
    }

    /// Replaces decode; the closure gets the 0-based index of the decode call.
    pub fn with_decode(
        mut self,
        decode: impl Fn(
            u32,
            HttpResponse,
            &DecisionRequest,
            &LogContext,
        ) -> Result<SystemOneResponse, DecisionError>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.decode = Box::new(decode);
        self
    }

    /// Encode calls so far.
    pub fn encodes(&self) -> u32 {
        self.encodes.load(Ordering::Acquire)
    }

    /// Decode calls so far.
    pub fn decodes(&self) -> u32 {
        self.decodes.load(Ordering::Acquire)
    }
}

impl fmt::Debug for ScriptedBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptedBackend")
            .field("encodes", &self.encodes())
            .field("decodes", &self.decodes())
            .finish_non_exhaustive()
    }
}

impl Backend for ScriptedBackend {
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        self.encodes.fetch_add(1, Ordering::AcqRel);
        (self.encode)(request)
    }

    fn decode(
        &self,
        response: HttpResponse,
        request: &DecisionRequest,
        ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        let index = self.decodes.fetch_add(1, Ordering::AcqRel);
        (self.decode)(index, response, request, ctx)
    }
}
