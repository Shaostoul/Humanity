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
/// first (the embedded copies in crates/humanity-core/src/embedded_data.rs and the loaders' own
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
///   data/reactions.json,
///   data/performance/budget_systems.ron   the same: embedded-only registries
///   data/fonts/NotoSans-Regular.ttf       the UI font
///   data/release/signing_pubkeys.json     the keys updates and local hand-offs
///                                         are verified against
///   docs/accord/humanity_accord.md        the Accord the Humanity page shows
///   data/server/default_channels.json     the channels every server makes (2026-11-02)
///   data/characters/details.ron, data/characters/garments.ron, data/cosmetics/cosmetics.csv
///                        the look's ids (2026-11-10): the one clamping reader of a player's look
///                        (humanity_core::player_look::LookIds::shipped) keeps only the sliders,
///                        garments and cosmetics its build shipped, at the relay and in the app alike
///   data/characters/interactions.ron
///                        the anchors' ids (2026-11-10): the one clamping reader of a movement
///                        message's act (humanity_core::stance::AnchorIds::shipped) keeps only the
///                        anchors its build shipped
///   data/homes/shipped   the shipped home designs: a neighbour's plot is drawn as its
///                        kind's built-in design, never the disk copy, so that read is
///                        embedded-only (ship homes increment 2, crates/humanity-ship/src/neighbours.rs,
///                        HomeDesign::built_in). Only this folder: data/homes/<kind>.ron
///                        is the player's OWN home, which every editor Save rewrites, read
///                        from disk first; stamping it made a Save in a repo checkout turn
///                        every rig away as stale (the increment 2 review, finding 5)
const FINGERPRINT_INPUTS: &[&str] = &[
    "src",
    "assets/shaders",
    // The workspace's crates (the crate split's rung 1, 2026-11-01): the core and the protocol
    // are compiled into the binary as surely as src/ is, so a stamp without them would call a
    // binary current after an edit to either.
    "crates",
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
    "data/reactions.json",
    "data/performance/budget_systems.ron",
    "data/fonts/NotoSans-Regular.ttf",
    "data/release/signing_pubkeys.json",
    "docs/accord/humanity_accord.md",
    // The admin action registry (src/relay/admin/mod.rs): built in on purpose, so the actions a
    // relay runs, and the role each needs, are its build's and not a file edited on its disk.
    "data/admin/actions.json",
    // The bug reporter's severities and categories (src/relay/bug_taxonomy.rs): built in for the
    // same reason, so a relay accepts the list it was built with, never a disk edit.
    "data/bugs/taxonomy.json",
    // The channels every server makes for itself (src/relay/default_channels.rs, 2026-11-02):
    // built in for the same reason, so a server makes the set its build was made with.
    "data/server/default_channels.json",
    // The AI catalog (src/ai/catalog.rs, embeddable AI rung 2, 2026-11-09): compiled in on purpose
    // and never read from the data folder, so the signed release vouches for every hash of the AI
    // files it may download and a mod cannot point it at another model.
    "data/ai/catalog.ron",
    // The look's ids (the look, 2026-11-10): the one clamping reader of a player's look
    // (crates/humanity-core/src/player_look.rs, LookIds::shipped) is compiled from these on purpose,
    // so the relay and every app keep exactly the sliders, garments and cosmetics their build
    // shipped, never a disk edit; an id outside them reads as the default.
    "data/characters/details.ron",
    "data/characters/garments.ron",
    "data/cosmetics/cosmetics.csv",
    // The anchors' ids (character animation rung 2b, 2026-11-10): the one clamping reader of a
    // movement message's act (crates/humanity-core/src/stance.rs, AnchorIds::shipped) is compiled
    // from this on purpose, so the relay and every app keep exactly the anchors their build shipped;
    // an id outside them reads as using nothing.
    "data/characters/interactions.ron",
    // The garment texture packs' catalog (src/gui/garment_textures/packs.rs, 2026-11-10): compiled in
    // on purpose, the AI catalog's way, so the signed release vouches for each pack's size and
    // SHA-256 and a mod editing the data folder cannot point the app at other files.
    "data/characters/garment_packs.ron",
    // Settings > Graphics' rows (src/gui/pages/settings/graphics.rs, 2026-11-10): each control's
    // words keys and how its cost is known, compiled in like the Performance page's registry.
    "data/gui/graphics_settings.ron",
];

fn main() {
    // The repo root, named ONE way in the Rust source: env!("HUMANITY_REPO_ROOT") (the crate
    // split's rung 1, 2026-11-01; tests/repo_root_lint.rs holds it). In a workspace each member's
    // CARGO_MANIFEST_DIR is its own folder; this package's IS the repo root, and every crate under
    // crates/ sets the same variable through crates/repo_root.rs.
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    println!("cargo:rustc-env=HUMANITY_REPO_ROOT={root}");

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
