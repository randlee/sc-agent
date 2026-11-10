//! The sync decision API: [`Decision`], [`DecisionRequest`] and [`DecisionResult`].
//!
//! A [`DecisionRequest`] is everything one send carries (the JSON body, the multipart image
//! parts, the long-context header) and is valid by construction: [`DecisionRequest::new`]
//! checks the image parts and limits and computes the per-question digests, so no backend
//! re-checks. [`Decision`] is implemented by sc-decision-runner; this crate has no queue, no
//! workers and no I/O.

use std::collections::BTreeMap;
use std::fmt;

use sc_agent_types::{CancelToken, SessionId};

use crate::DecisionError;
use crate::digest::{QuestionDigest, question_digests};
use crate::map::OrderedMap;
use crate::request::{ImageFormat, Question, SystemOneRequest};
use crate::response::SystemOneResponse;

/// The most image parts one request may carry (REQ-SAG-0014).
pub const MAX_IMAGES: usize = 20;

/// The most bytes one image part may carry: 20 MiB (REQ-SAG-0014).
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;

/// Everything one send carries: JSON body, multipart image parts, long-context header.
///
/// The `TypeSafe-Danger-Zone-Images` header is derived ([`Self::needs_images_header`]).
/// Valid by construction: the fields are private and [`Self::new`] is the only way in.
///
/// # Examples
///
/// ```
/// use sc_decision_schema::{DecisionRequest, ImageFormat, ImagePart, SystemOneRequest};
///
/// let body = SystemOneRequest::from_json(
///     r#"{"model":"m","state":"see the image","questions":{"q":{"type":"box"}},"assets":{"img":{"type":"image","format":"png","part":"img1"}}}"#,
/// )?;
/// let part = ImagePart { name: "img1".to_owned(), format: ImageFormat::Png, bytes: vec![0x89, b'P'] };
/// let request = DecisionRequest::new(body, vec![part], false)?;
/// assert!(request.needs_images_header());
/// assert_eq!(request.digests().len(), 1);
/// # Ok::<(), sc_decision_schema::DecisionError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionRequest {
    body: SystemOneRequest,
    image_parts: Vec<ImagePart>,
    long_context: bool,
    digests: OrderedMap<String, QuestionDigest>,
}

impl DecisionRequest {
    /// A request of `body`, its multipart `image_parts` and the long-context header flag.
    ///
    /// # Errors
    ///
    /// Checked in this order:
    ///
    /// - [`DecisionError::Limits`] with `what = "images"` when there are more than
    ///   [`MAX_IMAGES`] parts, or `what = "image_bytes"` when a part has more than
    ///   [`MAX_IMAGE_BYTES`] bytes;
    /// - [`DecisionError::ImageParts`], naming the asset id or part name, when an
    ///   `Asset.part` names no part or more than one, two parts share a name, or a part is
    ///   referenced by no asset;
    /// - [`DecisionError::Encode`] when the question digests cannot be computed (serde_json
    ///   refuses the digest input).
    pub fn new(
        body: SystemOneRequest,
        image_parts: Vec<ImagePart>,
        long_context: bool,
    ) -> Result<Self, DecisionError> {
        check_limits(&image_parts)?;
        check_image_parts(&body, &image_parts)?;
        let digests = question_digests(&body, &image_parts, long_context)?;
        Ok(Self {
            body,
            image_parts,
            long_context,
            digests,
        })
    }

    /// The JSON body.
    #[must_use]
    pub fn body(&self) -> &SystemOneRequest {
        &self.body
    }

    /// The multipart image parts; non-empty means the send is multipart/form-data.
    #[must_use]
    pub fn image_parts(&self) -> &[ImagePart] {
        &self.image_parts
    }

    /// Whether the send carries `TypeSafe-Danger-Zone-Long-Context: true`.
    #[must_use]
    pub fn long_context(&self) -> bool {
        self.long_context
    }

    /// One digest per question, in request order (REQ-SAG-0202).
    #[must_use]
    pub fn digests(&self) -> &OrderedMap<String, QuestionDigest> {
        &self.digests
    }

    /// Whether the send carries `TypeSafe-Danger-Zone-Images: true`: the body has assets or
    /// a box question.
    #[must_use]
    pub fn needs_images_header(&self) -> bool {
        let has_assets = self
            .body
            .assets
            .as_ref()
            .is_some_and(|assets| !assets.is_empty());
        has_assets
            || self
                .body
                .questions
                .iter()
                .any(|(_, question)| matches!(question, Question::Box { .. }))
    }
}

/// `DecisionRequest::new(body, Vec::new(), false)`.
///
/// `TryFrom`, not `From`: a body whose asset names a part has no valid request without its
/// image, so the conversion returns [`DecisionError::ImageParts`] for it.
impl TryFrom<SystemOneRequest> for DecisionRequest {
    type Error = DecisionError;

    fn try_from(body: SystemOneRequest) -> Result<Self, Self::Error> {
        Self::new(body, Vec::new(), false)
    }
}

fn check_limits(image_parts: &[ImagePart]) -> Result<(), DecisionError> {
    if image_parts.len() > MAX_IMAGES {
        return Err(DecisionError::Limits {
            what: "images",
            max: MAX_IMAGES as u64,
        });
    }
    if image_parts
        .iter()
        .any(|part| part.bytes.len() > MAX_IMAGE_BYTES)
    {
        return Err(DecisionError::Limits {
            what: "image_bytes",
            max: MAX_IMAGE_BYTES as u64,
        });
    }
    Ok(())
}

fn check_image_parts(
    body: &SystemOneRequest,
    image_parts: &[ImagePart],
) -> Result<(), DecisionError> {
    let mut parts_by_name: BTreeMap<&str, usize> = BTreeMap::new();
    for part in image_parts {
        *parts_by_name.entry(part.name.as_str()).or_default() += 1;
    }
    let referenced = body
        .assets
        .iter()
        .flat_map(OrderedMap::iter)
        .filter_map(|(id, asset)| asset.part.as_deref().map(|part| (id.as_str(), part)))
        .collect::<Vec<_>>();
    for &(asset_id, part_name) in &referenced {
        match parts_by_name.get(part_name).copied().unwrap_or(0) {
            0 => {
                return Err(DecisionError::ImageParts {
                    detail: format!(
                        "asset `{asset_id}` names part `{part_name}`, but no image part has that name; pass the image as an ImagePart named `{part_name}`"
                    ),
                });
            }
            1 => {}
            count => {
                return Err(DecisionError::ImageParts {
                    detail: format!(
                        "asset `{asset_id}` names part `{part_name}`, which {count} image parts share; give each image part a distinct name"
                    ),
                });
            }
        }
    }
    if let Some((name, count)) = parts_by_name.iter().find(|(_, count)| **count > 1) {
        return Err(DecisionError::ImageParts {
            detail: format!(
                "{count} image parts are named `{name}`; give each image part a distinct name"
            ),
        });
    }
    if let Some(part) = image_parts
        .iter()
        .find(|part| !referenced.iter().any(|&(_, name)| name == part.name))
    {
        return Err(DecisionError::ImageParts {
            detail: format!(
                "image part `{}` is referenced by no asset; add an asset with \"part\": \"{}\" or drop the part",
                part.name, part.name
            ),
        });
    }
    Ok(())
}

/// One multipart image part: its part name, format and bytes.
///
/// `Debug` shows the name, format and byte length, not the bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ImagePart {
    /// The multipart part name an `Asset.part` refers to.
    pub name: String,
    /// The image format.
    pub format: ImageFormat,
    /// The image bytes.
    pub bytes: Vec<u8>,
}

impl fmt::Debug for ImagePart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImagePart")
            .field("name", &self.name)
            .field("format", &self.format)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// One result of [`Decision::decide`] or [`Decision::decide_stream`].
///
/// The decided response is [`SystemOneResponse`], the one response form (ADR-SAG-0001); there
/// is no wrapper type.
#[derive(Debug)]
pub struct DecisionResult {
    /// The session of this request, a ULID (REQ-SAG-0001).
    pub session_id: SessionId,
    /// The index of the request in the input; 0 for `decide` (REQ-SAG-0701).
    pub position: u64,
    /// The request's digests (`request.digests().clone()`); always one per question.
    pub digests: OrderedMap<String, QuestionDigest>,
    /// The decided response, or why there is none.
    pub outcome: Result<SystemOneResponse, DecisionError>,
}

/// The sync decision API, implemented by sc-decision-runner.
///
/// Object-safe, no tokio, no associated error. Open trait: any crate may implement it
/// (REQ-SAG-0701; cancel through sc-agent-types' [`CancelToken`], REQ-SAG-0104).
///
/// # Cancel contract (both methods)
///
/// Cancelling `cancel` ends every unfinished request at once with
/// [`DecisionError::Cancelled`] (`decision.cancelled`): queued, held off, and also a request
/// whose send is in flight. The implementation does not wait for the transport to return; a
/// reply arriving later is discarded and produces no second result. Exactly one result per
/// request, always.
pub trait Decision: Send + Sync {
    /// Decides one request; its result has position 0.
    ///
    /// The implementation must not run the send on the caller's thread, so cancel is
    /// immediate here too.
    fn decide(&self, request: DecisionRequest, cancel: &CancelToken) -> DecisionResult;

    /// Decides every request of `requests`; results arrive fastest first.
    ///
    /// A failed request is returned in its result and the stream continues. `requests` is
    /// owned and `'static` (callers pass e.g. `Box::new(vec.into_iter())`) because the
    /// implementation pulls it on its own feeder thread.
    fn decide_stream<'a>(
        &'a self,
        requests: Box<dyn Iterator<Item = DecisionRequest> + Send + 'static>,
        cancel: &CancelToken,
    ) -> Box<dyn Iterator<Item = DecisionResult> + Send + 'a>;
}
