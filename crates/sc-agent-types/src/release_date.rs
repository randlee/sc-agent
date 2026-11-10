/// `YYYY-MM-DD` with a month 1..=12 and a day 1..=31.
fn check_date(text: &str) -> Result<(), JevError> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .filter(|part| part.iter().all(u8::is_ascii_digit))
            .and_then(|part| std::str::from_utf8(part).ok())
            .and_then(|part| part.parse::<u32>().ok())
    };
    let ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && digits(0..4).is_some()
        && digits(5..7).is_some_and(|month| (1..=12).contains(&month))
        && digits(8..10).is_some_and(|day| (1..=31).contains(&day));
    if ok {
        Ok(())
    } else {
        Err(JevError::config_invalid(format!(
            "models file: released {text:?} is not a YYYY-MM-DD date"
        )))
    }
}
