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
    pub(crate) fn read(stream: &UnixStream) -> Result<Self, IpcError> {
        // Linux can return pid=0 for a peer outside this PID namespace. Read
        // raw integer credentials; do not construct a nonzero PID prematurely.
        let credentials =
            nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
                .map_err(|_| IpcError::Io)?;
        Self::checked(credentials.pid(), credentials.uid(), credentials.gid())
    }

    fn checked(pid: i32, uid: u32, gid: u32) -> Result<Self, IpcError> {
        let pid = u32::try_from(pid)
            .ok()
            .filter(|value| *value > 0)
            .ok_or(IpcError::PermissionDenied)?;
        Ok(Self { uid, gid, pid })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_kernel_zero_or_negative_pid_is_rejected_before_peer_construction() {
        for pid in [0, -1, i32::MIN] {
            assert_eq!(
                PeerCredentials::checked(pid, 1000, 1000),
                Err(IpcError::PermissionDenied)
            );
        }
        let peer = PeerCredentials::checked(i32::MAX, 1000, 1001).unwrap();
        assert_eq!(peer.pid(), i32::MAX.cast_unsigned());
        assert_eq!(peer.uid(), 1000);
        assert_eq!(peer.gid(), 1001);
    }
}
