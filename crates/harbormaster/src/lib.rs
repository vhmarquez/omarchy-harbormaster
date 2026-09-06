//! Pure argument interpretation for the CLI foundation; no runtime operations.

use std::ffi::OsString;
use std::fmt;

pub mod ingestion;

/// The foundation's supported output requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Help,
    Version,
}

/// Usage errors contain no caller-supplied argument values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliError {
    UnsupportedArgument,
    TooManyArguments,
    InvalidUnicode,
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedArgument => formatter.write_str("unsupported argument; use --help"),
            Self::TooManyArguments => {
                formatter.write_str("expected at most one argument; use --help")
            }
            Self::InvalidUnicode => {
                formatter.write_str("arguments must be valid Unicode; use --help")
            }
        }
    }
}

impl std::error::Error for CliError {}

/// Static help describes only the currently available CLI surface.
pub const HELP: &str = "Harbormaster CLI foundation\n\n\
Usage: harbormaster [--help | --version]\n\n\
Options:\n  -h, --help     Show this help\n  -V, --version  Show package version\n\n\
No arguments show help. Runtime modes are not implemented.\n";

/// Interpret arguments excluding the executable name, without I/O.
///
/// # Errors
/// Returns a value-free usage error for excess, non-Unicode, or unsupported
/// arguments. Arity is checked before encoding.
pub fn interpret(args: &[OsString]) -> Result<Action, CliError> {
    if args.len() > 1 {
        return Err(CliError::TooManyArguments);
    }
    if args.is_empty() {
        return Ok(Action::Help);
    }
    match args.first().and_then(|arg| arg.to_str()) {
        Some("--help" | "-h") => Ok(Action::Help),
        Some("--version" | "-V") => Ok(Action::Version),
        Some(_) => Err(CliError::UnsupportedArgument),
        None => Err(CliError::InvalidUnicode),
    }
}

#[cfg(test)]
mod tests;

pub mod ipc;
