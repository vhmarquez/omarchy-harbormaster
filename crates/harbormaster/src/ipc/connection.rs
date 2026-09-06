use super::{Channel, FrameDecoder, IpcError, OutboundQueue, PeerCredentials};
use std::cell::Cell;
use std::io::{ErrorKind, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::time::{Duration, Instant};

const MAX_CONNECTIONS: usize = 64;
const FORWARD_LIMIT: Duration = Duration::from_millis(100);

/// Shared per-manager connection limit, including handshakes and idle clients.
#[derive(Debug, Clone)]
pub struct ConnectionBudget {
    active: Rc<Cell<usize>>,
    limit: usize,
}

impl ConnectionBudget {
    /// # Errors
    /// Rejects zero or more than 64 simultaneous connections.
    pub fn new(limit: usize) -> Result<Self, IpcError> {
        if !(1..=MAX_CONNECTIONS).contains(&limit) {
            return Err(IpcError::ResourceExhausted);
        }
        Ok(Self {
            active: Rc::new(Cell::new(0)),
            limit,
        })
    }

    #[must_use]
    pub fn active(&self) -> usize {
        self.active.get()
    }

    fn acquire(&self) -> Result<Permit, IpcError> {
        if self.active() >= self.limit {
            return Err(IpcError::ResourceExhausted);
        }
        self.active.set(self.active() + 1);
        Ok(Permit(self.active.clone()))
    }
}

struct Permit(Rc<Cell<usize>>);

impl Drop for Permit {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

/// An accepted nonblocking stream with immutable listener/peer evidence.
///
/// Each call performs at most one read/write syscall and bounded framing work.
/// The caller polls ready streams fairly; this library does not run a reactor.
pub struct Connection {
    stream: UnixStream,
    peer: PeerCredentials,
    channel: Channel,
    _permit: Permit,
    _manager_permit: Permit,
    decoder: FrameDecoder,
    outgoing: OutboundQueue,
    pending: [u8; 4096],
    offset: usize,
    available: usize,
    handshake_deadline: Option<Instant>,
    frame_deadline: Option<Instant>,
    write_deadline: Option<Instant>,
    closed: bool,
}

impl Connection {
    pub(super) fn accept(
        stream: UnixStream,
        channel: Channel,
        expected_uid: u32,
        budget: &ConnectionBudget,
        manager_budget: &ConnectionBudget,
        deadline: Instant,
    ) -> Result<Self, IpcError> {
        let permit = budget.acquire()?;
        let manager_permit = manager_budget.acquire()?;
        let peer = PeerCredentials::read(&stream)?;
        peer.require_uid(expected_uid)?;
        // Linux accounts roughly twice this requested size. Pinning the kernel
        // send buffer also bounds the additional bytes beyond our unsent queue.
        rustix::net::sockopt::set_socket_send_buffer_size(&stream, 16_384)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            peer,
            channel,
            _permit: permit,
            _manager_permit: manager_permit,
            decoder: FrameDecoder::default(),
            outgoing: OutboundQueue::default(),
            pending: [0; 4096],
            offset: 0,
            available: 0,
            handshake_deadline: Some(deadline.min(Instant::now() + FORWARD_LIMIT)),
            frame_deadline: None,
            write_deadline: None,
            closed: false,
        })
    }

    #[must_use]
    pub fn channel(&self) -> Channel {
        self.channel
    }

    #[must_use]
    pub fn peer(&self) -> PeerCredentials {
        self.peer
    }

    /// The trusted caller invokes this only after separate manager authorization.
    /// It grants no operation and preserves partial-frame/slow-writer deadlines.
    ///
    /// # Errors
    /// Closed or expired handshake.
    pub fn complete_handshake(&mut self, now: Instant) -> Result<(), IpcError> {
        self.check_deadlines(now)?;
        self.handshake_deadline = None;
        Ok(())
    }

    /// Consume at most one bounded chunk and return at most one complete frame.
    ///
    /// # Errors
    /// Malformed, truncated, oversized, timed-out, closed, or failed stream.
    pub fn read_frame(&mut self, now: Instant) -> Result<Option<Vec<u8>>, IpcError> {
        self.check_deadlines(now)?;
        if self.offset == self.available {
            match self.stream.read(&mut self.pending) {
                Ok(0) => {
                    let result = self.decoder.finish();
                    self.close();
                    return result.and(Err(IpcError::Closed));
                }
                Ok(count) => {
                    self.offset = 0;
                    self.available = count;
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
                {
                    return Ok(None);
                }
                Err(_) => {
                    self.close();
                    return Err(IpcError::Io);
                }
            }
        }
        let decoded = self
            .decoder
            .feed(&self.pending[self.offset..self.available]);
        let (consumed, frame) = match decoded {
            Ok(value) => value,
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        self.offset += consumed;
        if frame.is_some() {
            self.frame_deadline = None;
        }
        if self.decoder.has_partial() && self.frame_deadline.is_none() {
            self.frame_deadline = Some(now + FORWARD_LIMIT);
        }
        Ok(frame)
    }

    /// Queue one frame. Saturation disconnects and requires a fresh snapshot.
    ///
    /// # Errors
    /// Closed, expired, invalid, oversized, or saturated stream.
    pub fn queue_frame(&mut self, frame: &[u8]) -> Result<(), IpcError> {
        let now = Instant::now();
        self.check_deadlines(now)?;
        if let Err(error) = self.outgoing.push(frame) {
            if error == IpcError::ResourceExhausted {
                self.close();
            }
            return Err(error);
        }
        if self.write_deadline.is_none() {
            self.write_deadline = Some(now + FORWARD_LIMIT);
        }
        Ok(())
    }

    /// Perform one nonblocking write. `true` means the unsent queue is empty.
    ///
    /// # Errors
    /// Closed/expired stream, zero-byte write, or OS failure. An expired slow
    /// consumer must reconnect/resnapshot; transport success is never durable ACK.
    pub fn flush(&mut self, now: Instant) -> Result<bool, IpcError> {
        self.check_deadlines(now)?;
        let Some(frame) = self.outgoing.front() else {
            return Ok(true);
        };
        match self.stream.write(frame) {
            Ok(0) => {
                self.close();
                return Err(IpcError::Closed);
            }
            Ok(written) => self.outgoing.advance(written),
            Err(error)
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) =>
            {
                return Ok(false);
            }
            Err(_) => {
                self.close();
                return Err(IpcError::Io);
            }
        }
        let empty = self.outgoing.frames() == 0;
        if empty {
            self.write_deadline = None;
        }
        Ok(empty)
    }

    fn check_deadlines(&mut self, now: Instant) -> Result<(), IpcError> {
        if self.closed {
            return Err(IpcError::Closed);
        }
        if [
            self.handshake_deadline,
            self.frame_deadline,
            self.write_deadline,
        ]
        .into_iter()
        .flatten()
        .any(|deadline| now >= deadline)
        {
            self.close();
            return Err(IpcError::DeadlineExceeded);
        }
        Ok(())
    }

    fn close(&mut self) {
        self.closed = true;
        let _ = self.stream.shutdown(Shutdown::Both);
        self.outgoing = OutboundQueue::default();
    }
}
