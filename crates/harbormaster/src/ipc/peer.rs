use super::IpcError;
use std::os::unix::net::UnixStream;

/// Kernel evidence for this connection; PID alone is never target ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerCredentials {
    uid: u32,
    gid: u32,
    pid: u32,
}

impl PeerCredentials {
    pub(super) fn read(stream: &UnixStream) -> Result<Self, IpcError> {
        let credentials = rustix::net::sockopt::socket_peercred(stream)?;
        Ok(Self {
            uid: credentials.uid.as_raw(),
            gid: credentials.gid.as_raw(),
            pid: credentials.pid.as_raw_nonzero().get().cast_unsigned(),
        })
    }

    #[must_use]
    pub fn uid(self) -> u32 {
        self.uid
    }

    #[must_use]
    pub fn gid(self) -> u32 {
        self.gid
    }

    #[must_use]
    pub fn pid(self) -> u32 {
        self.pid
    }

    /// Check only UID equality. The manager must separately authorize operations.
    ///
    /// # Errors
    /// Returns `PermissionDenied` for a different expected UID.
    pub fn require_uid(self, expected: u32) -> Result<(), IpcError> {
        if self.uid == expected {
            Ok(())
        } else {
            Err(IpcError::PermissionDenied)
        }
    }
}
