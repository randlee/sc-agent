//! [`SystemOneBackend`]: encode and decode for `POST {base}/v1/systemone` and `GET {base}/v1/models`.

use sc_agent_http::{
    Endpoint, HttpError, HttpRequest, HttpResponse, LogContext, Method, SecretHeader,
};
use sc_agent_secrets::ApiKey;
use sc_decision_schema::{
    Backend, DecisionError, DecisionRequest, HttpBadRequestError, HttpValidationError,
    ModelMetadataList, SystemOneResponse,
};

use crate::base::BaseUrl;
use crate::multipart;

/// The base URL of the TypeSafe System One API.
pub const TYPESAFE_BASE_URL: &str = "https://api.typesafe.ai";

/// The decision path every System One server serves.
pub const SYSTEMONE_PATH: &str = "/v1/systemone";

/// The model-list path (TypeSafe, Mapika/decider, generic daemons).
pub const MODELS_PATH: &str = "/v1/models";

/// Set to `true` when the body has assets or a box question.
const IMAGES_HEADER: &str = "TypeSafe-Danger-Zone-Images";

/// Set to `true` when the caller flags a long-context request.
const LONG_CONTEXT_HEADER: &str = "TypeSafe-Danger-Zone-Long-Context";

/// Backend for any System One server: `POST {base}/v1/systemone`, `GET {base}/v1/models`.
///
/// Sans-IO: [`Backend::encode`] builds the [`HttpRequest`], the caller sends it through
/// sc-agent-http, and [`Backend::decode`] reads the final reply. The key, when set, is attached
/// as `Authorization: Bearer` with [`HttpRequest::with_secret`] and never appears in `Debug`
/// output ([`ApiKey`] redacts itself).
///
/// # Examples
///
/// ```
/// use sc_decision_systemone::{SystemOneBackend, TYPESAFE_BASE_URL};
///
/// let backend = SystemOneBackend::new(TYPESAFE_BASE_URL, None)?;
/// let models = backend.models_request()?;
/// assert_eq!(models.endpoint().as_str(), "https://api.typesafe.ai/v1/models");
/// assert!(SystemOneBackend::new("http://api.typesafe.ai", None).is_err());
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SystemOneBackend {
    systemone: Endpoint,
    models: Endpoint,
    key: Option<ApiKey>,
}

impl SystemOneBackend {
    /// A backend for the https server at `base`, e.g. [`TYPESAFE_BASE_URL`].
    ///
    /// `base` has no trailing path; one trailing `/` is dropped ([`BaseUrl::https`]).
    ///
    /// # Errors
    ///
    /// [`DecisionError::Http`] carrying [`HttpError::InvalidEndpoint`] when `base` is not an
    /// absolute `https://` URL with a host, or has a query or fragment.
    pub fn new(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError> {
        Self::at(&BaseUrl::https(base)?, key)
    }

    /// A backend for a configured local runner's `base`, e.g. `http://{host}:{port}`.
    ///
    /// Accepts http or https ([`BaseUrl::local`]). A key on a plain-http host that is not
    /// loopback is refused later, at [`Backend::encode`] (REQ-SAG-0013).
    ///
    /// # Errors
    ///
    /// [`DecisionError::Http`] carrying [`HttpError::InvalidEndpoint`] when `base` is not an
    /// absolute `http://` or `https://` URL with a host, or has a query or fragment.
    pub fn local(base: &str, key: Option<ApiKey>) -> Result<Self, DecisionError> {
        Self::at(&BaseUrl::local(base)?, key)
    }

    /// A backend for the server at `base`, already parsed and checked.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Http`] carrying [`HttpError::InvalidEndpoint`] when an endpoint path
    /// cannot be joined onto `base` ([`BaseUrl::join`]).
    pub fn at(base: &BaseUrl, key: Option<ApiKey>) -> Result<Self, DecisionError> {
        Ok(Self {
            systemone: base.join(SYSTEMONE_PATH)?,
            models: base.join(MODELS_PATH)?,
            key,
        })
    }

    /// The `GET {base}/v1/models` request, with the Bearer key when one is set.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Http`] carrying [`HttpError::KeyOnPlainHttp`] when a key is set and
    /// the base is plain http to a host that is not loopback.
    pub fn models_request(&self) -> Result<HttpRequest, DecisionError> {
        let request = HttpRequest::new(Method::Get, self.models.clone(), Vec::new());
        Ok(self.with_key(request)?)
    }

    /// The model list of a `GET /v1/models` reply.
    ///
    /// # Errors
    ///
    /// [`DecisionError::ResponseParse`] when a 2xx body is not a [`ModelMetadataList`];
    /// otherwise as [`Backend::decode`] for a non-2xx reply (`ApiBadRequest`,
    /// `ApiValidation` or `ApiStatus`).
    pub fn decode_models(
        &self,
        response: HttpResponse,
    ) -> Result<ModelMetadataList, DecisionError> {
        let status = response.status();
        if is_success(status) {
            ModelMetadataList::from_json(status, response.body())
        } else {
            Err(refusal(status, response.into_body()))
        }
    }

    /// `request` with the Bearer key attached, when one is set.
    fn with_key(&self, request: HttpRequest) -> Result<HttpRequest, HttpError> {
        match &self.key {
            Some(key) => request.with_secret(SecretHeader::bearer(key.clone())),
            None => Ok(request),
        }
    }
}

impl Backend for SystemOneBackend {
    /// The `POST {base}/v1/systemone` request for `request`.
    ///
    /// JSON when `request.image_parts()` is empty, else multipart/form-data: part `request`
    /// (application/json) first, then one part per image in `image_parts()` order, named by
    /// [`ImagePart::name`](sc_decision_schema::ImagePart::name), with boundary
    /// `sc-agent-<ULID>` (a fresh ULID per call). Headers: `content-type`;
    /// `TypeSafe-Danger-Zone-Images: true` when
    /// [`DecisionRequest::needs_images_header`]; `TypeSafe-Danger-Zone-Long-Context: true`
    /// when [`DecisionRequest::long_context`]; the Bearer key when one is set.
    ///
    /// `request` is valid by construction ([`DecisionRequest::new`]), so no part check runs.
    ///
    /// # Errors
    ///
    /// [`DecisionError::Encode`] when the body does not serialise;
    /// [`DecisionError::Http`] carrying [`HttpError::KeyOnPlainHttp`] when a key is set and
    /// the base is plain http to a host that is not loopback (before any send).
    fn encode(&self, request: &DecisionRequest) -> Result<HttpRequest, DecisionError> {
        let json = request.body().to_json()?;
        let (content_type, body) = if request.image_parts().is_empty() {
            ("application/json".to_owned(), json.into_bytes())
        } else {
            let boundary = multipart::boundary();
            let body = multipart::body(&json, request.image_parts(), &boundary);
            (format!("multipart/form-data; boundary={boundary}"), body)
        };
        let mut http = HttpRequest::new(Method::Post, self.systemone.clone(), body)
            .with_header("content-type", content_type)?;
        if request.needs_images_header() {
            http = http.with_header(IMAGES_HEADER, "true")?;
        }
        if request.long_context() {
            http = http.with_header(LONG_CONTEXT_HEADER, "true")?;
        }
        Ok(self.with_key(http)?)
    }

    /// The decided response of a final reply.
    ///
    /// 2xx: [`SystemOneResponse::from_json`], fields outside the TypeSafe schema kept in
    /// `extra`, numbers unrounded. `request` and `ctx` are unused: this crate writes no log.
    ///
    /// # Errors
    ///
    /// - 2xx whose body is not a `SystemOneResponse`: [`DecisionError::ResponseParse`];
    /// - 400 with the documented body: [`DecisionError::ApiBadRequest`];
    /// - 422 with the documented body: [`DecisionError::ApiValidation`];
    /// - any other status (402 and 503 included), or a 400/422 of another shape (a string
    ///   `detail` from Strands Decider or Laya): [`DecisionError::ApiStatus`] with the body
    ///   unchanged. A 503 is a final reply, not a busy signal (REQ-SAG-0601).
    fn decode(
        &self,
        response: HttpResponse,
        _request: &DecisionRequest,
        _ctx: &LogContext,
    ) -> Result<SystemOneResponse, DecisionError> {
        let status = response.status();
        if is_success(status) {
            SystemOneResponse::from_json(status, response.body())
        } else {
            Err(refusal(status, response.into_body()))
        }
    }
}

const fn is_success(status: u16) -> bool {
    matches!(status, 200..=299)
}

/// The error for a non-2xx final reply of HTTP `status` with `body`.
fn refusal(status: u16, body: Vec<u8>) -> DecisionError {
    match status {
        400 => match serde_json::from_slice::<HttpBadRequestError>(&body) {
            Ok(body) => DecisionError::ApiBadRequest { body },
            Err(_) => DecisionError::ApiStatus { status, body },
        },
        422 => match serde_json::from_slice::<HttpValidationError>(&body) {
            Ok(body) => DecisionError::ApiValidation { body },
            Err(_) => DecisionError::ApiStatus { status, body },
        },
        _ => DecisionError::ApiStatus { status, body },
    }
}
