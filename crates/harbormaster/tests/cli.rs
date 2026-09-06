#[path = "cli/projects.rs"]
mod projects;
mod support;

#[cfg(target_os = "linux")]
#[test]
fn failed_stdout_is_reported_without_os_error_details() {
    use std::fs::OpenOptions;
    use std::process::Stdio;

    let fixture = support::Fixture::new();
    for arg in ["--help", "--version"] {
        let full = OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("Linux full device");
        let output = fixture.run_with_output(&[arg], full.into(), Stdio::piped());
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"harbormaster: could not write output\n");
    }
}

#[cfg(target_os = "linux")]
#[test]
fn failed_stderr_returns_an_output_failure_status() {
    use std::fs::OpenOptions;
    use std::process::Stdio;

    let full = OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("Linux full device");
    let output = support::Fixture::new().run_with_output(&["private"], Stdio::piped(), full.into());
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn non_unicode_arguments_get_an_encoding_error_without_echo() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let output = support::Fixture::new().run(&[OsString::from_vec(b"private-\xff".to_vec())]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"harbormaster: arguments must be valid Unicode; use --help\n"
    );
}

#[cfg(unix)]
#[test]
fn argument_count_takes_precedence_over_invalid_encoding() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let fixture = support::Fixture::new();
    let invalid = OsString::from_vec(vec![0xff]);
    for args in [
        [OsString::from("--help"), invalid.clone()],
        [invalid, OsString::from("--version")],
    ] {
        let output = fixture.run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"harbormaster: expected at most one argument; use --help\n"
        );
    }
}

#[test]
fn excess_arguments_never_short_circuit_to_help_or_version() {
    let fixture = support::Fixture::new();
    for args in [
        &["--help", "synthetic-private-value"][..],
        &["-h", "--version"][..],
        &["--version", "--help"][..],
        &["-V", "-V"][..],
        &["daemon", "--help"][..],
        &["--help", "private", "third"][..],
    ] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"harbormaster: expected at most one argument; use --help\n"
        );
    }
}

#[test]
fn unsupported_arguments_get_a_fixed_private_diagnostic() {
    let fixture = support::Fixture::new();
    for arg in [
        "daemon",
        "bridge",
        "help",
        "version",
        "--",
        "",
        "-v",
        "--help=synthetic-private-value",
        "/synthetic/private/path\n\u{1b}[31m",
    ] {
        let output = fixture.run(&[arg]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"harbormaster: unsupported argument; use --help\n"
        );
    }
}

#[test]
fn version_flags_report_the_cargo_package_version() {
    let fixture = support::Fixture::new();
    for arg in ["--version", "-V"] {
        let output = fixture.run(&[arg]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            output.stdout,
            format!("harbormaster {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn short_help_and_no_arguments_match_long_help() {
    let fixture = support::Fixture::new();
    let expected = fixture.run(&["--help"]);
    for args in [&["-h"][..], &[][..]] {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, expected.stdout);
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn help_describes_available_commands_without_starting_a_mode() {
    let output = support::Fixture::new().run(&["--help"]);

    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 help");
    for command in ["project add", "preset add", "task add", "task plan"] {
        assert!(text.contains(command));
    }
    assert!(text.contains("Task plans do not execute Hermes"));
    assert!(output.stderr.is_empty());
}
