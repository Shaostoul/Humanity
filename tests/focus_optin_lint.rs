//! HUMANITY_TAKE_FOCUS is the operator's proof-of-human-launch (v0.1081,
//! src/engine/launch_focus.rs). If a script, agent definition, or workflow
//! ever sets it, an agent boot can steal the operator's focus again and the
//! whole inversion is undone. This lint pins the allowlist: the env var may
//! appear ONLY in the places that define, honour, document, or legitimately
//! propagate it.
//!
//! Std-only, compiled standalone like the other lints:
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/focus_optin_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

const NEEDLE: &str = "HUMANITY_TAKE_FOCUS";

/// Files where the string is allowed to appear.
const ALLOWED: &[&str] = &[
    "Justfile",                      // just play / just launch (operator recipes)
    "src/engine/launch_focus.rs",    // the policy itself
    "src/lib.rs",                    // call-site comment
    "src/updater.rs",                // restart script propagates current state
    "CLAUDE.md",                     // documents the rule
    "tests/focus_optin_lint.rs",     // this lint
];

/// Directory names never worth scanning.
const SKIP_DIRS: &[&str] = &[
    ".git", "target", "node_modules", ".probe-rig", ".claude", "app",
    "docs", "data",
];

// docs/ and data/ are excluded from the scan because prose mentioning the
// var is harmless (nothing executes it); .claude is excluded EXCEPT that
// agent/workflow definitions are exactly where a violation would be most
// dangerous, so those two subtrees are scanned explicitly in the test.

fn scan(dir: &Path, hits: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            scan(&path, hits);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("rs" | "js" | "mjs" | "sh" | "bat" | "ps1" | "py" | "toml" | "json" | "yml" | "yaml" | "md")
        ) || name == "Justfile"
        {
            if let Ok(text) = fs::read_to_string(&path) {
                if text.contains(NEEDLE) {
                    hits.push(path);
                }
            }
        }
    }
}

#[test]
fn take_focus_appears_only_in_sanctioned_files() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = Vec::new();
    scan(repo, &mut hits);
    // The dangerous .claude subtrees: agent prompts and workflow scripts.
    scan(&repo.join(".claude/agents"), &mut hits);
    scan(&repo.join(".claude/workflows"), &mut hits);

    let violations: Vec<String> = hits
        .iter()
        .filter_map(|p| {
            let rel = p
                .strip_prefix(repo)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            (!ALLOWED.contains(&rel.as_str())).then_some(rel)
        })
        .collect();

    assert!(
        violations.is_empty(),
        "\n\nHUMANITY_TAKE_FOCUS found outside the allowlist:\n  {}\n\n\
         That env var is the operator's proof-of-human-launch. A script or \
         agent that sets it can steal the operator's focus, undoing the \
         v0.1081 inversion. Scripts wanting a quiet boot need NOTHING (background \
         is the default for script launches); the operator's own interactive \
         launches go through `just play` / `just launch`.\n",
        violations.join("\n  ")
    );
}
