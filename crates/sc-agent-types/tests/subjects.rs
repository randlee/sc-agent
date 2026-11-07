//! `schema::subjects` (REQ-SAG-0108): one validated type and the id builder.

use std::path::PathBuf;

use sc_agent_types::schema::subjects::{Span, SubjectsBuilder, SubjectsDocument};

/// A named edit that makes the minimal document invalid.
type Mutation = (&'static str, fn(&mut serde_json::Value));

fn minimal() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/jev/subjects-schema/v1/minimal.json"),
    )
    .expect("minimal.json")
}

/// The builder assigns the golden's ids from paths and spans alone.
#[test]
fn test_subjects_builder_ids_match_golden() {
    let golden = SubjectsDocument::from_json_slice(minimal().as_bytes()).expect("golden");
    let mut builder = SubjectsBuilder::new(golden.source().clone(), golden.producer().clone());
    // Push in reverse order: the builder sorts before it assigns ids.
    for subject in golden.subjects().iter().rev() {
        builder.push_subject(
            golden.source().path.clone(),
            *subject.span(),
            subject.text(),
            subject.capture(),
        );
    }
    let built = builder.build().expect("build");
    assert_eq!(built, golden);
    assert_eq!(built.subjects()[0].id(), "src/example.py:42");
    assert_eq!(built.subjects()[1].id(), "src/example.py:42:1");
    let golden_json: serde_json::Value = serde_json::from_str(&minimal()).expect("json");
    assert_eq!(serde_json::to_value(&built).expect("json"), golden_json);
}

#[test]
fn test_unvalidated_subjects_rejected_on_deserialize() {
    let base: serde_json::Value = serde_json::from_str(&minimal()).expect("json");
    let mutations: [Mutation; 6] = [
        ("schema", |v| {
            v["schema"] = serde_json::json!("sc/subjects/0")
        }),
        ("empty", |v| v["subjects"] = serde_json::json!([])),
        ("id", |v| {
            v["subjects"][0]["id"] = serde_json::json!("src/example.py:1")
        }),
        ("helper", |v| {
            v["subjects"][0]["capture"] = serde_json::json!("_skip")
        }),
        ("span", |v| {
            v["subjects"][0]["span"]["end_byte"] = serde_json::json!(0)
        }),
        ("path", |v| {
            v["source"]["path"] = serde_json::json!("../escape.py")
        }),
    ];
    for (name, mutate) in mutations {
        let mut value = base.clone();
        mutate(&mut value);
        assert!(
            serde_json::from_value::<SubjectsDocument>(value.clone()).is_err(),
            "{name}"
        );
        let error = SubjectsDocument::from_json_slice(&serde_json::to_vec(&value).expect("bytes"))
            .expect_err(name);
        let expected = if name == "empty" {
            "EXTRACTION_EMPTY"
        } else {
            "EXTRACTION_INVALID"
        };
        assert_eq!(error.code(), expected, "{name}");
    }
    assert!(serde_json::from_value::<SubjectsDocument>(base).is_ok());
}

#[test]
fn test_builder_rejects_text_that_does_not_match_its_span() {
    let golden = SubjectsDocument::from_json_slice(minimal().as_bytes()).expect("golden");
    let mut builder = SubjectsBuilder::new(golden.source().clone(), golden.producer().clone());
    builder.push_subject(
        golden.source().path.clone(),
        Span::new(1, 1, 0, 3),
        "ab",
        "name",
    );
    assert_eq!(
        builder.build().expect_err("span").code(),
        "EXTRACTION_INVALID"
    );
    let empty = SubjectsBuilder::new(golden.source().clone(), golden.producer().clone());
    assert_eq!(empty.build().expect_err("empty").code(), "EXTRACTION_EMPTY");
}
