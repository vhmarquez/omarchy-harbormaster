use std::io::{self, Write};
use std::process::ExitCode;

use harbormaster::{Action, HELP, cli, interpret};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).take(9).collect();
    let operational = args
        .first()
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| matches!(arg, "project" | "preset" | "task"));
    let output = if operational {
        let request = match cli::parse(&args) {
            Ok(request) => request,
            Err(error) => return failure(&error, 2),
        };
        match cli::execute(request) {
            Ok(output) => output,
            Err(error) => return failure(&error, 1),
        }
    } else {
        match interpret(&args) {
            Ok(Action::Help) => HELP.as_bytes().to_vec(),
            Ok(Action::Version) => concat!("harbormaster ", env!("CARGO_PKG_VERSION"), "\n")
                .as_bytes()
                .to_vec(),
            Err(error) => return failure(&error, 2),
        }
    };
    if io::stdout().lock().write_all(&output).is_ok() {
        ExitCode::SUCCESS
    } else {
        let _ = writeln!(io::stderr().lock(), "harbormaster: could not write output");
        ExitCode::FAILURE
    }
}

fn failure(error: &impl std::fmt::Display, code: u8) -> ExitCode {
    if writeln!(io::stderr().lock(), "harbormaster: {error}").is_ok() {
        ExitCode::from(code)
    } else {
        ExitCode::FAILURE
    }
}
