//! Tests for describe_metadata.rs: the desktop's describe gives the web's answers on the shared
//! fixtures (tests/fixtures/describe/, the web's answers in expected.json, written by
//! scripts/make-describe-fixtures.js), reads a file from disk the same as from memory, and finds
//! nothing in what the relay's own strips leave. scripts/tests/describe-agree.test.js holds the two
//! together on thousands of generated files as well.

use super::*;

fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("describe")
}

/// Every fixture with what it was described with and the web's answer.
fn expected() -> Vec<(String, String, serde_json::Value)> {
    let text = std::fs::read_to_string(fixtures().join("expected.json")).expect("the fixtures' answers are there");
    let all: serde_json::Value = serde_json::from_str(&text).expect("expected.json reads");
    all.as_object()
        .expect("an object of files")
        .iter()
        .map(|(file, v)| (file.clone(), v["nameOrType"].as_str().unwrap_or("").to_string(), v["answer"].clone()))
        .collect()
}

/// The answer the web gave equals ours: every field exactly, the coordinates to the last few bits
/// (serde_json's own number reading is not always the closest double; the agreement test compares
/// them exactly through JavaScript's).
fn same(ours: &Description, web: &serde_json::Value, file: &str) {
    let mine: serde_json::Value = serde_json::from_str(&ours.to_json(true)).expect("our answer is JSON");
    for field in ["kind", "claimed", "hasLocation", "hasCamera", "details", "summary", "complete", "place"] {
        assert_eq!(mine[field], web[field], "{file}: {field}");
    }
    match (ours.coordinates, web["coordinates"].as_object()) {
        (None, None) => {}
        (Some((lat, lon)), Some(w)) => {
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(1.0);
            assert!(close(lat, w["latitude"].as_f64().unwrap()) && close(lon, w["longitude"].as_f64().unwrap()), "{file}: coordinates {lat},{lon} against {w:?}");
        }
        (a, b) => panic!("{file}: coordinates {a:?} against {b:?}"),
    }
}

/// The shared fixtures: the web's answer for each, from memory and from disk.
/// Seen red 2026-10-11 with the 3GPP place box read latitude first (it writes longitude first):
/// "3gp-loci-utf16.3gp: coordinates None against Some({"latitude": ...-33.85679626464844, ...})"
/// (151 is no latitude, so the swapped reading was thrown away); passed restored.
#[test]
fn every_fixture_gets_the_webs_answer_from_memory_and_from_disk() {
    let all = expected();
    assert!(all.len() >= 50, "the fixtures are there ({})", all.len());
    for (file, name, web) in &all {
        let path = fixtures().join(file);
        let bytes = std::fs::read(&path).expect("the fixture is there");
        same(&describe(&bytes, name), web, file);
        same(&describe_file(&path, name).expect("read from disk"), web, &format!("{file} (from disk)"));
    }
}

/// The attach step's words for a phone photo, word for word, and nothing for a clean one.
#[test]
fn a_phone_photo_says_where_it_was_taken_and_which_camera() {
    let bytes = std::fs::read(fixtures().join("jpeg-phone.jpg")).unwrap();
    let d = describe(&bytes, "IMG_0001.JPG");
    assert_eq!(d.summary.as_deref(), Some("This photo says where it was taken and which camera took it."));
    assert_eq!(d.claimed, Some("jpeg"));
    assert!(d.details.iter().any(|x| x.words == "Which camera or phone took it (Apple iPhone 15 Pro)"), "{:?}", d.details);
    assert!(d.details.iter().all(|x| !x.words.contains("37")), "no numbers in the words");
    let video = std::fs::read(fixtures().join("mov-apple.mov")).unwrap();
    assert_eq!(describe(&video, "x.mov").summary.as_deref(), Some("This video says where it was filmed and which camera filmed it."));
    let clean = std::fs::read(fixtures().join("jpeg-clean.jpg")).unwrap();
    assert_eq!(describe(&clean, "x.jpg").summary, None);
}

/// Describe looks only inside what the strips remove: after the relay's own strip, a picture or a
/// video that was read through says nothing at all. Seen red 2026-10-11 with describe also reading
/// a JPEG's kept colour segment (APP2, which the strip keeps): "jpeg-phone.jpg: still says
/// ["other"]" (and the fixture test failed on "jpeg-clean.jpg: details"); passed restored.
#[test]
fn what_the_strips_leave_says_nothing() {
    use crate::relay::core::{strip_metadata, strip_video_metadata};
    let mut checked = 0;
    for (file, name, web) in expected() {
        if web["complete"] != serde_json::Value::Bool(true) || web["details"].as_array().map_or(true, |d| d.is_empty()) {
            continue;
        }
        let bytes = std::fs::read(fixtures().join(&file)).unwrap();
        let cleaned = match web["kind"].as_str() {
            Some("jpeg") => strip_metadata::strip_image_metadata("image/jpeg", &bytes),
            Some("png") => strip_metadata::strip_image_metadata("image/png", &bytes),
            Some("webp") => strip_metadata::strip_image_metadata("image/webp", &bytes),
            Some("mp4") => {
                let mut b = bytes.clone();
                strip_video_metadata::strip_video_metadata_in_buffer(&mut b).unwrap();
                b
            }
            _ => continue,
        };
        let after = describe(&cleaned, &name);
        assert!(after.details.is_empty(), "{file}: still says {:?}", after.details.iter().map(|d| d.key).collect::<Vec<_>>());
        checked += 1;
    }
    assert!(checked >= 25, "enough files had something to remove ({checked})");
}
