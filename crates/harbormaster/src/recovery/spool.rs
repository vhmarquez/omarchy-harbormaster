//! One bounded staging attempt, fsync, then a no-replace atomic publication.
use super::{
    AdapterRecord, ArtifactStore, MAX_PRODUCER_BYTES, MAX_PRODUCER_FILES, MAX_RECORD_BYTES,
    MAX_REPLAY_BATCH, MAX_SPOOL_BYTES, MAX_SPOOL_FILES, RecoveryError,
    files::{Entry, EntryHandle, Identity},
    logs,
    names::SpoolName,
    paths::LOCK,
    provenance::{self, EligibleMetadata, Mapping},
};
use crate::protocol::{EventEnvelope, HarnessKind, parse_event};
use rustix::fs::{self, Mode, OFlags, RenameFlags};
use serde::{Deserialize, Serialize};

/// Proof only of an owned metadata spool entry, never of SQLite commit or ACK.
pub struct SpoolReceipt {
    entry: ReplayEntry,
}
impl SpoolReceipt {
    #[must_use]
    pub fn event(&self) -> &EventEnvelope {
        self.entry.event()
    }
}

/// Original event plus an unforgeable owned identity token. No generation retagging.
pub struct ReplayEntry {
    event: EventEnvelope,
    harness: HarnessKind,
    held: EntryHandle,
}
impl ReplayEntry {
    #[must_use]
    pub fn event(&self) -> &EventEnvelope {
        &self.event
    }
    #[must_use]
    pub const fn harness(&self) -> HarnessKind {
        self.harness
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    format: u8,
    harness: HarnessKind,
    mapping_version: String,
    observed_ms: u64,
    event: serde_json::Value,
}

impl ArtifactStore {
    /// Validate reviewed source provenance before serialization or any file creation.
    /// No live adapter version is qualified in M1.
    /// # Errors
    /// Reject unsupported/unproven input, occupied identities, limits or unsafe I/O.
    pub fn stage(
        &mut self,
        input: AdapterRecord<'_>,
        observed_ms: u64,
    ) -> Result<SpoolReceipt, RecoveryError> {
        self.stage_reviewed(input, observed_ms, provenance::REVIEWED)
    }

    pub(super) fn stage_reviewed(
        &mut self,
        input: AdapterRecord<'_>,
        observed_ms: u64,
        reviewed: &[Mapping],
    ) -> Result<SpoolReceipt, RecoveryError> {
        let result = provenance::eligible(input, reviewed)
            .and_then(|metadata| self.stage_metadata(metadata, observed_ms, reviewed));
        result.map_err(|error| self.failure(error))
    }

    fn stage_metadata(
        &mut self,
        metadata: EligibleMetadata,
        observed_ms: u64,
        reviewed: &[Mapping],
    ) -> Result<SpoolReceipt, RecoveryError> {
        let (event, harness, version) = metadata.into_parts();
        let entries = self.directory.scan()?;
        if let Some(index) = entries.iter().position(|entry| {
            entry
                .spool
                .as_ref()
                .is_some_and(|name| name.same_event_identity(&event))
        }) {
            let existing = entries
                .into_iter()
                .nth(index)
                .ok_or(RecoveryError::InvalidRecord)?;
            let replay = self.decode_entry(existing, reviewed)?;
            if replay.event != event {
                return Err(RecoveryError::InvalidRecord);
            }
            return Ok(SpoolReceipt { entry: replay });
        }
        let bytes = serde_json::to_vec(&Record {
            format: 1,
            harness,
            mapping_version: version.to_owned(),
            observed_ms,
            event: serde_json::to_value(&event).map_err(|_| RecoveryError::InvalidRecord)?,
        })
        .map_err(|_| RecoveryError::InvalidRecord)?;
        budget(&entries, &event, bytes.len())?;
        self.changing()?;
        self.write_stage(&event, observed_ms, &bytes, reviewed)
    }

    fn write_stage(
        &self,
        event: &EventEnvelope,
        observed_ms: u64,
        bytes: &[u8],
        reviewed: &[Mapping],
    ) -> Result<SpoolReceipt, RecoveryError> {
        self.directory.check()?;
        let partial = SpoolName::new(event, observed_ms, true).filename();
        let ready = SpoolName::new(event, observed_ms, false).filename();
        let fd = fs::openat(
            &self.directory.fd,
            partial.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let mut file = std::fs::File::from(fd);
        #[cfg(test)]
        super::tests::checkpoint("created");
        self.directory.file_identity(&partial, &file)?;
        super::files::write_once(&mut file, bytes)?;
        file.sync_all()?;
        fs::fsync(&self.directory.fd)?;
        let identity = self.directory.file_identity(&partial, &file)?;
        let held = self.directory.hold(Entry {
            name: partial.clone(),
            identity,
            spool: SpoolName::parse(&partial),
        })?;
        #[cfg(test)]
        super::tests::checkpoint("staged");
        self.directory.validate(&held)?;
        fs::renameat_with(
            &self.directory.fd,
            partial.as_str(),
            &self.directory.fd,
            ready.as_str(),
            RenameFlags::NOREPLACE,
        )?;
        #[cfg(test)]
        super::tests::checkpoint("renamed");
        fs::fsync(&self.directory.fd)?;
        drop(held);
        let entry = Entry {
            name: ready.clone(),
            identity: Identity::from_stat(&fs::fstat(&file)?)?,
            spool: SpoolName::parse(&ready),
        };
        Ok(SpoolReceipt {
            entry: self.decode_entry(entry, reviewed)?,
        })
    }

    /// Read at most 32 records / 512 KiB; revalidate mapping eligibility on replay.
    /// This neither acknowledges, deletes nor updates the original generation.
    /// # Errors
    /// Unsupported/malformed files and changed identities fail closed.
    pub fn read_batch(&self, now_ms: u64) -> Result<Vec<ReplayEntry>, RecoveryError> {
        self.read_reviewed(now_ms, provenance::REVIEWED)
    }

    pub(super) fn read_reviewed(
        &self,
        now_ms: u64,
        reviewed: &[Mapping],
    ) -> Result<Vec<ReplayEntry>, RecoveryError> {
        let entries = self.directory.scan()?;
        let mut result = Vec::new();
        for entry in entries
            .into_iter()
            .filter(|entry| {
                entry
                    .spool
                    .as_ref()
                    .is_some_and(|name| !name.partial && !name.expired(now_ms))
            })
            .take(MAX_REPLAY_BATCH)
        {
            result.push(self.decode_entry(entry, reviewed)?);
        }
        Ok(result)
    }

    fn decode_entry(
        &self,
        entry: Entry,
        reviewed: &[Mapping],
    ) -> Result<ReplayEntry, RecoveryError> {
        let name = entry
            .spool
            .clone()
            .filter(|name| !name.partial)
            .ok_or(RecoveryError::InvalidRecord)?;
        let held = self.directory.hold(entry)?;
        let bytes = self.directory.read(&held, MAX_RECORD_BYTES)?;
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|_| RecoveryError::InvalidRecord)?;
        let mapping = provenance::mapping(record.harness, &record.mapping_version, reviewed)?;
        if record.format != 1 || record.observed_ms != name.observed_ms {
            return Err(RecoveryError::InvalidRecord);
        }
        let mut frame =
            serde_json::to_vec(&record.event).map_err(|_| RecoveryError::InvalidRecord)?;
        frame.push(b'\n');
        let event = parse_event(&frame).map_err(|_| RecoveryError::InvalidRecord)?;
        provenance::validate_replay(&event, mapping)?;
        if !name.matches(&event) {
            return Err(RecoveryError::InvalidRecord);
        }
        Ok(ReplayEntry {
            event,
            harness: record.harness,
            held,
        })
    }

    /// Trusted coordinator disposal after its non-forgeable durable receipt lookup.
    /// Equality includes all event fields. Public wire admission cannot call this.
    #[allow(dead_code)] // Wired by the coordinator integration in the parent branch.
    pub(crate) fn confirm_committed(
        &mut self,
        entry: ReplayEntry,
        durable_event: &EventEnvelope,
    ) -> Result<(), RecoveryError> {
        let ReplayEntry { event, held, .. } = entry;
        if event != *durable_event {
            return Err(RecoveryError::InvalidRecord);
        }
        self.directory.validate(&held)?;
        self.changing()?;
        let result = self.directory.remove(&held);
        drop(held);
        result
    }
}

fn budget(
    entries: &[Entry],
    event: &EventEnvelope,
    additional: usize,
) -> Result<(), RecoveryError> {
    if additional > MAX_RECORD_BYTES {
        return Err(RecoveryError::BoundExceeded);
    }
    let mut total = additional as u64;
    let mut producer = additional as u64;
    let mut files = 1;
    let mut producer_files = 1;
    for entry in entries
        .iter()
        .filter(|entry| entry.name != LOCK && !logs::is_log(&entry.name))
    {
        total = total.saturating_add(entry.bytes());
        files += 1;
        if entry
            .spool
            .as_ref()
            .is_some_and(|name| name.producer == event.producer_id)
        {
            producer = producer.saturating_add(entry.bytes());
            producer_files += 1;
        }
    }
    if total > MAX_SPOOL_BYTES
        || producer > MAX_PRODUCER_BYTES
        || files > MAX_SPOOL_FILES
        || producer_files > MAX_PRODUCER_FILES
    {
        return Err(RecoveryError::BoundExceeded);
    }
    Ok(())
}
