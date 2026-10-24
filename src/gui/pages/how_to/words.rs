//! The How-to page's own words, from data/how_to/words.json: the one file both apps read, and the
//! one a translation copies (English only for now). `w(key)` is a word; `wf(key, &[..])` fills
//! its {name} marks. A key the file does not have reads as the key itself, so a gap shows on the
//! page instead of an empty space, and the test in tests.rs fails on it first.

use std::collections::HashMap;
use std::sync::OnceLock;

static WORDS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// The words, read once: the data folder's file, else the copy built into the exe.
fn words() -> &'static HashMap<String, String> {
    WORDS.get_or_init(|| {
        let text = crate::embedded_data::read_data_or_embedded(&crate::data_dir(), "how_to/words.json").unwrap_or_default();
        parse(&text)
    })
}

/// The `words` object of a words file.
pub fn parse(text: &str) -> HashMap<String, String> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("words").and_then(|w| w.as_object()).cloned())
        .map(|o| o.into_iter().filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string()))).collect())
        .unwrap_or_default()
}

/// A word of the page.
pub fn w(key: &'static str) -> &'static str {
    words().get(key).map(String::as_str).unwrap_or(key)
}

/// A word with its {name} marks filled.
pub fn wf(key: &'static str, fill: &[(&str, &str)]) -> String {
    let mut s = w(key).to_string();
    for (name, value) in fill {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

/// A duration in minutes, in words: "10 min", "12 h 0 min".
pub fn minutes(n: u64) -> String {
    if n < 60 {
        wf("minutes", &[("n", &n.to_string())])
    } else {
        wf("hours", &[("h", &(n / 60).to_string()), ("m", &(n % 60).to_string())])
    }
}

/// Time left on a timer, from milliseconds: "4:59", "11:59:59".
pub fn clock(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
