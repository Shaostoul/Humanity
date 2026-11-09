//! THE NVIDIA SIMREADY MODELS SHIP WITH THEIR CREDIT, OR NOT AT ALL (2026-11-09;
//! docs/design/simready-props.md, sections 8 and 11; the conditions are the dated finding's,
//! docs/reference/findings/2026-11-08-nvidia-simready-assets.md).
//!
//! These are the project's first 3D models that are not CC0. CC BY 4.0 makes the credit a
//! condition (failing it ends the licence until cured, section 6), so this lint holds, for every
//! folder under `assets/models/simready/`:
//!   * it holds its model, NVIDIA's `LICENSE.txt` byte for byte (the CC BY 4.0 legal code, checked
//!     by SHA-256), a `SOURCE.md` naming NVIDIA, the licence, its link and the folder's package, and
//!     its `physics.ron`;
//!   * the set's manifest lists it, with a brand check verdict of `clear` or `repaint` (a maker's
//!     name, logo or a protected emblem never ships unexamined);
//! and for the set: no MDL shader file anywhere (NVIDIA's BSD-style terms, never downloaded), no
//! folder named after another company's trademarked product (WD-40, Rubik's Cube), the
//! `nvidia_simready` row in `data/credits.ron` required and shown somewhere, CREDITS.md and
//! LICENSES.md naming the source, the folder under its 96 MiB cap, and every catalogue row's
//! SimReady model path pointing at a model that exists.
//!
//! RED FIRST (2026-11-09), each check seen failing on purpose before the set was trusted:
//!   * one LICENSE.txt with a byte changed: "toilet_a01: LICENSE.txt is not the CC BY 4.0 legal code";
//!   * one SOURCE.md without its package id: "...: SOURCE.md does not name its package";
//!   * an empty `OmniPBR.mdl` dropped into a folder: "an MDL shader ships";
//!   * the credits row's `attribution_required` set to false: "the nvidia_simready credit is not
//!     marked required";
//!   * the cap lowered to 1 MiB: "assets/models/simready/ is 53.3 MiB, over its 1 MiB cap".
//! The same run caught the lint itself: it read the credits row only up to the first "),", which
//! the row's own text holds ("a downloadable pack),"), so a correct row failed; it now reads to the
//! next row.
//!
//! Std-only, compiled standalone like the other lints (`just lints`):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/simready_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// SHA-256 of Creative Commons' CC BY 4.0 legal code (the finding, F2 and F3).
const CC_BY_40_SHA256: &str = "9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411";
/// The folder's size cap (design section 7).
const CAP_BYTES: u64 = 96 * 1024 * 1024;
/// Never shipped: products named after another company's trademark (the finding, F3).
const DENY: &[&str] = &["wd40", "rubikscube"];

/// SHA-256 (FIPS 180-4), std-only, for the licence check.
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// The value of `"key": "..."` in a JSON object's text, from `from` on.
fn json_str<'a>(text: &'a str, key: &str, from: usize) -> Option<(&'a str, usize)> {
    let pat = format!("\"{key}\": \"");
    let i = text[from..].find(&pat)? + from + pat.len();
    let j = text[i..].find('"')? + i;
    Some((&text[i..j], j))
}

#[test]
fn sha256_is_right() {
    assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn simready_models_ship_with_their_credit() {
    let root = repo().join("assets").join("models").join("simready");
    let manifest = fs::read_to_string(root.join("manifest.json")).expect("assets/models/simready/manifest.json");
    let mut failures = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&root).expect("the folder").flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no models under {}", root.display());
    for dir in &dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        if DENY.iter().any(|d| name.contains(d)) {
            failures.push(format!("{name}: named after another company's trademarked product"));
        }
        for f in [format!("{name}.glb"), "LICENSE.txt".into(), "SOURCE.md".into(), "physics.ron".into()] {
            if !dir.join(&f).exists() {
                failures.push(format!("{name}: no {f}"));
            }
        }
        if let Ok(lic) = fs::read(dir.join("LICENSE.txt")) {
            if sha256(&lic) != CC_BY_40_SHA256 {
                failures.push(format!("{name}: LICENSE.txt is not the CC BY 4.0 legal code"));
            }
        }
        // The manifest entry for this folder: its package and its brand check.
        let key = format!("\"dir\": \"{name}\"");
        let Some(at) = manifest.find(&key) else {
            failures.push(format!("{name}: not in manifest.json"));
            continue;
        };
        let start = manifest[..at].rfind('{').unwrap_or(0);
        let package = json_str(&manifest, "package_id", start).map(|(v, _)| v.to_string()).unwrap_or_default();
        match json_str(&manifest, "brand_check", start).map(|(v, _)| v) {
            Some("clear") | Some("repaint") => {}
            other => failures.push(format!("{name}: brand check {other:?}, not clear or repaint")),
        }
        if let Ok(src) = fs::read_to_string(dir.join("SOURCE.md")) {
            for must in ["NVIDIA", "CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/", "https://huggingface.co/datasets/nvidia/simready-assets"] {
                if !src.contains(must) {
                    failures.push(format!("{name}: SOURCE.md does not say {must}"));
                }
            }
            if package.is_empty() || !src.contains(&package) {
                failures.push(format!("{name}: SOURCE.md does not name its package"));
            }
        }
    }
    let mut all = Vec::new();
    files_under(&root, &mut all);
    files_under(&repo().join("data").join("models"), &mut all);
    if all.iter().any(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("mdl"))) {
        failures.push("an MDL shader ships".into());
    }
    let bytes: u64 = all.iter().filter(|p| p.starts_with(&root)).filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
    if bytes > CAP_BYTES {
        failures.push(format!("assets/models/simready/ is {:.1} MiB, over its {} MiB cap", bytes as f64 / 1048576.0, CAP_BYTES / 1048576));
    }
    let credits = fs::read_to_string(repo().join("data").join("credits.ron")).expect("data/credits.ron");
    match credits.find("id: \"nvidia_simready\"") {
        None => failures.push("no nvidia_simready row in data/credits.ron".into()),
        Some(i) => {
            // The row runs to the next row's id (its text may hold "),", as "a downloadable pack),"
            // does): the first version cut it there and missed both fields below.
            let after = i + "id: \"nvidia_simready\"".len();
            let row = &credits[i..credits[after..].find("id: \"").map(|j| after + j).unwrap_or(credits.len())];
            if !row.contains("attribution_required: true") {
                failures.push("the nvidia_simready credit is not marked required".into());
            }
            if !row.contains("shown_in: [\"Credits\"") {
                failures.push("the nvidia_simready credit is not shown on the Credits page".into());
            }
        }
    }
    for doc in ["CREDITS.md", "LICENSES.md", "assets/models/LICENSE-nvidia-simready.md"] {
        let text = fs::read_to_string(repo().join(doc)).unwrap_or_default();
        if !text.contains("NVIDIA SimReady Assets") || !text.contains("CC BY 4.0") {
            failures.push(format!("{doc} does not credit NVIDIA SimReady Assets under CC BY 4.0"));
        }
    }
    // Every catalogue row's in-app SimReady model exists.
    let mut rons = Vec::new();
    files_under(&repo().join("data").join("machines"), &mut rons);
    for ron in rons.iter().filter(|p| p.extension().and_then(|x| x.to_str()) == Some("ron")) {
        let text = fs::read_to_string(ron).unwrap_or_default();
        for (i, _) in text.match_indices("\"assets/models/simready/") {
            let path = &text[i + 1..i + 1 + text[i + 1..].find('"').unwrap_or(0)];
            if !repo().join(path).exists() {
                failures.push(format!("{}: names {path}, which does not exist", ron.display()));
            }
        }
    }
    assert!(failures.is_empty(), "{} problem(s):\n  {}", failures.len(), failures.join("\n  "));
}
