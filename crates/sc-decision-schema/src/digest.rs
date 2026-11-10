    pub fn of_text(text: &str) -> Self {
        let input = sc_sha::HashInput::TextFileBytes {
            utf8_file_bytes: text.as_bytes(),
        };
        match sc_sha::calculate_hash(input) {
            Ok(result) => Self(result.template().to_hex()),
            // A `&str` is valid UTF-8, the only error `calculate_hash` has.
            Err(error) => unreachable!("a &str is valid UTF-8: {error}"),
        }
    }
