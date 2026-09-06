use super::{Channel, Connection, ConnectionBudget, IpcError};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, Stat};
use std::io::ErrorKind;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Component, Path};
use std::rc::Rc;
use std::time::Instant;

const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const DIRECTORY_NAME: &str = "harbormaster";

struct Directory {
    parent: OwnedFd,
    fd: OwnedFd,
    identity: Stat,
    created: bool,
    uid: u32,
}

impl Directory {
    fn open(runtime: &Path) -> Result<Rc<Self>, IpcError> {
        let uid = rustix::process::geteuid().as_raw();
        let parent = open_runtime(runtime, uid)?;
        let created = match fs::mkdirat(&parent, DIRECTORY_NAME, Mode::from_raw_mode(0o700)) {
            Ok(()) => true,
            Err(rustix::io::Errno::EXIST) => false,
            Err(_) => return Err(IpcError::UnsafePath),
        };
        let fd = fs::openat(&parent, DIRECTORY_NAME, DIRECTORY_FLAGS, Mode::empty())
            .map_err(|_| IpcError::UnsafePath)?;
        let identity = fs::fstat(&fd)?;
        validate_directory(&identity, uid, true)?;
        Ok(Rc::new(Self {
            parent,
            fd,
            identity,
            created,
            uid,
        }))
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        if self.created && same_entry(&self.parent, DIRECTORY_NAME, &self.identity) {
            // Empty directory removal only; an unrelated new child is retained.
            let _ = fs::unlinkat(&self.parent, DIRECTORY_NAME, AtFlags::REMOVEDIR);
        }
    }
}

fn open_runtime(path: &Path, uid: u32) -> Result<OwnedFd, IpcError> {
    if !path.is_absolute() || path.as_os_str().as_bytes().len() > 4096 {
        return Err(IpcError::UnsafePath);
    }
    let mut directory = fs::open("/", DIRECTORY_FLAGS, Mode::empty())?;
    validate_directory(&fs::fstat(&directory)?, uid, false)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = fs::openat(&directory, name, DIRECTORY_FLAGS, Mode::empty())
                    .map_err(|_| IpcError::UnsafePath)?;
                validate_directory(&fs::fstat(&directory)?, uid, false)?;
            }
            _ => return Err(IpcError::UnsafePath),
        }
    }
    validate_directory(&fs::fstat(&directory)?, uid, true)?;
    Ok(directory)
}

fn validate_directory(stat: &Stat, uid: u32, private: bool) -> Result<(), IpcError> {
    let permissions = stat.st_mode & 0o7777;
    let owned = stat.st_uid == uid;
    let allowed = if private {
        owned && permissions == 0o700
    } else {
        (owned || stat.st_uid == 0) && permissions & 0o022 == 0
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory || !allowed {
        return Err(IpcError::UnsafePath);
    }
    Ok(())
}

fn same_entry(parent: &OwnedFd, name: &str, original: &Stat) -> bool {
    fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).is_ok_and(|stat| {
        stat.st_dev == original.st_dev
            && stat.st_ino == original.st_ino
            && stat.st_uid == original.st_uid
            && stat.st_mode == original.st_mode
    })
}

struct Listener {
    stream: UnixListener,
    node: OwnedFd,
    directory: Rc<Directory>,
    name: &'static str,
    identity: Stat,
}

impl Listener {
    fn bind(directory: Rc<Directory>, name: &'static str) -> Result<Self, IpcError> {
        // Linux lacks bindat. The held fd anchors resolution without chdir.
        // The private directory prevents cross-UID pathname replacement.
        let path = format!("/proc/self/fd/{}/{name}", directory.fd.as_raw_fd());
        let stream = UnixListener::bind(&path).map_err(|_| IpcError::UnsafePath)?;
        let node = fs::openat(
            &directory.fd,
            name,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let initial = fs::fstat(&node)?;
        if initial.st_uid != directory.uid
            || FileType::from_raw_mode(initial.st_mode) != FileType::Socket
        {
            return Err(IpcError::UnsafePath);
        }
        // O_PATH plus the kernel fd link pins the inode even during rename.
        // fchmod cannot operate on O_PATH; never chmod the ambient socket name.
        let mut listener = Self {
            stream,
            node,
            directory,
            name,
            identity: initial,
        };
        std::fs::set_permissions(
            format!("/proc/self/fd/{}", listener.node.as_raw_fd()),
            std::fs::Permissions::from_mode(0o600),
        )?;
        listener.identity = fs::fstat(&listener.node)?;
        if !same_entry(&listener.directory.fd, name, &listener.identity) {
            return Err(IpcError::UnsafePath);
        }
        listener.stream.set_nonblocking(true)?;
        Ok(listener)
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if same_entry(&self.directory.fd, self.name, &self.identity) {
            // There is no atomic conditional unlink. A malicious concurrent
            // same-UID replacer is outside the declared isolation boundary.
            let _ = fs::unlinkat(&self.directory.fd, self.name, AtFlags::empty());
        }
    }
}

/// Two independently accepted, private Unix channels with descriptor ownership.
///
/// Existing socket entries fail closed; startup never guesses that a socket is
/// stale or unlinks it. Cleanup is bounded, nonrecursive, and identity checked.
pub struct PrivateSockets {
    events: Listener,
    control: Listener,
    budget: ConnectionBudget,
}

impl PrivateSockets {
    /// Bind beneath an existing private runtime root with safe owned ancestors.
    ///
    /// # Errors
    /// Unsafe ownership/mode/type/symlink, existing socket, or OS failure. No
    /// shared temporary fallback, permission repair, or live-socket adoption.
    pub fn bind(runtime_root: &Path) -> Result<Self, IpcError> {
        let directory = Directory::open(runtime_root)?;
        let events = Listener::bind(directory.clone(), "events.sock")?;
        let control = Listener::bind(directory, "control.sock")?;
        Ok(Self {
            events,
            control,
            budget: ConnectionBudget::new(64)?,
        })
    }

    /// Accept one connection, verify real peer credentials and reserve capacity.
    ///
    /// # Errors
    /// Wrong peer UID, capacity exhausted, or OS failure. An internal 64-peer
    /// ceiling spans both sockets; the caller budget can impose a tighter limit.
    /// The handshake deadline is capped at 100ms from acceptance.
    pub fn accept(
        &self,
        channel: Channel,
        budget: &ConnectionBudget,
        deadline: Instant,
    ) -> Result<Option<Connection>, IpcError> {
        let listener = match channel {
            Channel::Event => &self.events,
            Channel::Control => &self.control,
        };
        match listener.stream.accept() {
            Ok((stream, _)) => Connection::accept(
                stream,
                channel,
                listener.directory.uid,
                budget,
                &self.budget,
                deadline,
            )
            .map(Some),
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
            {
                Ok(None)
            }
            Err(_) => Err(IpcError::Io),
        }
    }
}
