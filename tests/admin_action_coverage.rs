//! Admin action coverage (2026-10-22, docs/design/in-app-ops.md, the north star).
//!
//! Every action a server's admins and moderators can take is meant to be an entry in
//! `data/admin/actions.json`, the registry both apps draw "Every admin action" from and an AI
//! reads to know what it may do. An action that exists in the relay and not in the registry is
//! invisible to all three. This lint fails when that happens, in the two places an admin action
//! is born today:
//!
//! 1. A route under `/api/admin/` in `src/relay/mod.rs`: it must be the registry's own (list and
//!    run) or the `route` of some entry.
//! 2. A chat command the relay answers (an arm `"/name" => {` in `src/relay/relay.rs`): it must be
//!    the `chat` command of some entry, or be named below as a MEMBER command with its reason.
//!
//! And the other way: an entry's `chat` command must be one the relay answers, and its `route`
//! a route the relay has, so the registry cannot promise what is not there.
//!
//! The third place, actions that run through the registry itself (`"run": "here"`), is held by
//! `src/relay/admin/tests.rs` (`every_action_that_runs_here_has_a_runner_and_every_runner_an_action`),
//! which can see the runner table. NOT covered yet: admin messages on the signed-in socket
//! (`server_settings_update`, `game_ban`, `service_control`, `admin_erase`, ...). They are the
//! next step of this lint (docs/design/in-app-ops.md, "The action registry", what is left).
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/admin_action_coverage.rs
//! or through `just lints`.
//!
//! Seen red 2026-10-22 three ways: the `members.donor` entry taken out ("/donor is a chat command
//! the relay answers with no registry entry"), a route `/api/admin/probe` added to mod.rs
//! ("/api/admin/probe is an admin route with no registry entry"), and an entry's chat changed to
//! `/server-nap` ("an entry names the chat command /server-nap, which the relay does not answer").

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}")).replace("\r\n", "\n")
}

/// Chat commands that are not admin actions: what any member types. Each with its reason.
const MEMBER_COMMANDS: &[(&str, &str)] = &[
    ("/link", "links one of your own devices to your name"),
    ("/help", "lists the commands"),
    ("/revoke", "revokes one of your own devices"),
    ("/report", "says how to report someone"),
    ("/dm", "sends a direct message"),
    ("/dms", "lists your direct messages"),
    ("/friend-code", "makes your own friend code"),
    ("/redeem", "redeems a friend code"),
    ("/pins", "lists the pinned messages"),
    ("/edit", "edits your own last message"),
    ("/groups", "opens a view in the app (no server action)"),
    ("/servers", "opens a view in the app (no server action)"),
    ("/friends", "opens a view in the app (no server action)"),
];

/// The registry's own routes.
const REGISTRY_ROUTES: &[&str] = &["/api/admin/actions", "/api/admin/actions/run"];

/// Every value of `"<field>": "<value>"` in the registry file.
fn field_values(json: &str, field: &str) -> Vec<String> {
    let needle = format!("\"{field}\": \"");
    let mut out = Vec::new();
    let mut rest = json;
    while let Some(i) = rest.find(&needle) {
        rest = &rest[i + needle.len()..];
        let end = rest.find('"').expect("a closed string");
        out.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    out
}

/// Every route path in the relay's router.
fn relay_routes() -> BTreeSet<String> {
    let src = read("src/relay/mod.rs");
    let mut out = BTreeSet::new();
    let mut rest = src.as_str();
    while let Some(i) = rest.find(".route(\"") {
        rest = &rest[i + 8..];
        let end = rest.find('"').unwrap();
        out.insert(rest[..end].to_string());
    }
    out
}

/// Every chat command the relay answers: the arms `"/name" => {` (and `"/a" | "/b" => {`).
fn relay_commands() -> BTreeSet<String> {
    let src = read("src/relay/relay.rs");
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let t = line.trim_start();
        if !t.starts_with("\"/") {
            continue;
        }
        let Some(arrow) = t.find("=>") else { continue };
        let (pattern, after) = (&t[..arrow], t[arrow + 2..].trim_start());
        if !after.starts_with('{') {
            continue;
        }
        let names: Vec<&str> = pattern.split('|').map(|p| p.trim().trim_matches('"')).collect();
        let well_formed = names.iter().all(|n| n.starts_with('/') && n.len() > 1 && n[1..].chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        if well_formed {
            out.extend(names.into_iter().map(str::to_string));
        }
    }
    out
}

#[test]
fn every_admin_route_and_chat_command_has_a_registry_entry() {
    let registry = read("data/admin/actions.json");
    let chats: BTreeSet<String> = field_values(&registry, "chat")
        .iter()
        .filter_map(|c| c.split_whitespace().next().map(str::to_string))
        .collect();
    let routes: BTreeSet<String> = field_values(&registry, "route")
        .iter()
        .filter_map(|r| r.split_whitespace().nth(1).map(str::to_string))
        .collect();
    let relay_routes = relay_routes();
    let relay_commands = relay_commands();
    assert!(relay_routes.len() > 50 && relay_commands.len() > 30, "the scans found the router and the commands");

    let mut missing = Vec::new();
    for r in relay_routes.iter().filter(|r| r.starts_with("/api/admin/")) {
        if !REGISTRY_ROUTES.contains(&r.as_str()) && !routes.contains(r) {
            missing.push(format!("{r} is an admin route with no registry entry (give an entry \"route\": \"POST {r}\")"));
        }
    }
    for c in &relay_commands {
        let member = MEMBER_COMMANDS.iter().any(|(m, _)| m == c);
        if !member && !chats.contains(c) {
            missing.push(format!(
                "{c} is a chat command the relay answers with no registry entry (add one to data/admin/actions.json with \"chat\": \"{c} ...\", or, if any member may use it, to MEMBER_COMMANDS here with the reason)"
            ));
        }
        if member && chats.contains(c) {
            missing.push(format!("{c} is both a member command here and an admin entry in the registry"));
        }
    }
    for c in &chats {
        if !relay_commands.contains(c) {
            missing.push(format!("an entry names the chat command {c}, which the relay does not answer"));
        }
    }
    for r in &routes {
        if !relay_routes.contains(r) {
            missing.push(format!("an entry names the route {r}, which the relay does not have"));
        }
    }
    for (m, _) in MEMBER_COMMANDS {
        if !relay_commands.contains(*m) {
            missing.push(format!("{m} is listed as a member command but the relay no longer answers it: take it off the list"));
        }
    }
    assert!(missing.is_empty(), "\n\nADMIN ACTIONS WITHOUT A REGISTRY ENTRY:\n  {}\n", missing.join("\n  "));
}

#[test]
fn every_registry_route_is_get_or_post_and_every_chat_a_command() {
    let registry = read("data/admin/actions.json");
    for r in field_values(&registry, "route") {
        assert!(r.starts_with("GET /") || r.starts_with("POST /"), "a route is GET or POST and a path: {r}");
    }
    for c in field_values(&registry, "chat") {
        assert!(c.starts_with('/'), "a chat command starts with /: {c}");
    }
}
