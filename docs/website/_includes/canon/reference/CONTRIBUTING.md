# Contributing to Humanity

Thank you for your interest. This guide gets you from zero to making real contributions.

**New here?** Start with [ONBOARDING.md](ONBOARDING.md) for the full picture. Come back here when you're ready to contribute code or content.

---

## Quick Start

```bash
# 1. Clone
git clone https://github.com/Shaostoul/Humanity.git
cd Humanity

# 2. Build and run the server
cargo build --release -p humanity-relay
./target/release/humanity-relay
# Server starts at http://localhost:3210

# 3. Open the chat
# Visit http://localhost:3210 in your browser — that's it.
# No npm install, no build step, no Docker.
```

The HTML/CSS/JS client needs no build step. Edit a `.js` or `.html` file, refresh the browser — done.

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

```
Humanity/
├── ui/                 ← Application interface (browser + Tauri WebView + in-game GUI)
│   ├── chat/           ← Chat page + all chat JS/CSS
│   │   ├── index.html
│   │   ├── app.js          ← Core: state, connect, handleMessage, sendMessage
│   │   ├── chat-*.js       ← Feature modules (messages, dms, social, ui, voice, profile, p2p)
│   │   ├── crypto.js       ← Ed25519/ECDH/AES
│   │   └── *.css           ← 7 component CSS files (base, layout, sidebar, messages, etc.)
│   ├── pages/          ← Standalone pages (tasks, maps, settings, etc.)
│   ├── activities/     ← Game + real-world tools (gardening, download, etc.)
│   └── shared/
│       ├── shell.js        ← Nav bar shared by every page — edit here to add nav items
│       ├── settings.js     ← Theme/font persistence
│       └── theme.css       ← CSS variables (colors, fonts, spacing)
├── server/             ← Rust relay server
│   └── src/
│       ├── main.rs         ← Router, startup, axum
│       ├── relay.rs        ← WebSocket message handling (~5600 lines)
│       ├── handlers/       ← Extracted relay helpers
│       │   ├── broadcast.rs
│       │   ├── federation.rs
│       │   └── utils.rs
│       ├── storage/        ← SQLite domain modules (14 files)
│       └── api.rs          ← HTTP REST API
├── engine/             ← Rust game engine + systems
│   ├── src/            ← Renderer, ECS, physics, audio, input, hot-reload
│   ├── crates/         ← 19 sub-crates (core, modules, persistence)
│   └── src/systems/    ← Game systems (farming, construction, inventory, etc.)
├── data/               ← Hot-reloadable game data (CSV, TOML, RON, JSON)
├── assets/             ← All shared media (icons, shaders, models, textures, audio)
├── docs/               ← All docs (design, accord, history, website)
└── app/                ← Tauri v2 desktop app
```

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

2. **Add the nav button** in `shared/shell.js` (two places):
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

The relay server lives at `server/`.

```bash
# Check (fast)
cargo check -p humanity-relay

# Build
cargo build -p humanity-relay

# Run
cargo run -p humanity-relay
```

**relay.rs** is the WebSocket handler. It's one large `handle_connection()` function with a match statement routing message types. When you add a new message type:
1. Add the variant to the `RelayMessage` enum at the top of the file
2. Add the match arm in `handle_connection()`
3. Add any storage operations to the appropriate `storage/` module

**storage/** is split by domain. Add new SQL queries to the module that matches the domain (e.g., social features → `storage/social.rs`).

---

## Authority Model

This repo follows a layered authority model. Lower layers cannot contradict higher layers:

1. **`accord/`** — Civilizational principles. The highest authority. Changes require deep justification.
2. **`design/`** — Technical architecture and system constraints. Changes need design rationale.
3. **Code and content** — Implementation. Must conform to the layers above.

In practice: if you're adding a feature, check `design/` for relevant specs. If you're changing something that might conflict with the Accord, read it first.

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
5. `cargo check -p humanity-relay` passes with no new errors
6. Submit a PR with a clear description of what changed and why

If you're unsure whether a change is the right approach, open an issue or ask in the chat first. Small PRs are easier to review than large ones.

---

## Review Philosophy

- Clarity beats cleverness
- Explicit beats implicit
- Stability beats speed
- Revision beats rigidity

The project values long-term integrity. A feature that works simply and predictably is better than one that's sophisticated and fragile.
