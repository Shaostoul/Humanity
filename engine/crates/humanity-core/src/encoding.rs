//! Canonical CBOR encoding and decoding.
//!
//! Implements the rules from `design/conformance/canonical_cbor_rules.md`:
//! - Deterministic map key ordering (shorter encoding first, then lexicographic)
//! - Definite-length only (no indefinite-length strings/arrays/maps)
//! - Shortest integer encoding
//! - No floating point
//! - No CBOR tags (unless a future schema allows them)
//! - UTF-8 text strings only

use ciborium::Value;

use crate::error::{Error, Result};

/// Encode a CBOR Value to canonical bytes.
///
/// The value must already use canonical types (no floats, no tags).
/// Map keys are sorted by canonical CBOR rules before encoding.
pub fn to_canonical_bytes(value: &Value) -> Result<Vec<u8>> {
    let canonical = canonicalize(value)?;
    let mut buf = Vec::new();
    ciborium::into_writer(&canonical, &mut buf)
        .map_err(|e| Error::CborEncode(e.to_string()))?;
    Ok(buf)
}

/// Decode canonical CBOR bytes to a Value.
pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Value> {
    ciborium::from_reader(bytes)
        .map_err(|e| Error::CborDecode(e.to_string()))
}

/// Recursively canonicalize a CBOR Value:
/// - Sort map keys by canonical CBOR ordering
/// - Reject floats and tags
fn canonicalize(value: &Value) -> Result<Value> {
    match value {
        // Primitives — pass through
        Value::Integer(_) => Ok(value.clone()),
        Value::Bool(_) => Ok(value.clone()),
        Value::Null => Ok(value.clone()),
        Value::Bytes(b) => Ok(Value::Bytes(b.clone())),
        Value::Text(s) => Ok(Value::Text(s.clone())),

        // Arrays — canonicalize each element
        Value::Array(arr) => {
            let canonical: Result<Vec<Value>> = arr.iter().map(canonicalize).collect();
            Ok(Value::Array(canonical?))
        }

        // Maps — canonicalize values and sort keys
        Value::Map(entries) => {
            let mut canonical_entries: Vec<(Value, Value)> = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let ck = canonicalize(k)?;
                let cv = canonicalize(v)?;
                canonical_entries.push((ck, cv));
            }
            // Sort by canonical CBOR key ordering:
            // 1. Shorter encoded key first
            // 2. Lexicographic comparison of encoded bytes
            canonical_entries.sort_by(|(a, _), (b, _)| {
                let a_bytes = encode_for_sorting(a);
                let b_bytes = encode_for_sorting(b);
                a_bytes.len().cmp(&b_bytes.len()).then(a_bytes.cmp(&b_bytes))
            });
            Ok(Value::Map(canonical_entries))
        }

        // Floats — prohibited
        Value::Float(_) => Err(Error::CanonicalViolation(
            "floating point numbers are prohibited in canonical objects".into(),
        )),

        // Tags — prohibited by default
        Value::Tag(_, _) => Err(Error::CanonicalViolation(
            "CBOR tags are prohibited unless explicitly permitted by schema".into(),
        )),

        _ => Err(Error::CanonicalViolation(format!(
            "unsupported CBOR type: {:?}",
            value
        ))),
    }
}

/// Encode a value to bytes for sorting purposes.
/// Used internally for canonical map key ordering.
fn encode_for_sorting(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    // This should not fail for values we've already validated
    let _ = ciborium::into_writer(value, &mut buf);
    buf
}

/// Helper: build a CBOR map from key-value pairs.
/// Keys are text strings. Values are any CBOR Value.
/// The resulting map is NOT yet canonicalized — call `to_canonical_bytes`
/// to get canonical output.
pub fn cbor_map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (Value::Text(k.to_string()), v))
            .collect(),
    )
}

/// Helper: convert a u64 to a CBOR integer value.
pub fn cbor_int(n: u64) -> Value {
    Value::Integer(n.into())
}

/// Helper: convert bytes to a CBOR bytes value.
pub fn cbor_bytes(b: &[u8]) -> Value {
    Value::Bytes(b.to_vec())
}

/// Helper: convert a string to a CBOR text value.
pub fn cbor_text(s: &str) -> Value {
    Value::Text(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_map_key_ordering() {
        // Keys should be sorted: shorter encoding first, then lexicographic
        let map = cbor_map(vec![
            ("z", cbor_int(1)),
            ("a", cbor_int(2)),
            ("bb", cbor_int(3)),
        ]);
        let bytes = to_canonical_bytes(&map).unwrap();
        let decoded = from_canonical_bytes(&bytes).unwrap();

        if let Value::Map(entries) = decoded {
            // Single-char keys "a" and "z" come before "bb" (shorter encoding)
            // Among single-char keys, "a" < "z" lexicographically
            let keys: Vec<String> = entries
                .iter()
                .map(|(k, _)| match k {
                    Value::Text(s) => s.clone(),
                    _ => panic!("expected text key"),
                })
                .collect();
            assert_eq!(keys, vec!["a", "z", "bb"]);
        } else {
            panic!("expected map");
        }
    }

    #[test]
    fn reject_floats() {
        let val = Value::Float(3.14);
        assert!(to_canonical_bytes(&val).is_err());
    }

    #[test]
    fn reject_tags() {
        let val = Value::Tag(1, Box::new(Value::Integer(42.into())));
        assert!(to_canonical_bytes(&val).is_err());
    }

    #[test]
    fn roundtrip_simple_values() {
        let val = cbor_map(vec![
            ("name", cbor_text("test")),
            ("count", cbor_int(42)),
            ("data", cbor_bytes(&[0xDE, 0xAD])),
        ]);
        let bytes = to_canonical_bytes(&val).unwrap();
        let decoded = from_canonical_bytes(&bytes).unwrap();
        // Re-encode to verify stability
        let bytes2 = to_canonical_bytes(&decoded).unwrap();
        assert_eq!(bytes, bytes2, "canonical encoding must be stable across roundtrips");
    }
}
