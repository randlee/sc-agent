#[test]
fn test_backend_serde_documented_spelling() {
    let wire = |id: BackendId| serde_json::to_value(id).expect("serialize");
    assert_eq!(wire(BackendId::Jev), "jev");
    assert_eq!(wire(BackendId::Haiku), "haiku");
    assert_eq!(wire(BackendId::Luna), "luna");
    assert_eq!(
        serde_json::to_value(BackendKeyId::OpenAi).expect("key"),
        "openai"
    );
    assert_eq!(
        serde_json::to_value(BackendKeyId::TypeSafe).expect("key"),
        "typesafe"
    );
    for key in BackendKeyId::ALL.iter().copied() {
        assert_eq!(serde_json::to_value(key).expect("key"), key.to_string());
        assert_eq!(key.to_string().parse::<BackendKeyId>().expect("parse"), key);
    }
}
/// `SessionId` is a ULID in `Display`, `FromStr` and serde; a UUID is refused
/// with the ULID fix hint (REQ-SAG-0612).
#[test]
fn test_session_id_is_ulid() {
    let id = SessionId::new();
    let text = id.to_string();
    assert_eq!(text.len(), 26);
    assert!(text.bytes().all(|byte| byte.is_ascii_alphanumeric()));
    assert_eq!(text.parse::<SessionId>().expect("display roundtrip"), id);
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, format!("\"{text}\""));
    assert_eq!(
        serde_json::from_str::<SessionId>(&json).expect("deserialize"),
        id
    );
    assert_ne!(SessionId::new(), SessionId::new());

    let uuid = "3f2b8a9e-6c1d-4e7a-9b25-0d4c1a7e5f10";
    for bad in [uuid, "", "nope", &"0".repeat(25), &"Z".repeat(26)] {
        let error = bad.parse::<SessionId>().expect_err("rejected");
        assert_eq!(error.code(), "INPUT_INVALID", "{bad:?}");
        assert!(error.to_string().contains("ULID"), "{error}");
        assert!(
            serde_json::from_value::<SessionId>(serde_json::json!(bad)).is_err(),
            "{bad:?}"
        );
    }
}

/// `Sha256Digest::of_text` goes through `sc-sha` and keeps the SHA-256 of the
/// bytes for LF text (REQ-SAG-0618, REQ-SAG-0102).
