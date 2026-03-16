#!/usr/bin/env bash
# Seed the HumanityOS task board with all completed and planned features.
# Run: bash scripts/seed-tasks.sh <API_SECRET>
set -e

KEY="${1:?Usage: seed-tasks.sh <API_SECRET>}"
BASE="https://united-humanity.us"

t() {
  local title="$1" desc="$2" status="$3" priority="$4" labels="$5"
  local result
  result=$(curl -s -X POST "$BASE/api/tasks" \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "{\"title\":$(printf '%s' "$title" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'),\"description\":$(printf '%s' "$desc" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'),\"status\":\"$status\",\"priority\":\"$priority\",\"labels\":$labels}")
  local id
  id=$(echo "$result" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("id","err"))' 2>/dev/null)
  echo "  #$id [$status/$priority] $title"
}

echo ""
echo "=== DONE: Chat ==="
t "Real-time chat channels with server rooms" \
  "Multi-server chat with channels, invite links, moderation commands, and message history in SQLite." \
  done medium '["scope:city","chat"]'

t "Voice rooms and 1-on-1 video calls" \
  "WebRTC voice rooms, push-to-talk, noise suppression, 1-on-1 video calls with unified right sidebar." \
  done medium '["scope:city","chat","voice"]'

t "End-to-end encrypted direct messages" \
  "ECDH P-256 key exchange + AES-256-GCM. DMs encrypted client-side; server stores only ciphertext." \
  done high '["scope:city","chat","security"]'

t "Message reactions, editing, pinning, and threads" \
  "Emoji reactions, inline editing, channel pin list, thread panel with reply chains." \
  done low '["scope:city","chat"]'

t "Message search and command palette" \
  "Full-text search across channel history. /command palette with autocomplete." \
  done low '["scope:city","chat"]'

t "Image uploads and link previews" \
  "Drag-and-drop image upload with server-side storage. Automatic Open Graph link preview cards." \
  done low '["scope:city","chat"]'

echo ""
echo "=== DONE: Identity & Crypto ==="
t "Ed25519 cryptographic identity — no accounts, no passwords" \
  "Keypair generated locally in browser, stored in IndexedDB. Every message is signed. Server verifies all signatures. Zero server-side user accounts." \
  done critical '["scope:city","identity","security"]'

t "P2P contact cards with QR code and signature verification" \
  "Export signed contact card as QR code or text blob. Import verifies Ed25519 signature before adding to contacts." \
  done high '["scope:city","identity","p2p"]'

t "WebRTC DataChannel P2P messaging" \
  "Direct encrypted peer-to-peer data channel via WebRTC. Relay-assisted signaling, then direct connection." \
  done medium '["scope:city","identity","p2p"]'

echo ""
echo "=== DONE: Profiles & Social ==="
t "User profiles with avatar, banner, pronouns, and privacy controls" \
  "Full profile editor: avatar, banner, bio, pronouns, location, website. Per-field privacy toggles (public/friends/private). Server enforces privacy based on follow graph." \
  done medium '["scope:city","profile"]'

t "Follow system and friends (mutual follows)" \
  "Follow/unfollow any user. Friends = mutual follows. Friend status gates profile privacy and future features." \
  done low '["scope:city","profile","social"]'

t "Groups with roles and invite system" \
  "Create groups, invite members, assign admin/member roles. Group-scoped channel list." \
  done medium '["scope:city","groups"]'

echo ""
echo "=== DONE: Desktop App ==="
t "HumanityOS desktop app — Windows, Mac, Linux" \
  "Tauri v2 native wrapper. Auto-updater via tauri-plugin-updater. Signed releases on GitHub. Builds via GitHub Actions matrix (4 platforms)." \
  done high '["scope:city","desktop"]'

echo ""
echo "=== DONE: Infrastructure ==="
t "Automated CI/CD deploy pipeline" \
  "GitHub Actions: push to main → SSH to VPS → git pull → cargo build (server-side) → rsync assets → restart relay → Deploy Bot announces in #announcements." \
  done high '["scope:city","infrastructure"]'

t "Rust relay split into focused modules" \
  "relay.rs (6320→5600 LOC) + handlers/broadcast.rs, handlers/federation.rs, handlers/utils.rs. storage.rs split into 14 domain files (messages, channels, tasks, social, etc.)." \
  done low '["scope:city","infrastructure","backend"]'

t "Chat client split into 9 JavaScript modules" \
  "app.js (6400→1683 LOC) + chat-messages.js, chat-dms.js, chat-social.js, chat-ui.js, chat-voice.js, chat-profile.js, chat-p2p.js. No build step — ordered script tags." \
  done low '["scope:city","infrastructure","frontend"]'

t "Game/Hub split into 8 JS modules and 7 JSON data files" \
  "game/index.html (15216→2441 LOC). Extracted to game/js/core, reality, fantasy, celestial, streams, browse, map, info-market. JSON data: solar system, stars, constellations, cities, coastlines." \
  done low '["scope:city","infrastructure","frontend"]'

t "CSS split into 7 component files" \
  "style.css split into base.css, layout.css, sidebar.css, messages.css, modals.css, voice.css, inputs.css." \
  done low '["scope:city","infrastructure","frontend"]'

echo ""
echo "=== DONE: UI & Navigation ==="
t "Unified navigation shell across all 18 pages" \
  "shell.js injected via script tag on every page. data-active attribute highlights current tab. Auto-detects from pathname as fallback." \
  done medium '["scope:city","ui","nav"]'

t "Navigation bug fixes — canonical URLs and active state" \
  "Fixed 4 bugs: double-active state on landing page, 5 wrong nav URLs (/board→/systems etc), undefined pathname in mobile drawer, nginx 301 redirects for old routes." \
  done medium '["scope:city","ui","nav"]'

echo ""
echo "=== DONE: Documentation ==="
t "Documentation overhaul — README, CONTRIBUTING, ONBOARDING" \
  "README: full module tree, 18-page table, Adding-a-New-Page recipe. CONTRIBUTING: quick start, module guide, monkey-patch pattern. ONBOARDING.md: new file explaining what the project is, codebase in 5 minutes, first contribution paths." \
  done medium '["scope:city","docs"]'

t "Landing page rewrite" \
  "index.html added to git repo. Clearer messaging: 3-path cards (Use/Build/Host), 18-page grid with live/building/stub status dots, 6 differentiator cards, mission statement." \
  done low '["scope:city","docs","ui"]'

echo ""
echo "=== DONE: Tasks System ==="
t "Fibonacci-scoped task board (this board)" \
  "tasks.html: kanban board (backlog/in-progress/testing/done) with 10-scope Fibonacci selector (1·Self → 55·Cosmos). Scope stored as task labels. Connected to /api/tasks REST API." \
  done medium '["scope:city","tasks"]'

t "Deploy Bot — auto-announces deployments in chat" \
  "POST /api/send on every successful deploy. Shows commit SHA, author, and message in #announcements. API_SECRET stored in GitHub Secrets and server .env." \
  done low '["scope:city","infrastructure"]'

echo ""
echo "=== TESTING: Recently deployed ==="
t "Deploy pipeline — server-side Rust build (no binary upload)" \
  "Switched from GitHub Actions binary upload (timed out) to building on VPS directly. rsync installed. workflow_dispatch trigger added for manual runs." \
  testing medium '["scope:city","infrastructure"]'

echo ""
echo "=== BACKLOG: Tier 1 — Foundational ==="
t "Identity recovery — BIP39 seed phrase for key backup" \
  "If a user loses their device, their identity is gone forever. BIP39 12-word mnemonic → PBKDF2 stretch → AES-GCM wrap Ed25519 private key. Recovery: enter seed → unwrap → restore identity. Files: crypto.js + chat-profile.js." \
  backlog critical '["scope:city","identity","security"]'

t "Passphrase-wrapped key storage (encrypted at rest)" \
  "Currently Ed25519/ECDH keys stored unencrypted in IndexedDB/localStorage. Wrap with AES key derived from user passphrase via PBKDF2. Keys never touch disk in plaintext." \
  backlog high '["scope:city","identity","security"]'

t "Federation — server network with trust tiers" \
  "Servers discover each other, establish trust (Humanity Accord verification), route messages between networks. Skeleton in handlers/federation.rs. Needs protocol definition + UI." \
  backlog critical '["scope:world","federation"]'

t "Multi-device key sync via ECDH" \
  "Same identity on phone + desktop. ECDH-based secure channel between owned devices. QR scan or short numeric code to authorize second device. Reuse WebRTC DataChannel from chat-p2p.js." \
  backlog high '["scope:city","identity","p2p"]'

echo ""
echo "=== BACKLOG: Tier 2 — Core OS ==="
t "Calendar with events and RSVP" \
  "calendar.html exists as stub. Events, recurring schedules, group calendars, RSVP flow, reminders. Needs relay storage + WebSocket messages." \
  backlog medium '["scope:city","calendar"]'

t "Skills — verifiable peer-endorsed reputation" \
  "Self-sovereign résumé. Claim a skill → request peer endorsement → endorser signs with Ed25519. Verifiable without a central authority. skills.html stub exists." \
  backlog high '["scope:city","skills","identity"]'

t "Personal data store — encrypted notes and files" \
  "Notes, files, journal entries that travel with your identity. Local-first, encrypted with identity key. Optional relay sync." \
  backlog medium '["scope:self","identity"]'

t "Task creation via WebSocket for regular users" \
  "Currently task creation requires the admin API_SECRET. Add a WebSocket message type so any connected+authenticated user can propose tasks. Server validates Ed25519 signature." \
  backlog medium '["scope:city","tasks","backend"]'

t "Task detail panel with comments and history" \
  "Click a task card to open a slide-in panel. Show full description, comments thread, status history, assignee, linked tasks." \
  backlog medium '["scope:city","tasks","ui"]'

echo ""
echo "=== BACKLOG: Tier 3 — Civilization Scale ==="
t "Group governance — proposals and voting" \
  "Turns chat groups into cooperatives. Create proposals, vote (ranked-choice or simple majority), set quorum rules. All votes signed with Ed25519 — verifiable, tamper-evident." \
  backlog high '["scope:world","groups","governance"]'

t "Marketplace — offer and request skills, goods, and time" \
  "Listings API + /api/listings already exist. Build real UI: post an offer, post a request, match + exchange. Reputation-gated. market.html stub exists." \
  backlog medium '["scope:city","market"]'

t "Learning paths — structured skill progression" \
  "Complete a module → get peer-endorsed → unlock next level. Design detail in design/education_model.md. web.html has curated sites." \
  backlog medium '["scope:city","learn","skills"]'

echo ""
echo "=== BACKLOG: Tier 4 — Reach ==="
t "Progressive Web App (PWA) — installable on any device" \
  "service worker + manifest.json + offline cache + push notifications. Gets HOS onto phones without a native app. ~1 day of work." \
  backlog high '["scope:city","ui","mobile"]'

t "Mobile app — Tauri Mobile or React Native" \
  "Full native mobile app. Lower priority than PWA. Tauri 2 has mobile support (beta). Shares codebase with desktop." \
  backlog low '["scope:city","desktop","mobile"]'

t "Public API — user-facing API keys and webhooks" \
  "Let users and third parties integrate with HumanityOS. API key management UI, webhook endpoints, rate limiting. Turns HOS into a platform." \
  backlog medium '["scope:city","infrastructure","backend"]'

echo ""
echo "All tasks created."
