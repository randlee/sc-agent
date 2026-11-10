use std::io::{self, Write};
use std::process::ExitCode;
fn main() -> ExitCode {
    match cli::parse(std::env::args_os()).and_then(run) {
        Ok(bytes) => {
            if let Err(error) = io::stdout().write_all(&bytes) {
                return emit(&JevError::io_failed("stdout", &error));
            }
            ExitCode::SUCCESS
        }
        Err(error) => emit(&error),
    }
}
fn run(parsed: Parsed) -> Result<Vec<u8>, JevError> {
    let cli = match parsed {
        Parsed::Print(text) => return Ok(text.into_bytes()),
        Parsed::Run(cli) => cli,
    };
    match cli.command {
        Command::Languages => {
            let ids: Vec<&str> = languages().map(|info| info.id).collect();
            json_line(&serde_json::json!(ids))
        }
        Command::Queries(args) => {
            let listed = queries(args.language.as_deref())?;
            let value: Vec<serde_json::Value> = listed
                .iter()
                .map(|info| {
                    serde_json::json!({
                        "language": info.language,
                        "query": info.name,
                    })
                })
                .collect();
            json_line(&serde_json::Value::Array(value))
        }
        Command::Extract(args) => run_extract(&args),
    }
}

fn run_extract(args: &ExtractArgs) -> Result<Vec<u8>, JevError> {
    let capture_refs: Option<Vec<&str>> = args
        .captures
        .as_ref()
        .map(|captures| captures.names().collect());
fn json_line(value: &serde_json::Value) -> Result<Vec<u8>, JevError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| JevError::encode_failed("listing JSON", &error))?;
    bytes.push(b'\n');
    Ok(bytes)
}
