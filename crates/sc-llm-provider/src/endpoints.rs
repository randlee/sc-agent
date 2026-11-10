pub(crate) const ANTHROPIC_HOST: &str = "api.anthropic.com";
pub(crate) const OPENAI_HOST: &str = "api.openai.com";
pub(crate) const HAIKU_MESSAGE_PATH: &str = "/v1/messages";
pub(crate) const HAIKU_COUNT_PATH: &str = "/v1/messages/count_tokens";
pub(crate) const LUNA_MESSAGE_PATH: &str = "/v1/responses";
pub(crate) const LUNA_COUNT_PATH: &str = "/v1/responses/input_tokens";

/// Anthropic REST version sent on every Haiku call.
///
/// Pinned here because this crate does not read `check.toml`. A different
/// version is a different wire contract.
pub(crate) const ANTHROPIC_VERSION: &str = "2026-10-21";

/// Haiku completion cap.
///
/// A reply whose `stop_reason` is `max_tokens` is `RESPONSE_INVALID`. The
/// token-count body omits this field.
pub(crate) const HAIKU_MAX_TOKENS: u64 = 16_000;
/// Provider headers that do not carry the API key.
///
/// The credential goes separately, as [`secret_header`]; only the sending
/// transport writes its value (ADR-SAG-0020).
pub(crate) fn unsigned_headers(kind: ProviderKind) -> Vec<(HeaderName, &'static str)> {
    let mut headers = match kind {
        ProviderKind::Anthropic => vec![(ANTHROPIC_VERSION_HEADER, ANTHROPIC_VERSION)],
        ProviderKind::OpenAi => Vec::new(),
    };
    headers.push((CONTENT_TYPE, "application/json"));
    headers
}

/// The credential header each provider expects (ADR-SAG-0020).
pub(crate) fn secret_header(kind: ProviderKind, key: ResolvedApiKey) -> SecretHeader {
    match kind {
        ProviderKind::Anthropic => SecretHeader::new(X_API_KEY, key),
        ProviderKind::OpenAi => SecretHeader::bearer(key),
    }
}

const ANTHROPIC_VERSION_HEADER: HeaderName = HeaderName::new("anthropic-version");
const CONTENT_TYPE: HeaderName = HeaderName::new("Content-Type");
const X_API_KEY: HeaderName = HeaderName::new("x-api-key");
