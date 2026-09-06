//! Single bounded strict JSON boundary shared by all typed wire decoders.

use std::{
    collections::BTreeSet,
    fmt,
    io::{self, Write},
};

use serde::{
    Deserialize, Serialize,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;

use super::{MAX_FRAME_BYTES, MAX_JSON_DEPTH, ProtocolError};

struct StrictValue {
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictValue {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for StrictValue {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON without duplicate object keys")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(de::Error::custom("excessive depth"));
        }
        let mut result = Vec::new();
        while let Some(value) = sequence.next_element_seed(Self {
            depth: self.depth + 1,
        })? {
            result.push(value);
        }
        Ok(Value::Array(result))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_JSON_DEPTH {
            return Err(de::Error::custom("excessive depth"));
        }
        let mut keys = BTreeSet::new();
        let mut result = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(de::Error::custom("duplicate key"));
            }
            let value = map.next_value_seed(Self {
                depth: self.depth + 1,
            })?;
            result.insert(key, value);
        }
        Ok(Value::Object(result))
    }
}

pub(super) fn decode(frame: &[u8]) -> Result<Value, ProtocolError> {
    if frame.len() > MAX_FRAME_BYTES || frame.last() != Some(&b'\n') {
        return Err(ProtocolError::InvalidFrame);
    }
    let json = &frame[..frame.len() - 1];
    if json.iter().any(|byte| matches!(byte, b'\n' | b'\r')) {
        return Err(ProtocolError::InvalidFrame);
    }
    let mut deserializer = serde_json::Deserializer::from_slice(json);
    let value = StrictValue { depth: 0 }
        .deserialize(&mut deserializer)
        .map_err(|_| ProtocolError::InvalidFrame)?;
    deserializer
        .end()
        .map_err(|_| ProtocolError::InvalidFrame)?;
    if !value.is_object() {
        return Err(ProtocolError::InvalidFrame);
    }
    match value.get("protocol").and_then(Value::as_u64) {
        Some(0) => Ok(value),
        Some(_) => Err(ProtocolError::UnsupportedVersion),
        None => Err(ProtocolError::InvalidFrame),
    }
}

pub(super) fn typed<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, ProtocolError> {
    serde_json::from_value(value).map_err(|_| ProtocolError::InvalidFrame)
}

/// Encode one bounded NDJSON object and validate the common version/framing rules.
///
/// # Errors
/// Returns a value-free error for serialization failures or invalid frame bounds.
/// Only the protocol's typed envelope serializers should be used for wire output.
pub fn encode_frame<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtocolError> {
    let mut writer = FrameWriter(Vec::new());
    serde_json::to_writer(&mut writer, value).map_err(|_| ProtocolError::InvalidFrame)?;
    let mut bytes = writer.0;
    bytes.push(b'\n');
    decode(&bytes)?;
    Ok(bytes)
}

struct FrameWriter(Vec<u8>);
impl Write for FrameWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() >= MAX_FRAME_BYTES - self.0.len() {
            return Err(io::Error::other("frame limit exceeded"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_depth_eight_accepts_and_nine_rejects() {
        for depth in 1..=10 {
            let frame = format!(
                "{{\"protocol\":0,\"value\":{}0{}}}\n",
                "[".repeat(depth - 1),
                "]".repeat(depth - 1)
            );
            assert_eq!(decode(frame.as_bytes()).is_ok(), depth <= MAX_JSON_DEPTH);
        }
    }

    #[test]
    fn bounded_writer_refuses_growth_before_copying_excess_input() {
        let mut writer = FrameWriter(Vec::new());
        writer.write_all(&vec![b' '; MAX_FRAME_BYTES - 1]).unwrap();
        assert!(writer.write_all(b"x").is_err());
        assert_eq!(writer.0.len(), MAX_FRAME_BYTES - 1);
    }
}
