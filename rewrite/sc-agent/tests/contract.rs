use sc_agent::{config, digest, key, schema};
use serde_json::json;
use std::{ffi::OsString, fs, path::PathBuf};

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(ulid::Ulid::new().to_string());
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn key_precedence_and_redaction() {
    let root = directory();
    fs::write(root.join(".env"), "TYPESAFE_API_KEY=dotenv-key\n").unwrap();
    let env = [(
        OsString::from("TYPESAFE_API_KEY"),
        OsString::from("env-key"),
    )];
    let forbidden = |_: &str| -> Result<String, String> { panic!("keyring must not be read") };
    assert_eq!(
        key("TYPESAFE_API_KEY", &env, &root, &forbidden).unwrap(),
        "env-key"
    );
    assert_eq!(
        key("TYPESAFE_API_KEY", &[], &root, &forbidden).unwrap(),
        "dotenv-key"
    );
    fs::remove_file(root.join(".env")).unwrap();
    assert_eq!(
        key(
            "TYPESAFE_API_KEY",
            &[],
            &root,
            &|_| Ok("keyring-key".into())
        )
        .unwrap(),
        "keyring-key"
    );
    let invalid = [(
        OsString::from("TYPESAFE_API_KEY"),
        OsString::from("secret invalid"),
    )];
    let error = key("TYPESAFE_API_KEY", &invalid, &root, &forbidden).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("secret invalid"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_layers_and_missing_places() {
    let root = directory();
    let local = root.join("local");
    let global = root.join("global");
    fs::create_dir_all(&local).unwrap();
    fs::create_dir_all(&global).unwrap();
    fs::write(
        global.join("decidr.json"),
        r#"{"project":"global","workers":4}"#,
    )
    .unwrap();
    fs::write(local.join("sc-agent.json"), r#"{"project":"shared-local"}"#).unwrap();
    let env = [(OsString::from("SC_PROJECT"), OsString::from("environment"))];
    let got = config("decidr", Some(&local), &global, &[], None).unwrap();
    assert_eq!(got.config.project.as_deref(), Some("shared-local"));
    assert_eq!(got.config.workers, 4);
    assert_eq!(
        config("decidr", Some(&local), &global, &env, None)
            .unwrap()
            .config
            .project
            .as_deref(),
        Some("environment")
    );
    assert_eq!(
        config("decidr", Some(&local), &global, &env, Some("flag"))
            .unwrap()
            .config
            .project
            .as_deref(),
        Some("flag")
    );
    let error = config("decidr", None, &root.join("missing"), &[], None)
        .unwrap()
        .config
        .project_required()
        .unwrap_err();
    for place in ["--project", "SC_PROJECT", "sc-agent.json", "decidr.json"] {
        assert!(error.to_string().contains(place));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_answers_and_question_identity() {
    let request =
        json!({"state":"evidence", "questions":{"q":{"type":"noul","question":"How many?"}}});
    let mut changed = request.clone();
    changed["state"] = json!("different");
    assert_ne!(
        digest(&request, "q").unwrap(),
        digest(&changed, "q").unwrap()
    );
    changed = request.clone();
    changed["questions"]["other"] = json!({"type":"noul","question":"Other?"});
    assert_eq!(
        digest(&request, "q").unwrap(),
        digest(&changed, "q").unwrap()
    );
    assert!(
        schema(
            &request,
            &json!({"answers":{"q":{"type":"noul","noul":0.123456789}}})
        )
        .is_ok()
    );
    assert!(schema(&request, &json!({"answers":{"q":{"type":"choice","choice":"a","confidence":0.9,"probabilities":{"a":0.9}}}})).is_err());
}
