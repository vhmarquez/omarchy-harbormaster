use crate::{
    ipc::Connection,
    manager::{Command, ManagerError, Reply, Request, operations::Work},
    protocol::encode_frame,
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub(super) enum Progress {
    Pending,
    Finished,
    Stopped,
}
pub(super) struct Session {
    connection: Connection,
    pending: Option<(Instant, mpsc::Receiver<Reply>)>,
    responding: bool,
    stop_requested: bool,
    stop_accepted: bool,
}
impl Session {
    pub(super) fn new(connection: Connection) -> Self {
        Self {
            connection,
            pending: None,
            responding: false,
            stop_requested: false,
            stop_accepted: false,
        }
    }

    pub(super) fn poll(
        &mut self,
        sender: &mpsc::SyncSender<Work>,
        now: Instant,
    ) -> Result<Progress, ManagerError> {
        if self.responding {
            return if self.connection.flush(now)? {
                Ok(if self.stop_accepted {
                    Progress::Stopped
                } else {
                    Progress::Finished
                })
            } else {
                Ok(Progress::Pending)
            };
        }
        if let Some((start, receiver)) = &self.pending {
            if now.duration_since(*start) >= Duration::from_secs(25) {
                return Err(ManagerError::UnknownOutcome);
            }
            return match receiver.try_recv() {
                Ok(reply) => self.respond(reply),
                Err(mpsc::TryRecvError::Empty) => Ok(Progress::Pending),
                Err(mpsc::TryRecvError::Disconnected) => Err(ManagerError::UnknownOutcome),
            };
        }
        let Some(frame) = self.connection.read_frame(now)? else {
            return Ok(Progress::Pending);
        };
        let request = Request::parse(&frame)?;
        self.connection.complete_handshake(now)?;
        self.stop_requested = matches!(request.command, Command::Stop);
        let id = request.request_id.clone();
        let (reply, receiver) = mpsc::sync_channel(1);
        match sender.try_send(Work { request, reply }) {
            Ok(()) => self.pending = Some((now, receiver)),
            Err(_) => {
                return self.respond(Reply::Error {
                    protocol: 0,
                    request_id: id,
                    error: ManagerError::Busy,
                });
            }
        }
        Ok(Progress::Pending)
    }

    fn respond(&mut self, mut reply: Reply) -> Result<Progress, ManagerError> {
        self.stop_accepted = self.stop_requested && matches!(reply, Reply::Ok { .. });
        let bytes = fit(&mut reply)?;
        self.connection.queue_frame(&bytes)?;
        self.responding = true;
        self.pending = None;
        Ok(Progress::Pending)
    }
}

fn fit(reply: &mut Reply) -> Result<Vec<u8>, ManagerError> {
    for _ in 0..101 {
        if let Ok(bytes) = encode_frame(reply) {
            return Ok(bytes);
        }
        let Reply::Ok { value, .. } = reply else {
            return Err(ManagerError::UnknownOutcome);
        };
        let page = if value.get("items").is_some() {
            value
        } else {
            value
                .as_object_mut()
                .and_then(|value| value.values_mut().next())
                .ok_or(ManagerError::UnknownOutcome)?
        };
        let items = page
            .get_mut("items")
            .and_then(serde_json::Value::as_array_mut)
            .ok_or(ManagerError::UnknownOutcome)?;
        if items.len() <= 1 {
            return Err(ManagerError::UnknownOutcome);
        }
        items.pop();
        let last = items.last().ok_or(ManagerError::UnknownOutcome)?;
        let id = last
            .get("id")
            .or_else(|| last.get("name"))
            .or_else(|| last.get("run").and_then(|run| run.get("id")))
            .cloned()
            .ok_or(ManagerError::UnknownOutcome)?;
        page["next_after"] = id;
    }
    Err(ManagerError::UnknownOutcome)
}
