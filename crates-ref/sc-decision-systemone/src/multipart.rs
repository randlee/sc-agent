//! multipart/form-data bodies for requests with image parts (RFC 7578).

use std::borrow::Cow;

use sc_decision_schema::{ImageFormat, ImagePart};

/// The boundary prefix; a fresh ULID follows it on every encode (REQ-SAG-0001).
const BOUNDARY_PREFIX: &str = "sc-agent-";

/// A fresh boundary, `sc-agent-<ULID>`.
pub(crate) fn boundary() -> String {
    format!("{BOUNDARY_PREFIX}{}", ulid::Ulid::new())
}

/// The multipart body: part `request` (the JSON body) first, then one part per image in
/// `parts` order, each named by its [`ImagePart::name`].
///
/// The parts are written as given: the request is valid by construction, so no part is
/// checked here. Part names are escaped as the HTML form encoder does (`"` as `%22`, CR as
/// `%0D`, LF as `%0A`), so a name can never end a header line early.
pub(crate) fn body(json: &str, parts: &[ImagePart], boundary: &str) -> Vec<u8> {
    let image_bytes: usize = parts.iter().map(|part| part.bytes.len()).sum();
    let mut out = Vec::with_capacity(json.len() + image_bytes + 256 * (parts.len() + 1));
    push_headers(&mut out, boundary, "request", None, "application/json");
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(b"\r\n");
    for part in parts {
        let name = escape_name(&part.name);
        push_headers(
            &mut out,
            boundary,
            &name,
            Some(&*name),
            content_type(part.format),
        );
        out.extend_from_slice(&part.bytes);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

fn push_headers(
    out: &mut Vec<u8>,
    boundary: &str,
    name: &str,
    filename: Option<&str>,
    content_type: &str,
) {
    let disposition = match filename {
        Some(filename) => format!("form-data; name=\"{name}\"; filename=\"{filename}\""),
        None => format!("form-data; name=\"{name}\""),
    };
    out.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: {disposition}\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
}

/// The part's media type. `ImageFormat` is `#[non_exhaustive]` (a-2), so a format added
/// later is sent as `application/octet-stream` until it is named here.
const fn content_type(format: ImageFormat) -> &'static str {
    if matches!(format, ImageFormat::Png) {
        "image/png"
    } else if matches!(format, ImageFormat::Jpg | ImageFormat::Jpeg) {
        "image/jpeg"
    } else {
        "application/octet-stream"
    }
}

/// `name` with `"`, CR and LF percent-escaped; borrowed when it has none of them.
fn escape_name(name: &str) -> Cow<'_, str> {
    if !name.contains(['"', '\r', '\n']) {
        return Cow::Borrowed(name);
    }
    let mut escaped = String::with_capacity(name.len() + 8);
    for ch in name.chars() {
        match ch {
            '"' => escaped.push_str("%22"),
            '\r' => escaped.push_str("%0D"),
            '\n' => escaped.push_str("%0A"),
            other => escaped.push(other),
        }
    }
    Cow::Owned(escaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one place tests build an `ImagePart` (a-2 adds `ImagePart::new`).
    fn image_part(name: &str, format: ImageFormat, bytes: Vec<u8>) -> ImagePart {
        ImagePart::new(name, format, bytes)
    }

    #[test]
    fn test_escape_name_keeps_header_line_whole() {
        assert_eq!(escape_name("a\"b\r\nc"), "a%22b%0D%0Ac");
        assert!(matches!(
            escape_name("door-photo"),
            Cow::Borrowed("door-photo")
        ));
    }

    #[test]
    fn test_body_layout() {
        let parts = vec![image_part("p1", ImageFormat::Jpg, vec![1, 2, 3])];
        let body = body("{}", &parts, "B");
        let mut expected = b"--B\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{}\r\n--B\r\nContent-Disposition: form-data; name=\"p1\"; filename=\"p1\"\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
        expected.extend_from_slice(&[1, 2, 3]);
        expected.extend_from_slice(b"\r\n--B--\r\n");
        assert_eq!(body, expected);
    }
}
