fn request_bytes(repo: &RepoRoot, spec: &str, stdin: &mut dyn Read) -> Result<Vec<u8>, JevError> {
    if spec == "-" {
        let mut bytes = Vec::new();
        stdin
            .read_to_end(&mut bytes)
            .map_err(|error| JevError::io_failed("stdin", &error))?;
        return Ok(bytes);
    }
    let path = joined_path(repo.as_path(), Path::new(spec));
    std::fs::read(&path).map_err(|error| {
        JevError::read_failed_with(
            path.display().to_string(),
            format!("request JSON {}", path.display()),
            "Pass an existing file as --request-json (or - for stdin).",
            &error,
        )
    })
}
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
fn read_line_bounded<R: BufRead>(
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
            if out.len() + take > max {
                too_long = true;
                out.clear();
            } else {
                out.extend_from_slice(&available[..take]);
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
