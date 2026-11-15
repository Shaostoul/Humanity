//! Guilds retired into Organisations (2026-11-15; docs/design/organisations.md, "Decided": "guilds
//! retire into organisations"). A guild kept its member list in the clear on one server
//! (`guild_members`), the shape the privacy hardening removed everywhere else; an organisation does
//! the same job with its log on its members' devices. This lint holds the retirement in place, the
//! way the removed-tables rule is held (CLAUDE.md: when something is removed, its name goes
//! everywhere in the same piece of work, and nothing keeps it "so links keep working"):
//!
//!   1. the Guilds pages are gone (native `src/gui/pages/guilds.rs`, web `web/pages/guilds.html`)
//!      and so is the relay's guild storage (`src/relay/storage/guilds.rs`);
//!   2. `GuiPage` has no `Guilds` variant, and still has `Orgs`, where it leads now;
//!   3. the top menu (data/gui/navigation.json) names no guilds page, and still names orgs;
//!   4. the web: the nginx config the repo ships sends `/guilds` to `/orgs` with a 301, the deploy's
//!      sync removes a stale `guilds.html` from the web root (its pages copy has no --delete), and
//!      the shell's drawer links no `/guilds`;
//!   5. the relay creates no guild table, drops all three at start (so the LIVE database sheds
//!      them), and routes no `/api/guilds`;
//!   6. the words: en.json holds no `guilds` group and no `nav.guilds` label or hint.
//!
//! Seen red 2026-11-15 against HEAD before the retirement (92c54320c: this file compiled with
//! CARGO_MANIFEST_DIR at an export of that commit's files), failing with 21 problems, every check
//! but the Orgs ones, among them
//!   "src/gui/pages/guilds.rs still exists (Guilds retired into Organisations)",
//!   "src/gui/mod.rs: GuiPage still has a `Guilds` variant",
//!   "scripts/nginx/humanity.conf does not send /guilds to /orgs (want `location = /guilds { return 301 /orgs; }`)",
//!   "src/relay/storage/mod.rs still creates the table `guilds`" and
//!   "src/relay/storage/mod.rs does not drop `guild_members` at start (the live database keeps it)".
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/guilds_retired_lint.rs

use std::fs;
use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The body of `pub enum GuiPage { ... }` in src/gui/mod.rs: its variants, one per line, with
/// comments and attributes left out.
fn gui_page_variants(src: &str) -> Vec<String> {
    let start = src.find("pub enum GuiPage {").expect("src/gui/mod.rs declares `pub enum GuiPage {`");
    let body = &src[start + "pub enum GuiPage {".len()..];
    let end = body.find("\n}").expect("GuiPage's closing brace");
    body[..end]
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_end_matches(',').split(|c: char| c == '(' || c == '{' || c == ' ').next().unwrap_or("").to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

/// Every problem found in the repo, as one line each (empty: the retirement holds).
fn problems() -> Vec<String> {
    let mut bad = Vec::new();

    // 1. The pages and the relay's storage are gone.
    for gone in ["src/gui/pages/guilds.rs", "web/pages/guilds.html", "src/relay/storage/guilds.rs"] {
        if repo().join(gone).exists() {
            bad.push(format!("{gone} still exists (Guilds retired into Organisations)"));
        }
    }

    // 2. No GuiPage::Guilds; GuiPage::Orgs is where it leads.
    let variants = gui_page_variants(&read("src/gui/mod.rs"));
    if variants.iter().any(|v| v == "Guilds") {
        bad.push("src/gui/mod.rs: GuiPage still has a `Guilds` variant".into());
    }
    if !variants.iter().any(|v| v == "Orgs") {
        bad.push(format!("src/gui/mod.rs: GuiPage has no `Orgs` variant to lead to (found {variants:?})"));
    }

    // 3. The top menu.
    let nav = read("data/gui/navigation.json");
    let squeeze = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let nav_tight = squeeze(&nav);
    for stale in ["\"page\":\"guilds\"", "\"web\":\"/guilds\"", "\"id\":\"guilds\"", "nav.guilds"] {
        if nav_tight.contains(stale) {
            bad.push(format!("data/gui/navigation.json still names the guilds page ({stale})"));
        }
    }
    if !nav_tight.contains("\"page\":\"orgs\"") {
        bad.push("data/gui/navigation.json has no orgs page entry".into());
    }

    // 4. The web: the redirect, the stale file removed on deploy, the drawer.
    let nginx = read("scripts/nginx/humanity.conf");
    let redirects = nginx.lines().map(squeeze).any(|l| l.starts_with("location=/guilds{return301/orgs;}"));
    if !redirects {
        bad.push("scripts/nginx/humanity.conf does not send /guilds to /orgs (want `location = /guilds { return 301 /orgs; }`)".into());
    }
    if !read("scripts/sync-web-root.sh").lines().any(|l| l.trim() == "rm -f \"$DST/guilds.html\"") {
        bad.push("scripts/sync-web-root.sh does not remove a stale guilds.html from the web root (its pages copy has no --delete)".into());
    }
    if read("web/shared/shell.js").contains("'/guilds'") {
        bad.push("web/shared/shell.js still links '/guilds'".into());
    }

    // 5. The relay.
    let storage = read("src/relay/storage/mod.rs");
    let storage_tight = squeeze(&storage).to_lowercase();
    for table in ["guilds", "guild_members", "guild_invites"] {
        if storage_tight.contains(&format!("createtableifnotexists{table}(")) || storage_tight.contains(&format!("createtable{table}(")) {
            bad.push(format!("src/relay/storage/mod.rs still creates the table `{table}`"));
        }
        if !storage_tight.contains(&format!("droptableifexists{table};")) {
            bad.push(format!("src/relay/storage/mod.rs does not drop `{table}` at start (the live database keeps it)"));
        }
    }
    if read("src/relay/mod.rs").contains("\"/api/guilds") {
        bad.push("src/relay/mod.rs still routes /api/guilds".into());
    }

    // 6. The words.
    let en = read("data/i18n/en.json");
    if en.lines().any(|l| l.trim_end() == "  \"guilds\": {") {
        bad.push("data/i18n/en.json still holds the `guilds` group".into());
    }
    if en.lines().any(|l| l.trim_start().starts_with("\"guilds_hint\":")) {
        bad.push("data/i18n/en.json still holds nav.guilds_hint".into());
    }
    if en.lines().any(|l| l.trim() == "\"guilds\": \"Guilds\",") {
        bad.push("data/i18n/en.json still holds the nav.guilds label".into());
    }

    bad
}

#[test]
fn guilds_are_retired_into_organisations() {
    let bad = problems();
    assert!(
        bad.is_empty(),
        "\n\nGuilds retired into Organisations on 2026-11-15 (docs/design/organisations.md, \"Decided\"), \
         but {} problem(s) remain:\n  {}\n",
        bad.len(),
        bad.join("\n  ")
    );
}

/// The enum reader finds what it must, so a green run is not a reader that found nothing.
#[test]
fn the_enum_reader_reads_variants() {
    let src = "pub enum GuiPage {\n    None,\n    // a comment\n    Wallet,\n    #[allow(dead_code)]\n    Orgs, // trailing\n    Settings { tab: u8 },\n}\n";
    assert_eq!(gui_page_variants(src), vec!["None", "Wallet", "Orgs", "Settings"]);
    let real = gui_page_variants(&read("src/gui/mod.rs"));
    assert!(real.len() > 20 && real.iter().any(|v| v == "Chat"), "the real enum reads ({real:?})");
}
