//! Reading requests: one JSON request, a JSONL batch, image files, and the REQ-SAG-0014 limits.
//!
//! Every request is built with `DecisionRequest::new` (ADR-SAG-0001). A batch is read and
//! validated whole before anything is sent; the first bad line stops decidr (REQ-SAG-0504).
//! The in-memory forms ([`build_request_bytes`], [`build_batch`]) apply the same limits for
//! sc-agent-py.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use sc_decision_schema::{
    DecisionError, DecisionRequest, ImageFormat, ImagePart, LimitKind as SchemaLimit, MAX_IMAGES,
    OrderedMap, SystemOneRequest,
};
use serde_json::value::RawValue;

use crate::DecidrError;

/// The most bytes of one request: a request file, stdin, or one batch line (8 MiB).
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
/// The most requests in one batch.
pub const MAX_BATCH_LINES: u64 = 10_000;
/// The most bytes of one batch, lines plus image bytes (256 MiB).
pub const MAX_BATCH_BYTES: u64 = 256 * 1024 * 1024;
/// The most bytes of one image (20 MiB); also enforced by `DecisionRequest::new` (A-2).
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// Which REQ-SAG-0014 input limit a [`DecidrError::LimitExceeded`] names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LimitKind {
    /// One request, or one batch line, over [`MAX_REQUEST_BYTES`].
    RequestBytes,
    /// A batch over [`MAX_BATCH_LINES`] physical lines.
    BatchLines,
    /// A batch over [`MAX_BATCH_BYTES`] (every byte read, plus image bytes).
    BatchBytes,
    /// One image over [`MAX_IMAGE_BYTES`].
    ImageBytes,
}

impl LimitKind {
    /// `"request_bytes"`, `"batch_lines"`, `"batch_bytes"` or `"image_bytes"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestBytes => "request_bytes",
            Self::BatchLines => "batch_lines",
            Self::BatchBytes => "batch_bytes",
            Self::ImageBytes => "image_bytes",
        }
    }
}

impl std::fmt::Display for LimitKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// `usize` -> `u64` for limits and sizes (lossless on every supported target).
fn to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Reads one request: the file `spec`, or `stdin` when `spec` is `-`.
///
/// Reads at most [`MAX_REQUEST_BYTES`] + 1 bytes, so an oversized input is refused without
/// being held whole. Paths are relative to the working directory.
///
/// # Errors
///
/// [`DecidrError::LimitExceeded`] (`request_bytes`) past the cap; [`DecidrError::InputNotFound`]
/// for a missing file; [`DecidrError::InputRead`] for any other io error.
pub fn request_bytes(spec: &str, stdin: &mut dyn Read) -> Result<Vec<u8>, DecidrError> {
    let mut bytes = Vec::new();
    let cap = to_u64(MAX_REQUEST_BYTES).saturating_add(1);
    if spec == "-" {
        stdin
            .take(cap)
            .read_to_end(&mut bytes)
            .map_err(|error| DecidrError::input("stdin", error))?;
    } else {
        File::open(spec)
            .and_then(|file| file.take(cap).read_to_end(&mut bytes))
            .map_err(|error| DecidrError::input(spec, error))?;
    }
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(DecidrError::LimitExceeded {
            what: LimitKind::RequestBytes,
            line: None,
            max: to_u64(MAX_REQUEST_BYTES),
        });
    }
    Ok(bytes)
}

/// The JSONL lines of `spec`: `stdin` when `spec` is `-`, else the file, buffered.
///
/// # Errors
///
/// [`DecidrError::InputNotFound`] or [`DecidrError::InputRead`] when the file cannot be opened.
pub fn open_lines(
    spec: &str,
    stdin: Box<dyn BufRead + Send>,
) -> Result<Box<dyn BufRead + Send>, DecidrError> {
    if spec == "-" {
        return Ok(stdin);
    }
    let file = File::open(spec).map_err(|error| DecidrError::input(spec, error))?;
    Ok(Box::new(BufReader::new(file)))
}

/// Reads one image file: its size is checked from metadata before it is read.
///
/// The format comes from the extension (`.png`, `.jpg`, `.jpeg`, any case).
///
/// # Errors
///
/// [`DecidrError::LimitExceeded`] (`image_bytes`) over [`MAX_IMAGE_BYTES`];
/// [`DecidrError::ImageFormat`] for another extension; [`DecidrError::ImageRead`] for an io
/// error.
pub fn read_image(name: &str, path: &Path) -> Result<ImagePart, DecidrError> {
    read_image_at(name, path, None)
}

fn read_image_at(name: &str, path: &Path, line: Option<u64>) -> Result<ImagePart, DecidrError> {
    let image_error = |source: std::io::Error| DecidrError::ImageRead {
        name: name.to_owned(),
        path: path.to_path_buf(),
        source,
    };
    let too_large = || DecidrError::LimitExceeded {
        what: LimitKind::ImageBytes,
        line,
        max: MAX_IMAGE_BYTES,
    };
    let format = extension_format(path).ok_or_else(|| DecidrError::ImageFormat {
        name: name.to_owned(),
        path: path.to_path_buf(),
    })?;
    let metadata = std::fs::metadata(path).map_err(image_error)?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| {
            file.take(MAX_IMAGE_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(image_error)?;
    // The file may have grown between the metadata check and the read.
    if to_u64(bytes.len()) > MAX_IMAGE_BYTES {
        return Err(too_large());
    }
    Ok(ImagePart::new(name, format, bytes))
}

fn extension_format(path: &Path) -> Option<ImageFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" => Some(ImageFormat::Jpg),
        "jpeg" => Some(ImageFormat::Jpeg),
        _ => None,
    }
}

/// The format of in-memory image bytes, from their magic bytes.
fn magic_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else {
        None
    }
}

/// `DecisionError::Limits` for more than [`MAX_IMAGES`] images, checked before any image is read.
fn check_image_count(count: usize, line: Option<u64>) -> Result<(), DecidrError> {
    if count > MAX_IMAGES {
        return Err(DecidrError::request_invalid(
            line,
            DecisionError::Limits {
                what: SchemaLimit::Images,
                max: to_u64(MAX_IMAGES),
            },
        ));
    }
    Ok(())
}

/// The request body of `json`: at most [`MAX_REQUEST_BYTES`], UTF-8, a `SystemOneRequest`.
fn parse_body(json: &[u8], line: Option<u64>) -> Result<SystemOneRequest, DecidrError> {
    if json.len() > MAX_REQUEST_BYTES {
        return Err(DecidrError::LimitExceeded {
            what: LimitKind::RequestBytes,
            line,
            max: to_u64(MAX_REQUEST_BYTES),
        });
    }
    let text = std::str::from_utf8(json).map_err(|error| {
        DecidrError::request_invalid(
            line,
            DecisionError::RequestParse {
                path: ".".to_owned(),
                message: format!(
                    "the request is not UTF-8 (invalid byte at offset {})",
                    error.valid_up_to()
                ),
            },
        )
    })?;
    SystemOneRequest::from_json(text).map_err(|source| DecidrError::request_invalid(line, source))
}

/// `DecisionRequest::new`, its refusal wrapped as `RequestInvalid` naming `line`.
fn new_request(
    body: SystemOneRequest,
    parts: Vec<ImagePart>,
    long_context: bool,
    line: Option<u64>,
) -> Result<DecisionRequest, DecidrError> {
    DecisionRequest::new(body, parts, long_context)
        .map_err(|source| DecidrError::request_invalid(line, source))
}

/// Builds a request from JSON bytes, image files and the long-context flag.
///
/// `SystemOneRequest::from_json`, [`read_image`] for each image, then
/// `DecisionRequest::new(body, parts, long_context)`, which checks the parts and the image
/// limits (A-2). `line` names the JSONL line in errors (`None` for a single request).
///
/// # Errors
///
/// [`DecidrError::RequestInvalid`] (cause `decision.request_parse`, `decision.image_parts` or
/// `decision.limits`), [`DecidrError::LimitExceeded`], or an image error of [`read_image`].
pub fn build_request(
    json: &[u8],
    images: &[(String, PathBuf)],
    long_context: bool,
    line: Option<u64>,
) -> Result<DecisionRequest, DecidrError> {
    let body = parse_body(json, line)?;
    check_image_count(images.len(), line)?;
    let parts = images
        .iter()
        .map(|(name, path)| read_image_at(name, path, line))
        .collect::<Result<Vec<_>, _>>()?;
    new_request(body, parts, long_context, line)
}

/// The in-memory form of [`build_request`] (sc-agent-py).
///
/// `json` over [`MAX_REQUEST_BYTES`] and an image over [`MAX_IMAGE_BYTES`] are
/// `LimitExceeded`; the format comes from the magic bytes (PNG `89 50 4E 47`, JPEG
/// `FF D8 FF`).
///
/// # Errors
///
/// As [`build_request`]; [`DecidrError::ImageFormat`] (empty path) for unknown magic bytes.
pub fn build_request_bytes(
    json: &[u8],
    images: Vec<(String, Vec<u8>)>,
    long_context: bool,
    line: Option<u64>,
) -> Result<DecisionRequest, DecidrError> {
    let body = parse_body(json, line)?;
    check_image_count(images.len(), line)?;
    let parts = images
        .into_iter()
        .map(|(name, bytes)| {
            if to_u64(bytes.len()) > MAX_IMAGE_BYTES {
                return Err(DecidrError::LimitExceeded {
                    what: LimitKind::ImageBytes,
                    line,
                    max: MAX_IMAGE_BYTES,
                });
            }
            let format = magic_format(&bytes).ok_or_else(|| DecidrError::ImageFormat {
                name: name.clone(),
                path: PathBuf::new(),
            })?;
            Ok(ImagePart::new(name, format, bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    new_request(body, parts, long_context, line)
}

/// One in-memory batch item (sc-agent-py): JSON text, image bytes by name, long-context flag.
#[derive(Debug, Clone, Default)]
pub struct BatchInput {
    /// The Jev request JSON.
    pub json: String,
    /// Image parts by name.
    pub images: Vec<(String, Vec<u8>)>,
    /// Whether the request carries the long-context header.
    pub long_context: bool,
}

/// Counts physical lines and bytes of a batch against [`MAX_BATCH_LINES`] and [`MAX_BATCH_BYTES`].
#[derive(Default)]
struct BatchBudget {
    requests: u64,
    bytes: u64,
}

impl BatchBudget {
    /// Adds one line (or in-memory item) of `bytes` bytes at `line`.
    fn add(&mut self, bytes: u64, line: u64) -> Result<(), DecidrError> {
        self.requests = self.requests.saturating_add(1);
        if self.requests > MAX_BATCH_LINES {
            return Err(DecidrError::LimitExceeded {
                what: LimitKind::BatchLines,
                line: Some(line),
                max: MAX_BATCH_LINES,
            });
        }
        self.add_bytes(bytes, line)
    }

    /// Adds `bytes` more bytes (an image) at `line`.
    fn add_bytes(&mut self, bytes: u64, line: u64) -> Result<(), DecidrError> {
        self.bytes = self.bytes.saturating_add(bytes);
        if self.bytes > MAX_BATCH_BYTES {
            return Err(DecidrError::LimitExceeded {
                what: LimitKind::BatchBytes,
                line: Some(line),
                max: MAX_BATCH_BYTES,
            });
        }
        Ok(())
    }
}

/// The batch caps over item sizes (JSON plus image bytes), in input order; item `n` is line `n`.
fn check_batch_sizes(sizes: impl Iterator<Item = u64>) -> Result<(), DecidrError> {
    let mut budget = BatchBudget::default();
    for (line, bytes) in (1_u64..).zip(sizes) {
        budget.add(bytes, line)?;
    }
    Ok(())
}

/// The in-memory batch path (sc-agent-py), with the limits of [`read_batch`].
///
/// More than [`MAX_BATCH_LINES`] items, or more than [`MAX_BATCH_BYTES`] in total (JSON plus
/// image bytes), is refused before any item is built; then each item goes through
/// [`build_request_bytes`] with `line` = its 1-based index.
///
/// # Errors
///
/// [`DecidrError::LimitExceeded`] (`batch_lines`, `batch_bytes`) or the first item's error of
/// [`build_request_bytes`].
pub fn build_batch(items: Vec<BatchInput>) -> Result<Vec<DecisionRequest>, DecidrError> {
    check_batch_sizes(items.iter().map(|item| {
        let images: usize = item.images.iter().map(|(_, bytes)| bytes.len()).sum();
        to_u64(item.json.len()).saturating_add(to_u64(images))
    }))?;
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let line = to_u64(index).saturating_add(1);
            build_request_bytes(
                item.json.as_bytes(),
                item.images,
                item.long_context,
                Some(line),
            )
        })
        .collect()
}

/// Outcome of one bounded line read.
enum LineRead {
    /// End of input with no further bytes.
    End,
    /// A line (with its newline, if any) is in the buffer.
    Line,
    /// The line exceeded the cap; it was consumed and discarded.
    TooLong,
}

/// Reads one line into `out`, holding at most `max` bytes: the rest of a
/// longer line is consumed and dropped, so memory stays bounded.
fn read_line_bounded<R: BufRead + ?Sized>(
    reader: &mut R,
    max: usize,
    out: &mut Vec<u8>,
) -> std::io::Result<LineRead> {
    out.clear();
    let mut too_long = false;
    let mut any = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(match (too_long, any) {
                (true, _) => LineRead::TooLong,
                (false, true) => LineRead::Line,
                (false, false) => LineRead::End,
            });
        }
        any = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |at| at + 1);
        if !too_long {
            match available.get(..take) {
                Some(chunk) if out.len() + take <= max => out.extend_from_slice(chunk),
                _ => {
                    too_long = true;
                    out.clear();
                }
            }
        }
        reader.consume(take);
        if newline.is_some() {
            return Ok(if too_long {
                LineRead::TooLong
            } else {
                LineRead::Line
            });
        }
    }
}

/// One batch line in its wrapper form, chosen by the presence of the `"request"` key.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchLine {
    request: Box<RawValue>,
    #[serde(default)]
    images: Option<OrderedMap<String, PathBuf>>,
    #[serde(default)]
    long_context: bool,
}

/// Whether `text` is a JSON object with a top-level `"request"` key.
fn is_wrapper(text: &str) -> bool {
    serde_json::from_str::<std::collections::BTreeMap<String, &RawValue>>(text)
        .is_ok_and(|object| object.contains_key("request"))
}

/// The request of one non-blank batch line `text` at `line`.
fn batch_request(
    text: &str,
    line: u64,
    budget: &mut BatchBudget,
) -> Result<DecisionRequest, DecidrError> {
    if !is_wrapper(text) {
        return build_request(text.as_bytes(), &[], false, Some(line));
    }
    let wrapper: BatchLine = serde_json::from_str(text).map_err(|error| {
        DecidrError::request_invalid(
            Some(line),
            DecisionError::RequestParse {
                path: ".".to_owned(),
                message: format!("the batch line wrapper is invalid: {error}"),
            },
        )
    })?;
    let images: Vec<(String, PathBuf)> = wrapper
        .images
        .as_ref()
        .map(|images| {
            images
                .into_iter()
                .map(|(name, path)| (name.clone(), path.clone()))
                .collect()
        })
        .unwrap_or_default();
    check_image_count(images.len(), Some(line))?;
    let request = build_request(
        wrapper.request.get().as_bytes(),
        &images,
        wrapper.long_context,
        Some(line),
    )?;
    // The bytes actually read count, not the metadata length checked before the read (RSH-003).
    let read: usize = request
        .image_parts()
        .iter()
        .map(|part| part.bytes.len())
        .sum();
    budget.add_bytes(to_u64(read), line)?;
    Ok(request)
}

/// Reads and validates a whole JSONL batch before anything is sent (REQ-SAG-0504, REQ-SAG-0014).
///
/// Each non-blank line is a bare Jev request or `{"request": {..}, "images": {"NAME": "path"},
/// "long_context": true}` (paths relative to the working directory). Blank lines and a
/// leading UTF-8 BOM are skipped, but every physical line and byte read counts against
/// [`MAX_BATCH_LINES`] and [`MAX_BATCH_BYTES`]; line numbers count every physical line from 1.
///
/// # Errors
///
/// The first bad line: [`DecidrError::LimitExceeded`] (`request_bytes` for a line over
/// [`MAX_REQUEST_BYTES`], `batch_lines`, `batch_bytes`, `image_bytes`),
/// [`DecidrError::LineNotUtf8`], [`DecidrError::RequestInvalid`] naming the line, the field
/// and the reason, or an image error; [`DecidrError::InputRead`] for an io error.
pub fn read_batch(reader: &mut dyn BufRead) -> Result<Vec<DecisionRequest>, DecidrError> {
    let mut requests = Vec::new();
    let mut budget = BatchBudget::default();
    let mut buffer = Vec::new();
    let mut line: u64 = 0;
    loop {
        // The terminator (`\n` or `\r\n`) does not count against the cap (QA-005).
        let read = read_line_bounded(reader, MAX_REQUEST_BYTES + 2, &mut buffer)
            .map_err(|error| DecidrError::input("batch", error))?;
        line = line.saturating_add(1);
        match read {
            LineRead::End => return Ok(requests),
            LineRead::TooLong => {
                return Err(DecidrError::LimitExceeded {
                    what: LimitKind::RequestBytes,
                    line: Some(line),
                    max: to_u64(MAX_REQUEST_BYTES),
                });
            }
            LineRead::Line => {}
        }
        // Every physical line and byte read counts, blank lines and the BOM included (RSH-002).
        budget.add(to_u64(buffer.len()), line)?;
        let content = buffer
            .strip_suffix(b"\n")
            .map_or(buffer.as_slice(), |line| {
                line.strip_suffix(b"\r").unwrap_or(line)
            });
        if content.len() > MAX_REQUEST_BYTES {
            return Err(DecidrError::LimitExceeded {
                what: LimitKind::RequestBytes,
                line: Some(line),
                max: to_u64(MAX_REQUEST_BYTES),
            });
        }
        let mut bytes = buffer.as_slice();
        if line == 1 {
            bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        }
        let text = std::str::from_utf8(bytes).map_err(|error| DecidrError::LineNotUtf8 {
            line,
            byte_offset: error.valid_up_to(),
        })?;
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        requests.push(batch_request(text, line, &mut budget)?);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn read_all(data: &[u8], max: usize) -> Vec<(&'static str, Vec<u8>)> {
        let mut reader = Cursor::new(data.to_vec());
        let mut out = Vec::new();
        let mut buffer = Vec::new();
        loop {
            match read_line_bounded(&mut reader, max, &mut buffer).expect("read") {
                LineRead::End => return out,
                LineRead::Line => out.push(("line", buffer.clone())),
                LineRead::TooLong => out.push(("long", Vec::new())),
            }
        }
    }

    #[test]
    fn lines_are_split_and_a_final_line_without_newline_is_kept() {
        let got = read_all(b"ab\ncd\nef", 64);
        assert_eq!(
            got,
            vec![
                ("line", b"ab\n".to_vec()),
                ("line", b"cd\n".to_vec()),
                ("line", b"ef".to_vec()),
            ]
        );
    }

    #[test]
    fn an_over_long_line_is_dropped_whole_and_the_next_line_survives() {
        let mut data = vec![b'x'; 50];
        data.push(b'\n');
        data.extend_from_slice(b"ok\n");
        let got = read_all(&data, 10);
        assert_eq!(got, vec![("long", Vec::new()), ("line", b"ok\n".to_vec())]);
    }

    /// The buffer never holds more than the cap, however long the line is.
    #[test]
    fn reading_never_buffers_more_than_the_cap() {
        let data = vec![b'x'; 10_000];
        let mut reader = Cursor::new(data);
        let mut buffer = Vec::new();
        let read = read_line_bounded(&mut reader, 100, &mut buffer).expect("read");
        assert!(matches!(read, LineRead::TooLong));
        assert!(buffer.len() <= 100);
    }

    const NOUL: &str = r#"{"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}}"#;

    #[test]
    fn blank_lines_and_a_bom_are_skipped() {
        let data = format!("\u{feff}{NOUL}\n\n  \n{NOUL}\r\n");
        let requests = read_batch(&mut Cursor::new(data)).expect("batch");
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn a_non_utf8_line_is_named() {
        let mut data = format!("{NOUL}\n").into_bytes();
        data.extend_from_slice(b"{\"x\xff\n");
        let error = read_batch(&mut Cursor::new(data)).expect_err("not utf-8");
        assert!(matches!(
            error,
            DecidrError::LineNotUtf8 {
                line: 2,
                byte_offset: 3
            }
        ));
    }

    #[test]
    fn a_wrapper_with_an_unknown_key_is_refused() {
        let data = format!(r#"{{"request":{NOUL},"extra":1}}"#);
        let error = read_batch(&mut Cursor::new(data)).expect_err("unknown key");
        assert!(matches!(
            error,
            DecidrError::RequestInvalid { line: Some(1), .. }
        ));
    }

    #[test]
    fn in_memory_images_need_png_or_jpeg_magic() {
        let error = build_request_bytes(
            NOUL.as_bytes(),
            vec![("i".into(), vec![1, 2, 3])],
            false,
            None,
        )
        .expect_err("unknown magic");
        assert!(matches!(error, DecidrError::ImageFormat { .. }));
    }

    /// Blank lines of `width` spaces each, generated on demand: a batch of any size with a
    /// 64 KiB buffer, never held whole.
    struct BlankLines {
        width: usize,
        remaining: u64,
        column: usize,
    }

    impl Read for BlankLines {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.remaining == 0 || out.is_empty() {
                return Ok(0);
            }
            if self.column == self.width {
                self.column = 0;
                self.remaining -= 1;
                out[0] = b'\n';
                return Ok(1);
            }
            let spaces = (self.width - self.column).min(out.len());
            out[..spaces].fill(b' ');
            self.column += spaces;
            Ok(spaces)
        }
    }

    fn blank_lines(width: usize, lines: u64) -> std::io::BufReader<BlankLines> {
        std::io::BufReader::with_capacity(
            64 * 1024,
            BlankLines {
                width,
                remaining: lines,
                column: 0,
            },
        )
    }

    /// REQ-QA-002, RSH-002: a batch past 256 MiB is `batch_bytes` at the first line over the
    /// cap, blank lines counted, without the batch ever being held in memory.
    #[test]
    fn a_batch_over_256_mib_is_batch_bytes() {
        // Lines of 1 MiB plus a newline: line 256 passes 256 MiB because the newlines count.
        let error = read_batch(&mut blank_lines(1024 * 1024, 300)).expect_err("too big");
        assert!(
            matches!(
                error,
                DecidrError::LimitExceeded {
                    what: LimitKind::BatchBytes,
                    line: Some(256),
                    max: MAX_BATCH_BYTES,
                }
            ),
            "{error:?}"
        );
    }

    /// RSH-002: blank lines count against the line cap too.
    #[test]
    fn blank_lines_count_against_batch_lines() {
        let error = read_batch(&mut blank_lines(0, MAX_BATCH_LINES + 5)).expect_err("too many");
        assert!(
            matches!(
                error,
                DecidrError::LimitExceeded {
                    what: LimitKind::BatchLines,
                    line: Some(10_001),
                    ..
                }
            ),
            "{error:?}"
        );
        let requests = read_batch(&mut blank_lines(0, MAX_BATCH_LINES)).expect("at the cap");
        assert!(requests.is_empty());
    }

    /// REQ-QA-002: the in-memory batch applies the same byte cap to item sizes (JSON plus
    /// image bytes) before any item is built; counted from sizes, so no 256 MiB is allocated.
    #[test]
    fn in_memory_item_sizes_over_256_mib_are_batch_bytes() {
        let mib = 1024 * 1024;
        let error = check_batch_sizes([200 * mib, 56 * mib, 1].into_iter()).expect_err("over");
        assert!(matches!(
            error,
            DecidrError::LimitExceeded {
                what: LimitKind::BatchBytes,
                line: Some(3),
                ..
            }
        ));
        check_batch_sizes([200 * mib, 56 * mib].into_iter()).expect("exactly 256 MiB");
    }

    /// A request padded with spaces to exactly `len` bytes.
    fn padded(len: usize) -> Vec<u8> {
        let mut bytes = NOUL.as_bytes().to_vec();
        bytes.resize(len, b' ');
        bytes
    }

    /// QA-005: a line of exactly MAX_REQUEST_BYTES plus its terminator is accepted; one byte
    /// more is `request_bytes`. The terminator does not count.
    #[test]
    fn a_line_of_exactly_the_cap_is_accepted() {
        for terminator in [&b"\n"[..], b"\r\n", b""] {
            let mut line = padded(MAX_REQUEST_BYTES);
            line.extend_from_slice(terminator);
            let requests = read_batch(&mut Cursor::new(line)).expect("exactly the cap");
            assert_eq!(requests.len(), 1);
            let mut over = padded(MAX_REQUEST_BYTES + 1);
            over.extend_from_slice(terminator);
            let error = read_batch(&mut Cursor::new(over)).expect_err("one byte over");
            assert!(matches!(
                error,
                DecidrError::LimitExceeded {
                    what: LimitKind::RequestBytes,
                    line: Some(1),
                    ..
                }
            ));
        }
        assert!(build_request(&padded(MAX_REQUEST_BYTES), &[], false, None).is_ok());
        assert!(build_request(&padded(MAX_REQUEST_BYTES + 1), &[], false, None).is_err());
    }

    /// QA-002 (RSH-003): the image bytes a wrapper line actually read count toward the batch
    /// budget; a budget one image short of the cap is pushed over by it.
    #[test]
    fn wrapper_line_image_bytes_count_toward_the_batch_budget() {
        let dir = tempfile::tempdir().expect("dir");
        let image = dir.path().join("i.png");
        let png = [0x89_u8, b'P', b'N', b'G', 1, 2, 3, 4];
        std::fs::write(&image, png).expect("image");
        let line = format!(
            r#"{{"request":{{"model":"m","state":{{"d":"a"}},"assets":{{"a":{{"type":"image","part":"p"}}}},"questions":{{"q":{{"type":"noul"}}}}}},"images":{{"p":{:?}}}}}"#,
            image.display().to_string()
        );
        let image_len = to_u64(png.len());
        // Room for the image minus one byte: the image's bytes are what pushes it over.
        let mut budget = BatchBudget {
            requests: 0,
            bytes: MAX_BATCH_BYTES - image_len + 1,
        };
        let error = batch_request(&line, 7, &mut budget).expect_err("over by the image");
        assert!(matches!(
            error,
            DecidrError::LimitExceeded {
                what: LimitKind::BatchBytes,
                line: Some(7),
                ..
            }
        ));
        let mut budget = BatchBudget {
            requests: 0,
            bytes: MAX_BATCH_BYTES - image_len,
        };
        batch_request(&line, 7, &mut budget).expect("exactly at the cap");
        assert_eq!(budget.bytes, MAX_BATCH_BYTES);
    }

    /// QA-008: an image's size is checked from metadata before it is read. The file is far
    /// over the cap (sparse) and unreadable (mode 000): a read first would be `image_read`;
    /// the metadata check gives `image_bytes`.
    #[cfg(unix)]
    #[test]
    fn image_size_is_checked_before_the_file_is_read() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let image = dir.path().join("big.png");
        let file = std::fs::File::create(&image).expect("create");
        file.set_len(1024 * MAX_IMAGE_BYTES).expect("sparse length");
        drop(file);
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::File::open(&image).is_ok() {
            // Running as root: mode 000 does not stop the read, so the order is not observable.
            std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o600))
                .expect("chmod");
            panic!("run this test as a non-root user: root can read a mode-000 file");
        }
        let error = read_image("p", &image).expect_err("too large");
        std::fs::set_permissions(&image, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        assert!(
            matches!(
                error,
                DecidrError::LimitExceeded {
                    what: LimitKind::ImageBytes,
                    ..
                }
            ),
            "{error:?}"
        );
    }
}
