//! A how-to's hash: BLAKE3 of its canonical CBOR without its `review` block, the same encoding and
//! hash the signed objects use (src/relay/core/encoding.rs, docs/design/how-to.md "Sources and
//! review"). A review records this hash, so the review binds to the exact words it checked: change
//! a word and the hash no longer matches (gate review_hash_stale).
//!
//! The JavaScript twin is `HowToLogic.hash` in web/shared/how-to-logic.js, which
//! scripts/build-how-to.js writes into data/how_to/index.json for every how-to; the test
//! `the_two_hashes_agree_on_every_shipped_how_to` holds the two to the same bytes.

use serde_json::Value as Json;

/// The review-free canonical bytes of a how-to file's JSON, or why it has none. Fails on a number
/// that is not a whole number of zero or more (canonical CBOR has no floats; the `whole_numbers`
/// gate refuses those first, so this is the backstop).
pub fn canonical_bytes(how_to: &Json) -> Result<Vec<u8>, String> {
    let mut v = how_to.clone();
    if let Some(map) = v.as_object_mut() {
        map.remove("review");
    }
    let cbor = to_cbor(&v)?;
    crate::relay::core::encoding::to_canonical_bytes(&cbor).map_err(|e| e.to_string())
}

/// BLAKE3 of [`canonical_bytes`], lower-case hex.
pub fn hash(how_to: &Json) -> Result<String, String> {
    let bytes = canonical_bytes(how_to)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn to_cbor(v: &Json) -> Result<ciborium::Value, String> {
    use ciborium::Value as C;
    Ok(match v {
        Json::Null => C::Null,
        Json::Bool(b) => C::Bool(*b),
        Json::Number(n) => match n.as_u64() {
            Some(u) => C::Integer(u.into()),
            None => return Err(format!("{n} is not a whole number of zero or more")),
        },
        Json::String(s) => C::Text(s.clone()),
        Json::Array(a) => C::Array(a.iter().map(to_cbor).collect::<Result<_, _>>()?),
        Json::Object(o) => C::Map(
            o.iter()
                .map(|(k, v)| Ok((C::Text(k.clone()), to_cbor(v)?)))
                .collect::<Result<_, String>>()?,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The review block is not part of what a review checks, and key order is not part of the
    /// words; everything else is.
    #[test]
    fn the_hash_ignores_review_and_key_order_and_nothing_else() {
        let a: Json = serde_json::from_str(r#"{"id":"x","title":"T","review":{"status":"stub"}}"#).unwrap();
        let b: Json = serde_json::from_str(r#"{"title":"T","review":{"status":"verified","hash":"z"},"id":"x"}"#).unwrap();
        let c: Json = serde_json::from_str(r#"{"id":"x","title":"T."}"#).unwrap();
        assert_eq!(hash(&a).unwrap(), hash(&b).unwrap(), "review and key order do not change the hash");
        assert_ne!(hash(&a).unwrap(), hash(&c).unwrap(), "one character of the words does");
        let f: Json = serde_json::from_str(r#"{"n":1.5}"#).unwrap();
        assert!(hash(&f).is_err(), "a float cannot be canonical");
    }

    /// The bytes themselves, so a change to the encoding is caught here and not first as a
    /// mismatch with the website. {"a":1,"bb":[true,null],"c":"é"}: a map of three, keys ordered
    /// shorter first then bytewise.
    #[test]
    fn the_canonical_bytes_are_the_signed_object_encoding() {
        let v: Json = serde_json::from_str(r#"{"c":"é","bb":[true,null],"a":1}"#).unwrap();
        let bytes = canonical_bytes(&v).unwrap();
        let want: Vec<u8> = vec![
            0xa3, // map of 3
            0x61, b'a', 0x01, // "a": 1
            0x61, b'c', 0x62, 0xc3, 0xa9, // "c": "é" (two UTF-8 bytes)
            0x62, b'b', b'b', 0x82, 0xf5, 0xf6, // "bb": [true, null]
        ];
        assert_eq!(bytes, want);
    }
}
