//! The TypeSafe API examples through decide's library and the real runner (REQ-SAG-0501).

mod common;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::common::send;
    use decide::input::{build_request, read_batch};
    use decide::{ModelsOptions, models, send_batch};
    use sc_agent_http::testing::ScriptedTransport;
    use sc_agent_log::testing::MemoryLog;
    use sc_agent_types::CancelToken;
    use sc_decision_schema::{ModelMetadataList, SystemOneResponse};

    use crate::common::{
        TYPESAFE_KEY, config_dir, context, empty_config_dir, fixture, header, json_response,
        options,
    };

    /// Every request example of openapi.json v0.2.0 and the TypeSafe guides that needs no
    /// image part (those are in `every_part_example_is_sent_as_multipart`).
    const REQUESTS: [(&str, &str); 11] = [
        ("request_noul", "response_noul"),
        ("request_choice", "response_choice"),
        ("request_score", "response_score"),
        ("request_box", "response_box"),
        ("request_noul_criteria", "response_noul"),
        ("request_null_fields", "response_box_confidence_number"),
        ("property_examples_request", "docs_quickstart_response_1"),
        ("docs_p_models_request_1", "docs_noul_response_1"),
        ("docs_p_images_request_1", "docs_noul_response_2"),
        ("docs_p_images_request_3", "docs_noul_response_1"),
        ("docs_p_images_request_4", "docs_choice_response_1"),
    ];

    /// The request examples sent as multipart in `every_part_example_is_sent_as_multipart`.
    const PART_REQUESTS: [&str; 2] = ["request_assets_part", "docs_box_request_1"];

    /// REQ-SAG-0501 AC1: every request fixture (openapi.json and the guides) is sent by one
    /// of the two tests; none is left out.
    #[test]
    fn every_request_fixture_is_sent() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("sc-decision-schema")
            .join("tests")
            .join("fixtures")
            .join("typesafe");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .expect("fixtures")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter_map(|file| file.strip_suffix(".json").map(str::to_owned))
            .filter(|name| name.contains("request"))
            .collect();
        names.sort();
        let mut sent: Vec<String> = REQUESTS
            .iter()
            .map(|(request, _)| (*request).to_owned())
            .chain(PART_REQUESTS.iter().map(|name| (*name).to_owned()))
            .collect();
        sent.sort();
        assert_eq!(names, sent);
    }

    /// AC: REQ-SAG-0501 AC1. Each example request goes through `decide::send` (real runner,
    /// SystemOneBackend), reaches the transport unchanged, and returns the API's response.
    #[test]
    fn every_typesafe_request_example_returns_the_api_response() {
        for (request_name, response_name) in REQUESTS {
            let request_text = fixture(request_name);
            let response_text = fixture(response_name);
            let transport = ScriptedTransport::responses(vec![json_response(200, &response_text)]);
            let log = MemoryLog::new().expect("log");
            let dir = empty_config_dir();
            let context = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let options = options("jev", None, context, &log);
            let request =
                build_request(request_text.as_bytes(), &[], false, None).expect("request");
            let result = send(&options, request, &CancelToken::new()).expect("send");

            let sent = transport.requests();
            assert_eq!(sent.len(), 1, "{request_name}");
            assert_eq!(
                sent[0].endpoint().as_str(),
                "https://api.typesafe.ai/v1/systemone"
            );
            assert_eq!(sent[0].body(), request_text.as_bytes(), "{request_name}");
            assert!(sent[0].secret().is_some(), "the TypeSafe key is attached");
            let printed = decide::output::result_line(&result).expect("line");
            let printed: serde_json::Value = serde_json::from_slice(&printed).expect("json line");
            assert_eq!(
                printed["response"],
                serde_json::from_str::<serde_json::Value>(&response_text).expect("json"),
                "{request_name}"
            );
            assert_eq!(printed["position"], 0);
            assert!(printed["session_id"].is_string());
            let response = result.outcome.expect(request_name);
            let expected =
                SystemOneResponse::from_json(200, response_text.as_bytes()).expect("fixture");
            assert_eq!(response, expected, "{request_name}");
        }
    }

    /// The part names each `assets[].part` example refers to.
    fn part_names(request: &str) -> Vec<String> {
        let value: serde_json::Value = serde_json::from_str(request).expect("json");
        value["assets"]
            .as_object()
            .expect("assets")
            .values()
            .filter_map(|asset| asset["part"].as_str().map(str::to_owned))
            .collect()
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    /// AC: REQ-SAG-0501 images. Each example with `assets[].part` is sent with
    /// `--image NAME=FILE` and as a batch wrapper line; the transport sees a multipart body
    /// with one part per asset, and `--long-context` adds the long-context header.
    #[test]
    fn every_part_example_is_sent_as_multipart() {
        for name in PART_REQUESTS {
            let request_text = fixture(name);
            let parts = part_names(&request_text);
            assert!(!parts.is_empty(), "{name} names a part");
            let images_dir = tempfile::tempdir().expect("dir");
            let images: Vec<(String, PathBuf)> = parts
                .iter()
                .map(|part| {
                    let path = images_dir.path().join(format!("{part}.png"));
                    std::fs::write(&path, PNG).expect("image");
                    (part.clone(), path)
                })
                .collect();

            // Single request: --image NAME=FILE and --long-context.
            let transport =
                ScriptedTransport::responses(vec![json_response(200, &fixture("response_noul"))]);
            let log = MemoryLog::new().expect("log");
            let dir = empty_config_dir();
            let ctx = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let options = options("jev", None, ctx, &log);
            let request =
                build_request(request_text.as_bytes(), &images, true, None).expect("request");
            let result = send(&options, request, &CancelToken::new()).expect("send");
            assert!(result.outcome.is_ok(), "{name}: {:?}", result.outcome);
            let sent = transport.requests();
            assert_eq!(sent.len(), 1);
            let content_type = header(&sent[0], "content-type").expect("content type");
            assert!(
                content_type.starts_with("multipart/form-data; boundary="),
                "{content_type}"
            );
            let body = String::from_utf8_lossy(sent[0].body());
            assert!(body.contains("name=\"request\""));
            for part in &parts {
                assert!(
                    body.contains(&format!("name=\"{part}\"")),
                    "{name}: part {part}"
                );
            }
            assert_eq!(
                header(&sent[0], "TypeSafe-Danger-Zone-Long-Context"),
                Some("true")
            );
            assert_eq!(
                header(&sent[0], "TypeSafe-Danger-Zone-Images"),
                Some("true")
            );

            // Batch wrapper line: {"request", "images", "long_context"}.
            let images_json: serde_json::Map<String, serde_json::Value> = images
                .iter()
                .map(|(part, path)| (part.clone(), path.display().to_string().into()))
                .collect();
            let line = serde_json::json!({
                "request": serde_json::from_str::<serde_json::Value>(&request_text).expect("json"),
                "images": images_json,
                "long_context": true,
            });
            let requests = read_batch(&mut line.to_string().as_bytes()).expect("batch");
            let transport =
                ScriptedTransport::responses(vec![json_response(200, &fixture("response_noul"))]);
            let ctx = context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            );
            let options = crate::common::options("jev", None, ctx, &log);
            let mut results = Vec::new();
            send_batch(&options, requests, &CancelToken::new(), &mut |result| {
                results.push(result);
                Ok(())
            })
            .expect("batch");
            assert_eq!(results.len(), 1);
            assert!(results[0].outcome.is_ok());
            let sent = transport.requests();
            let body = String::from_utf8_lossy(sent[0].body());
            for part in &parts {
                assert!(
                    body.contains(&format!("name=\"{part}\"")),
                    "{name}: batch part {part}"
                );
            }
            assert_eq!(
                header(&sent[0], "TypeSafe-Danger-Zone-Long-Context"),
                Some("true")
            );
        }
    }

    /// Without `--long-context` no long-context header is sent.
    #[test]
    fn the_long_context_header_is_only_sent_when_asked() {
        let transport =
            ScriptedTransport::responses(vec![json_response(200, &fixture("response_noul"))]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let ctx = context(
            dir.path(),
            &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
            Arc::new(transport.clone()),
            &log,
        );
        let options = options("jev", None, ctx, &log);
        let request =
            build_request(fixture("request_noul").as_bytes(), &[], false, None).expect("request");
        send(&options, request, &CancelToken::new()).expect("send");
        assert_eq!(
            header(
                &transport.requests()[0],
                "TypeSafe-Danger-Zone-Long-Context"
            ),
            None
        );
        assert_eq!(
            header(&transport.requests()[0], "content-type"),
            Some("application/json")
        );
    }

    /// AC: REQ-SAG-0501 AC2. `decide models` returns the TypeSafe models list.
    #[test]
    fn models_returns_the_typesafe_models_list() {
        let list_text = fixture("models");
        let transport = ScriptedTransport::responses(vec![json_response(200, &list_text)]);
        let log = MemoryLog::new().expect("log");
        let dir = empty_config_dir();
        let options = ModelsOptions {
            runner: None,
            context: context(
                dir.path(),
                &[(sc_agent_secrets::TYPESAFE_API_KEY, TYPESAFE_KEY)],
                Arc::new(transport.clone()),
                &log,
            ),
        };
        let list = models(&options, &CancelToken::new()).expect("models");
        assert_eq!(
            list,
            ModelMetadataList::from_json(200, list_text.as_bytes()).expect("fixture")
        );
        let sent = transport.requests();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0].endpoint().as_str(),
            "https://api.typesafe.ai/v1/models"
        );
        assert!(sent[0].secret().is_some());
        assert_eq!(serde_json::to_string(&list).expect("json"), list_text);
    }

    /// `models --runner laya` lists a configured runner's models, with no key.
    #[test]
    fn models_of_a_local_runner() {
        let list_text = fixture("models");
        let transport = ScriptedTransport::responses(vec![json_response(200, &list_text)]);
        let log = MemoryLog::new().expect("log");
        let dir = config_dir(
            r#"{"local_runners":{"laya":{"kind":"laya","host":"127.0.0.1","port":9000,"model":"jev-latest"}}}"#,
        );
        let options = ModelsOptions {
            runner: Some(sc_agent_config::RunnerName::try_from("laya".to_owned()).expect("name")),
            context: context(dir.path(), &[], Arc::new(transport.clone()), &log),
        };
        let list = models(&options, &CancelToken::new()).expect("models");
        assert_eq!(list.models.len(), 1);
        let sent = transport.requests();
        assert_eq!(
            sent[0].endpoint().as_str(),
            "http://127.0.0.1:9000/v1/models"
        );
        assert!(sent[0].secret().is_none());

        let unknown = ModelsOptions {
            runner: Some(sc_agent_config::RunnerName::try_from("nope".to_owned()).expect("name")),
            context: context(dir.path(), &[], Arc::new(transport.clone()), &log),
        };
        let error = models(&unknown, &CancelToken::new()).expect_err("unknown runner");
        assert_eq!(sc_agent_types::ErrorId::id(&error), "decide.unknown_runner");
    }
}
