fn classify(value: Option<u8>) -> &'static str {
    match value {
        Some(0) => "zero",
        Some(_) => "some",
        None => "none",
    }
}
