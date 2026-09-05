use std::ffi::OsString;

use super::{Action, CliError, interpret};

#[cfg(unix)]
#[test]
fn non_unicode_is_a_typed_value_free_error() {
    use std::os::unix::ffi::OsStringExt;

    let invalid = OsString::from_vec(b"private-\xff".to_vec());
    assert_eq!(interpret(&[invalid]), Err(CliError::InvalidUnicode));
}

#[cfg(unix)]
#[test]
fn excess_arity_is_checked_before_encoding() {
    use std::os::unix::ffi::OsStringExt;

    let invalid = OsString::from_vec(vec![0xff]);
    assert_eq!(
        interpret(&[invalid, OsString::from("--help")]),
        Err(CliError::TooManyArguments)
    );
}

#[test]
fn excess_arguments_have_a_typed_error() {
    for first in ["--help", "-h", "--version", "-V", "daemon", ""] {
        assert_eq!(
            interpret(&[OsString::from(first), OsString::from("private")]),
            Err(CliError::TooManyArguments)
        );
    }
}

#[test]
fn unsupported_modes_and_values_have_no_runtime_action() {
    for arg in [
        "daemon",
        "bridge",
        "help",
        "version",
        "",
        "--",
        "private\nvalue",
    ] {
        assert_eq!(
            interpret(&[OsString::from(arg)]),
            Err(CliError::UnsupportedArgument)
        );
    }
}

#[test]
fn version_aliases_are_pure_version_requests() {
    for arg in ["--version", "-V"] {
        assert_eq!(interpret(&[OsString::from(arg)]), Ok(Action::Version));
    }
}

#[test]
fn no_arguments_request_help_without_a_default_mode() {
    assert_eq!(interpret(&[]), Ok(Action::Help));
}

#[test]
fn short_help_requests_help() {
    assert_eq!(interpret(&[OsString::from("-h")]), Ok(Action::Help));
}

#[test]
fn long_help_is_a_pure_help_request() {
    assert_eq!(interpret(&[OsString::from("--help")]), Ok(Action::Help));
}
