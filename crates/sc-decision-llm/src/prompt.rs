//! The built-in system prompt and the `DecisionRequest` -> `LlmRequest` translation.

use std::borrow::Cow;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sc_agent_types::ModelId;
use sc_decision_schema::{Asset, DecisionRequest, ImageFormat, Question};
use sc_llm_provider::{ImageMediaType, LlmRequest, Message, Output, Part, Role};
use serde_json::{Map, Value};

use crate::DecisionLlmError;
use crate::schema::{SCHEMA_NAME, answer_schema};

/// The built-in system prompt (ADR-SAG-0001; no consumer prompt, REQ-SAG-0006).
///
/// Owner-approved as written (2026-11-10).
pub(crate) const SYSTEM_PROMPT: &str = "You answer decision questions in place of a decision model. \
The user message is JSON with a state and questions keyed by id; images, if any, follow it. \
Answer every question in the required JSON schema. For a choice or score question give your own \
probability for every option and your confidence from 0 to 1; for a noul question give the \
probability that the answer is true; for a box question give the box edges and your confidence. \
Return only the JSON.";

/// The output form every decision call asks for: the required answer schema.
pub(crate) fn output(request: &DecisionRequest) -> Output {
    Output::Json {
        name: SCHEMA_NAME.to_owned(),
        schema: answer_schema(request.body()),
    }
}

/// Refuses a question type this backend has no schema for.
///
/// `Question` is `#[non_exhaustive]`: a type added to sc-decision-schema later reaches here
/// before this crate learns it. It is refused at encode, before any send, so it is never a
/// schema mismatch and never resent.
///
/// # Errors
///
/// [`DecisionLlmError::Encode`] naming the question.
pub(crate) fn check_question_types(request: &DecisionRequest) -> Result<(), DecisionLlmError> {
    for (id, question) in &request.body().questions {
        match question {
            Question::Noul { .. }
            | Question::Choice { .. }
            | Question::Score { .. }
            | Question::Box { .. } => {}
            _ => {
                return Err(DecisionLlmError::Encode {
                    message: format!(
                        "question `{id}` has a type the LLM backend does not support (noul, choice, score, box)"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The LLM call for `request`: system prompt, one user message (the state and questions as
/// compact JSON, then `asset <id>:` and the image for every asset, in request order) and the
/// required schema.
///
/// The request is valid by construction (`DecisionRequest::new`), so image limits and part
/// names are not checked again here. An asset with inline `data_base64` sends it as given
/// (borrowed, not copied); one that names a multipart `part` sends that part's bytes,
/// base64-encoded.
///
/// # Errors
///
/// [`DecisionLlmError::Encode`] when a question type is unsupported
/// ([`check_question_types`]), an asset carries no image, or the state or questions do not
/// serialize.
pub(crate) fn translate<'r>(
    request: &'r DecisionRequest,
    model: &ModelId,
    max_tokens: u32,
) -> Result<LlmRequest<'r>, DecisionLlmError> {
    check_question_types(request)?;
    let body = request.body();
    let encode = |error: serde_json::Error| DecisionLlmError::Encode {
        message: format!("the state or questions do not serialize: {error}"),
    };
    let mut payload = Map::new();
    payload.insert(
        "state".to_owned(),
        serde_json::to_value(&body.state).map_err(encode)?,
    );
    payload.insert(
        "questions".to_owned(),
        serde_json::to_value(&body.questions).map_err(encode)?,
    );

    let mut content = vec![Part::Text(Value::Object(payload).to_string())];
    for (id, asset) in body.assets.iter().flatten() {
        let (media_type, data_base64) = image(request, id, asset)?;
        content.push(Part::Text(format!("asset {id}:")));
        content.push(Part::Image {
            media_type,
            data_base64,
        });
    }
    Ok(LlmRequest {
        model: model.clone(),
        system: Some(SYSTEM_PROMPT.to_owned()),
        messages: vec![Message {
            role: Role::User,
            content,
        }],
        max_tokens,
        output: output(request),
    })
}

/// The media type and base64 data of asset `id`.
///
/// # Errors
///
/// [`DecisionLlmError::Encode`] when the asset has neither `data_base64` nor a `part`
/// (`DecisionRequest::new` allows such an asset, but it carries no image to send). A `part`
/// always has its `ImagePart`: `DecisionRequest::new` refuses an `Asset.part` that names no
/// part (`decision.image_parts`), so the missing-part arm is a broken invariant, reported as
/// [`DecisionLlmError::Invariant`] rather than dropped.
fn image<'r>(
    request: &'r DecisionRequest,
    id: &str,
    asset: &'r Asset,
) -> Result<(ImageMediaType, Cow<'r, str>), DecisionLlmError> {
    if let Some(data) = &asset.data_base64 {
        let media_type = asset.format.map_or_else(|| sniff(data), media_type);
        return Ok((media_type, Cow::Borrowed(data.as_str())));
    }
    let Some(name) = asset.part.as_deref() else {
        return Err(DecisionLlmError::Encode {
            message: format!(
                "asset `{id}` has neither data_base64 nor a part, so no image to send"
            ),
        });
    };
    let part = request
        .image_parts()
        .iter()
        .find(|part| part.name == name)
        .ok_or_else(|| DecisionLlmError::Invariant {
            message: format!(
                "asset `{id}` names part `{name}`, which DecisionRequest::new guarantees exists"
            ),
        })?;
    let media_type = media_type(asset.format.unwrap_or(part.format));
    Ok((media_type, Cow::Owned(STANDARD.encode(&part.bytes))))
}

fn media_type(format: ImageFormat) -> ImageMediaType {
    match format {
        ImageFormat::Png => ImageMediaType::Png,
        ImageFormat::Jpg | ImageFormat::Jpeg => ImageMediaType::Jpeg,
        // ImageFormat is #[non_exhaustive]: a format added later is sent as raw bytes and a
        // provider that rejects it returns its own error (REQ-SAG-0901).
        _ => ImageMediaType::OctetStream,
    }
}

/// The media type of inline base64 data with no `format`, from its leading characters (PNG
/// `iVBORw0KGgo`, JPEG `/9j/`). Unknown data is sent as `application/octet-stream`; a provider
/// that rejects it returns its own error (REQ-SAG-0901).
fn sniff(data_base64: &str) -> ImageMediaType {
    if data_base64.starts_with("iVBORw0KGgo") {
        ImageMediaType::Png
    } else if data_base64.starts_with("/9j/") {
        ImageMediaType::Jpeg
    } else {
        ImageMediaType::OctetStream
    }
}
