/// Outcome of [`parse`].
#[derive(Debug)]
pub(crate) enum Parsed {
    /// Run this command.
    Run(Cli),
    /// Print this text to stdout and exit 0 (`--help`, `--version`).
    Print(String),
}

/// Parses `args` (including the program name, as from [`std::env::args_os`]).
///
/// # Errors
///
/// [`JevError::InputInvalid`] for every clap error other than the help and
/// version displays; a value parser's own [`JevError`] (always
/// `INPUT_INVALID` here) is returned as is.
pub(crate) fn parse<I, T>(args: I) -> Result<Parsed, JevError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => Ok(Parsed::Run(cli)),
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            Ok(Parsed::Print(error.render().to_string()))
        }
        Err(error) => Err(value_parser_error(&error).unwrap_or_else(|| usage_error(&error))),
    }
}
fn non_empty_text(value: &str) -> Result<String, JevError> {
    if value.is_empty() {
        Err(JevError::input_invalid(
            "a flag value must not be empty. See sc-ai jev --help.",
        ))
    } else {
        Ok(value.to_owned())
    }
}

fn non_empty_path(value: &str) -> Result<PathBuf, JevError> {
    non_empty_text(value).map(PathBuf::from)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
