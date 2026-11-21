#![forbid(unsafe_code)]
//! Any chat model as a decision backend.

use indexmap::IndexMap;
use sc_agent_http::{Codec, http};
use sc_agent_types::Error;
use sc_decision_schema::{Answer, DecisionRequest, DecisionResponse, Question};
use sc_llm_provider::{Chat, Llm, Message, Reply};
use serde_json::{Value, json};

const SYSTEM: &str = "You answer decision questions about the given state. Reply only with JSON \
matching the schema: one answer per question id, of that question's type. Probabilities are \
between 0 and 1.";

/// Any chat model as a decision backend: the decision becomes a chat with a JSON schema,
/// the reply text becomes typed answers. Haiku, Luna and every chat provider use this.
pub struct FromLlm<L> {
    llm: L,
    max_tokens: u32,
}

impl<L: Llm> FromLlm<L> {
    pub fn new(llm: L) -> Self {
        Self { llm, max_tokens: 4096 }
    }

    fn chat(&self, request: &DecisionRequest) -> Result<Chat, Error> {
        let prompt = json!({ "state": request.state, "questions": request.questions });
        let answers: serde_json::Map<String, Value> =
            request.questions.iter().map(|(id, q)| (id.clone(), answer_schema(q))).collect();
        let required: Vec<&String> = request.questions.keys().collect();
        let schema = json!({ "type": "object", "additionalProperties": false, "required": ["answers"],
            "properties": { "answers": { "type": "object", "additionalProperties": false,
                "required": required, "properties": answers } } });
        Ok(Chat {
            model: request.model.clone(),
            system: Some(SYSTEM.to_owned()),
            messages: vec![Message::user(prompt.to_string())],
            max_tokens: self.max_tokens,
            output_schema: Some(schema),
            ..Chat::default()
        })
    }
}

/// The JSON schema of one question's answer; it is the [`Answer`] wire form.
fn answer_schema(question: &Question) -> Value {
    let p = json!({ "type": "number", "minimum": 0, "maximum": 1 });
    let (kind, properties) = match question {
        Question::Noul { .. } => ("noul", json!({ "noul": p })),
        Question::Choice { criteria, .. } => {
            let names: Vec<&String> = criteria.keys().collect();
            let each: serde_json::Map<String, Value> = names.iter().map(|n| ((*n).clone(), p.clone())).collect();
            ("choice", json!({ "choice": { "enum": names }, "confidence": p,
                "probabilities": { "type": "object", "properties": each, "required": names, "additionalProperties": false } }))
        }
        Question::Score { criteria, .. } => {
            let levels: Vec<String> = (0..criteria.len()).map(|i| i.to_string()).collect();
            let each: serde_json::Map<String, Value> = levels.iter().map(|n| (n.clone(), p.clone())).collect();
            ("score", json!({ "score": { "type": "number" }, "confidence": p,
                "probabilities": { "type": "object", "properties": each, "required": levels, "additionalProperties": false } }))
        }
        Question::Box { .. } => {
            let side = json!({ "type": "number" });
            ("box", json!({ "box": { "type": "object", "additionalProperties": false,
                "required": ["top", "bottom", "left", "right"],
                "properties": { "top": side, "bottom": side, "left": side, "right": side } }, "confidence": p }))
        }
    };
    let mut required: Vec<String> = properties.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    required.push("type".into());
    let mut properties = properties;
    if let Some(object) = properties.as_object_mut() {
        object.insert("type".into(), json!({ "const": kind }));
    }
    json!({ "type": "object", "additionalProperties": false, "required": required, "properties": properties })
}

impl<L: Llm> Codec for FromLlm<L> {
    type Request = DecisionRequest;
    type Response = DecisionResponse;

    fn encode(&self, request: &DecisionRequest) -> Result<http::Request<Vec<u8>>, Error> {
        self.llm.encode(&self.chat(request)?)
    }

    fn decode(&self, request: &DecisionRequest, status: u16, body: &[u8]) -> Result<DecisionResponse, Error> {
        let reply: Reply = self.llm.decode(&self.chat(request)?, status, body)?;
        #[derive(serde::Deserialize)]
        struct Answers {
            answers: IndexMap<String, Answer>,
        }
        let parsed: Answers = serde_json::from_str(&reply.text()).map_err(|e| Error::Invalid(e.to_string()))?;
        request.check(&parsed.answers)?;
        let usage = serde_json::to_value(&reply.usage).unwrap_or_default();
        Ok(DecisionResponse { model: request.model.clone(), answers: parsed.answers, usage, extra: Default::default() })
    }
}
