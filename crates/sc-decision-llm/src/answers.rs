/// What a model's reply text parses to before a variant validates it.
#[derive(Debug, Clone, Deserialize)]
pub struct LlmOutput {
    answers: Map<String, Value>,
}

impl LlmOutput {
    /// Parses the model's reply text.
    ///
    /// # Errors
    ///
    /// `RESPONSE_INVALID` when the text is not JSON ("LLM reply is not
    /// JSON") or has no `answers` object ("LLM answer ids differ from the
    /// question ids").
    pub fn parse(text: &str) -> Result<Self, JevError> {
        let value: Value = serde_json::from_str(text).map_err(|_| {
            JevError::response_invalid_because(
                ResponseInvalidCause::NotJson,
                "LLM reply is not JSON",
            )
        })?;
        match value {
            Value::Object(mut object) => match object.remove("answers") {
                Some(Value::Object(answers)) => Ok(Self { answers }),
                _ => Err(ids_differ()),
            },
            _ => Err(ids_differ()),
        }
    }

    /// The raw `answers` object.
    #[must_use]
    pub fn answers(&self) -> &Map<String, Value> {
        &self.answers
    }

    pub(crate) fn into_answers(self) -> Map<String, Value> {
        self.answers
    }
}

pub(crate) fn ids_differ() -> JevError {
    JevError::response_invalid("LLM answer ids differ from the question ids")
}
    /// Validates a parsed reply against `payload` and returns typed answers.
    ///
    /// # Errors
    ///
    /// `RESPONSE_INVALID`; see `llm::v1::parse_output` for the cases.
    pub fn answers(
        output: LlmOutput,
        payload: &PayloadBundle,
    ) -> Result<DecisionAnswers, JevError> {
        let answers = output.into_answers();
        let questions = payload.question_choices();
        let expected: BTreeSet<&str> = questions.keys().map(QuestionId::as_str).collect();
        if expected != id_set(&answers) {
            return Err(ids_differ());
        }
        let mut typed = DecisionAnswers::new();
        for (id, choices) in questions {
            let key = id.as_str();
            let item = answers
                .get(key)
                .and_then(Value::as_object)
                .ok_or_else(|| bad_answer(key))?;
            let choice = item.get("choice").and_then(Value::as_str).unwrap_or("");
            if !choices.iter().any(|label| label.as_str() == choice) {
                return Err(bad_answer(key));
            }
            let confidence = item.get("confidence").ok_or_else(|| bad_answer(key))?;
            if !is_number(confidence) {
                return Err(not_a_number());
            }
            let probabilities = item
                .get("probabilities")
                .and_then(Value::as_object)
                .ok_or_else(|| bad_answer(key))?;
            let expected_labels: BTreeSet<&str> = choices.iter().map(ChoiceId::as_str).collect();
            if expected_labels != id_set(probabilities) {
                return Err(bad_answer(key));
            }
            if !probabilities.values().all(is_number) {
                return Err(not_a_number());
            }
            let mut answer = Map::new();
            answer.insert("choice".to_owned(), Value::String(choice.to_owned()));
            answer.insert("confidence".to_owned(), confidence.clone());
            answer.insert(
                "probabilities".to_owned(),
                Value::Object(probabilities.clone()),
            );
            let answer: QuestionAnswer =
                serde_json::from_value(Value::Object(answer)).map_err(|error| {
                    JevError::response_invalid(format!("answer {key} is invalid: {error}"))
                })?;
            typed.insert(id.clone(), answer);
        }
        Ok(typed)
    }

    fn is_number(value: &Value) -> bool {
        matches!(value, Value::Number(_))
    }

    fn bad_answer(id: &str) -> JevError {
        JevError::response_invalid(format!(
            "LLM answer {id} needs a criterion choice and a probability per criterion"
        ))
    }

    fn not_a_number() -> JevError {
        JevError::response_invalid("LLM confidence or probability is not a number")
    }
