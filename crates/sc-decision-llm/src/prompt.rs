//! The built-in system prompt and the `DecisionRequest` -> `LlmRequest` translation.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sc_agent_types::ModelId;
use sc_decision_schema::{Asset, DecisionRequest, ImageFormat};
use sc_llm_provider::{LlmRequest, Message, Output, Part, Role};
use serde_json::{Map, Value};

use crate::DecisionLlmError;
use crate::schema::{SCHEMA_NAME, answer_schema};

/// The built-in system prompt (ADR-SAG-0001; no consumer prompt, REQ-SAG-0006).
///
/// Assistant draft, pending owner review.
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

/// The LLM call for `request`: system prompt, one user message (the state and questions as
/// compact JSON, then `asset <id>:` and the image for every asset, in request order) and the
/// required schema.
///
/// The request is valid by construction (`DecisionRequest::new`), so no part is checked
/// here. An asset with inline `data_base64` sends it as given; one that names a multipart
/// `part` sends that part's bytes, base64-encoded. An asset with neither carries no image and
/// adds no part.
///
/// # Errors
///
/// [`DecisionLlmError::Encode`] when the state or questions do not serialize.
pub(crate) fn translate(
    request: &DecisionRequest,
    model: &ModelId,
    max_tokens: u32,
) -> Result<LlmRequest, DecisionLlmError> {
    let body = request.body();
    let encode = |error: serde_json::Error| DecisionLlmError::Encode {
        message: error.to_string(),
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
    let text = serde_json::to_string(&Value::Object(payload)).map_err(encode)?;

    let mut content = vec![Part::Text(text)];
    for (id, asset) in body.assets.iter().flat_map(|assets| assets.iter()) {
        if let Some((media_type, data_base64)) = image(request, asset) {
            content.push(Part::Text(format!("asset {id}:")));
            content.push(Part::Image {
                media_type,
                data_base64,
            });
        }
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

/// The media type and base64 data of `asset`, if it carries an image.
fn image(request: &DecisionRequest, asset: &Asset) -> Option<(&'static str, String)> {
    if let Some(data) = &asset.data_base64 {
        let media_type = asset.format.map_or_else(|| sniff(data), media_type);
        return Some((media_type, data.clone()));
    }
    let name = asset.part.as_deref()?;
    let part = request
        .image_parts()
        .iter()
        .find(|part| part.name == name)?;
    let media_type = asset.format.map_or(media_type(part.format), media_type);
    Some((media_type, STANDARD.encode(&part.bytes)))
}

fn media_type(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpg | ImageFormat::Jpeg => "image/jpeg",
        // ImageFormat is #[non_exhaustive]: a format added later is sent as raw bytes and a
        // provider that rejects it returns its own error (REQ-SAG-0901).
        _ => "application/octet-stream",
    }
}

/// The media type of inline base64 data with no `format`, from its leading characters (PNG
/// `iVBORw0KGgo`, JPEG `/9j/`). Unknown data is sent as `application/octet-stream`; a provider
/// that rejects it returns its own error (REQ-SAG-0901).
fn sniff(data_base64: &str) -> &'static str {
    if data_base64.starts_with("iVBORw0KGgo") {
        "image/png"
    } else if data_base64.starts_with("/9j/") {
        "image/jpeg"
    } else {
        "application/octet-stream"
    }
}
