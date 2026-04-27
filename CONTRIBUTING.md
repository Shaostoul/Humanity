# Contributing to Humanity

Thank you for your interest. This guide gets you from zero to making real contributions.

**New here?** Start with [ONBOARDING.md](ONBOARDING.md) for the full picture. Come back here when you're ready to contribute code or content.

---

## Quick Start

```bash
# 1. Clone
git clone https://github.com/Shaostoul/Humanity.git
cd Humanity

# 2. Build and run the relay (headless mode — backend only)
cargo build --release --features relay --no-default-features
./target/release/HumanityOS --headless
# Relay starts at http://localhost:3210

# 3. Open the chat
# Visit http://localhost:3210 in your browser — that's it.
# No npm install, no build step, no Docker.

# 4. Or build the full desktop client (renderer + relay + game)
cargo build --release --features native
./target/release/HumanityOS
```

The HTML/CSS/JS web client needs no build step. Edit a `.js` or `.html` file, refresh the browser — done.

---

## What Needs Doing

The best way to find work is to:
1. Join the chat: [united-humanity.us/chat](https://united-humanity.us/chat) — ask "what needs doing"
2. Check [GitHub Issues](https://github.com/Shaostoul/Humanity/issues)
3. Look at `TODO` comments in the code: `grep -r "TODO\|FIXME" --include="*.js" --include="*.rs"`

Broad areas that always need help:
- **UI/UX** — every page has rough edges; designers welcome
- **Docs** — keeping these up to date as the project moves fast
- **Testing** — manual testing across browsers, devices, screen sizes
- **Rust** — relay features, storage queries, federation
- **Content** — writing, examples, tutorials

---

## Project Structure

Single Rust crate at the repo root since v0.90.0. Feature flags `native`,
`relay`, and `wasm` select what gets compiled in.

```
Humanity/
├── src/                    ← Single Rust crate. Feature-gated.
│   ├── main.rs             ← Entry point. Picks --headless (relay) or full desktop.
│   ├── lib.rs              ← Engine init, main loop.
│   ├── relay/              ← Backend (was server/src/ pre-v0.90.0).
│   │   ├── relay.rs        ← WebSocket routing (~5800 LOC).
│   │   ├── api.rs          ← REST API (~2800 LOC).
│   │   ├── core/           ← Crypto, encoding, identity, signing.
│   │   ├── handlers/       ← broadcast, federation, game_state, msg_handlers.
│   │   └── storage/        ← SQLite domain modules (~30 files).
│   ├── gui/                ← egui native UI: theme, widgets, pages.
│   ├── renderer/           ← wgpu PBR pipeline, particles, bloom, sky.
│   ├── ecs/                ← hecs ECS, components, System trait, SystemRunner.
│   ├── systems/            ← Game systems (farming, AI, vehicles, weather, etc.).
│   ├── terrain/            ← Icosphere planets, voxel asteroids, heightmaps.
│   ├── ship/               ← Ship layouts, room mesh generation.
│   ├── physics/            ← rapier3d wrapper.
│   ├── audio/              ← kira spatial audio.
│   ├── assets/             ← AssetManager, FileWatcher, hot-reload.
│   ├── net/                ← Multiplayer client + protocol.
│   └── mods/               ← Mod manifest, load order, data overrides.
├── web/                    ← Website frontend (HTML/JS/CSS, served by nginx).
│   ├── shared/
│   │   ├── shell.js        ← Nav bar shared by every page — edit here to add nav items.
│   │   ├── settings.js     ← Theme/font persistence.
│   │   └── theme.css       ← Auto-generated from data/gui/theme.ron — do not hand-edit colors.
│   ├── chat/               ← Chat client (app.js, crypto.js, chat-*.js).
│   ├── pages/              ← Standalone pages (tasks, maps, settings, etc.).
│   └── activities/         ← Game + real-world tools (gardening, download, etc.).
├── data/                   ← Hot-reloadable game/config data (CSV/TOML/RON/JSON, ~108 files).
├── schemas/                ← TOML schema definitions for data files.
├── assets/                 ← Shared media (icons, shaders, models, textures, audio).
├── docs/
│   ├── accord/             ← Humanity Accord — civilizational principles (highest authority).
│   ├── design/             ← Architecture and design docs.
│   └── history/            ← Session journals (archival).
├── scripts/                ← Build/deploy/version tooling.
└── Cargo.toml              ← Single root manifest. No workspace.
```

Binary output is `target/release/HumanityOS.exe`. Run with `--headless` for
relay-only mode (VPS, Raspberry Pi); default mode loads the full desktop client.

---

## Adding a New Page

Every page follows the same pattern. Copy an existing simple page (like `logbook.html`) and:

1. **Create the HTML file** at the repo root: `yourpage.html`
   ```html
   <!-- Required in <head>: -->
   <script src="/shared/shell.js" data-active="yourkey"></script>
   <script src="/shared/settings.js"></script>
   ```
   The `data-active` value is a short identifier used to highlight the nav button.

2. **Add the nav button** in `web/shared/shell.js` (two places):
   ```js
   // Desktop nav (around line 394):
   navTab('/yourpage', 'icon.png', 'Your Page', 'yourkey') +

   // Mobile drawer (around line 453):
   mobileLink('/yourpage', 'Your Page') +
   ```
   Icon files live in `assets/icons/`. Use an emoji string instead of a PNG if no icon exists yet.

3. **Add a nginx route** on the server:
   ```nginx
   location = /yourpage { try_files /yourpage.html =404; }
   ```

4. **Deploy**: `scp yourpage.html server:/var/www/humanity/`

That's the complete recipe. The nav auto-highlights the correct button because `shell.js` reads the `data-active` attribute.

---

## Working on the Chat Client

The chat client lives in `web/chat/`. It's split into modules loaded in order:

| File | Responsibility |
|------|---------------|
| `crypto.js` | Ed25519, ECDH, AES — all cryptographic primitives |
| `app.js` | Core state, WebSocket connection, `handleMessage()`, `sendMessage()`, `switchChannel()`, peer/channel list management |
| `chat-messages.js` | Reactions, editing, pins, typing indicator, image upload, threads |
| `chat-dms.js` | DM state, `openDmConversation()`, `addDmMessage()`, conversation list |
| `chat-social.js` | Follow/friend system, `isFriend()`, groups |
| `chat-ui.js` | Notifications, sidebar nav, search, help modal, command palette, unread indicators |
| `chat-voice.js` | Voice rooms, 1-on-1 calls, video panel, unified right sidebar |
| `chat-profile.js` | Profile edit and view modals |
| `chat-p2p.js` | P2P contact cards (signed, QR), WebRTC DataChannel |

**All modules share global scope** — functions defined in `app.js` are callable from `chat-ui.js` without imports. This is intentional; no build step required.

When adding a feature:
- If it touches message display → `chat-messages.js`
- If it touches DMs → `chat-dms.js`
- If it touches the sidebar or notifications → `chat-ui.js`
- If it touches voice/video → `chat-voice.js`
- Core WebSocket protocol → `app.js`

To extend `handleMessage()` without editing `app.js`, use the monkey-patch pattern already used throughout the modules:
```js
const _orig = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'my_new_type') {
    // handle it
    return;
  }
  _orig(msg);
};
```

---

## Working on the Rust Server

The relay server is feature-gated inside the single root crate, at `src/relay/`.

```bash
# Check (fast)
cargo check --features relay --no-default-features

# Build relay only (headless server, no GPU dependencies)
cargo build --release --features relay --no-default-features

# Run relay locally
cargo run --features relay --no-default-features -- --headless
```

For the full desktop client (renderer + relay + game) use `--features native`
instead. The desktop binary still serves the relay; `--headless` just skips the
GPU/window subsystem.

**`src/relay/relay.rs`** is the WebSocket handler. The `RelayMessage` enum at
the top defines every message type, with the dispatch loop matching on it.
When you add a new message type:
1. Add the variant to `RelayMessage`
2. Add the match arm in the dispatch loop
3. Add any storage operations to the appropriate `src/relay/storage/` module
4. Add a handler function under `src/relay/handlers/` if logic grows past a
   few lines (`msg_handlers.rs` is the catch-all)

**`src/relay/storage/`** is split by domain (~30 modules). Add new SQL queries
to the module that matches the domain (e.g., social features → `storage/social.rs`)
and use parameterised `params![]` macros — never string-format SQL.

---

## Authority Model

This repo follows a layered authority model. Lower layers cannot contradict higher layers:

1. **`docs/accord/`** — Civilizational principles. The highest authority. Changes require deep justification.
2. **`docs/design/`** — Technical architecture and system constraints. Changes need design rationale.
3. **Code and content** — Implementation. Must conform to the layers above.

In practice: if you're adding a feature, check `docs/design/` for relevant specs. If you're changing something that might conflict with the Accord, read it first.

---

## Code Style

- **JS**: No TypeScript, no bundler, no framework. Vanilla JS. Existing patterns over new ones.
- **Rust**: Standard `rustfmt` formatting. `cargo check` must pass before submitting.
- **CSS**: Component files (`messages.css`, `sidebar.css`, etc.) — keep styles with the component they style.
- **HTML**: Each page is a standalone file. Keep the `<head>` consistent with existing pages.
- **Comments**: Explain *why*, not *what*. Use `// ── Section Name ──` for section headers.

---

## Submitting Changes

1. Fork the repo
2. Create a branch: `git checkout -b feature/your-thing`
3. Make your changes
4. Test it (load the page, verify the feature works)
5. `cargo check --features relay --no-default-features` (or `--features native`) passes with no new errors
6. Submit a PR with a clear description of what changed and why

If you're unsure whether a change is the right approach, open an issue or ask in the chat first. Small PRs are easier to review than large ones.

---

## Review Philosophy

- Clarity beats cleverness
- Explicit beats implicit
- Stability beats speed
- Revision beats rigidity

The project values long-term integrity. A feature that works simply and predictably is better than one that's sophisticated and fragile.
