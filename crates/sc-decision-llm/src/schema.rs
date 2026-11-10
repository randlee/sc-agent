//! The required answer schema (REQ-SAG-0901).
//!
//! Every value is required: the model self-reports probabilities and confidence. Anthropic
//! structured outputs accept no numeric minimum or maximum, so ranges are not in the schema
//! and values are not range-checked anywhere (REQ-SAG-0007).

use sc_decision_schema::{Question, SystemOneRequest};
use serde_json::{Map, Value, json};

/// The schema name sent as Chat Completions `json_schema.name`.
pub(crate) const SCHEMA_NAME: &str = "jev_answers";

/// `{"answers": {<id>: <per-type schema>, ...}}`, every id required, nothing else allowed.
pub(crate) fn answer_schema(request: &SystemOneRequest) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (id, question) in request.questions.iter() {
        properties.insert(id.clone(), question_schema(question));
        required.push(Value::String(id.clone()));
    }
    json!({
        "type": "object",
        "required": ["answers"],
        "additionalProperties": false,
        "properties": {
            "answers": {
                "type": "object",
                "additionalProperties": false,
                "required": required,
                "properties": properties
            }
        }
    })
}

/// The index keys `"0".."n-1"` of a score question with `levels` levels.
pub(crate) fn score_keys(levels: usize) -> Vec<String> {
    (0..levels).map(|index| index.to_string()).collect()
}

fn number() -> Value {
    json!({ "type": "number" })
}

fn object(properties: Map<String, Value>) -> Value {
    let required: Vec<Value> = properties.keys().cloned().map(Value::String).collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

/// An object with a number under every key of `keys`.
fn numbers<'a>(keys: impl Iterator<Item = &'a String>) -> Value {
    object(keys.map(|key| (key.clone(), number())).collect())
}

fn question_schema(question: &Question) -> Value {
    let mut fields = Map::new();
    match question {
        Question::Noul { .. } => {
            fields.insert("noul".to_owned(), number());
        }
        Question::Choice { criteria, .. } => {
            let options: Vec<&String> = criteria.iter().map(|(key, _)| key).collect();
            fields.insert(
                "choice".to_owned(),
                json!({ "type": "string", "enum": options }),
            );
            fields.insert("confidence".to_owned(), number());
            fields.insert("probabilities".to_owned(), numbers(options.into_iter()));
        }
        Question::Score { criteria, .. } => {
            fields.insert("score".to_owned(), number());
            fields.insert("confidence".to_owned(), number());
            fields.insert(
                "probabilities".to_owned(),
                numbers(score_keys(criteria.len()).iter()),
            );
        }
        Question::Box { .. } => {
            let edges = ["top", "bottom", "left", "right"]
                .into_iter()
                .map(|edge| (edge.to_owned(), number()))
                .collect();
            fields.insert("box".to_owned(), object(edges));
            fields.insert(
                "confidence".to_owned(),
                json!({ "type": ["number", "null"] }),
            );
        }
    }
    object(fields)
}
