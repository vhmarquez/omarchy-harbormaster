//! Narrow compositor metadata boundary. Unselected fields are discarded on decode.
use super::{Backend, command, control, private_directory};
use crate::manager::ManagerError;
use serde::Deserialize;
use std::{
    os::unix::fs::{FileTypeExt, MetadataExt},
    process::Command,
};

pub(super) struct Desktop {
    pub display: String,
    pub compositor: String,
}
#[derive(Debug, Deserialize)]
pub(super) struct Window {
    pub address: String,
    pub pid: u32,
    #[serde(rename = "class")]
    pub app_id: String,
}
impl Desktop {
    pub(super) fn new(backend: &Backend) -> Result<Self, ManagerError> {
        let display =
            std::env::var("WAYLAND_DISPLAY").map_err(|_| ManagerError::ActionUnavailable)?;
        let compositor = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .map_err(|_| ManagerError::ActionUnavailable)?;
        if !control::component(&display) || !control::component(&compositor) {
            return Err(ManagerError::ActionUnavailable);
        }
        let root = backend
            .session_runtime
            .as_ref()
            .ok_or(ManagerError::ActionUnavailable)?;
        private_directory(root, false)?;
        let socket = root.join(&display).symlink_metadata()?;
        if !socket.file_type().is_socket() || socket.uid() != rustix::process::geteuid().as_raw() {
            return Err(ManagerError::ActionUnavailable);
        }
        for tool in ["/usr/bin/foot", "/usr/bin/hyprctl"] {
            trusted_tool(tool)?;
        }
        Ok(Self {
            display,
            compositor,
        })
    }

    pub(super) fn command(&self, backend: &Backend) -> Command {
        let mut command = backend.command("/usr/bin/hyprctl");
        command.env("HYPRLAND_INSTANCE_SIGNATURE", &self.compositor);
        command
    }

    pub(super) fn windows(&self, backend: &Backend) -> Result<Vec<Window>, ManagerError> {
        let mut query = self.command(backend);
        query.args(["-j", "clients"]);
        let text = command::compositor(query)?;
        let windows: Vec<Window> =
            serde_json::from_str(&text).map_err(|_| ManagerError::ActionUnavailable)?;
        if windows.len() > 128 {
            return Err(ManagerError::ActionUnavailable);
        }
        Ok(windows)
    }

    pub(super) fn active(&self, backend: &Backend) -> Result<Window, ManagerError> {
        let mut query = self.command(backend);
        query.args(["-j", "activewindow"]);
        serde_json::from_str(&command::compositor(query)?)
            .map_err(|_| ManagerError::OwnershipUnverified)
    }
}

pub(super) fn trusted_tool(path: &str) -> Result<(), ManagerError> {
    let info = std::fs::metadata(path)?;
    if !info.is_file() || info.uid() != 0 || info.mode() & 0o6022 != 0 || info.mode() & 0o111 == 0 {
        return Err(ManagerError::ActionUnavailable);
    }
    Ok(())
}
