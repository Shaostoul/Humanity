//! Every role check asks the one helper (BUG-332's sweep, 2026-11-20).
//!
//! The server's owner (role `owner`) counts as an admin everywhere, and a moderator is spelled
//! `mod` or `moderator`. The relay checked `role == "admin"` (or admin or mod) on its own in some
//! forty places, and both apps hid controls the same way, so the owner was refused or shown less
//! than an admin while other places let her through. The rule now lives in two helpers per side:
//! the relay's `is_admin_role` and `is_staff_role` (src/relay/handlers/msg_handlers.rs), the
//! desktop app's (src/net/roles.rs); the website's twin is web/shared/roles.js
//! (scripts/tests/roles-web.test.js holds the pages to it).
//!
//! This lint fails on a role name compared on its own anywhere in the Rust sources: `== "admin"`,
//! `!= "mod"`, `"owner" ==`, or a role name inside `matches!(..)`, for each of admin, owner, mod
//! and moderator. A role's own NAME is not a gate and stays a plain string: a `match` arm
//! (`"admin" => "A"`, a badge or a rank), the value a role is set to (`set_role(key, "admin")`),
//! a SQL value; and the few places a role's name is compared for what it IS rather than who it
//! lets in, each listed in NAMES with its reason. Test code is not scanned (`*_tests.rs`,
//! `tests.rs`, `crossing_tests/`, and the `#[cfg(test)] mod .. { .. }` blocks): a test compares
//! what it set.
//!
//! Seen red 2026-11-20 before the sweep: it listed the gates the brief named (api.rs,
//! api_market.rs, api_v2_agents.rs, relay.rs's slash commands and message arms, chat_gate.rs,
//! chat_rate.rs, reach.rs, stream.rs) and the app's (chat/left_panel.rs, line_limit.rs,
//! modals.rs, rows.rs, files/shared_files.rs), and the checks that already counted the owner but
//! by a copy of the rule (services.rs, dm_rate.rs, reports.rs, server_settings_update.rs,
//! files.rs, backup_stream/api.rs, mod.rs, moderation.rs, moderation_held.rs, relay.rs's owner
//! arms, chat/commons.rs, pages/files.rs, server_settings.rs, net/admin_erase.rs).

use std::fs;
use std::path::{Path, PathBuf};

const ROLES: [&str; 4] = ["admin", "owner", "mod", "moderator"];

/// The helpers themselves: the only places the rule is written out.
const HELPERS: [(&str, &str); 4] = [
    ("src/relay/handlers/msg_handlers.rs", r#"matches!(role, "admin" | "owner")"#),
    ("src/relay/handlers/msg_handlers.rs", r#"matches!(role, "mod" | "moderator")"#),
    ("src/net/roles.rs", r#"eq_ignore_ascii_case("admin")"#),
    ("src/net/roles.rs", r#"eq_ignore_ascii_case("mod")"#),
];

/// A role's own NAME compared, not who counts as an admin or a moderator: (file, the code, why).
const NAMES: [(&str, &str, &str); 3] = [
    (
        "src/relay/handlers/msg_handlers.rs",
        r#"matches!(action, "ban" | "mod""#,
        "a moderation ACTION's name (`/mod` makes someone a moderator), not a role",
    ),
    (
        "src/relay/storage/moderation.rs",
        r#"!GRANTABLE.contains(&role.as_str()) && role != "mod""#,
        "the role VALUE a signed grant may set (the server's own \"mod\", never \"moderator\"), not a caller's role",
    ),
    (
        "src/gui/pages/chat/modals.rs",
        "let target_is_mod =",
        "whether the TARGET is a moderator, to offer Make moderator or Remove moderator (an admin target is neither)",
    ),
];

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().map_or(false, |n| n == "crossing_tests" || n == "tests") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().map_or(false, |x| x == "rs")
                && !p.to_string_lossy().ends_with("_tests.rs")
                && p.file_name().map_or(false, |n| n != "tests.rs")
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo().join("src"), &mut out);
    if let Ok(rd) = fs::read_dir(repo().join("crates")) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                walk(&e.path().join("src"), &mut out);
            }
        }
    }
    out.sort();
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(repo()).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The line without a trailing `//` comment (a `//` inside a string, as in a URL, is kept).
fn code_of(line: &str) -> &str {
    let mut in_str = false;
    let mut prev = '\0';
    for (i, c) in line.char_indices() {
        if c == '"' && prev != '\\' {
            in_str = !in_str;
        }
        if !in_str && c == '/' && prev == '/' {
            return &line[..i - 1];
        }
        prev = c;
    }
    line
}

/// Whether `code` compares a role name on its own.
fn compares_a_role(code: &str) -> bool {
    let squash: String = code.split_whitespace().collect::<Vec<_>>().join(" ");
    ROLES.iter().any(|r| {
        let lit = format!("\"{r}\"");
        squash.contains(&format!("== {lit}"))
            || squash.contains(&format!("!= {lit}"))
            || squash.contains(&format!("{lit} =="))
            || squash.contains(&format!("{lit} !="))
            || (squash.contains("matches!(") && squash[squash.find("matches!(").unwrap()..].contains(&lit))
    })
}

/// The lines of `src` outside its `#[cfg(test)] mod .. { .. }` blocks, numbered from 1.
fn non_test_lines(src: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t == "#[cfg(test)]" {
            let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
            if (next.starts_with("mod ") || next.starts_with("pub mod ")) && next.ends_with('{') {
                let mut depth = 0i32;
                let mut j = i + 1;
                while j < lines.len() {
                    let c = code_of(lines[j]);
                    depth += c.matches('{').count() as i32 - c.matches('}').count() as i32;
                    j += 1;
                    if depth <= 0 {
                        break;
                    }
                }
                i = j;
                continue;
            }
        }
        out.push((i + 1, lines[i]));
        i += 1;
    }
    out
}

#[test]
fn every_role_check_asks_the_helper() {
    let mut found = Vec::new();
    for path in rust_sources() {
        let name = rel(&path);
        let Ok(src) = fs::read_to_string(&path) else { continue };
        for (n, line) in non_test_lines(&src) {
            let code = code_of(line);
            if code.trim_start().starts_with("//") || !compares_a_role(code) {
                continue;
            }
            if HELPERS.iter().any(|(f, text)| *f == name && code.contains(text))
                || NAMES.iter().any(|(f, text, _)| *f == name && code.contains(text))
            {
                continue;
            }
            found.push(format!("  {name}:{n}: {}", line.trim()));
        }
    }
    assert!(
        found.is_empty(),
        "these check a role name on their own; ask crate::relay::handlers::msg_handlers::is_admin_role / \
         is_staff_role (the relay) or crate::net::roles (the app) instead, so the owner counts as an admin \
         and both spellings of moderator count (BUG-332):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_lint_sees_a_comparison_and_leaves_a_name_alone() {
    for gate in [r#"if role != "admin" {"#, r#"let a = r == "owner";"#, r#"matches!(r.as_str(), "mod" | "x")"#, r#"if "moderator" == role {"#] {
        assert!(compares_a_role(gate), "a gate: {gate}");
    }
    for name in [r#""admin" => "A","#, r#"db.set_role(key, "admin")"#, r#"WHERE role IN ('admin', 'owner')"#, r#"is_admin_role(&role)"#] {
        assert!(!compares_a_role(name), "a name: {name}");
    }
    assert_eq!(code_of(r#"let a = 1; // role == "admin""#).trim(), "let a = 1;");
    let src = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn b() { let x = r == \"admin\"; }\n}\nfn c() {}\n";
    let kept: Vec<usize> = non_test_lines(src).into_iter().map(|(n, _)| n).collect();
    assert_eq!(kept, vec![1, 6], "the test block is skipped and the code after it is read");
}
