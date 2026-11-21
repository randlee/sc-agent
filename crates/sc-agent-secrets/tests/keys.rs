#![allow(clippy::unwrap_used)]

use sc_agent_secrets as keys;

#[test]
fn key_lookup_order_and_redaction() {
    let dir = std::env::temp_dir().join(format!("sc-core-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".env"), "K=from-dotenv\n").unwrap();
    let none = |_: &str| Ok(None);
    let env = |v: &str| (v == "K").then(|| "from-env".to_owned());
    assert_eq!(
        keys::lookup("K", env, Some(&dir), none).unwrap().unwrap().1,
        keys::KeySource::Env
    );
    assert_eq!(
        keys::lookup("K", |_| Some(" ".into()), Some(&dir), none)
            .unwrap()
            .unwrap()
            .0,
        "from-dotenv"
    );
    let ring = |_: &str| Ok(Some("from-ring".to_owned()));
    assert_eq!(
        keys::lookup("K", |_| None, None, ring).unwrap().unwrap().1,
        keys::KeySource::Keyring
    );
    assert_eq!(
        keys::redact("x from-env y", Some("from-env")),
        "x [REDACTED] y"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
