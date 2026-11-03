//! `sc-ai` binary. JSON on stdout, typed errors on stderr, exit 2 on failure.

#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::process::ExitCode;

use mimalloc::MiMalloc;
use sc_agent_types::{JevError, ScAgentErrorResponse};
use sc_ai::{ExecuteOverrides, execute, reads_stdin};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdin = if reads_stdin(&args) {
        let mut buf = Vec::new();
        if let Err(error) = io::stdin().read_to_end(&mut buf) {
            emit(&JevError::InputInvalid {
                detail: format!("stdin could not be read ({error})"),
            });
            return ExitCode::from(2);
        }
        buf
    } else {
        Vec::new()
    };
    match execute(&args, &stdin, ExecuteOverrides::default()) {
        Ok(bytes) => {
            if let Err(error) = io::stdout().write_all(&bytes) {
                emit(&JevError::InputInvalid {
                    detail: format!("stdout write failed ({error})"),
                });
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            emit(&error);
            ExitCode::from(2)
        }
    }
}

fn emit(error: &JevError) {
    let body = ScAgentErrorResponse::from_jev_error(error);
    let line = serde_json::to_string(&body).unwrap_or_else(|_| {
        serde_json::to_string(&ScAgentErrorResponse::from_jev_error(
            &JevError::InputInvalid {
                detail: "stderr encoding failed".into(),
            },
        ))
        .expect("ScAgentErrorResponse from JevError must serialize")
    });
    let _ = writeln!(io::stderr(), "{line}");
}
