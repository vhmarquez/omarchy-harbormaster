//! Bounded metadata-only subprocess boundary. Never return subprocess diagnostics.
use crate::manager::ManagerError;
use std::{
    io::{ErrorKind, Read},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn run(mut command: Command) -> Result<String, ManagerError> {
    bounded(&mut command, 8192)
}

pub(super) fn compositor(mut command: Command) -> Result<String, ManagerError> {
    bounded(&mut command, 262_144)
}

fn bounded(command: &mut Command, limit: usize) -> Result<String, ManagerError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    let result = collect(&mut child, limit);
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn collect(child: &mut std::process::Child, limit: usize) -> Result<String, ManagerError> {
    let mut stdout = child
        .stdout
        .take()
        .ok_or(ManagerError::RuntimeUnavailable)?;
    let flags = rustix::fs::fcntl_getfl(&stdout).map_err(|_| ManagerError::RuntimeUnavailable)?;
    rustix::fs::fcntl_setfl(&stdout, flags | rustix::fs::OFlags::NONBLOCK)
        .map_err(|_| ManagerError::RuntimeUnavailable)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let eof = match stdout.read(&mut buffer) {
            Ok(0) => true,
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                false
            }
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
            {
                false
            }
            Err(_) => return Err(ManagerError::RuntimeUnavailable),
        };
        if bytes.len() > limit || Instant::now() >= deadline {
            return Err(ManagerError::UnknownOutcome);
        }
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(ManagerError::RuntimeUnavailable);
            }
            if eof {
                return String::from_utf8(bytes).map_err(|_| ManagerError::RuntimeUnavailable);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
