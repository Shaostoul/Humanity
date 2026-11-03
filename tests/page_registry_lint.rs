//! Page-registry coverage lint (2026-07-02). PAGES.md's own header has warned
//! since 2026-05-03 that nothing mechanically verifies the registry against the
//! code -- "which is exactly how this file went stale." This closes that gap.
//!
//! Std-only file scanner (no crate imports), compiled standalone like the other
//! lints so it never links the native bin (Windows LNK1318 gotcha):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/page_registry_lint.rs
//! or just run it through `just lints`.
//!
//! What it checks:
//! 1. Every `GuiPage` enum variant in src/gui/mod.rs is MENTIONED somewhere in
//!    docs/PAGES.md (a page the registry doesn't know about = doc rot).
//! 2. Every `web/pages/*.html` file is mentioned in docs/PAGES.md, and the
//!    "N standalone" count in the web-pages heading equals the real file count.
//! 3. Every native page row's `file.rs` reference points at a real file under
//!    src/gui/pages/.
//! 4. The native-pages heading's variant count equals the real `GuiPage` count
//!    (excluding `None`). Added 2026-07-23: only the web count was guarded, so
//!    the native prose silently drifted to 35 while the enum grew to 37, and
//!    CLAUDE.md tells agents to trust that heading.
//! 5. The top menu's file, data/gui/navigation.json (2026-11-03, navigation rung 1): every
//!    website address in it is a page, and every web page is in it exactly once (none lost,
//!    none twice). Its desktop twin is a lib test (src/gui/navigation_tests.rs).
//! Deliberately NOT checked: prose accuracy (only a human can audit purpose
//! text) and per-variant row format (the doc groups variants into tables of
//! different shapes; a mention anywhere is the invariant that prevents rot).

use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Extract GuiPage variant names from src/gui/mod.rs (the enum block only).
fn gui_page_variants() -> Vec<String> {
    let src = fs::read_to_string(repo().join("src/gui/mod.rs")).expect("read src/gui/mod.rs");
    let start = src.find("pub enum GuiPage").expect("GuiPage enum present");
    let body_start = src[start..].find('{').expect("enum opens") + start + 1;
    // Find the matching closing brace (the enum has no nested braces today; if
    // data variants are ever added, this scanner needs a depth counter).
    let body_end = src[body_start..].find("\n}").expect("enum closes") + body_start;
    let mut out = Vec::new();
    for line in src[body_start..body_end].lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") || t.starts_with("///") || t.starts_with('#') {
            continue;
        }
        let name: String = t
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() && name.chars().next().unwrap().is_ascii_uppercase() {
            out.push(name);
        }
    }
    assert!(out.len() > 30, "sanity: found {} GuiPage variants, expected 50+", out.len());
    out
}

#[test]
fn every_gui_page_variant_is_in_the_registry() {
    let doc = fs::read_to_string(repo().join("docs/PAGES.md")).expect("read docs/PAGES.md");
    let mut missing = Vec::new();
    for v in gui_page_variants() {
        if v == "None" {
            continue; // the in-game/no-menu state, documented in the heading
        }
        if !doc.contains(&v) {
            missing.push(v);
        }
    }
    assert!(
        missing.is_empty(),
        "GuiPage variants missing from docs/PAGES.md (add a row or a removed-variants note): {missing:?}"
    );
}

#[test]
fn every_native_page_file_reference_exists() {
    let doc = fs::read_to_string(repo().join("docs/PAGES.md")).expect("read docs/PAGES.md");
    let pages_dir = repo().join("src/gui/pages");
    let mut missing = Vec::new();
    // Registry rows reference files as `name.rs` in backticks.
    let mut i = 0;
    let bytes = doc.as_bytes();
    while let Some(start) = doc[i..].find('`') {
        let s = i + start + 1;
        let Some(end) = doc[s..].find('`') else { break };
        let token = &doc[s..s + end];
        if token.ends_with(".rs")
            && !token.contains('/')
            && !token.contains(' ')
            && !token.contains(':')
        {
            // Only enforce for rows that clearly refer to page files (a bare
            // `foo.rs` in the native tables); skip if it exists anywhere else
            // it plausibly refers to (gui root like theme.rs / laws.rs loader).
            let in_pages = pages_dir.join(token).exists();
            let in_gui_root = repo().join("src/gui").join(token).exists();
            if !in_pages && !in_gui_root {
                missing.push(token.to_string());
            }
        }
        i = s + end + 1;
        let _ = bytes;
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "docs/PAGES.md references page files that do not exist (deleted without a doc update?): {missing:?}"
    );
}

#[test]
fn web_page_files_and_count_match_the_registry() {
    let doc = fs::read_to_string(repo().join("docs/PAGES.md")).expect("read docs/PAGES.md");
    let dir = repo().join("web/pages");
    let mut files: Vec<String> = fs::read_dir(&dir)
        .expect("read web/pages")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".html"))
        .collect();
    files.sort();

    // 1. Every real file is at least mentioned.
    let missing: Vec<&String> = files.iter().filter(|f| !doc.contains(f.as_str())).collect();
    assert!(
        missing.is_empty(),
        "web/pages files missing from docs/PAGES.md: {missing:?}"
    );

    // 2. The "N standalone" claim in the web-pages heading equals reality.
    let heading = doc
        .lines()
        .find(|l| l.starts_with("## Web pages"))
        .expect("web-pages heading present");
    let claimed: usize = heading
        .split(':')
        .nth(1)
        .and_then(|s| s.trim().split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("heading carries a count like `web/pages/*.html`: 40 standalone");
    assert_eq!(
        claimed,
        files.len(),
        "docs/PAGES.md claims {claimed} standalone web pages but web/pages/ holds {} -- update the heading (and add/remove the page rows)",
        files.len()
    );
}

/// Every `"web": "<address>"` value in a JSON text, in order (a `null` or an array is skipped).
fn web_addresses(json: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = json[from..].find("\"web\":") {
        let start = from + i + 6;
        let rest = json[start..].trim_start();
        if let Some(stripped) = rest.strip_prefix('"') {
            if let Some(end) = stripped.find('"') {
                out.push(stripped[..end].to_string());
            }
        }
        from = start;
    }
    out
}

/// The file a website address is served from: `/` the front door, `/chat` the chat's own
/// folder, `/<name>` web/pages/<name>.html (the server's `try_files $uri.html`). A `#part` is a
/// part of that page.
fn web_page_file(address: &str) -> std::path::PathBuf {
    let path = address.split('#').next().unwrap_or("");
    match path {
        "/" => repo().join("web/pages/index.html"),
        "/chat" => repo().join("web/chat/index.html"),
        p => repo().join("web/pages").join(format!("{}.html", p.trim_start_matches('/'))),
    }
}

/// THE TOP MENU'S WEBSITE HALF OF "NONE LOST, NONE TWICE" (navigation rung 1, 2026-11-03,
/// docs/design/navigation.md section 13). Every website address in data/gui/navigation.json is a
/// page the website has, and every web/pages/*.html page is in the file exactly once as a whole
/// page (a category, an item, or `outside` with its reason; `/dev#claude` is a part of /dev and
/// does not count). The desktop half, every GuiPage in exactly one place, is a lib test that reads
/// the file through the app's own reader: src/gui/navigation_tests.rs
/// (`every_desktop_page_is_in_the_menu_file_once`).
///
/// Seen red 2026-11-03 with Governance's address written `/governanse`: "the menu's website
/// addresses that are not pages: [\"/governanse (web/pages/governanse.html)\"]"; and with the `hi`
/// entry taken out of `outside`: "web pages in 0 or 2+ places of the menu file (none lost, none
/// twice): [\"hi.html in 0\"]".
#[test]
fn the_menu_file_places_every_web_page_once() {
    let nav = fs::read_to_string(repo().join("data/gui/navigation.json")).expect("read data/gui/navigation.json");
    let addresses = web_addresses(&nav);
    assert!(addresses.len() > 30, "sanity: the menu file names {} website addresses", addresses.len());
    let missing: Vec<String> = addresses
        .iter()
        .filter(|a| !a.starts_with('/') || !web_page_file(a).is_file())
        .map(|a| {
            let file = web_page_file(a);
            let shown = file.strip_prefix(repo()).unwrap_or(&file).display().to_string().replace('\\', "/");
            format!("{a} ({shown})")
        })
        .collect();
    assert!(missing.is_empty(), "the menu's website addresses that are not pages: {missing:?}");

    let mut wrong = Vec::new();
    let mut pages: Vec<String> = fs::read_dir(repo().join("web/pages"))
        .expect("read web/pages")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".html"))
        .collect();
    pages.sort();
    pages.push("chat (web/chat/index.html)".to_string());
    for page in &pages {
        let address = match page.as_str() {
            "index.html" => "/".to_string(),
            p if p.starts_with("chat ") => "/chat".to_string(),
            p => format!("/{}", p.trim_end_matches(".html")),
        };
        let n = addresses.iter().filter(|a| **a == address).count();
        if n != 1 {
            wrong.push(format!("{page} in {n}"));
        }
    }
    assert!(wrong.is_empty(), "web pages in 0 or 2+ places of the menu file (none lost, none twice): {wrong:?}");
}

/// The NATIVE heading count (2026-07-23). Only the web count was guarded, so the
/// native prose drifted unnoticed: RelayControl (v0.846) and Watch (v0.857) were
/// added to the tables but the heading still claimed 35, and CLAUDE.md tells
/// agents to trust that heading for the live native page count. Guard it the
/// same way the web count is guarded.
#[test]
fn native_page_count_in_the_heading_matches_the_enum() {
    let doc = fs::read_to_string(repo().join("docs/PAGES.md")).expect("read docs/PAGES.md");
    let variants = gui_page_variants();
    // `None` is the in-game/no-menu state, called out separately in the heading.
    let pages = variants.iter().filter(|v| *v != "None").count();

    let heading = doc
        .lines()
        .find(|l| l.starts_with("## Native pages"))
        .expect("native-pages heading present in docs/PAGES.md");
    let claimed: usize = heading
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .expect("native-pages heading carries a count like `## Native pages (37 GuiPage variants,`");

    assert_eq!(
        claimed, pages,
        "docs/PAGES.md claims {claimed} native GuiPage page variants but src/gui/mod.rs has {pages} \
         (excluding `None`) -- update the heading in docs/PAGES.md (and add/remove the page rows)"
    );
}
