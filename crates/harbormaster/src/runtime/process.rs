use crate::manager::ManagerError;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, os::fd::OwnedFd, os::unix::fs::MetadataExt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}

impl ProcessIdentity {
    pub(crate) fn read(pid: u32) -> Result<Self, ManagerError> {
        if pid == 0
            || std::fs::metadata(format!("/proc/{pid}"))?.uid()
                != rustix::process::geteuid().as_raw()
        {
            return Err(ManagerError::RuntimeUnavailable);
        }
        let stat = read_bounded(&format!("/proc/{pid}/stat"), 4096)?;
        let (_, suffix) = stat
            .rsplit_once(')')
            .ok_or(ManagerError::RuntimeUnavailable)?;
        let fields = suffix.split_whitespace().collect::<Vec<_>>();
        if matches!(fields.first(), Some(&"Z" | &"X" | &"x") | None) {
            return Err(ManagerError::RuntimeUnavailable);
        }
        let start_ticks = fields
            .get(19)
            .and_then(|field| field.parse().ok())
            .ok_or(ManagerError::RuntimeUnavailable)?;
        let boot_id = read_bounded("/proc/sys/kernel/random/boot_id", 64)?
            .trim()
            .to_owned();
        let _: crate::protocol::RunId = boot_id
            .parse()
            .map_err(|_| ManagerError::RuntimeUnavailable)?;
        Ok(Self {
            pid,
            start_ticks,
            boot_id,
        })
    }

    pub(crate) fn current(&self) -> bool {
        Self::read(self.pid).is_ok_and(|actual| actual == *self)
    }

    pub(crate) fn validate(&self) -> Result<(), ManagerError> {
        if self.pid == 0
            || self.start_ticks == 0
            || self.boot_id.parse::<crate::protocol::RunId>().is_err()
        {
            Err(ManagerError::InvalidRequest)
        } else {
            Ok(())
        }
    }

    pub(crate) fn pin(&self) -> Result<HeldProcess, ManagerError> {
        self.validate()?;
        if !self.current() {
            return Err(ManagerError::OwnershipUnverified);
        }
        let raw = i32::try_from(self.pid).map_err(|_| ManagerError::OwnershipUnverified)?;
        let pid = rustix::process::Pid::from_raw(raw).ok_or(ManagerError::OwnershipUnverified)?;
        let fd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::NONBLOCK)
            .map_err(|_| ManagerError::ActionUnavailable)?;
        if !self.current() {
            return Err(ManagerError::OwnershipUnverified);
        }
        Ok(HeldProcess { fd })
    }

    pub(crate) fn gone(&self) -> bool {
        if let Ok(actual) = Self::read(self.pid) {
            return actual != *self;
        }
        matches!(std::fs::metadata(format!("/proc/{}", self.pid)), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    }
}

pub(crate) struct HeldProcess {
    fd: OwnedFd,
}
impl HeldProcess {
    pub(crate) fn terminate(&self) -> Result<(), ManagerError> {
        if self.exited(0)? {
            return Err(ManagerError::OwnershipUnverified);
        }
        rustix::process::pidfd_send_signal(&self.fd, rustix::process::Signal::TERM)
            .map_err(|_| ManagerError::UnknownOutcome)
    }

    pub(crate) fn exited(&self, seconds: i64) -> Result<bool, ManagerError> {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        let mut fds = [PollFd::new(&self.fd, PollFlags::IN)];
        poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: seconds,
                tv_nsec: 0,
            }),
        )
        .map_err(|_| ManagerError::UnknownOutcome)?;
        Ok(fds[0].revents().contains(PollFlags::IN))
    }
}

pub(super) fn read_bounded(path: &str, limit: u64) -> Result<String, ManagerError> {
    let mut text = String::new();
    File::open(path)?
        .take(limit + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > limit {
        return Err(ManagerError::RuntimeUnavailable);
    }
    Ok(text)
}
