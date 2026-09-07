use crate::manager::ManagerError;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_ticks: u64,
    pub boot_id: String,
}

impl ProcessIdentity {
    pub(crate) fn read(pid: u32) -> Result<Self, ManagerError> {
        if pid == 0 {
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
