use std::io::{self, Write};
use std::process::ExitCode;

use harbormaster::{Action, HELP, interpret};

fn main() -> ExitCode {
    // Seeing a second argument is sufficient to reject any excess arguments.
    let args: Vec<_> = std::env::args_os().skip(1).take(2).collect();
    let text = match interpret(&args) {
        Ok(Action::Help) => HELP,
        Ok(Action::Version) => concat!("harbormaster ", env!("CARGO_PKG_VERSION"), "\n"),
        Err(error) => {
            return if writeln!(io::stderr().lock(), "harbormaster: {error}").is_ok() {
                ExitCode::from(2)
            } else {
                ExitCode::FAILURE
            };
        }
    };
    if io::stdout().lock().write_all(text.as_bytes()).is_ok() {
        ExitCode::SUCCESS
    } else {
        let _ = writeln!(io::stderr().lock(), "harbormaster: could not write output");
        ExitCode::FAILURE
    }
}
