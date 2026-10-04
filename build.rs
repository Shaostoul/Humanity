use sha2::{Digest, Sha256};
use std::path::Path;

/// The sources that are COMPILED INTO the binary, for the source stamp below.
///
/// This is the ONE definition of that list. `scripts/check-fresh-exe.js` (the
/// gate every rig runs before it boots the exe) reads it out of THIS FILE
/// through `scripts/lib/src-fingerprint.js`, so the stamp and the check can
/// never disagree about what counts. Change the list here and nowhere else;
/// keep it a flat array of plain string literals on one statement, because the
/// gate parses it with a regex and refuses (loudly) when it cannot.
///
/// Entries are paths relative to the crate root: a directory is walked
/// recursively (every regular file in it counts, compiled or not, so a stray
/// file shows up as a difference rather than hiding one), a file is a file, and
/// a missing entry is skipped on both sides.
///
/// data/ as a whole is deliberately absent: the game reads data/ from disk
/// first (the embedded copies in src/embedded_data.rs and the loaders' own
/// include_str! fallbacks only serve a bare exe) and the probe rig junctions
/// data/ live, so a data edit needs no rebuild. Shaders are here because the
/// first pipeline compile uses the include_str! copies. Cargo.lock is here
/// because a dependency bump changes the binary as surely as a source edit
/// does.
///
/// After those five, the files compiled in with NO disk read on any path, so
/// that the binary's behaviour changes only with a rebuild (BUG-133, the third
/// gap: before they were listed, editing one left the stamp, and so the gate,
/// saying "current" for a binary that still carried the old file). Every
/// include_str!/include_bytes! in src/ is classified by
/// scripts/lib/compiled-in.js, and scripts/tests/compiled-in.test.js fails on
/// one that is neither listed here, disk-first data, test-only, nor allowlisted
/// with a reason. The same check reads Cargo.toml: every `path = ".."` there
/// (a path dependency, a [patch] entry) is Rust source compiled into the
/// binary and must sit under an entry here. As of 2026-10-03:
///   vendor               rav1d, the AV1 decoder, patched for BUG-093 and
///                        compiled in through [patch.crates-io]; Cargo.lock
///                        carries no hash for a path dependency, so without
///                        this an edit to vendor/rav1d left the stamp current
///   assets/icon.ico      the exe's own icon (winres, below)
///   assets/icon.png      the window icon (lib.rs)
///   data/blueprints/*    the ship-structure registries (structure, zone, zone
///                        filler, extrusion, road and lock types, wall
///                        materials, opening styles): parsed from the embedded
///                        copy only, and they shape the ship every rig photographs
///   data/lighting/light_types.ron, data/lod/categories.ron,
///   data/utilities/conduits.ron, data/reactions.json,
///   data/performance/budget_systems.ron   the same: embedded-only registries
///   data/fonts/NotoSans-Regular.ttf       the UI font
///   data/release/signing_pubkeys.json     the keys updates and local hand-offs
///                                         are verified against
///   docs/accord/humanity_accord.md        the Accord the Humanity page shows
///   data/homes/shipped   the shipped home designs: a neighbour's plot is drawn as its
///                        kind's built-in design, never the disk copy, so that read is
///                        embedded-only (ship homes increment 2, src/ship/neighbours.rs,
///                        HomeDesign::built_in). Only this folder: data/homes/<kind>.ron
///                        is the player's OWN home, which every editor Save rewrites, read
///                        from disk first; stamping it made a Save in a repo checkout turn
///                        every rig away as stale (the increment 2 review, finding 5)
const FINGERPRINT_INPUTS: &[&str] = &[
    "src",
    "assets/shaders",
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "vendor",
    "assets/icon.ico",
    "assets/icon.png",
    "data/blueprints/extrusion_profiles.ron",
    "data/homes/shipped",
    "data/blueprints/lock_types.ron",
    "data/blueprints/opening_styles.ron",
    "data/blueprints/road_types.ron",
    "data/blueprints/structure_types.ron",
    "data/blueprints/wall_materials.ron",
    "data/blueprints/zone_filler.ron",
    "data/blueprints/zone_types.ron",
    "data/lighting/light_types.ron",
    "data/lod/categories.ron",
    "data/utilities/conduits.ron",
    "data/reactions.json",
    "data/performance/budget_systems.ron",
    "data/fonts/NotoSans-Regular.ttf",
    "data/release/signing_pubkeys.json",
    "docs/accord/humanity_accord.md",
];

fn main() {
    // Set BUILD_VERSION for the relay module (git hash + timestamp). Display
    // only (/api/stats, /api/server-info, the admin System panel). Since the
    // rerun-if-changed lines below, it refreshes when a compiled-in source
    // changes, not on every edit anywhere in the package.
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    println!("cargo:rustc-env=BUILD_VERSION={}-{}", hash.trim(), timestamp);

    write_source_stamp();

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        // Version info embedded in the exe properties
        res.set("ProductName", "HumanityOS");
        res.set("FileDescription", "HumanityOS - End poverty, unite humanity");
        res.set("LegalCopyright", "Public Domain (CC0)");
        if let Err(e) = res.compile() {
            eprintln!("winres error: {}", e);
        }
    }
}

// ── The source stamp (BUG-133) ───────────────────────────────────────────────
//
// What failed: the freshness gate judged a binary by file DATES (newer than
// every compiled-in source = current). A binary built from a DIFFERENT tree (main
// tested in a worktree, or the reverse) passed whenever its date happened to be
// newer, so a rig could report a green run of code that was not in the tree.
//
// The fix: hash the CONTENT of every compiled-in source here, at build time, and
// compile the result into the binary (src/main.rs `SOURCE_STAMP`). The gate
// recomputes the same hash from the tree it runs in and compares. Dates play no
// part, so a build from another tree cannot pass by being newer.
//
// The definition, mirrored byte for byte by scripts/lib/src-fingerprint.js (its
// tests pin the two together on a fixed input):
//   every regular file under FINGERPRINT_INPUTS, as a path relative to the crate
//   root with '/' separators, sorted by the UTF-8 bytes of that path;
//   each file's content with every CRLF turned into LF (so a checkout's line
//   endings do not matter), hashed with SHA-256;
//   manifest = "hos-src-manifest v1\n" + one "<sha256 hex> <path>\n" line per file;
//   fingerprint = SHA-256 hex of the manifest.
// The whole manifest goes into the binary too (about 60 KB of text, never read
// at runtime), so on a mismatch the gate can say WHICH files differ: one or two
// means "edited after the build", hundreds means "a build of another tree".
//
// Why the stamp cannot go stale: cargo re-runs this script whenever anything
// under FINGERPRINT_INPUTS changes (the rerun-if-changed lines), and the gate
// recomputes the tree side fresh every time. If cargo misses an edit (a file
// copied in with its old date), the binary AND its stamp are old together, and
// the gate, which hashes what is on disk now, refuses.
fn write_source_stamp() {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let root = Path::new(&root);

    let mut files: Vec<String> = Vec::new();
    for input in FINGERPRINT_INPUTS {
        collect(root, input, &mut files);
        // Only for inputs that exist: cargo treats a missing path as always
        // changed, which would re-run this script (and relink) on every build.
        if root.join(input).exists() {
            println!("cargo:rerun-if-changed={}", input);
        }
    }
    // assets/icon.ico (which winres compiles into the exe) is in the list, so the
    // loop above already asks cargo to re-run this script when it changes.
    files.sort();
    files.dedup();

    let mut manifest = String::from("hos-src-manifest v1\n");
    for rel in &files {
        let bytes = std::fs::read(root.join(rel))
            .unwrap_or_else(|e| panic!("source stamp: cannot read {rel}: {e}"));
        let digest = Sha256::digest(crlf_to_lf(&bytes));
        manifest.push_str(&hex(&digest));
        manifest.push(' ');
        manifest.push_str(rel);
        manifest.push('\n');
    }
    let fingerprint = hex(&Sha256::digest(manifest.as_bytes()));

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into());
    let mut features: Vec<String> = std::env::vars()
        .filter_map(|(k, _)| k.strip_prefix("CARGO_FEATURE_").map(|f| f.to_lowercase()))
        .collect();
    features.sort();
    let features = if features.is_empty() { "-".to_string() } else { features.join(",") };

    // The layout scripts/lib/src-fingerprint.js `parseStamp` reads. The two
    // marker lines are what it searches the exe's bytes for; nothing else in
    // the binary may contain them (src/main.rs only include_str!s this file).
    let stamp = format!(
        "HOS-SRC-STAMP v1\nfingerprint {fingerprint}\nfiles {}\nprofile {profile}\nfeatures {features}\n{manifest}HOS-SRC-STAMP END\n",
        files.len()
    );

    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let path = Path::new(&out).join("hos_src_stamp.txt");
    // Rewrite only on a change, so an unchanged stamp keeps its date and does
    // not by itself make the bin look dirty to cargo.
    if std::fs::read_to_string(&path).ok().as_deref() != Some(stamp.as_str()) {
        std::fs::write(&path, stamp).expect("source stamp: cannot write OUT_DIR/hos_src_stamp.txt");
    }
}

/// Every regular file at or under `rel`, as '/'-separated crate-relative paths.
/// Symlinks and junctions are skipped (not followed), the same as the gate.
fn collect(root: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(meta) = std::fs::symlink_metadata(root.join(rel)) else { return };
    if meta.is_dir() {
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = format!("{rel}/{name}");
            match entry.file_type() {
                Ok(t) if t.is_dir() => collect(root, &child, out),
                Ok(t) if t.is_file() => out.push(child),
                _ => {}
            }
        }
    } else if meta.is_file() {
        out.push(rel.to_string());
    }
}

/// CRLF -> LF; a lone CR is kept (it is not a line ending git converts).
fn crlf_to_lf(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
            i += 1;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}
