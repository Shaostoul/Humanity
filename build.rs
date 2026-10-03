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
/// data/ is deliberately absent: the game reads data/ from disk first (the
/// embedded copies in src/embedded_data.rs are only a fallback for a bare exe)
/// and the probe rig junctions data/ live, so a data edit needs no rebuild.
/// Shaders are here because the first pipeline compile uses the include_str!
/// copies. Cargo.lock is here because a dependency bump changes the binary as
/// surely as a source edit does.
const FINGERPRINT_INPUTS: &[&str] = &["src", "assets/shaders", "Cargo.toml", "Cargo.lock", "build.rs"];

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
    // winres compiles the icon into the exe; once any rerun-if-changed line is
    // printed cargo stops re-running this script on unlisted files, so list it.
    println!("cargo:rerun-if-changed=assets/icon.ico");
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
