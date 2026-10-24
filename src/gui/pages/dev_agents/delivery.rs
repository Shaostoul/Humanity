//! Is the taskbar exe the work in this checkout? The Rust twin of `scripts/check-delivery.js`
//! (`judgeDelivery`, `codeDiff`, `DELIVERY_IGNORES`) and of the stamp half of
//! `scripts/lib/src-fingerprint.js`, so the panel answers the same question `just
//! check-delivery` does without starting node every minute.
//!
//! THE DEFINITION is build.rs's (`write_source_stamp`): every regular file under its
//! FINGERPRINT_INPUTS list (read out of the checkout's own build.rs, so the list travels with
//! the code), '/'-separated paths sorted by their UTF-8 bytes, each file's bytes with CRLF
//! turned into LF hashed with SHA-256. Every build carries that manifest inside the exe between
//! two marker lines; the verdict compares the exe's manifest with the files on disk now.
//!
//! THE MARKERS ARE NEVER WRITTEN OUT IN THIS FILE. The scripts find a build's stamp by
//! searching the exe's bytes for the begin marker, and refuse an exe that holds a marker without
//! a well-formed stamp after it ("BROKEN STAMP"). A string literal holding the marker would be
//! compiled into every HumanityOS.exe and break every rig's freshness gate and `just
//! check-delivery`. So the markers are stored reversed and turned round at run time, behind
//! `black_box` so the optimizer cannot fold them back into a literal.
//! (`markers_are_not_in_the_binary` in tests.rs checks the test program for them.)

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

/// The manifest's first line (not a marker: the scripts only look for it inside a stamp).
const MANIFEST_HEADER: &str = "hos-src-manifest v1\n";

fn unreverse(reversed: &'static str) -> String {
    std::hint::black_box(reversed).chars().rev().collect()
}

/// The begin marker, built at run time (see the module note).
pub fn stamp_begin() -> String {
    unreverse("v PMATS-CRS-SOH")
}

/// The end marker, built at run time.
pub fn stamp_end() -> String {
    unreverse("\nDNE PMATS-CRS-SOH")
}

/// A file list: '/'-separated path -> SHA-256 hex of its bytes (CRLF as LF).
pub type Manifest = BTreeMap<String, String>;

/// FINGERPRINT_INPUTS out of build.rs: a flat list of plain string literals, or an error naming
/// what is wrong (an empty or half-read list would fingerprint nothing and match everything).
pub fn read_inputs(build_rs: &str) -> Result<Vec<String>, String> {
    let at = build_rs.find("const FINGERPRINT_INPUTS").ok_or("build.rs has no FINGERPRINT_INPUTS list")?;
    let rest = &build_rs[at..];
    let eq = rest.find('=').ok_or("FINGERPRINT_INPUTS has no value")?;
    let open = rest[eq..].find('[').map(|i| eq + i + 1).ok_or("FINGERPRINT_INPUTS is not a list")?;
    let close = rest[open..].find(']').map(|i| open + i).ok_or("FINGERPRINT_INPUTS never closes")?;
    let body = &rest[open..close];
    let mut items = Vec::new();
    let mut leftover = String::new();
    for line in body.lines() {
        let code = line.split("//").next().unwrap_or("");
        let mut parts = code.split('"');
        leftover.push_str(parts.next().unwrap_or(""));
        loop {
            let (Some(item), Some(after)) = (parts.next(), parts.next()) else { break };
            items.push(item.to_string());
            leftover.push_str(after);
        }
    }
    if items.is_empty() || leftover.chars().any(|c| !(c.is_whitespace() || c == ',')) {
        return Err("FINGERPRINT_INPUTS must be a flat list of plain string literals".into());
    }
    Ok(items)
}

/// CRLF -> LF; a lone CR stays (git does not treat it as a line ending).
pub fn crlf_to_lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Every regular file at or under `rel`; symlinks and junctions are skipped, not followed.
fn collect(root: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(meta) = std::fs::symlink_metadata(root.join(rel)) else { return };
    if meta.file_type().is_symlink() {
        return;
    }
    if meta.is_dir() {
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else { return };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else { continue };
            let child = format!("{rel}/{}", entry.file_name().to_string_lossy());
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                collect(root, &child, out);
            } else if kind.is_file() {
                out.push(child);
            }
        }
    } else if meta.is_file() {
        out.push(rel.to_string());
    }
}

/// The tree's manifest, from the files on disk now.
pub fn fingerprint_tree(root: &Path) -> Result<Manifest, String> {
    let build_rs = std::fs::read_to_string(root.join("build.rs")).map_err(|e| format!("build.rs: {e}"))?;
    let mut paths = Vec::new();
    for input in read_inputs(&build_rs)? {
        collect(root, &input, &mut paths);
    }
    let mut out = Manifest::new();
    for rel in paths {
        let bytes = std::fs::read(root.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        out.insert(rel, sha256_hex(&crlf_to_lf(&bytes)));
    }
    Ok(out)
}

/// One stamp read out of an exe.
#[derive(Debug, Clone, PartialEq)]
pub struct Stamp {
    pub fingerprint: String,
    pub files: Manifest,
}

/// Parse one stamp's text, from its begin marker to just past its end marker. The manifest must
/// hash to the fingerprint it carries, so a truncated or garbled read never passes as a stamp.
pub fn parse_stamp(text: &str) -> Result<Stamp, String> {
    let begin = stamp_begin();
    let end = stamp_end();
    let lines: Vec<&str> = text.split('\n').collect();
    let version = lines.first().and_then(|l| l.strip_prefix(begin.as_str())).unwrap_or("");
    if version != "1" {
        return Err(format!("stamp version \"{version}\" is not one this reads (it reads v1)"));
    }
    let field = |i: usize, name: &str| lines.get(i).and_then(|l| l.strip_prefix(name)).and_then(|l| l.strip_prefix(' '));
    let fingerprint = field(1, "fingerprint").filter(|f| f.len() == 64 && f.bytes().all(|b| b.is_ascii_hexdigit()));
    let count = field(2, "files").and_then(|c| c.parse::<usize>().ok());
    let (Some(fingerprint), Some(count), Some(_), Some(_)) = (fingerprint, count, field(3, "profile"), field(4, "features")) else {
        return Err("the stamp's header lines are not as build.rs writes them".into());
    };
    let start = lines[..5].join("\n").len() + 1;
    let stop = text.rfind(end.as_str()).ok_or("the stamp has no end")?;
    let manifest = text.get(start..stop).ok_or("the stamp is cut short")?;
    let Some(body) = manifest.strip_prefix(MANIFEST_HEADER) else {
        return Err("the stamp has no manifest header".into());
    };
    let mut files = Manifest::new();
    for line in body.split('\n').filter(|l| !l.is_empty()) {
        let (hash, path) = line.split_once(' ').ok_or("a manifest line is malformed")?;
        if hash.len() != 64 || path.is_empty() {
            return Err("a manifest line is malformed".into());
        }
        files.insert(path.to_string(), hash.to_string());
    }
    if files.len() != count {
        return Err(format!("the stamp says {count} files but lists {}", files.len()));
    }
    if sha256_hex(manifest.as_bytes()) != fingerprint {
        return Err("the manifest does not hash to the fingerprint it carries".into());
    }
    Ok(Stamp { fingerprint: fingerprint.to_string(), files })
}

/// The first place `needle` occurs in `hay` at or after `from`.
pub fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() {
        return None;
    }
    let first = needle[0];
    let mut i = from;
    while i + needle.len() <= hay.len() {
        match hay[i..hay.len() - needle.len() + 1].iter().position(|&b| b == first) {
            None => return None,
            Some(p) => {
                let at = i + p;
                if &hay[at..at + needle.len()] == needle {
                    return Some(at);
                }
                i = at + 1;
            }
        }
    }
    None
}

/// The one stamp an exe's bytes carry, or None: no stamp, two stamps, or one that does not
/// parse (none of those can vouch for the exe's code), the same rule as check-delivery's stampOf.
pub fn stamp_in(bytes: &[u8]) -> Option<Stamp> {
    let begin = stamp_begin();
    let end = stamp_end();
    let mut stamps = Vec::new();
    let mut broken = false;
    let mut at = find(bytes, begin.as_bytes(), 0);
    while let Some(start) = at {
        let Some(stop) = find(bytes, end.as_bytes(), start) else {
            broken = true;
            break;
        };
        match std::str::from_utf8(&bytes[start..stop + end.len()]).map_err(|e| e.to_string()).and_then(parse_stamp) {
            Ok(s) => {
                stamps.push(s);
                at = find(bytes, begin.as_bytes(), stop + end.len());
            }
            Err(_) => {
                broken = true;
                at = find(bytes, begin.as_bytes(), start + begin.len());
            }
        }
    }
    if broken || stamps.len() != 1 {
        return None;
    }
    stamps.pop()
}

/// An exe as the verdict sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExeInfo {
    pub exists: bool,
    pub stamp: Option<Stamp>,
}

pub fn exe_info(path: &Path) -> ExeInfo {
    match std::fs::read(path) {
        Ok(bytes) => ExeInfo { exists: true, stamp: stamp_in(&bytes) },
        Err(_) => ExeInfo { exists: path.exists(), stamp: None },
    }
}

/// Files that differ without meaning the operator is missing code (check-delivery's
/// DELIVERY_IGNORES): the PBR shader parts are read from disk at run time, and Cargo.toml and
/// Cargo.lock change with every version bump (the version is compared on its own).
pub fn ignored(rel: &str) -> bool {
    rel.starts_with("assets/shaders/pbr/") || rel == "Cargo.toml" || rel == "Cargo.lock"
}

/// How many files differ between a build's manifest and the tree's, and the first of them.
pub fn code_diff(exe: &Manifest, tree: &Manifest) -> (usize, Option<String>) {
    let mut changed = Vec::new();
    let mut only_tree = Vec::new();
    for (path, hash) in tree {
        match exe.get(path) {
            None => only_tree.push(path),
            Some(h) if h != hash => changed.push(path),
            _ => {}
        }
    }
    let only_exe: Vec<&String> = exe.keys().filter(|p| !tree.contains_key(*p)).collect();
    let keep = |v: Vec<&String>| -> Vec<String> { v.into_iter().filter(|p| !ignored(p)).cloned().collect() };
    let (changed, only_tree, only_exe) = (keep(changed), keep(only_tree), keep(only_exe));
    let first = changed.first().or(only_tree.first()).or(only_exe.first()).cloned();
    (changed.len() + only_tree.len() + only_exe.len(), first)
}

/// What can be wrong with the delivery, in check-delivery's words.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// NOT DELIVERED: HumanityOS.exe does not exist.
    TaskbarMissing,
    /// NO STAMP: HumanityOS.exe carries no source fingerprint.
    TaskbarNoStamp,
    /// NOT BUILT: the taskbar exe's code differs and target/release does not hold the tree's either.
    NotBuilt { differ: usize, first: String },
    /// NOT DELIVERED: target/release holds the tree's code and the taskbar exe does not.
    NotDelivered { differ: usize, first: String },
    /// NOT BUILT: target/release/HumanityOS.exe does not exist.
    BuiltMissing,
    /// NOT BUILT: target/release does not hold the tree's code either.
    BuiltStale,
}

/// check-delivery's judgeDelivery. Pure. Empty = the taskbar exe holds the tree's code.
pub fn judge(stable: &ExeInfo, built: &ExeInfo, tree: &Manifest) -> Vec<Problem> {
    let built_current = built.exists && built.stamp.as_ref().is_some_and(|s| code_diff(&s.files, tree).0 == 0);
    let mut problems = Vec::new();
    if !stable.exists {
        problems.push(Problem::TaskbarMissing);
    } else if let Some(stamp) = &stable.stamp {
        let (differ, first) = code_diff(&stamp.files, tree);
        if differ == 0 {
            return problems;
        }
        let first = first.unwrap_or_default();
        problems.push(if built_current { Problem::NotDelivered { differ, first } } else { Problem::NotBuilt { differ, first } });
        return problems;
    } else {
        problems.push(Problem::TaskbarNoStamp);
    }
    if !built.exists {
        problems.push(Problem::BuiltMissing);
    } else if !built_current {
        problems.push(Problem::BuiltStale);
    }
    problems
}

/// The whole answer for a checkout.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub problems: Vec<Problem>,
    /// The version the taskbar exe's title bar reads (HumanityOS.exe.build.json), when known.
    pub taskbar_version: Option<String>,
}

/// Read both exes and the tree, and judge. Seconds of work at most: run it off the frame.
pub fn check(checkout: &Path) -> Result<Delivery, String> {
    let tree = fingerprint_tree(checkout)?;
    let stable = exe_info(&checkout.join("HumanityOS.exe"));
    let built = exe_info(&checkout.join("target").join("release").join("HumanityOS.exe"));
    let taskbar_version = std::fs::read_to_string(checkout.join("HumanityOS.exe.build.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(str::to_string));
    Ok(Delivery { problems: judge(&stable, &built, &tree), taskbar_version })
}
