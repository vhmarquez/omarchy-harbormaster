use super::{Command, ManagerError, Reply, Request};
use crate::{
    ipc::{FrameDecoder, PeerCredentials},
    projects::CatalogRequest,
    protocol::{RequestId, Revision, encode_frame, strict},
};
use std::{
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, MetadataExt},
            net::UnixStream,
        },
    },
    path::Path,
    time::Duration,
};

pub(crate) fn catalog(request: CatalogRequest) -> Result<Option<Vec<u8>>, ManagerError> {
    if std::env::var_os("XDG_RUNTIME_DIR").is_none()
        && std::env::var_os("HARBORMASTER_RUNTIME_DIR").is_none()
    {
        return Ok(None);
    }
    let root = super::runtime_root()?;
    let Some(connection) = connect(&root)? else {
        return Ok(None);
    };
    let command = Command::Catalog { request };
    let reply = if command.stateful() {
        let revision = revision(&send(connection, Command::Status, None)?)?;
        send(
            connect(&root)?.ok_or(ManagerError::Unavailable)?,
            command,
            Some(revision),
        )?
    } else {
        send(connection, command, None)?
    };
    output(reply).map(Some)
}

pub(crate) fn execute(command: Command) -> Result<Vec<u8>, ManagerError> {
    let root = super::runtime_root()?;
    let connection = connect(&root)?.ok_or(ManagerError::Unavailable)?;
    let reply = if command.stateful() {
        let revision = revision(&send(connection, Command::Status, None)?)?;
        send(
            connect(&root)?.ok_or(ManagerError::Unavailable)?,
            command,
            Some(revision),
        )?
    } else {
        send(connection, command, None)?
    };
    output(reply)
}

fn revision(reply: &Reply) -> Result<Revision, ManagerError> {
    match reply {
        Reply::Ok { revision, .. } => Ok(*revision),
        Reply::Error { error, .. } => Err(*error),
    }
}

fn output(reply: Reply) -> Result<Vec<u8>, ManagerError> {
    match reply {
        Reply::Ok { value, .. } => {
            let mut bytes =
                serde_json::to_vec_pretty(&value).map_err(|_| ManagerError::InvalidRequest)?;
            bytes.push(b'\n');
            Ok(bytes)
        }
        Reply::Error { error, .. } => Err(error),
    }
}

fn connect(root: &Path) -> Result<Option<UnixStream>, ManagerError> {
    let directory = root.join("harbormaster");
    if !directory.try_exists()? {
        return Ok(None);
    }
    let fd = crate::runtime::private_directory(&directory, false)?;
    let path = format!("/proc/self/fd/{}/control.sock", fd.as_raw_fd());
    let stat = match std::fs::symlink_metadata(&path) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(ManagerError::UnsafePath),
    };
    if !stat.file_type().is_socket()
        || stat.mode() & 0o7777 != 0o600
        || stat.uid() != rustix::process::geteuid().as_raw()
    {
        return Err(ManagerError::UnsafePath);
    }
    let stream = UnixStream::connect(path).map_err(|_| ManagerError::Unavailable)?;
    PeerCredentials::read(&stream)?.require_uid(rustix::process::geteuid().as_raw())?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_millis(100)))?;
    Ok(Some(stream))
}

fn send(
    mut stream: UnixStream,
    command: Command,
    expected_revision: Option<Revision>,
) -> Result<Reply, ManagerError> {
    let request_id: RequestId = crate::generation::fresh()
        .map_err(|_| ManagerError::Unavailable)?
        .as_str()
        .parse()
        .map_err(|_| ManagerError::InvalidRequest)?;
    let request = Request {
        protocol: 0,
        channel: "control".to_owned(),
        request_id: request_id.clone(),
        expected_revision,
        command,
    };
    let frame = encode_frame(&request).map_err(|_| ManagerError::InvalidRequest)?;
    stream
        .write_all(&frame)
        .map_err(|_| ManagerError::UnknownOutcome)?;
    let mut decoder = FrameDecoder::default();
    let mut bytes = [0_u8; 4096];
    loop {
        let count = stream
            .read(&mut bytes)
            .map_err(|_| ManagerError::UnknownOutcome)?;
        if count == 0 {
            return Err(ManagerError::UnknownOutcome);
        }
        let (_, frame) = decoder.feed(&bytes[..count])?;
        if let Some(frame) = frame {
            let reply: Reply =
                strict::typed(strict::decode(&frame).map_err(|_| ManagerError::UnknownOutcome)?)
                    .map_err(|_| ManagerError::UnknownOutcome)?;
            let (protocol, id) = match &reply {
                Reply::Ok {
                    protocol,
                    request_id,
                    ..
                }
                | Reply::Error {
                    protocol,
                    request_id,
                    ..
                } => (*protocol, request_id),
            };
            if protocol != 0 || *id != request_id {
                return Err(ManagerError::UnknownOutcome);
            }
            return Ok(reply);
        }
    }
}
