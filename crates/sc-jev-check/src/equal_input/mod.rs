//! Equal-input record for one check run (REQ-SAG-0602).
//!
//! Every backend in the run is attested against the same canonical payload
//! bytes. `prompt_sha256` is the digest of those bytes. LLM system-prompt text
//! is not part of the digest (ADR-SAG-0019).

#![warn(missing_docs)]

use std::collections::BTreeMap;

use sc_agent_types::{BackendId, EqualInputRecord, JevError, PayloadBundle, SubjectsProducer};

use crate::extract::Extracted;

/// Builds the equal-input record for one run.
///
/// `commands` are the backend id strings, in run order. `leak` is true when a
/// requested backend was withheld from the comparison. `equal` is true when
/// every participating backend was given the same canonical bytes. `producer`
/// is the subjects document producer when the check used a `subjects` or
/// `command` input.
///
/// # Examples
///
/// ```
/// use sc_agent_types::PayloadBundle;
/// use sc_jev_check::equal_input::record;
///
/// let bundle = PayloadBundle::from_canonical_bytes(b"{}".to_vec());
/// let record = record(&bundle, &["jev".to_owned()], false, true, None);
/// assert_eq!(&record.prompt_sha256, bundle.sha256());
/// assert!(record.equal);
/// assert!(!record.leak);
/// assert!(record.producer.is_none());
/// ```
#[must_use]
pub fn record(
    bundle: &PayloadBundle,
    commands: &[String],
    leak: bool,
    equal: bool,
    producer: Option<SubjectsProducer>,
) -> EqualInputRecord {
    EqualInputRecord {
        prompt_sha256: bundle.sha256().clone(),
        leak,
        commands: commands.to_vec(),
        equal,
        producer,
    }
}

/// Backend id strings in `backends` order.
#[must_use]
pub fn commands_for(backends: &[BackendId]) -> Vec<String> {
    backends.iter().map(ToString::to_string).collect()
}

/// Copies the producer from the `subjects` or `command` extractions.
///
/// File and table inputs have no subjects document, so they contribute
/// nothing. Several documents from the same producer yield that producer.
/// The equal-input record attests to one producer, so documents from
/// different producers are refused rather than silently dropped.
///
/// # Errors
///
/// Returns [`JevError::ConfigInvalid`] when two inputs carry different producers.
pub fn producer_from_extractions(
    extractions: &BTreeMap<String, Extracted>,
) -> Result<Option<SubjectsProducer>, JevError> {
    let mut found: Option<(&str, &SubjectsProducer)> = None;
    for (name, extracted) in extractions {
        let Some(document) = &extracted.document else {
            continue;
        };
        match found {
            None => found = Some((name, &document.producer)),
            Some((first, producer)) if producer != &document.producer => {
                return Err(JevError::ConfigInvalid {
                    detail: format!(
                        "inputs {first} and {name} come from different subjects producers; one check attests to one producer, so split them into separate checks"
                    ),
                });
            }
            Some(_) => {}
        }
    }
    Ok(found.map(|(_, producer)| producer.clone()))
}

/// Reports whether every entry is the same byte string.
///
/// An empty slice is not equal: a run with no payloads did not compare
/// backends.
#[must_use]
pub fn same_bytes(payloads: &[impl AsRef<[u8]>]) -> bool {
    let Some(first) = payloads.first() else {
        return false;
    };
    payloads
        .iter()
        .all(|payload| payload.as_ref() == first.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::{Sha256Digest, SubjectsDocument};

    #[test]
    fn record_round_trips_readme_fields() {
        let bundle = PayloadBundle::from_canonical_bytes(br#"{"ok":true}"#.to_vec());
        let producer = sample_producer();
        let built = record(
            &bundle,
            &["jev".to_owned(), "anthropic_haiku".to_owned()],
            true,
            false,
            Some(producer.clone()),
        );
        assert_eq!(built.prompt_sha256, Sha256Digest::of(br#"{"ok":true}"#));
        assert!(built.leak);
        assert!(!built.equal);
        assert_eq!(built.commands, ["jev", "anthropic_haiku"]);
        assert_eq!(built.producer, Some(producer));
        let value = serde_json::to_value(&built).expect("json");
        let mut keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["commands", "equal", "leak", "producer", "prompt_sha256"]
        );
        let parsed: EqualInputRecord = serde_json::from_value(value).expect("parse");
        assert_eq!(parsed, built);
    }

    #[test]
    fn producer_comes_from_subjects_document_only() {
        let producer = sample_producer();
        let mut extractions = BTreeMap::new();
        extractions.insert(
            "note".to_owned(),
            Extracted {
                subjects: Vec::new(),
                document: None,
            },
        );
        extractions.insert(
            "branches".to_owned(),
            Extracted {
                subjects: Vec::new(),
                document: Some(SubjectsDocument {
                    schema: sc_agent_types::SUBJECTS_SCHEMA_V1.to_owned(),
                    source: sc_agent_types::SubjectsSource {
                        path: "src/example.py".to_owned(),
                        rev: sc_agent_types::Rev::new("HEAD"),
                        blob_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                    },
                    producer: producer.clone(),
                    subjects: Vec::new(),
                }),
            },
        );
        assert_eq!(
            producer_from_extractions(&extractions).expect("one producer"),
            Some(producer.clone())
        );
        let mut other = producer;
        other.name = "another-producer".to_owned();
        let mut second = extractions["branches"].clone();
        if let Some(document) = second.document.as_mut() {
            document.producer = other;
        }
        extractions.insert("zeta".to_owned(), second);
        let error = producer_from_extractions(&extractions).expect_err("two producers");
        assert_eq!(error.code(), "CONFIG_INVALID");
        assert!(same_bytes(&[b"abc".as_slice(), b"abc".as_slice()]));
        assert!(!same_bytes(&[b"abc".as_slice(), b"abd".as_slice()]));
        assert!(!same_bytes(&[] as &[&[u8]]));
    }

    fn sample_producer() -> SubjectsProducer {
        SubjectsProducer {
            name: "sc-treesitter".to_owned(),
            version: "0.1.0".to_owned(),
            language: "python".to_owned(),
            grammar: "tree-sitter-python".to_owned(),
            grammar_version: "0.23.6".to_owned(),
            query_sha256: Sha256Digest::parse(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            )
            .expect("digest"),
            mode: "matches".to_owned(),
        }
    }
}
