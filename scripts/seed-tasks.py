#!/usr/bin/env python3
"""Seed the HumanityOS task board with all completed and planned features.
Usage: python3 seed-tasks.py <API_SECRET>
"""
import subprocess, json, sys

KEY = sys.argv[1] if len(sys.argv) > 1 else "379201d60f65978d086e311f8fe08414f25a3ce2b6e2fa422579e26f253b4de1"
BASE = "https://united-humanity.us"

def t(title, desc, status, priority, labels):
    body = json.dumps({
        "title": title,
        "description": desc,
        "status": status,
        "priority": priority,
        "labels": json.dumps(labels)
    })
    r = subprocess.run(
        ["curl", "-s", "-X", "POST", f"{BASE}/api/tasks",
         "-H", f"Authorization: Bearer {KEY}",
         "-H", "Content-Type: application/json",
         "-d", body],
        capture_output=True, text=True
    )
    try:
        d = json.loads(r.stdout)
        print(f"  #{d.get('id','?')} [{status}/{priority}] {title}")
    except Exception:
        print(f"  ERR [{status}] {title}: {r.stdout[:100]}")

print("\n=== DONE: Chat ===")
t("Real-time chat channels with server rooms",
  "Multi-server chat with channels, invite links, moderation commands, and full message history in SQLite.",
  "done", "medium", ["scope:city","chat"])
t("Voice rooms and 1-on-1 video calls",
  "WebRTC voice rooms, push-to-talk, noise suppression toggle, 1-on-1 video calls, unified right sidebar.",
  "done", "medium", ["scope:city","chat","voice"])
t("End-to-end encrypted direct messages",
  "ECDH P-256 key exchange + AES-256-GCM. DMs encrypted client-side; server stores only ciphertext.",
  "done", "high", ["scope:city","chat","security"])
t("Message reactions, editing, pinning, and threads",
  "Emoji reactions, inline editing, channel pin list, thread panel with reply chains.",
  "done", "low", ["scope:city","chat"])
t("Message search and command palette",
  "Full-text search across channel history. /command palette with autocomplete.",
  "done", "low", ["scope:city","chat"])
t("Image uploads and link previews",
  "Drag-and-drop image upload with server storage. Automatic Open Graph link preview cards.",
  "done", "low", ["scope:city","chat"])

print("\n=== DONE: Identity ===")
t("Ed25519 cryptographic identity — no accounts, no passwords",
  "Keypair generated locally in browser, stored in IndexedDB. Every message signed. Server verifies all signatures. Zero server-side user accounts.",
  "done", "critical", ["scope:city","identity","security"])
t("P2P contact cards with QR code and signature verification",
  "Export signed contact card as QR code or text. Import verifies Ed25519 signature before adding to contacts.",
  "done", "high", ["scope:city","identity","p2p"])
t("WebRTC DataChannel P2P messaging",
  "Direct encrypted peer-to-peer data channel via WebRTC. Relay-assisted signaling then direct connection.",
  "done", "medium", ["scope:city","identity","p2p"])

print("\n=== DONE: Profiles & Social ===")
t("User profiles with avatar, banner, pronouns, and privacy controls",
  "Full profile editor with per-field privacy toggles (public/friends/private). Server enforces privacy based on follow graph.",
  "done", "medium", ["scope:city","profile"])
t("Follow system and friends (mutual follows)",
  "Follow/unfollow any user. Friends = mutual follows. Gates profile privacy and future features.",
  "done", "low", ["scope:city","profile","social"])
t("Groups with roles and invite system",
  "Create groups, invite members, assign admin/member roles. Group-scoped channel list.",
  "done", "medium", ["scope:city","groups"])

print("\n=== DONE: Desktop App ===")
t("HumanityOS desktop app — Windows, Mac, Linux",
  "Tauri v2 native wrapper with auto-updater. Signed releases via GitHub Actions on 4 platforms: win-x64, linux-x64, mac-arm64, mac-x64.",
  "done", "high", ["scope:city","desktop"])

print("\n=== DONE: Infrastructure ===")
t("Automated CI/CD deploy pipeline",
  "Push to main: SSH to VPS, git pull, cargo build on server, rsync assets, restart relay, Deploy Bot announces in #announcements.",
  "done", "high", ["scope:city","infrastructure"])
t("Rust relay split into focused modules",
  "relay.rs split + handlers/broadcast.rs, federation.rs, utils.rs. storage.rs split into 14 domain files.",
  "done", "low", ["scope:city","infrastructure","backend"])
t("Chat client split into 9 JavaScript modules",
  "app.js (6400 to 1683 LOC) + 7 module files. No build step — ordered script tags, shared global scope.",
  "done", "low", ["scope:city","infrastructure","frontend"])
t("Game/Hub split into 8 JS modules and 7 JSON data files",
  "game/index.html 15216 to 2441 LOC. JSON: solar system, stars, constellations, cities, coastlines.",
  "done", "low", ["scope:city","infrastructure","frontend"])
t("CSS split into 7 component stylesheets",
  "style.css split into: base, layout, sidebar, messages, modals, voice, inputs.",
  "done", "low", ["scope:city","infrastructure","frontend"])

print("\n=== DONE: UI & Navigation ===")
t("Unified navigation shell across all 18 pages",
  "shell.js injected via script tag on every page. data-active highlights current tab. Falls back to pathname auto-detection.",
  "done", "medium", ["scope:city","ui","nav"])
t("Navigation bug fixes — canonical URLs and active state",
  "Fixed 4 bugs: double-active on landing, 5 wrong nav URLs, undefined pathname in mobile drawer, nginx 301 redirects.",
  "done", "medium", ["scope:city","ui","nav"])
t("Landing page rewrite with clearer messaging",
  "3-path cards (Use/Build/Host), 18-page grid with status dots, 6 differentiator cards, mission statement.",
  "done", "low", ["scope:city","docs","ui"])

print("\n=== DONE: Documentation ===")
t("Documentation overhaul — README, CONTRIBUTING, ONBOARDING",
  "README: module tree, 18-page table, Adding-a-New-Page recipe. CONTRIBUTING: quick start, monkey-patch pattern. ONBOARDING.md: project overview, codebase in 5 min, contribution paths by role.",
  "done", "medium", ["scope:city","docs"])

print("\n=== DONE: Tasks & DevOps ===")
t("Fibonacci-scoped task board — this board",
  "Kanban (backlog/in-progress/testing/done) with 10-scope Fibonacci selector from 1-Self to 55-Cosmos. Scope stored as task labels.",
  "done", "medium", ["scope:city","tasks"])
t("Deploy Bot — auto-announces deployments in chat",
  "POST /api/send on every successful deploy. Shows commit SHA, author, and message in #announcements.",
  "done", "low", ["scope:city","infrastructure"])

print("\n=== TESTING: Recently deployed ===")
t("Deploy pipeline — server-side Rust build",
  "Switched from GitHub Actions binary upload (timed out) to building on VPS directly. rsync installed. workflow_dispatch trigger added.",
  "testing", "medium", ["scope:city","infrastructure"])

print("\n=== BACKLOG: Tier 1 — Foundational ===")
t("Identity recovery — BIP39 seed phrase",
  "CRITICAL: losing your device means losing your identity forever. BIP39 mnemonic -> PBKDF2 -> AES-GCM wrap Ed25519 key. Files: crypto.js + chat-profile.js.",
  "backlog", "critical", ["scope:city","identity","security"])
t("Passphrase-wrapped key storage (encrypted at rest)",
  "Ed25519/ECDH keys stored unencrypted in IndexedDB today. Wrap with AES key derived from passphrase via PBKDF2. Keys never touch disk unencrypted.",
  "backlog", "high", ["scope:city","identity","security"])
t("Federation — server network with Accord trust tiers",
  "Servers discover each other, establish trust, route messages between networks. Skeleton in handlers/federation.rs. Needs protocol + UI.",
  "backlog", "critical", ["scope:world","federation"])
t("Multi-device key sync via ECDH",
  "Same identity on phone + desktop. ECDH secure channel between owned devices. QR scan or numeric code to authorize second device.",
  "backlog", "high", ["scope:city","identity","p2p"])

print("\n=== BACKLOG: Tier 2 — Core OS ===")
t("Calendar with events, recurring schedules, and RSVP",
  "calendar.html stub exists. Events, recurring schedules, group calendars, RSVP flow, reminders. Needs relay storage + WebSocket messages.",
  "backlog", "medium", ["scope:city","calendar"])
t("Skills — verifiable peer-endorsed reputation",
  "Claim a skill, request peer endorsement, endorser signs with Ed25519. Self-sovereign resume. skills.html stub exists.",
  "backlog", "high", ["scope:city","skills","identity"])
t("Personal data store — encrypted notes and files",
  "Notes, files, journal entries that travel with your identity. Local-first, encrypted at rest, optional relay sync.",
  "backlog", "medium", ["scope:self","identity"])
t("Task creation via WebSocket for regular users",
  "Task creation requires admin API key today. Add WebSocket message so any authenticated user can propose tasks (server validates Ed25519).",
  "backlog", "medium", ["scope:city","tasks","backend"])
t("Task detail panel with comments and history",
  "Click a card to open slide-in panel: full description, comment thread, status history, assignee, linked tasks.",
  "backlog", "medium", ["scope:city","tasks","ui"])

print("\n=== BACKLOG: Tier 3 — Civilization Scale ===")
t("Group governance — proposals and voting",
  "Turns groups into cooperatives. Proposals, ranked-choice votes, quorum rules. All votes signed with Ed25519 — verifiable, tamper-evident.",
  "backlog", "high", ["scope:world","groups","governance"])
t("Marketplace — offer and request skills, goods, and time",
  "Listings API exists at /api/listings. Build real UI: post offer, post request, match, exchange. Reputation-gated. market.html stub.",
  "backlog", "medium", ["scope:city","market"])
t("Learning paths — structured skill progression",
  "Complete module -> peer-endorsed -> unlock next level. Design detail in design/education_model.md. learn.html stub.",
  "backlog", "medium", ["scope:city","learn","skills"])

print("\n=== BACKLOG: Tier 4 — Reach ===")
t("Progressive Web App — installable on any device",
  "service worker + manifest.json + offline cache + push notifications. Gets HOS onto phones without a native app. ~1 day of work.",
  "backlog", "high", ["scope:city","ui","mobile"])
t("Mobile app — Tauri Mobile",
  "Full native mobile. Tauri 2 has mobile support. Shares most of the codebase with desktop.",
  "backlog", "low", ["scope:city","desktop","mobile"])
t("Public API — user-facing API keys and webhooks",
  "API key management UI, webhook endpoints, rate limiting. Turns HOS into a platform for third-party integrations.",
  "backlog", "medium", ["scope:city","infrastructure","backend"])

print("\nAll tasks created.")
