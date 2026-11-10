/// What a busy reply said, for `PROVIDER_BUSY`: the same fields as a failing
/// reply, with no provider label yet.
pub(crate) fn busy_reply(request: &HttpRequest, response: &HttpResponse) -> ProviderFailure {
    read_failure("", request, response).0
}

/// The provider's status, type, code, message, parameter, request id and
/// `Retry-After` from a failing reply, bounded. `provider` may be empty (the
/// busy retry does not know the backend name; the caller's label is added
/// later by `JevError::prefix_detail`).
fn read_failure(
    provider: &str,
    request: &HttpRequest,
    response: &HttpResponse,
) -> (ProviderFailure, Body) {
    // The secret is removed from every provider text *before* it is cut to
    // the length bound: a key that straddles the cut would otherwise leave
    // its front half behind.
    let scrub = |text: &str| match request.secret() {
        Some(secret) => secret.key().redact_text(text),
        None => text.to_owned(),
    };
    let bytes = response.body();
    let body = Body::read(bytes);
    // The whole body is scrubbed before anything is cut: the 1024-byte bound
    // happens in `ProviderFailure`, after the key is gone.
    let raw = String::from_utf8_lossy(bytes);
    let message = body.message.as_deref().map_or_else(|| scrub(&raw), scrub);
    let mut failure = ProviderFailure::new(provider, response.status(), &message)
        .with_retry_after(retry_after_seconds(response.header("retry-after")));
    if let Some(text) = &body.provider_type {
        failure = failure.with_type(&scrub(text));
    }
    if let Some(text) = &body.provider_code {
        failure = failure.with_code(&scrub(text));
    }
    if let Some(text) = &body.param {
        failure = failure.with_param(&scrub(text));
    }
    let request_id = body
        .request_id
        .as_deref()
        .or_else(|| response.header("request-id"))
        .or_else(|| response.header("x-request-id"));
    if let Some(text) = request_id {
        failure = failure.with_request_id(&scrub(text));
    }
    (failure, body)
}
    /// A body far over any read cut, with the key straddling the 1024-byte
    /// bound: nothing of the key survives.
    #[test]
    fn key_in_a_large_raw_body_is_scrubbed_before_the_cut() {
        use sc_agent_types::{BackendKeyId, ResolvedApiKey};
        const KEY: &str = "sk-big-body-secret-0123456789";
        let key = ResolvedApiKey::try_new(BackendKeyId::OpenAi, KEY.to_owned()).expect("key");
        let request = HttpRequest::new(
            crate::Method::Post,
            crate::Endpoint::parse("https://api.example.test/v1/x").expect("https"),
            std::sync::Arc::from("{}"),
        )
        .with_secret(crate::SecretHeader::bearer(key));
        for lead in [1000_usize, 1010, 1020, 4090, 4096, 4100] {
            let body = format!("{}{KEY}{}", "a".repeat(lead), "z".repeat(6000));
            let error = api_failure("p", ReplyShape::Llm, &request, &reply(500, &body));
            let rendered = error.message();
            for size in 4..=KEY.len() {
                assert!(
                    !rendered.contains(&KEY[..size])
                        && !rendered.contains(&KEY[KEY.len() - size..]),
                    "lead {lead}: a {size}-byte key fragment survived"
                );
            }
        }
    }

/// Longest provider-supplied text kept in an error, in bytes.
///
/// One kibibyte. It applies to each provider field (message, type, code,
/// parameter, request id) and to the raw-body fallback. Longer text is cut
/// at a character boundary and ends in `...`.
pub const PROVIDER_TEXT_MAX_BYTES: usize = 1024;
/// Makes provider-supplied `text` safe to carry in an error.
///
/// Control characters (newlines included) become spaces, surrounding space
/// is trimmed, and the result is cut to [`PROVIDER_TEXT_MAX_BYTES`] bytes at
/// a character boundary, ending in `...` when cut. Redaction of the secret
/// is the HTTP layer's job and happens after this.
///
/// # Examples
///
/// ```
/// use sc_agent_types::bound_provider_text;
///
/// assert_eq!(bound_provider_text(" a\nb "), "a b");
/// assert!(bound_provider_text(&"x".repeat(5000)).len() <= 1024);
/// ```
#[must_use]
pub fn bound_provider_text(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.len() <= PROVIDER_TEXT_MAX_BYTES {
        return cleaned.to_owned();
    }
    let mut end = PROVIDER_TEXT_MAX_BYTES - 3;
    while !cleaned.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &cleaned[..end])
}

/// What a provider said when it refused a request with a failing status.
///
/// Built by the HTTP layer from the status line, the headers and the reply
/// body (`sc_agent_http::api_failure`). Every text field is bounded by
/// [`bound_provider_text`]; the HTTP layer redacts the secret afterwards.
/// When the body is not the provider's JSON error shape, `message` holds the
/// bounded raw body text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProviderFailure {
    /// The provider's label in messages, such as `Jev` or `luna`.
    pub provider: String,
    /// The HTTP status.
    pub status: u16,
    /// The provider's error type (`invalid_request_error`), when it sent one.
    pub provider_type: Option<String>,
    /// The provider's error code (`insufficient_quota`), when it sent one.
    pub provider_code: Option<String>,
    /// The provider's message, or the bounded raw body.
    pub message: String,
    /// The request field the provider blamed, when it named one.
    pub param: Option<String>,
    /// The provider's request id, when it sent one.
    pub request_id: Option<String>,
    /// The `Retry-After` value in whole seconds, when the provider sent a
    /// valid one.
    pub retry_after_seconds: Option<u64>,
}

impl ProviderFailure {
    /// A failure of `provider` with `status` and `message` (bounded here).
    #[must_use]
    pub fn new(provider: impl Into<String>, status: u16, message: &str) -> Self {
        Self {
            provider: provider.into(),
            status,
            provider_type: None,
            provider_code: None,
            message: bound_provider_text(message),
            param: None,
            request_id: None,
            retry_after_seconds: None,
        }
    }

    /// Sets the provider's error type (bounded).
    #[must_use]
    pub fn with_type(mut self, text: &str) -> Self {
        self.provider_type = Some(bound_provider_text(text));
        self
    }

    /// Sets the provider's error code (bounded).
    #[must_use]
    pub fn with_code(mut self, text: &str) -> Self {
        self.provider_code = Some(bound_provider_text(text));
        self
    }

    /// Sets the blamed request field (bounded).
    #[must_use]
    pub fn with_param(mut self, text: &str) -> Self {
        self.param = Some(bound_provider_text(text));
        self
    }

    /// Sets the provider's request id (bounded).
    #[must_use]
    pub fn with_request_id(mut self, text: &str) -> Self {
        self.request_id = Some(bound_provider_text(text));
        self
    }

    /// Sets the `Retry-After` seconds.
    #[must_use]
    pub fn with_retry_after(mut self, seconds: Option<u64>) -> Self {
        self.retry_after_seconds = seconds;
        self
    }

    fn map_text(self, map: &impl Fn(&str) -> String) -> Self {
        Self {
            provider: map(&self.provider),
            status: self.status,
            provider_type: self.provider_type.as_deref().map(map),
            provider_code: self.provider_code.as_deref().map(map),
            message: map(&self.message),
            param: self.param.as_deref().map(map),
            request_id: self.request_id.as_deref().map(map),
            retry_after_seconds: self.retry_after_seconds,
        }
    }
}

impl fmt::Display for ProviderFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} HTTP {}", self.provider, self.status)?;
        match (&self.provider_type, &self.provider_code) {
            (Some(kind), Some(code)) => write!(formatter, " ({kind}, {code})")?,
            (Some(text), None) | (None, Some(text)) => write!(formatter, " ({text})")?,
            (None, None) => {}
        }
        if !self.message.is_empty() {
            write!(formatter, ": {}", self.message)?;
        }
        Ok(())
    }
}
