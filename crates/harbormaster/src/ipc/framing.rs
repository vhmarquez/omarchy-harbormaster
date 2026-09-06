use super::IpcError;
use crate::protocol::MAX_FRAME_BYTES;
use std::collections::VecDeque;

const MAX_UNSENT_FRAMES: usize = 256;
const MAX_UNSENT_BYTES: usize = 1_048_576;

/// Incremental bounded NDJSON framing; it never accumulates multiple frames.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    partial: Vec<u8>,
    closed: bool,
}

impl FrameDecoder {
    /// Consume through the first newline, returning consumed bytes and a frame.
    ///
    /// # Errors
    /// A malformed/overlong stream poisons this decoder. Caller must disconnect.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(usize, Option<Vec<u8>>), IpcError> {
        if self.closed {
            return Err(IpcError::Closed);
        }
        let room = MAX_FRAME_BYTES - self.partial.len();
        let inspected = &bytes[..bytes.len().min(room)];
        let newline = inspected.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(inspected.len(), |index| index + 1);
        self.partial.extend_from_slice(&inspected[..consumed]);
        if newline.is_some() {
            if !valid_frame(&self.partial) {
                self.close();
                return Err(IpcError::InvalidFrame);
            }
            return Ok((consumed, Some(std::mem::take(&mut self.partial))));
        }
        if self.partial.len() == MAX_FRAME_BYTES {
            self.close();
            return Err(IpcError::FrameTooLarge);
        }
        Ok((consumed, None))
    }

    /// Finish at EOF. No incomplete frame is silently discarded as success.
    ///
    /// # Errors
    /// Returns `TruncatedFrame` for any remaining partial input.
    pub fn finish(&mut self) -> Result<(), IpcError> {
        let partial = !self.partial.is_empty();
        self.close();
        if partial {
            Err(IpcError::TruncatedFrame)
        } else {
            Ok(())
        }
    }

    pub(super) fn has_partial(&self) -> bool {
        !self.partial.is_empty()
    }

    fn close(&mut self) {
        self.closed = true;
        self.partial.clear();
    }
}

fn valid_frame(frame: &[u8]) -> bool {
    frame.len() > 1
        && frame.last() == Some(&b'\n')
        && !frame[..frame.len() - 1]
            .iter()
            .any(|b| matches!(b, b'\n' | b'\r'))
        && std::str::from_utf8(frame).is_ok()
}

/// Bounded unsent allocation; partial writes retain the full allocation charge.
#[derive(Debug, Default)]
pub struct OutboundQueue {
    frames: VecDeque<Vec<u8>>,
    bytes: usize,
    offset: usize,
}

impl OutboundQueue {
    /// Queue exactly one encoded frame, checking both limits before allocation.
    ///
    /// # Errors
    /// Invalid frame, oversized frame, or exhausted byte/frame capacity.
    pub fn push(&mut self, frame: &[u8]) -> Result<(), IpcError> {
        if frame.len() > MAX_FRAME_BYTES {
            return Err(IpcError::FrameTooLarge);
        }
        if !valid_frame(frame) {
            return Err(IpcError::InvalidFrame);
        }
        if self.frames.len() == MAX_UNSENT_FRAMES || frame.len() > MAX_UNSENT_BYTES - self.bytes {
            return Err(IpcError::ResourceExhausted);
        }
        self.bytes += frame.len();
        self.frames.push_back(frame.to_vec());
        Ok(())
    }

    #[must_use]
    pub fn frames(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn front(&self) -> Option<&[u8]> {
        self.frames.front().map(|frame| &frame[self.offset..])
    }

    pub(super) fn advance(&mut self, written: usize) {
        self.offset += written;
        if let Some(frame) = self.frames.pop_front_if(|frame| self.offset == frame.len()) {
            self.bytes -= frame.len();
            self.offset = 0;
        }
    }
}
