//! The `unchecked` field of a file marker (docs/design/media-showcase.md, the files section, part
//! 2), held to the shared fixture both apps read: scripts/tests/fixtures/file-marker-unchecked.json
//! (the web wrote it the way its `pqBuildFileMarker` builds). `built`: a meta, its exact JSON text
//! and the exact marker text both apps must build; `read`: marker texts and whether each says
//! unchecked (only JSON true does).

use super::*;

const FIXTURE: &str = include_str!("../../scripts/tests/fixtures/file-marker-unchecked.json");

/// This app builds exactly the fixture's bytes from each meta (with `"unchecked":true` last only for
/// a file it could not check) and reads exactly the fixture's answers; a marker's other fields read
/// as before. Seen red 2026-10-11 with `file_marker_unchecked` reading the word "checked" instead:
/// "unchecked: true: read as unchecked" failed.
#[test]
fn the_unchecked_marker_is_the_shared_fixture() {
    let fx: serde_json::Value = serde_json::from_str(FIXTURE).expect("the fixture reads");
    assert_eq!(fx["marker"].as_str(), Some(FILE_MARKER));
    let built = fx["built"].as_array().expect("built cases");
    assert!(built.len() >= 2);
    for case in built {
        let name = case["name"].as_str().unwrap_or("");
        let meta = &case["meta"];
        let att: DmAttachment = serde_json::from_value(meta.clone()).expect("the meta is an attachment");
        let unchecked = meta.get("unchecked").and_then(|u| u.as_bool()) == Some(true);
        let text = if unchecked { build_file_marker_unchecked(&att) } else { build_file_marker(&att) };
        assert_eq!(text, case["text"].as_str().unwrap(), "{name}: the marker text, byte for byte");
        let json = String::from_utf8(B64.decode(text.strip_prefix(FILE_MARKER).unwrap()).unwrap()).unwrap();
        assert_eq!(json, case["json"].as_str().unwrap(), "{name}: the JSON text");
        let back = parse_file_marker(&text).expect("it reads back");
        assert_eq!((back.url, back.size), (att.url.clone(), att.size), "{name}");
    }
    for case in fx["read"].as_array().expect("read cases") {
        let name = case["name"].as_str().unwrap_or("");
        assert_eq!(file_marker_unchecked(case["text"].as_str().unwrap()), case["unchecked"].as_bool().unwrap(), "{name}: read as unchecked");
    }
    assert!(!file_marker_unchecked("[[hum:file:v1]]not base64"));
    assert!(!file_marker_unchecked("plain words"));
}
