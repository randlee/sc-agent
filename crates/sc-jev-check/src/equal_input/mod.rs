//! Equal-input record for one check run (REQ-SAG-0602).
//!
//! Every backend in the run is attested against the same canonical payload
//! bytes. `prompt_sha256` is the digest of those bytes. LLM system-prompt text
//! is not part of the digest (ADR-SAG-0019). `equal` is derived from the
//! per-backend [`Attestation`]s; until backend-side hashing lands (r-2b) the
//! engine records [`Attestation::Unattested`] for every backend it sent to,
//! so a live run reports `equal = false`.

#![warn(missing_docs)]

use std::collections::BTreeMap;

use sc_agent_types::schema::subjects::SubjectsProducer;
use sc_agent_types::{
    Attestation, BackendAttestation, BackendId, EqualInputRecord, JevError, MultiBackendMode,
    PayloadBundle,
};

use crate::extract::Extracted;

/// What one run attests about its backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunAttestation {
    /// How later backends saw earlier answers.
    pub(crate) mode: MultiBackendMode,
    /// One entry per requested backend, in run order.
    pub(crate) backends: Vec<BackendAttestation>,
    /// True when a requested backend was withheld from the comparison.
    pub(crate) leak: bool,
}

impl RunAttestation {
    /// Every backend in `ids` with the same `attestation`.
    pub(crate) fn uniform(
        mode: MultiBackendMode,
        ids: &[BackendId],
        attestation: &Attestation,
        leak: bool,
    ) -> Self {
        Self {
            mode,
            backends: ids
                .iter()
                .map(|id| BackendAttestation::new(*id, attestation.clone()))
                .collect(),
            leak,
        }
    }
}

/// Builds the equal-input record for one run.
///
/// `producer` is the subjects document producer when the check used a
/// `subjects` or `command` input.
///
/// # Errors
///
/// `INPUT_INVALID` when the run names no backend or one backend twice
/// (callers validate the backend list before any send, so this is an
/// internal invariant).
pub(crate) fn record(
    bundle: &PayloadBundle,
    run: RunAttestation,
    producer: Option<SubjectsProducer>,
) -> Result<EqualInputRecord, JevError> {
    EqualInputRecord::new(
        bundle.sha256().clone(),
        run.mode,
        run.backends,
        run.leak,
        producer,
    )
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
            None => found = Some((name, document.producer())),
            Some((first, producer)) if producer != document.producer() => {
                return Err(JevError::config_invalid(format!(
                    "inputs {first} and {name} come from different subjects producers; one check attests to one producer, so split them into separate checks"
                )));
            }
            Some(_) => {}
        }
    }
    Ok(found.map(|(_, producer)| producer.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_agent_types::schema::subjects::SubjectsDocument;
    use sc_agent_types::{CanonicalJudgePayload, Sha256Digest};

    fn bundle() -> PayloadBundle {
        PayloadBundle::encode(&CanonicalJudgePayload::new(Vec::new(), Vec::new())).expect("bundle")
    }

    #[test]
    fn record_round_trips_readme_fields() {
        let bundle = bundle();
        let producer = sample_producer();
        let built = record(
            &bundle,
            RunAttestation::uniform(
                MultiBackendMode::ChainContext,
                &[BackendId::Jev, BackendId::AnthropicHaiku],
                &Attestation::Unattested,
                true,
            ),
            Some(producer.clone()),
        )
        .expect("record");
        assert_eq!(built.prompt_sha256(), &Sha256Digest::of(bundle.bytes()));
        assert!(built.leak());
        assert!(!built.equal());
        assert_eq!(built.backends().len(), 2);
        assert_eq!(built.producer(), Some(&producer));
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
            [
                "backends",
                "equal",
                "leak",
                "mode",
                "producer",
                "prompt_sha256"
            ]
        );
        let parsed: EqualInputRecord = serde_json::from_value(value).expect("parse");
        assert_eq!(parsed, built);
    }

    fn document(producer: &SubjectsProducer) -> SubjectsDocument {
        let mut value = serde_json::json!({
            "schema": sc_agent_types::schema::subjects::SUBJECTS_SCHEMA_V1,
            "source": {
                "path": "src/example.py",
                "rev": "HEAD",
                "blob_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            },
            "subjects": [{
                "id": "src/example.py:1",
                "capture": "name",
                "text": "x",
                "span": {"start_line": 1, "end_line": 1, "start_byte": 0, "end_byte": 1}
            }]
        });
        value["producer"] = serde_json::to_value(producer).expect("producer");
        serde_json::from_value(value).expect("document")
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
                document: Some(document(&producer)),
            },
        );
        assert_eq!(
            producer_from_extractions(&extractions).expect("one producer"),
            Some(producer.clone())
        );
        let mut other = producer;
        other.name = "another-producer".to_owned();
        extractions.insert(
            "zeta".to_owned(),
            Extracted {
                subjects: Vec::new(),
                document: Some(document(&other)),
            },
        );
        let error = producer_from_extractions(&extractions).expect_err("two producers");
        assert_eq!(error.code(), "CONFIG_INVALID");
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
