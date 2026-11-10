//! `DecisionRequest::new` checks and the per-question digests.
//!
//! AC "DecisionRequest::new", AC "REQ-SAG-0014", AC "ADR-SAG-0002", AC "REQ-SAG-0202".

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod tests {
    use sc_agent_types::ErrorId;
    use sc_decision_schema::{
        DecisionError, DecisionRequest, ImageFormat, ImagePart, MAX_IMAGE_BYTES, MAX_IMAGES,
        QuestionDigest, SystemOneRequest,
    };

    fn body(text: &str) -> SystemOneRequest {
        SystemOneRequest::from_json(text).expect("body")
    }

    fn part(name: &str, bytes: &[u8]) -> ImagePart {
        ImagePart {
            name: name.to_owned(),
            format: ImageFormat::Png,
            bytes: bytes.to_vec(),
        }
    }

    /// A body whose assets name parts `names`, one asset per name.
    fn body_with_parts(names: &[&str]) -> SystemOneRequest {
        let assets = names
            .iter()
            .map(|name| format!(r#""asset-{name}":{{"type":"image","part":"{name}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        body(&format!(
            r#"{{"model":"m","state":"s","questions":{{"q":{{"type":"box"}}}},"assets":{{{assets}}}}}"#
        ))
    }

    fn image_parts_detail(result: Result<DecisionRequest, DecisionError>) -> String {
        match result {
            Err(DecisionError::ImageParts { detail }) => detail,
            other => panic!("expected ImageParts, got {other:?}"),
        }
    }

    #[test]
    fn new_checks_image_parts() {
        // An asset names a part no ImagePart has.
        let detail = image_parts_detail(DecisionRequest::new(
            body_with_parts(&["p1"]),
            vec![],
            false,
        ));
        assert!(
            detail.contains("asset `asset-p1`") && detail.contains("`p1`"),
            "{detail}"
        );

        // Two ImageParts of one name, referenced by an asset.
        let detail = image_parts_detail(DecisionRequest::new(
            body_with_parts(&["p1"]),
            vec![part("p1", b"a"), part("p1", b"b")],
            false,
        ));
        assert!(
            detail.contains("`p1`") && detail.contains("2 image parts"),
            "{detail}"
        );

        // Two ImageParts of one name that no asset references.
        let detail = image_parts_detail(DecisionRequest::new(
            body_with_parts(&["p1"]),
            vec![part("p1", b"a"), part("p2", b"b"), part("p2", b"c")],
            false,
        ));
        assert!(
            detail.contains("`p2`") && detail.contains("2 image parts"),
            "{detail}"
        );

        // An ImagePart no asset references.
        let detail = image_parts_detail(DecisionRequest::new(
            body_with_parts(&["p1"]),
            vec![part("p1", b"a"), part("stray", b"b")],
            false,
        ));
        assert!(detail.contains("image part `stray`"), "{detail}");

        // A matching set.
        let request = DecisionRequest::new(
            body_with_parts(&["p1", "p2"]),
            vec![part("p2", b"b"), part("p1", b"a")],
            true,
        )
        .expect("matching set");
        assert_eq!(request.image_parts().len(), 2);
        assert!(request.long_context());
        assert!(request.needs_images_header());

        // TryFrom of a body whose asset names a part is refused; of a plain body, accepted.
        let error =
            DecisionRequest::try_from(body_with_parts(&["p1"])).expect_err("needs its part");
        assert_eq!(error.id(), "decision.image_parts");
        let plain = DecisionRequest::try_from(body(
            r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
        ))
        .expect("plain");
        assert!(plain.image_parts().is_empty());
        assert!(!plain.long_context());
        assert!(!plain.needs_images_header());
    }

    #[test]
    fn images_header_follows_assets_or_box() {
        let inline = DecisionRequest::try_from(body(
        r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}},"assets":{"a":{"type":"image","format":"png","data_base64":"AA=="}}}"#,
    ))
    .expect("inline asset");
        assert!(inline.needs_images_header());
        let boxed = DecisionRequest::try_from(body(
            r#"{"model":"m","state":"s","questions":{"q":{"type":"box"}}}"#,
        ))
        .expect("box");
        assert!(boxed.needs_images_header());
    }

    #[test]
    fn new_checks_limits() {
        let names: Vec<String> = (0..=MAX_IMAGES).map(|index| format!("p{index}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let parts = refs.iter().map(|name| part(name, b"x")).collect();
        match DecisionRequest::new(body_with_parts(&refs), parts, false) {
            Err(DecisionError::Limits { what, max }) => {
                assert_eq!((what, max), ("images", 20));
            }
            other => panic!("expected Limits, got {other:?}"),
        }

        let big = ImagePart {
            name: "p1".to_owned(),
            format: ImageFormat::Jpg,
            bytes: vec![0; MAX_IMAGE_BYTES + 1],
        };
        let error = DecisionRequest::new(body_with_parts(&["p1"]), vec![big], false)
            .expect_err("too large");
        assert!(
            matches!(error, DecisionError::Limits { what: "image_bytes", max } if max == 20 * 1024 * 1024),
            "{error:?}"
        );
        assert_eq!(error.id(), "decision.limits");

        // Exactly at the limits is accepted.
        let names: Vec<String> = (0..MAX_IMAGES).map(|index| format!("p{index}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut parts: Vec<ImagePart> = refs.iter().map(|name| part(name, b"x")).collect();
        parts[0].bytes = vec![7; MAX_IMAGE_BYTES];
        DecisionRequest::new(body_with_parts(&refs), parts, false).expect("at the limits");
    }

    #[test]
    fn image_part_debug_hides_bytes() {
        let debug = format!("{:?}", part("p1", &[1, 2, 3]));
        assert_eq!(debug, r#"ImagePart { name: "p1", format: Png, bytes: 3 }"#);
    }

    #[test]
    fn digest_deserialize_validates() {
        let lower = "a".repeat(64);
        let digest: QuestionDigest = serde_json::from_str(&format!("\"{lower}\"")).expect("valid");
        assert_eq!(digest.as_str(), lower);
        assert_eq!(
            serde_json::to_string(&digest).expect("encode"),
            format!("\"{lower}\"")
        );
        for refused in [
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(64),
            format!("{}g", "a".repeat(63)),
        ] {
            assert!(
                serde_json::from_str::<QuestionDigest>(&format!("\"{refused}\"")).is_err(),
                "{refused}"
            );
            assert_eq!(
                QuestionDigest::try_from(refused.clone()),
                Err(sc_sha::ShaError::InvalidDigestHex)
            );
        }

        let request = DecisionRequest::try_from(body(
            r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#,
        ))
        .expect("request");
        let (_, digest) = request.digests().iter().next().expect("one");
        let text = serde_json::to_string(digest).expect("encode");
        let back: QuestionDigest = serde_json::from_str(&text).expect("decode");
        assert_eq!(&back, digest);
        assert_eq!(String::from(back.clone()), digest.as_str());
        assert_eq!(back.as_ref(), digest.as_str());
        assert!(
            digest
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
    }

    const BASE: &str = r#"{"model":"m","state":{"text":"see img"},"questions":{"q":{"type":"noul","instructions":"Is it red?"},"r":{"type":"box"},"s":{"type":"score","criteria":["low","high"]}},"assets":{"img":{"type":"image","part":"p1"}}}"#;

    fn digest_of(
        text: &str,
        parts: Vec<ImagePart>,
        long_context: bool,
        id: &str,
    ) -> QuestionDigest {
        let request = DecisionRequest::new(body(text), parts, long_context).expect("request");
        request
            .digests()
            .get(&id.to_owned())
            .cloned()
            .unwrap_or_else(|| panic!("no digest for {id}"))
    }

    fn base_q(text: &str) -> QuestionDigest {
        digest_of(text, vec![part("p1", b"\x89PNG")], false, "q")
    }

    #[test]
    fn digests() {
        let q = base_q(BASE);
        // The same information gives the same digest.
        assert_eq!(base_q(BASE), q);

        // Three questions give three digests, in request order, all different.
        let request =
            DecisionRequest::new(body(BASE), vec![part("p1", b"\x89PNG")], false).expect("request");
        let ids: Vec<&str> = request
            .digests()
            .iter()
            .map(|(id, _)| id.as_str())
            .collect();
        assert_eq!(ids, ["q", "r", "s"]);
        let values: Vec<&QuestionDigest> =
            request.digests().iter().map(|(_, digest)| digest).collect();
        assert_ne!(values[0], values[1]);
        assert_ne!(values[1], values[2]);
        assert_ne!(values[0], values[2]);

        // Model, state, assets, question q each change q's digest.
        for changed in [
            BASE.replace(r#""model":"m""#, r#""model":"m2""#),
            BASE.replace("see img", "see image"),
            BASE.replace(
                r#""type":"image","part""#,
                r#""type":"image","format":"png","part""#,
            ),
            BASE.replace("Is it red?", "Is it blue?"),
        ] {
            assert_ne!(base_q(&changed), q, "{changed}");
        }
        // An image part's byte, format or name, and long_context, change q's digest.
        assert_ne!(digest_of(BASE, vec![part("p1", b"\x89PNH")], false, "q"), q);
        let mut jpg = part("p1", b"\x89PNG");
        jpg.format = ImageFormat::Jpg;
        assert_ne!(digest_of(BASE, vec![jpg], false, "q"), q);
        assert_ne!(digest_of(BASE, vec![part("p1", b"\x89PNG")], true, "q"), q);
        let renamed = BASE.replace(r#""part":"p1""#, r#""part":"p2""#);
        assert_ne!(
            digest_of(&renamed, vec![part("p2", b"\x89PNG")], false, "q"),
            q
        );

        // Changing, removing or reordering another question leaves q's digest unchanged.
        assert_eq!(
            base_q(&BASE.replace(r#"["low","high"]"#, r#"["low","mid","high"]"#)),
            q
        );
        assert_eq!(base_q(&BASE.replace(r#","r":{"type":"box"}"#, "")), q);
        let reordered = BASE.replace(
            r#""q":{"type":"noul","instructions":"Is it red?"},"r":{"type":"box"}"#,
            r#""r":{"type":"box"},"q":{"type":"noul","instructions":"Is it red?"}"#,
        );
        assert_ne!(reordered, BASE);
        assert_eq!(base_q(&reordered), q);
    }

    #[test]
    fn digest_is_sc_sha_of_documented_input() {
        // The digest of a body with no assets or parts equals sc-sha over the documented input.
        let text = r#"{"model":"m","state":"s","questions":{"q":{"type":"noul"}}}"#;
        let input = r#"{"body":{"model":"m","state":"s","questions":{"q":{"type":"noul"}}},"long_context":false,"parts":[]}"#;
        let expected = sc_sha::calculate_hash(sc_sha::HashInput::TextFileBytes {
            utf8_file_bytes: input.as_bytes(),
        })
        .expect("hash")
        .template()
        .to_hex();
        assert_eq!(digest_of(text, vec![], false, "q").as_str(), expected);
    }
}
