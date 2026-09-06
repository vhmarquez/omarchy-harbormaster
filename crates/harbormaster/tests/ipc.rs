//! Disposable IPC fixtures; this binary is run only in the verifier sandbox.
use harbormaster::ipc::{
    Channel, ConnectionBudget, FrameDecoder, IpcError, OutboundQueue, PrivateSockets,
};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        assert_eq!(std::env::var("XDG_RUNTIME_DIR").unwrap(), "/state/runtime");
        let name = format!(
            "ipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = PathBuf::from("/state/runtime").join(name);
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }

    fn path(&self, channel: Channel) -> PathBuf {
        self.0.join("harbormaster").join(match channel {
            Channel::Event => "events.sock",
            Channel::Control => "control.sock",
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(3)
}

#[test]
fn framing_is_incremental_bounded_and_poisoned_after_failure() {
    let mut decoder = FrameDecoder::default();
    assert_eq!(decoder.feed(b"{\"x\":"), Ok((5, None)));
    assert_eq!(
        decoder.feed(b"1}\n{}\n"),
        Ok((3, Some(b"{\"x\":1}\n".to_vec())))
    );
    let mut max = vec![b' '; 16_383];
    max.push(b'\n');
    assert_eq!(decoder.feed(&max).unwrap().1.unwrap().len(), 16_384);
    assert_eq!(
        decoder.feed(&vec![b' '; 16_384]),
        Err(IpcError::FrameTooLarge)
    );
    assert_eq!(decoder.feed(b"{}\n"), Err(IpcError::Closed));
}

#[test]
fn malformed_encoding_and_eof_reject_buffered_frame() {
    let mut decoder = FrameDecoder::default();
    assert_eq!(decoder.feed(b"\xff\n"), Err(IpcError::InvalidFrame));
    let mut decoder = FrameDecoder::default();
    decoder.feed(b"{}").unwrap();
    assert_eq!(decoder.finish(), Err(IpcError::TruncatedFrame));
    assert_eq!(decoder.feed(b"\n"), Err(IpcError::Closed));
}

#[test]
fn outbound_enforces_both_caps_before_copying() {
    let mut frames = OutboundQueue::default();
    for _ in 0..256 {
        frames.push(b"{}\n").unwrap();
    }
    assert_eq!(frames.push(b"{}\n"), Err(IpcError::ResourceExhausted));
    assert_eq!(frames.frames(), 256);
    let mut bytes = OutboundQueue::default();
    let mut frame = vec![b' '; 16_383];
    frame.push(b'\n');
    for _ in 0..64 {
        bytes.push(&frame).unwrap();
    }
    assert_eq!(bytes.bytes(), 1_048_576);
    assert_eq!(bytes.push(b"{}\n"), Err(IpcError::ResourceExhausted));
    assert_eq!(bytes.frames(), 64);
    assert_eq!(
        OutboundQueue::default().push(b"{}\n{}\n"),
        Err(IpcError::InvalidFrame)
    );
}

#[test]
fn private_socket_paths_are_owned_and_channels_are_listener_evidence() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(2).unwrap();
    for channel in [Channel::Event, Channel::Control] {
        let _client = UnixStream::connect(fixture.path(channel)).unwrap();
        let connection = sockets
            .accept(channel, &budget, deadline())
            .unwrap()
            .unwrap();
        assert_eq!(connection.channel(), channel);
        assert_eq!(connection.peer().pid(), std::process::id());
        assert!(
            connection
                .peer()
                .require_uid(connection.peer().uid())
                .is_ok()
        );
        // Predicate rejection is distinct from connecting as another OS user.
        assert_eq!(
            connection
                .peer()
                .require_uid(connection.peer().uid().wrapping_add(1)),
            Err(IpcError::PermissionDenied)
        );
        assert_eq!(
            fs::metadata(fixture.path(channel))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    drop(sockets);
    assert!(!fixture.path(Channel::Event).exists());
    assert!(!fixture.path(Channel::Control).exists());
}

#[test]
fn unsafe_runtime_and_symlink_ancestors_are_rejected() {
    let fixture = Fixture::new();
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(PrivateSockets::bind(&fixture.0).is_err());
    fs::set_permissions(&fixture.0, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&fixture.0, fixture.0.join("alias")).unwrap();
    assert!(PrivateSockets::bind(&fixture.0.join("alias")).is_err());
    assert!(PrivateSockets::bind(&fixture.0.join("alias/..")).is_err());
    fs::write(fixture.0.join("harbormaster"), b"unrelated").unwrap();
    assert!(PrivateSockets::bind(&fixture.0).is_err());
    assert_eq!(
        fs::read(fixture.0.join("harbormaster")).unwrap(),
        b"unrelated"
    );
}

#[test]
fn bind_failure_and_cleanup_never_delete_existing_or_replaced_files() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    assert!(PrivateSockets::bind(&fixture.0).is_err());
    fs::remove_file(fixture.path(Channel::Event)).unwrap();
    fs::write(fixture.path(Channel::Event), b"replacement").unwrap();
    drop(sockets);
    assert_eq!(
        fs::read(fixture.path(Channel::Event)).unwrap(),
        b"replacement"
    );
    assert!(!fixture.path(Channel::Control).exists());
}

#[test]
fn held_directory_survives_rename_without_touching_replacement_directory() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    fs::rename(fixture.0.join("harbormaster"), fixture.0.join("old")).unwrap();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(fixture.0.join("harbormaster"))
        .unwrap();
    fs::write(fixture.path(Channel::Event), b"unrelated").unwrap();
    drop(sockets);
    assert!(!fixture.0.join("old/events.sock").exists());
    assert_eq!(
        fs::read(fixture.path(Channel::Event)).unwrap(),
        b"unrelated"
    );
}

#[test]
fn nonblocking_io_preserves_fragmented_frames_and_separate_consumers() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(2).unwrap();
    let mut first = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    let mut a = sockets
        .accept(Channel::Event, &budget, deadline())
        .unwrap()
        .unwrap();
    assert_eq!(a.read_frame(Instant::now()).unwrap(), None);
    first.write_all(b"{\"a\":").unwrap();
    assert_eq!(a.read_frame(Instant::now()).unwrap(), None);
    first.write_all(b"1}\n{}\n").unwrap();
    assert_eq!(
        a.read_frame(Instant::now()).unwrap(),
        Some(b"{\"a\":1}\n".to_vec())
    );
    assert_eq!(
        a.read_frame(Instant::now()).unwrap(),
        Some(b"{}\n".to_vec())
    );
    let mut second = UnixStream::connect(fixture.path(Channel::Control)).unwrap();
    let mut b = sockets
        .accept(Channel::Control, &budget, deadline())
        .unwrap()
        .unwrap();
    for _ in 0..256 {
        a.queue_frame(b"{}\n").unwrap();
    }
    assert_eq!(a.queue_frame(b"{}\n"), Err(IpcError::ResourceExhausted));
    b.queue_frame(b"{\"ok\":true}\n").unwrap();
    assert!(b.flush(Instant::now()).unwrap());
    let mut response = [0; 12];
    second.read_exact(&mut response).unwrap();
    assert_eq!(&response, b"{\"ok\":true}\n");
}

#[test]
fn connection_cap_releases_on_drop_and_deadline_closes_connection() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(1).unwrap();
    assert!(ConnectionBudget::new(0).is_err());
    assert!(ConnectionBudget::new(65).is_err());
    let _first = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    let until = deadline();
    let mut a = sockets
        .accept(Channel::Event, &budget, until)
        .unwrap()
        .unwrap();
    let _second = UnixStream::connect(fixture.path(Channel::Control)).unwrap();
    assert!(matches!(
        sockets.accept(Channel::Control, &budget, deadline()),
        Err(IpcError::ResourceExhausted)
    ));
    assert_eq!(a.read_frame(until), Err(IpcError::DeadlineExceeded));
    assert_eq!(a.read_frame(Instant::now()), Err(IpcError::Closed));
    drop(a);
    assert_eq!(budget.active(), 0);
    let _third = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    assert!(
        sockets
            .accept(Channel::Event, &budget, deadline())
            .unwrap()
            .is_some()
    );
}

#[test]
fn cross_process_peer_credentials_are_from_the_kernel() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let mut child = Command::new("/usr/bin/python3")
        .args(["-c", "import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); print('ready',flush=True); sys.stdin.read()"])
        .arg(fixture.path(Channel::Event))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut ready = [0; 6];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut ready)
        .unwrap();
    assert_eq!(&ready, b"ready\n");
    let budget = ConnectionBudget::new(1).unwrap();
    let peer = sockets
        .accept(Channel::Event, &budget, deadline())
        .unwrap()
        .unwrap()
        .peer();
    assert_eq!(peer.pid(), child.id());
    assert_ne!(peer.pid(), std::process::id());
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}

#[test]
fn queue_overflow_disconnects_so_a_client_must_resynchronize() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(1).unwrap();
    let _client = UnixStream::connect(fixture.path(Channel::Control)).unwrap();
    let mut connection = sockets
        .accept(Channel::Control, &budget, deadline())
        .unwrap()
        .unwrap();
    connection.complete_handshake(Instant::now()).unwrap();
    for _ in 0..256 {
        connection.queue_frame(b"{}\n").unwrap();
    }
    assert_eq!(
        connection.queue_frame(b"{}\n"),
        Err(IpcError::ResourceExhausted)
    );
    assert_eq!(connection.flush(Instant::now()), Err(IpcError::Closed));
}

#[test]
fn partial_frame_deadline_does_not_extend_with_dripped_bytes() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(1).unwrap();
    let mut client = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    let mut connection = sockets
        .accept(Channel::Event, &budget, deadline())
        .unwrap()
        .unwrap();
    let started = Instant::now();
    connection.complete_handshake(started).unwrap();
    client.write_all(b"{").unwrap();
    assert_eq!(connection.read_frame(started), Ok(None));
    client.write_all(b" ").unwrap();
    assert_eq!(
        connection.read_frame(started + Duration::from_millis(50)),
        Ok(None)
    );
    assert_eq!(
        connection.read_frame(started + Duration::from_millis(100)),
        Err(IpcError::DeadlineExceeded)
    );
}

#[test]
fn authenticated_slow_writer_expires_while_another_client_stays_responsive() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let budget = ConnectionBudget::new(2).unwrap();
    let _slow = UnixStream::connect(fixture.path(Channel::Control)).unwrap();
    let mut slow = sockets
        .accept(Channel::Control, &budget, deadline())
        .unwrap()
        .unwrap();
    slow.complete_handshake(Instant::now()).unwrap();
    let mut frame = vec![b' '; 16_383];
    frame.push(b'\n');
    for _ in 0..64 {
        slow.queue_frame(&frame).unwrap();
    }
    // Real nonblocking writes eventually encounter the unread kernel socket.
    for _ in 0..128 {
        assert!(!slow.flush(Instant::now()).unwrap());
    }
    let mut healthy = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    let mut connection = sockets
        .accept(Channel::Event, &budget, deadline())
        .unwrap()
        .unwrap();
    connection.queue_frame(b"{}\n").unwrap();
    assert!(connection.flush(Instant::now()).unwrap());
    let mut response = [0; 3];
    healthy.read_exact(&mut response).unwrap();
    assert_eq!(&response, b"{}\n");
    assert_eq!(
        slow.flush(Instant::now() + Duration::from_millis(100)),
        Err(IpcError::DeadlineExceeded)
    );
}

#[test]
fn manager_connection_cap_cannot_be_bypassed_with_new_caller_budgets() {
    let fixture = Fixture::new();
    let sockets = PrivateSockets::bind(&fixture.0).unwrap();
    let mut clients = Vec::new();
    let mut accepted = Vec::new();
    for index in 0..64 {
        let channel = if index % 2 == 0 {
            Channel::Event
        } else {
            Channel::Control
        };
        clients.push(UnixStream::connect(fixture.path(channel)).unwrap());
        accepted.push(
            sockets
                .accept(channel, &ConnectionBudget::new(1).unwrap(), deadline())
                .unwrap()
                .unwrap(),
        );
    }
    let _extra = UnixStream::connect(fixture.path(Channel::Event)).unwrap();
    assert!(matches!(
        sockets.accept(
            Channel::Event,
            &ConnectionBudget::new(1).unwrap(),
            deadline()
        ),
        Err(IpcError::ResourceExhausted)
    ));
}

#[test]
fn second_bind_failure_rolls_back_only_the_socket_created_by_this_call() {
    let fixture = Fixture::new();
    fs::DirBuilder::new()
        .mode(0o700)
        .create(fixture.0.join("harbormaster"))
        .unwrap();
    fs::write(fixture.path(Channel::Control), b"preexisting").unwrap();
    assert!(PrivateSockets::bind(&fixture.0).is_err());
    assert!(!fixture.path(Channel::Event).exists());
    assert_eq!(
        fs::read(fixture.path(Channel::Control)).unwrap(),
        b"preexisting"
    );
}

#[test]
fn file_descriptor_exhaustion_cleans_only_created_socket_names() {
    const CHILD: &str = "HARBORMASTER_IPC_FD_LIMIT_FIXTURE";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "file_descriptor_exhaustion_cleans_only_created_socket_names",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    check_cleanup_under_descriptor_pressure();
}

fn check_cleanup_under_descriptor_pressure() {
    // Process-wide RLIMIT belongs only to this disposable child, never parallel tests.
    let old = rustix::process::getrlimit(rustix::process::Resource::Nofile);
    rustix::process::setrlimit(
        rustix::process::Resource::Nofile,
        rustix::process::Rlimit {
            current: Some(32),
            maximum: old.maximum,
        },
    )
    .unwrap();
    let mut leftovers = Vec::new();
    for free in 1..8 {
        let fixture = Fixture::new();
        let mut held = Vec::new();
        while let Ok(file) = fs::File::open("/dev/null") {
            held.push(file);
        }
        for _ in 0..free {
            held.pop();
        }
        let result = PrivateSockets::bind(&fixture.0);
        let failed = result.is_err();
        drop(result);
        drop(held);
        for channel in [Channel::Event, Channel::Control] {
            if failed && fixture.path(channel).exists() {
                leftovers.push((free, channel));
            }
        }
    }
    rustix::process::setrlimit(rustix::process::Resource::Nofile, old).unwrap();
    assert!(
        leftovers.is_empty(),
        "owned socket remains after bind failure: {leftovers:?}"
    );
}
