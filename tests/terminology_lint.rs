//! Terminology lint (2026-09-29): the words people read about their account.
//!
//! Operator decisions, 2026-09-29:
//!
//!   * **"recovery phrase", never "seed phrase".** The 24 words that back up an
//!     account are its recovery phrase. "Seed" is kept for plant seeds and
//!     world seeds, so a newcomer never meets one word meaning two things.
//!     (The in-app dictionary's Recovery Phrase entry is the one place that
//!     says crypto wallets call it a seed phrase, for people who have heard
//!     that name elsewhere.)
//!   * **"no sign-up", never "no account(s)".** People do have an account:
//!     it is theirs, a key on their own device, and the app says "your
//!     account" (export, erase). What they do not do is sign up with anyone:
//!     no email, no phone number, no password held by a company. Saying "no
//!     account" also hides the one thing they must know, that only their
//!     recovery phrase gets them back in.
//!
//! Scans what people read: the native UI (`src/gui/`), the website (`web/`),
//! the help, dictionary, onboarding and wallet data, the user, website,
//! mission and outreach docs (not posts already published or applications
//! already sent: those are records), and the README. Code names, element ids
//! and saved-file fields (`seed_phrase_visible`, `seed-phrase-overlay`) are
//! not words people read and do not match: only the spaced phrase does.
//!
//! Escape hatch: `terminology-exempt` on the line, for a quotation or a
//! sentence that is genuinely about something else.
//!
//! Std-only, compiled standalone like the other lints:
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/terminology_lint.rs

use std::fs;
use std::path::{Path, PathBuf};

const EXEMPT: &str = "terminology-exempt";

/// Directories and files scanned, relative to the repo root.
const SCAN: &[&str] = &[
    "src/gui",
    "web",
    "data/help",
    "data/glossary.json",
    "data/onboarding",
    "data/wallet",
    "data/ai/onboarding.json",
    "data/admin/ops_registry.json",
    "data/language/acronyms.json",
    "docs/user",
    "docs/website",
    "docs/mission",
    "docs/outreach",
    "README.md",
    "CREDITS.md",
];
/// Phrases where "seed" can only mean the account's key (2026-10-10). Kept specific so a plant's
/// or a world's seed never matches.
const ACCOUNT_SEED: &[&str] = &[
    "24-word seed",
    "12-word seed",
    "splits your seed",
    "existing seed",
    "recover from seed",
    "restore from seed",
    "your seed is",
    "contains your seed",
    "holds your seed",
    "your current seed",
    "back up your seed",
    "backup your seed",
];
/// Records, not copy: what was posted or sent stays as it was.
const SKIP: &[&str] = &["docs/outreach/posts", "docs/outreach/applications"];

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn wanted(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("rs") | Some("html") | Some("js") | Some("json") | Some("md") | Some("ron") | Some("csv")
    )
}

fn walk(root: &Path, p: &Path, out: &mut Vec<PathBuf>) {
    let rel = p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
    if SKIP.iter().any(|s| rel == *s || rel.starts_with(&format!("{s}/"))) {
        return;
    }
    if p.is_dir() {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                walk(root, &e.path(), out);
            }
        }
    } else if wanted(p) {
        out.push(p.to_path_buf());
    }
}

/// "no account" or "no accounts" as words (not "no accountability").
fn says_no_account(lower: &str) -> bool {
    let mut from = 0;
    while let Some(i) = lower[from..].find("no account") {
        let end = from + i + "no account".len();
        let rest = &lower[end..];
        let next = rest.chars().next();
        let plural = rest.starts_with('s') && !rest[1..].chars().next().map_or(false, |c| c.is_alphabetic());
        if plural || !next.map_or(false, |c| c.is_alphabetic()) {
            return true;
        }
        from = end;
    }
    false
}

#[test]
fn account_words_are_the_plain_ones() {
    let root = project_root();
    let mut files = Vec::new();
    for s in SCAN {
        walk(&root, &root.join(s), &mut files);
    }
    assert!(files.len() > 50, "the lint found only {} files: is CARGO_MANIFEST_DIR the repo?", files.len());
    let mut bad: Vec<String> = Vec::new();
    for f in &files {
        let Ok(text) = fs::read_to_string(f) else { continue };
        for (n, line) in text.lines().enumerate() {
            if line.contains(EXEMPT) {
                continue;
            }
            let lower = line.to_lowercase();
            let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
            // The dictionary's one deliberate mention of the other name.
            let alias = lower.contains("wallets call this a seed phrase");
            if !alias && (lower.contains("seed phrase") || lower.contains("seed-phrase ")) {
                bad.push(format!("{rel}:{}: \"seed phrase\" -> say \"recovery phrase\"", n + 1));
            }
            // The chat command was renamed with the phrase (/seed -> /recovery);
            // a tip that still names the old one sends people to "Unknown
            // command" (review of 2026-09-29).
            if lower.contains("<code>/seed</code>") || lower.contains("'/seed'") || lower.contains("\"/seed\"") {
                bad.push(format!("{rel}:{}: the chat command is /recovery now, not /seed", n + 1));
            }
            // "Seed" alone for the account key (2026-10-10): the Settings page still said "a fresh
            // 24-word seed" and "recover an existing seed" after "seed phrase" was retired. These
            // phrases only ever mean the account's key, never a plant or a world seed.
            if !alias && ACCOUNT_SEED.iter().any(|p| lower.contains(p)) {
                bad.push(format!("{rel}:{}: \"seed\" for the account key -> say \"recovery phrase\" (or \"key\")", n + 1));
            }
            if says_no_account(&lower) {
                bad.push(format!("{rel}:{}: \"no account\" -> say \"no sign-up\" (people do have an account; it is theirs)", n + 1));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "\n\n[FAIL] {} line(s) use account words the operator retired (2026-09-29):\n  {}\n\n\
         \"recovery phrase\", never \"seed phrase\" (seed means plant and world seeds);\n\
         \"no sign-up\", never \"no account\". A genuine quotation or an unrelated use:\n\
         put `terminology-exempt` on the line.\n",
        bad.len(),
        bad.join("\n  ")
    );
}

#[test]
fn the_no_account_matcher_reads_words() {
    assert!(says_no_account("no account needed"));
    assert!(says_no_account("no accounts, no tracking"));
    assert!(says_no_account("there is no account."));
    assert!(!says_no_account("no accountability here"));
    assert!(!says_no_account("your account export"));
}
