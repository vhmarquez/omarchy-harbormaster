//! Responses represent explicit status, never synthetic durable success.

use serde::{Deserialize, Serialize};

use super::{
    CapabilityRecord, Operation, ProtocolError, RequestId, Revision, Version,
    event::unique_bounded, strict,
};

/// Implemented wire result vocabulary. Runtime and snapshot records are not scaffolded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields, rename_all = "snake_case")]
pub enum ResponseResult {
    Capabilities { capabilities: Vec<CapabilityRecord> },
    ResyncRequired { snapshot_revision: Revision },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", deny_unknown_fields, rename_all = "snake_case")]
pub enum ResponseStatus {
    Ok {
        committed_revision: Option<Revision>,
        result: ResponseResult,
    },
    Error {
        error: ProtocolError,
    },
    AcceptedPending {},
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseEnvelope {
    pub request_id: RequestId,
    pub response: ResponseStatus,
}

impl ResponseEnvelope {
    fn validate(&self) -> Result<(), ProtocolError> {
        if let ResponseStatus::Ok {
            result: ResponseResult::Capabilities { capabilities },
            ..
        } = &self.response
        {
            let operations: Vec<_> = capabilities
                .iter()
                .map(|capability| capability.operation)
                .collect();
            unique_bounded(&operations, Operation::ALL.len())?;
        }
        Ok(())
    }
}

impl Serialize for ResponseEnvelope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Wire<'a> {
            protocol: Version,
            request_id: &'a RequestId,
            #[serde(flatten)]
            response: &'a ResponseStatus,
        }
        self.validate().map_err(serde::ser::Error::custom)?;
        Wire {
            protocol: Version,
            request_id: &self.request_id,
            response: &self.response,
        }
        .serialize(serializer)
    }
}

/// Decode an explicit response status without applying it to durable state.
/// # Errors
/// Rejects unsupported versions, unexpected fields and duplicated capabilities.
pub fn parse_response(frame: &[u8]) -> Result<ResponseEnvelope, ProtocolError> {
    let mut value = strict::decode(frame)?;
    let object = value.as_object_mut().ok_or(ProtocolError::InvalidFrame)?;
    let _: Version = strict::typed(
        object
            .remove("protocol")
            .ok_or(ProtocolError::InvalidFrame)?,
    )?;
    let request_id = strict::typed(
        object
            .remove("request_id")
            .ok_or(ProtocolError::InvalidFrame)?,
    )?;
    let response = ResponseEnvelope {
        request_id,
        response: strict::typed(value)?,
    };
    response.validate()?;
    Ok(response)
}
