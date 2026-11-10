fn print_stdout(bytes: &[u8]) -> ExitCode {
    match io::stdout().write_all(bytes) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail(&JevError::io_failed("stdout", &error)),
    }
}
/// One `ScAgentErrorResponse` line on stderr (NFR-SAG-0001).
fn emit(body: &ScAgentErrorResponse) {
    let line = serde_json::to_string(body).unwrap_or_else(|_| {
        serde_json::to_string(&ScAgentErrorResponse::from_jev_error(
            &JevError::io_failed_with(
                "stderr (the error text could not be encoded as JSON)",
                io::ErrorKind::InvalidData,
                Some("Retry the command.".to_owned()),
            ),
        ))
        .expect("ScAgentErrorResponse from JevError must serialize")
    });
    // Best effort: a closed stderr leaves nowhere to report the failure.
    let _ = writeln!(io::stderr(), "{line}");
}
