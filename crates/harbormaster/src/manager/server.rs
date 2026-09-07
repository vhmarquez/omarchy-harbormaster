use super::{
    ManagerError,
    operations::{Operations, Work},
};
use crate::{
    ipc::{Channel, ConnectionBudget, PrivateSockets},
    runtime::Backend,
    storage::DatabaseWorker,
};
use nix::sys::{
    signal::{SigSet, Signal},
    signalfd::{SfdFlags, SignalFd},
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
mod session;

/// Serve private bounded control IPC until a graceful stop or SIGTERM/SIGINT.
/// # Errors
/// Invalid state/runtime paths, existing manager/socket, or unavailable OS resources.
pub fn serve() -> Result<(), ManagerError> {
    let signals = signals()?;
    let root = super::runtime_root()?;
    let state = crate::cli::state_home().map_err(|_| ManagerError::UnsafePath)?;
    crate::projects::paths::directory(&state, true)?;
    let database = DatabaseWorker::open(&state)?;
    let sockets = PrivateSockets::bind(&root)?;
    let operations = Operations {
        database,
        runtime: Backend::new(&root)?,
    };
    let (sender, receiver) = mpsc::sync_channel::<Work>(8);
    let worker = std::thread::Builder::new()
        .name("manager-operations".into())
        .spawn(move || operations.run(receiver))?;
    let result = reactor(&sockets, &signals, &sender);
    // Release owned endpoints before draining accepted work. A service stop
    // deadline must not leave stale sockets merely because a worker is slow.
    drop(sockets);
    drop(sender);
    let joined = worker.join().map_err(|_| ManagerError::UnknownOutcome);
    result.and(joined)
}

fn signals() -> Result<SignalFd, ManagerError> {
    let mut mask = SigSet::empty();
    mask.add(Signal::SIGTERM);
    mask.add(Signal::SIGINT);
    mask.thread_block().map_err(|_| ManagerError::Unavailable)?;
    SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC)
        .map_err(|_| ManagerError::Unavailable)
}

fn reactor(
    sockets: &PrivateSockets,
    signals: &SignalFd,
    sender: &mpsc::SyncSender<Work>,
) -> Result<(), ManagerError> {
    let budget = ConnectionBudget::new(16)?;
    let mut clients = Vec::new();
    let mut stopping = None;
    loop {
        let now = Instant::now();
        if signals
            .read_signal()
            .map_err(|_| ManagerError::Unavailable)?
            .is_some()
        {
            stopping.get_or_insert(now);
        }
        if stopping.is_none() {
            // Event transport is deliberately dormant until an eligible adapter
            // exists. Its frames never enter the control worker.
            let _ = sockets.accept(Channel::Event, &budget, now + Duration::from_millis(100));
            if let Ok(Some(connection)) =
                sockets.accept(Channel::Control, &budget, now + Duration::from_millis(100))
            {
                clients.push(session::Session::new(connection));
            }
        }
        clients.retain_mut(|client| match client.poll(sender, now) {
            Ok(session::Progress::Pending) => true,
            Ok(session::Progress::Stopped) => {
                stopping.get_or_insert(now);
                false
            }
            Ok(session::Progress::Finished) | Err(_) => false,
        });
        if stopping.is_some_and(|start| {
            clients.is_empty() || now.duration_since(start) >= Duration::from_secs(5)
        }) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(if clients.is_empty() {
            40
        } else {
            5
        }));
    }
}
