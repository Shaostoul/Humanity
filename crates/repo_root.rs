//! The build script every crate under crates/ shares (`build = "../repo_root.rs"` in its Cargo.toml):
//! it names the repo root for the crate's code as `env!("HUMANITY_REPO_ROOT")`, the one way the Rust
//! source names it (the crate split's rung 1, docs/design/crate-split.md; tests/repo_root_lint.rs holds
//! it). A member's own CARGO_MANIFEST_DIR is its own folder (crates/<name>), so the root is the nearest
//! folder above it whose Cargo.toml holds `[workspace]`: found, never assumed two levels up. The root
//! package's build.rs sets the same variable to its own folder, which is that root.
//!
//! Why a build script and not the one alternative cargo offers, an `[env]` entry with `relative = true`
//! in .cargo/config.toml: that file is machine-local here (scripts/build-slot.js writes it in the main
//! checkout only), so a worktree would read the MAIN checkout's config and every test in it would read
//! the main checkout's data/ (silently, and green), and CI's fresh checkout has no such file at all.

use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let mut dir = Path::new(&manifest);
    let root = loop {
        let toml = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default();
        if toml.lines().any(|l| l.trim() == "[workspace]") {
            break dir;
        }
        dir = dir.parent().unwrap_or_else(|| panic!("no Cargo.toml with a [workspace] table above {manifest}"));
    };
    println!("cargo:rustc-env=HUMANITY_REPO_ROOT={}", root.display());
    // The folder never moves under a build, so only this file's own change reruns it.
    println!("cargo:rerun-if-changed=../repo_root.rs");
}
