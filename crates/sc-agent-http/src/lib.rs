#![forbid(unsafe_code)]
//! The shared core every sc-agent call goes through.
//!
//! One trait, [`Codec`], describes a remote API: build the HTTP request, read the reply.
//! One client, [`Client`], does the I/O for any codec: send, retry, cancel, batch.
//! LLM chat backends, decision backends and local-runner probes are all codecs, so none of
//! them carries its own send or retry code, and a plain tool loop gets the same client.

mod client;

/// Generic queue and deadlines for retrying held work.
pub mod holdoff;

pub use client::{Client, Item, Policy, retry_after};
pub use holdoff::Cancel;
use sc_agent_types::Error;
pub use ureq::http;

/// A remote API, sans I/O: how to build a request and how to read its reply.
/// This is an open trait; downstream crates may implement it.
pub trait Codec: Send + Sync {
    type Request;
    type Response;

    /// The HTTP request for `request`. Called again for every resend.
    fn encode(&self, request: &Self::Request) -> Result<http::Request<Vec<u8>>, Error>;

    /// The reply of a non-busy status to `request`. An [`Error::Invalid`] reply is resent.
    fn decode(
        &self,
        request: &Self::Request,
        status: u16,
        body: &[u8],
    ) -> Result<Self::Response, Error>;
}

impl<C: Codec + ?Sized> Codec for Box<C> {
    type Request = C::Request;
    type Response = C::Response;

    fn encode(&self, request: &Self::Request) -> Result<http::Request<Vec<u8>>, Error> {
        (**self).encode(request)
    }

    fn decode(
        &self,
        request: &Self::Request,
        status: u16,
        body: &[u8],
    ) -> Result<Self::Response, Error> {
        (**self).decode(request, status, body)
    }
}

/// A JSON POST with an optional auth header: the request shape every codec here sends.
pub fn post_json(
    url: &str,
    auth: Option<(&str, &str)>,
    headers: &[(&str, &str)],
    body: &serde_json::Value,
) -> Result<http::Request<Vec<u8>>, Error> {
    let bytes = serde_json::to_vec(body).map_err(|e| Error::Encode(e.to_string()))?;
    let mut builder = http::Request::post(url).header("content-type", "application/json");
    for (name, value) in auth.iter().chain(headers) {
        builder = builder.header(*name, *value);
    }
    builder
        .body(bytes)
        .map_err(|e| Error::Encode(e.to_string()))
}

/// A GET with an optional auth header.
pub fn get(url: &str, auth: Option<(&str, &str)>) -> Result<http::Request<Vec<u8>>, Error> {
    let mut builder = http::Request::get(url);
    if let Some((name, value)) = auth {
        builder = builder.header(name, value);
    }
    builder
        .body(Vec::new())
        .map_err(|e| Error::Encode(e.to_string()))
}

/// A 2xx body parsed as JSON, any other status as [`Error::ApiStatus`] with the body whole.
pub fn json_reply<T: serde::de::DeserializeOwned>(status: u16, body: &[u8]) -> Result<T, Error> {
    if (200..300).contains(&status) {
        serde_json::from_slice(body).map_err(|e| Error::ResponseParse(e.to_string()))
    } else {
        Err(Error::ApiStatus {
            status,
            body: String::from_utf8_lossy(body).into_owned(),
        })
    }
}

/// Checked once, when a backend is built: a key never goes over plain http to another host.
pub fn check_base(base: &str, has_key: bool) -> Result<String, Error> {
    let base = base.trim_end_matches('/');
    let uri: http::Uri = base
        .parse()
        .map_err(|e| Error::Config(format!("base URL {base}: {e}")))?;
    let loopback = matches!(uri.host(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if has_key && uri.scheme_str() != Some("https") && !loopback {
        return Err(Error::Config(format!(
            "a key is never sent over plain http ({base})"
        )));
    }
    Ok(base.to_owned())
}

#[cfg(feature = "test-server")]
pub mod test_server;
