//! Narrow OS entropy boundary for manager-issued reconciliation generations.
use super::AdmissionError;
use crate::protocol::ProducerGeneration;
use rustix::rand::{GetRandomFlags, getrandom};
use std::fmt::Write;

pub(super) fn fresh() -> Result<ProducerGeneration, AdmissionError> {
    let mut bytes = [0_u8; 16];
    let count = getrandom(bytes.as_mut_slice(), GetRandomFlags::NONBLOCK)
        .map_err(|_| AdmissionError::EntropyUnavailable)?;
    if count != bytes.len() {
        return Err(AdmissionError::EntropyUnavailable);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut hex = String::with_capacity(32);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").map_err(|_| AdmissionError::EntropyUnavailable)?;
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
    .parse()
    .map_err(|_| AdmissionError::EntropyUnavailable)
}
