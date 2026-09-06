//! Canonical wire identities and bounded metadata; these confer no authority.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::ProtocolError;

fn canonical_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

macro_rules! string_type {
    ($name:ident, $validator:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = ProtocolError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if ($validator)(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(ProtocolError::InvalidFrame)
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(de::Error::custom)
            }
        }
    };
}

string_type!(
    ProjectId,
    canonical_uuid,
    "Opaque manager-issued project key."
);
string_type!(TaskId, canonical_uuid, "Opaque manager-issued task key.");
string_type!(RunId, canonical_uuid, "Opaque manager-issued run key.");
string_type!(
    ProducerId,
    canonical_uuid,
    "Opaque manager-issued producer key."
);
string_type!(
    EventId,
    canonical_uuid,
    "Opaque event identity within a producer generation."
);
string_type!(
    RequestId,
    canonical_uuid,
    "Opaque idempotency identity for control requests."
);
string_type!(
    ProducerGeneration,
    canonical_uuid,
    "Trusted manager-issued generation; never minted by parsing."
);

fn opaque(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

string_type!(
    TurnId,
    |s: &str| opaque(s, 256),
    "Adapter-scoped turn metadata, 1–256 UTF-8 bytes."
);
string_type!(
    HarnessSessionId,
    |s: &str| opaque(s, 256),
    "Harness metadata identity, 1–256 UTF-8 bytes."
);
string_type!(
    AdapterVersion,
    |s: &str| opaque(s, 64),
    "Adapter version metadata, 1–64 UTF-8 bytes."
);
string_type!(
    SnapshotCursor,
    |s: &str| opaque(s, 256),
    "Opaque snapshot cursor, 1–256 UTF-8 bytes; grants no authority."
);
string_type!(
    TaskLabel,
    |s: &str| opaque(s, 1024),
    "Explicit control-only label, 1–1,024 UTF-8 bytes."
);
string_type!(
    LocalPath,
    |s: &str| s.starts_with('/') && opaque(s, 4096),
    "Explicit control-only absolute local path, at most 4,096 UTF-8 bytes; not filesystem proof."
);

macro_rules! decimal_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }
            #[must_use]
            pub const fn value(self) -> u64 {
                self.0
            }
            #[must_use]
            pub fn checked_next(self) -> Option<Self> {
                self.0.checked_add(1).map(Self)
            }
        }

        impl FromStr for $name {
            type Err = ProtocolError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.is_empty()
                    || (value.len() > 1 && value.starts_with('0'))
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(ProtocolError::InvalidFrame);
                }
                value
                    .parse()
                    .map(Self)
                    .map_err(|_| ProtocolError::InvalidFrame)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(de::Error::custom)
            }
        }
    };
}

decimal_type!(
    Seq,
    "Canonical nonnegative decimal u64 sequence, encoded as a JSON string."
);
decimal_type!(
    Revision,
    "Canonical decimal u64 projection revision, encoded as a JSON string."
);
