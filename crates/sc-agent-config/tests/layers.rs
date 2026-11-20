//! The generic loader over a toy schema: temporary files and explicit environment values only.
#![cfg(test)]
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use sc_agent_config::layers::{
    EnvValue, FileLayer, Inputs, Key, Layered, LayersError, MAX_FILE_BYTES, Source, load, resolved,
};
use serde_json::{Value, json};

const KEYS: &[Key] = &[
    Key {
        path: "name",
        env: EnvValue::String,
    },
    Key {
        path: "count",
        env: EnvValue::Json,
    },
    Key {
        path: "items",
        env: EnvValue::Json,
    },
    Key {
        path: "items.*",
        env: EnvValue::Json,
    },
    Key {
        path: "items.*.host",
        env: EnvValue::String,
    },
    Key {
        path: "items.*.port",
        env: EnvValue::Json,
    },
    Key {
        path: "items.*.model",
        env: EnvValue::String,
    },
];
const NAME_ONLY: &[Key] = KEYS.split_at(1).0;

/// The toy decode error: a rejected value at a key path.
#[derive(Debug)]
struct Rejected(String);

/// Accepts any object, except a `count` of 0.
fn decode(value: Value) -> Result<Value, Rejected> {
    if value.get("count") == Some(&json!(0)) {
        return Err(Rejected("count".into()));
    }
    Ok(value)
}

fn load_toy(
    files: &[(&Path, &'static [Key])],
    environment: &[(OsString, OsString)],
    flags: BTreeMap<String, Value>,
) -> Result<Layered<Value>, LayersError<Rejected>> {
    let inputs = Inputs {
        files: files
            .iter()
            .map(|(path, accepts)| FileLayer {
                path: path.to_path_buf(),
                accepts,
            })
            .collect(),
        env_prefix: "APP_",
        env_aliases: &[],
        env_accepts: KEYS,
        flags,
        flag_names: BTreeMap::new(),
        keys: KEYS,
        environment,
    };
    load(&inputs, decode, |error| Some(error.0.as_str()))
}

fn write(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value.to_string()).unwrap();
}
fn var(name: &str, value: &str) -> (OsString, OsString) {
    (name.into(), value.into())
}
fn row(layered: &Layered<Value>, path: &str) -> (String, String) {
    let row = resolved(layered, KEYS)
        .into_iter()
        .find(|r| r.path == path)
        .unwrap();
    (row.value, row.source)
}
fn files(root: &Path) -> (PathBuf, PathBuf) {
    (root.join("high.json"), root.join("low.json"))
}

#[test]
fn nested_merge_keeps_untouched_leaves_and_their_sources() {
    let root = tempfile::tempdir().unwrap();
    let (high, low) = files(root.path());
    write(
        &low,
        &json!({"count":4,"items":{"a":{"host":"127.0.0.1","port":8000,"model":"m"}}}),
    );
    write(&high, &json!({"count":2,"items":{"a":{"model":"local"}}}));
    let layers = [(high.as_path(), KEYS), (low.as_path(), KEYS)];
    let mut environment = vec![var("APP_ITEMS__A__PORT", "9000")];
    let layered = load_toy(&layers, &environment, BTreeMap::new()).unwrap();
    assert_eq!(layered.config["items"]["a"]["port"], 9000);
    assert_eq!(layered.config["items"]["a"]["host"], "127.0.0.1");
    assert_eq!(layered.config["items"]["a"]["model"], "local");
    assert_eq!(layered.config["count"], 2);
    assert_eq!(
        row(&layered, "items.a.port"),
        ("9000".into(), "APP_ITEMS__A__PORT".into())
    );
    assert_eq!(row(&layered, "items.a.host").1, low.display().to_string());
    assert_eq!(row(&layered, "count").1, high.display().to_string());
    environment.push(var("APP_COUNT", "3"));
    let flags = BTreeMap::from([("count".to_owned(), json!(1))]);
    let layered = load_toy(&layers, &environment, flags).unwrap();
    assert_eq!(layered.config["count"], 1);
    assert_eq!(layered.sources["count"], Source::Flag("--count".into()));
}

#[test]
fn env_segments_are_matched_by_position() {
    let root = tempfile::tempdir().unwrap();
    let (high, _) = files(root.path());
    write(&high, &json!({"items":{"model":{"model":"original"}}}));
    let environment = [var("APP_ITEMS__MODEL__MODEL", "123")];
    let layered = load_toy(&[(high.as_path(), KEYS)], &environment, BTreeMap::new()).unwrap();
    assert_eq!(layered.config["items"]["model"]["model"], "123");
    assert_eq!(
        row(&layered, "items.model.model"),
        ("123".into(), "APP_ITEMS__MODEL__MODEL".into())
    );
}

#[test]
fn file_errors_are_typed_and_a_file_rejects_keys_it_does_not_accept() {
    let root = tempfile::tempdir().unwrap();
    let (file, _) = files(root.path());
    let layers = [(file.as_path(), NAME_ONLY)];
    write(&file, &json!({"count":2}));
    let error = load_toy(&layers, &[], BTreeMap::new()).unwrap_err();
    let LayersError::At { source, cause } = error else {
        panic!("{error:?}")
    };
    assert_eq!(source, Source::File(file.clone()));
    assert!(matches!(*cause, LayersError::InvalidKey { ref path, .. } if path == "count"));
    fs::write(&file, "{").unwrap();
    assert!(matches!(
        load_toy(&layers, &[], BTreeMap::new()).unwrap_err(),
        LayersError::Json { file: ref f, .. } if *f == file
    ));
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    assert!(matches!(
        load_toy(&layers, &[], BTreeMap::new()).unwrap_err(),
        LayersError::NotAFile { .. }
    ));
    fs::remove_dir(&file).unwrap();
    fs::File::create(&file)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    assert!(matches!(
        load_toy(&layers, &[], BTreeMap::new()).unwrap_err(),
        LayersError::TooLarge { .. }
    ));
}

#[test]
fn invalid_lower_layer_is_not_hidden_by_a_flag() {
    let root = tempfile::tempdir().unwrap();
    let (file, _) = files(root.path());
    write(&file, &json!({"count":0}));
    let flags = BTreeMap::from([("count".to_owned(), json!(2))]);
    let layers = [(file.as_path(), KEYS)];
    assert!(load_toy(&layers, &[], flags.clone()).is_err());
    fs::remove_file(&file).unwrap();
    let error = load_toy(&layers, &[var("APP_COUNT", "0")], flags).unwrap_err();
    let LayersError::At { source, cause } = error else {
        panic!("{error:?}")
    };
    assert_eq!(source, Source::Env("APP_COUNT".into()));
    assert!(matches!(*cause, LayersError::Decode(Rejected(ref path)) if path == "count"));
}

#[cfg(unix)]
#[test]
fn non_utf8_env_value_names_the_variable() {
    use std::os::unix::ffi::OsStringExt;
    let environment = [("APP_COUNT".into(), OsString::from_vec(vec![255]))];
    let error = load_toy(&[], &environment, BTreeMap::new()).unwrap_err();
    assert!(matches!(
        error,
        LayersError::At { source: Source::Env(ref name), .. } if name == "APP_COUNT"
    ));
}
