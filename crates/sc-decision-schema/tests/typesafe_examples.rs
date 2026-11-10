//! Every TypeSafe request and response example round-trips byte-identical.
//!
//! AC "REQ-SAG-0201 AC1", the round-trip clause of AC "ANC-001", AC "REQ-SAG-0007".
//!
//! Fixtures (`tests/fixtures/typesafe/`, one compact JSON line each), from openapi.json v0.2.0:
//! - `request_{noul,choice,score,box}`, `response_{noul,choice,score,box}`: the operation
//!   examples of POST /v1/systemone, verbatim.
//! - `request_noul_criteria`, `response_box_confidence_number`, `models`, `error_422`: built
//!   from the schema property examples (NoulQuestion.criteria/instructions,
//!   SystemOneRequest.state, BoxAnswer.confidence, ModelMetadataList.models,
//!   HTTPValidationError.detail, ValidationError.*), keys in the schema's property order
//!   (the property examples list keys alphabetically).
//! - `request_assets_part`: the multipart example of the TypeSafe images guide, keys in the
//!   schema's property order (`questions` before `assets`).
//! - `request_null_fields`: explicit `null` instructions and criteria (allowed by the schema).
//! - `error_400_{string,object}`: the HTTPBadRequestError schema's two `detail` forms;
//!   openapi.json gives no example value for them.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sc_decision_schema::{
        Answer, HttpBadRequestError, HttpValidationError, LocItem, ModelMetadataList, Question,
        SystemOneRequest, SystemOneResponse,
    };
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn fixture(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/typesafe")
            .join(format!("{name}.json"));
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
        text.trim_end().to_owned()
    }

    fn round_trip<T: Serialize + DeserializeOwned>(name: &str) -> T {
        let text = fixture(name);
        let value: T =
            serde_json::from_str(&text).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            serde_json::to_string(&value).expect("encode"),
            text,
            "{name}"
        );
        value
    }

    #[test]
    fn requests_round_trip_byte_identical() {
        for name in [
            "request_noul",
            "request_choice",
            "request_score",
            "request_box",
            "request_noul_criteria",
            "request_null_fields",
            "request_assets_part",
        ] {
            let text = fixture(name);
            let request = SystemOneRequest::from_json(&text).expect(name);
            assert_eq!(request.to_json().expect("encode"), text, "{name}");
            round_trip::<SystemOneRequest>(name);
        }
    }

    #[test]
    fn null_and_absent_fields_stay_distinct() {
        let request = SystemOneRequest::from_json(&fixture("request_null_fields")).expect("parse");
        let questions: Vec<_> = request
            .questions
            .iter()
            .map(|(_, question)| question)
            .collect();
        assert_eq!(
            questions[0],
            &Question::Noul {
                instructions: Some(serde_json::Value::Null),
                criteria: Some(None),
            }
        );
        let Question::Noul {
            instructions,
            criteria: Some(Some(criteria)),
        } = questions[2]
        else {
            panic!("noul with criteria: {:?}", questions[2]);
        };
        assert_eq!(instructions, &None);
        assert_eq!(criteria.yes, Some(serde_json::Value::Null));
        assert_eq!(criteria.no, None);
    }

    #[test]
    fn assets_carry_data_base64_and_part() {
        let inline = SystemOneRequest::from_json(&fixture("request_box")).expect("parse");
        let (_, asset) = inline
            .assets
            .as_ref()
            .expect("assets")
            .iter()
            .next()
            .expect("one");
        assert!(
            asset
                .data_base64
                .as_deref()
                .is_some_and(|data| data.starts_with("iVBOR"))
        );
        let multipart =
            SystemOneRequest::from_json(&fixture("request_assets_part")).expect("parse");
        let (_, asset) = multipart
            .assets
            .as_ref()
            .expect("assets")
            .iter()
            .next()
            .expect("one");
        assert_eq!(asset.part.as_deref(), Some("door-photo"));
    }

    #[test]
    fn responses_round_trip_byte_identical() {
        for name in [
            "response_noul",
            "response_choice",
            "response_score",
            "response_box",
            "response_box_confidence_number",
        ] {
            let text = fixture(name);
            let response = SystemOneResponse::from_json(200, text.as_bytes()).expect(name);
            assert_eq!(
                serde_json::to_string(&response).expect("encode"),
                text,
                "{name}"
            );
            assert!(response.extra.is_empty(), "{name}");
        }
        let null = round_trip::<SystemOneResponse>("response_box");
        let number = round_trip::<SystemOneResponse>("response_box_confidence_number");
        let confidence = |response: &SystemOneResponse| match response.answers.iter().next() {
            Some((_, Answer::Box { confidence, .. })) => *confidence,
            other => panic!("box answer: {other:?}"),
        };
        assert_eq!(confidence(&null), None);
        assert_eq!(confidence(&number), Some(0.9));
    }

    #[test]
    fn models_and_error_bodies_round_trip_byte_identical() {
        let models =
            ModelMetadataList::from_json(200, fixture("models").as_bytes()).expect("models");
        assert_eq!(
            serde_json::to_string(&models).expect("encode"),
            fixture("models")
        );
        assert_eq!(models.models[0].name, "jev-latest");

        let text = round_trip::<HttpBadRequestError>("error_400_string");
        assert!(text.detail.is_string());
        let object = round_trip::<HttpBadRequestError>("error_400_object");
        assert!(object.detail.is_object());

        let validation = round_trip::<HttpValidationError>("error_422");
        assert_eq!(validation.detail.len(), 2);
        assert_eq!(
            validation.detail[1].loc,
            ["body", "questions", "urgency", "score", "criteria"]
                .map(|name| LocItem::Name(name.to_owned()))
        );
        let index: Vec<LocItem> = serde_json::from_str(r#"["body",0]"#).expect("loc");
        assert_eq!(index[1], LocItem::Index(0));
    }

    /// Non-TypeSafe System One servers add fields; they round-trip through `extra`.
    #[test]
    fn extra_fields_kept() {
        // Strands Decider: a root `latency_ms`.
        let decider = r#"{"model":"decider","answers":{"is_urgent":{"type":"noul","noul":0.8581}},"usage":{"input_tokens":86,"output_tokens":1},"latency_ms":489.01}"#;
        let response = SystemOneResponse::from_json(200, decider.as_bytes()).expect("decider");
        assert_eq!(
            response.extra.get("latency_ms"),
            Some(&serde_json::json!(489.01))
        );
        assert_eq!(serde_json::to_string(&response).expect("encode"), decider);

        // Laya: extra usage counters and a root `routing`.
        let laya = r#"{"model":"laya","answers":{"q":{"type":"noul","noul":0.5}},"usage":{"input_tokens":10,"output_tokens":1,"state_tokens_dropped":0,"truncated":false,"truncated_questions":[]},"routing":{"model":"english","reason":"English Latin text"}}"#;
        let response = SystemOneResponse::from_json(200, laya.as_bytes()).expect("laya");
        assert_eq!(
            response.usage.extra.get("truncated"),
            Some(&serde_json::json!(false))
        );
        assert!(response.extra.contains_key("routing"));
        assert_eq!(serde_json::to_string(&response).expect("encode"), laya);
    }

    /// REQ-SAG-0007: numbers come back exactly as sent; nothing rounds.
    #[test]
    fn exact_floats() {
        let body = r#"{"model":"m","answers":{"a":{"type":"noul","noul":0.123456789},"b":{"type":"score","score":1.7,"confidence":0.1234567890123,"legend":{"0":"low"},"probabilities":{"0":0.30000000000000004}}},"usage":{"input_tokens":1,"output_tokens":1}}"#;
        let response = SystemOneResponse::from_json(200, body.as_bytes()).expect("parse");
        let answers: Vec<_> = response.answers.iter().map(|(_, answer)| answer).collect();
        assert_eq!(answers[0], &Answer::Noul { noul: 0.123456789 });
        let Answer::Score { score, .. } = answers[1] else {
            panic!("score answer: {:?}", answers[1]);
        };
        assert_eq!(*score, 1.7);
        assert_eq!(serde_json::to_string(&response).expect("encode"), body);
    }
}
