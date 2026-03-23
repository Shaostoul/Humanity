# Humanity

**Wholesomely aiding humanity's betterment.**

An open-source cooperative project to end poverty through education and technology. Not charity — capability. Public domain. Built by volunteers. For everyone.

🌐 [united-humanity.us](https://united-humanity.us) · 💬 [Chat](https://united-humanity.us/chat) · 📦 [GitHub](https://github.com/Shaostoul/Humanity) · 💜 [Discord](https://discord.gg/9XxmmeQnWC)

---

## What It Is

Humanity is two things built together:

1. **HumanityOS** — a real-world platform for communication, collaboration, and life management. Chat, DMs, voice calls, project boards, marketplace, skill tracking, inventory, maps. Think of it as an operating system for your life that you actually own.

2. **Project Universe** — a free game that teaches practical skills (homesteading, agriculture, building, health) by using the same underlying data layer as the real platform. Your in-game skills reflect real-world capability.

Both share the same server, identity system, and data layer. The game is how people learn to use the tools for real.

---

## What's Live

### Pages (18 standalone pages at `/`)

| URL | Page | What it does |
|-----|------|--------------|
| `/chat` | Network | Chat, DMs, voice, video, streaming |
| `/home` | Home | Multi-location manager (real, digital, fleet homes) |
| `/profile` | Profile | Your identity, bio, links, streaming platforms |
| `/skills` | Skills | Browse and manage skills by domain |
| `/dashboard` | Dashboard | 10 customizable widget types |
| `/inventory` | Inventory | Track what you own |
| `/equipment` | Equipment | Gear and loadout management |
| `/quests` | Quests | Mission and goal tracker |
| `/calendar` | Calendar | Events and planning |
| `/logbook` | Logbook | Personal journal |
| `/systems` | Systems | Kanban project board with task IDs |
| `/maps` | Maps | World map, earth view, sky view |
| `/market` | Market | Peer-to-peer listings for goods, services, skills |
| `/learn` | Learn | Web directory — 52 curated sites |
| `/knowledge` | Knowledge | Knowledge base and wiki |
| `/streams` | Streams | Live streaming interface |
| `/settings` | Settings | Themes, fonts, account management |
| `/ops` | Ops | Server admin and debug tools |

### Communication (the core, fully working)
- **Channels** — admin-created rooms with descriptions and categories
- **E2E encrypted DMs** — ECDH P-256 + AES-256-GCM, server never sees plaintext
- **Threaded replies** — reply to any message with collapsible threads
- **Voice channels** — persistent, always-on WebRTC mesh rooms
- **1-on-1 video calls** — WebRTC peer-to-peer with audio, video, screen share, PiP
- **@mentions**, emoji reactions, message editing, message search
- **Image sharing** with lazy-loaded placeholders
- **Browser push notifications** and 6 notification sound options
- **Typing indicators** and unread markers

### Identity & Privacy
- **Ed25519 cryptographic identity** — keys stored in your browser, never on server
- **Multi-device key linking** — same identity across devices
- **Device management** — list, label, and revoke linked keys
- **Key backup, export, and import**
- **Encrypted user data sync** — settings, follows, profile encrypted at rest
- **No IP logging, no analytics, no tracking**

### Social
- **Follow/friend system** — mutual follow = friends, friends unlock DMs
- **Friend codes** — 8-character codes with 24-hour expiry, auto-mutual-follow
- **User profiles** with bio, pronouns, location, website, privacy controls
- **Unique pixel-art identicons** per user
- **Groups** — private group conversations
- **Client-side user blocking** and report system

### Productivity Tools
- **Project board** — kanban-style with visible task IDs
- **Marketplace** — peer-to-peer listings, kiosks, donation pricing presets
- **Asset library** — file upload, browse, tag, preview
- **118-element catalog**, 44 materials, processing chains
- **Browse directory** — 52 curated sites with uptime pings and domain info
- **Dashboard** — 10 widget types, customizable layout
- **Notes**, **Todos**, **Garden tracker**

### Platform
- **PWA installable** — works on mobile, add to homescreen
- **Desktop app** — Native Rust/wgpu/egui binary for Windows, macOS, Linux
- **Command palette** — quick navigation
- **Dark/light themes**, accent colors, font size controls
- **Auto-reload on deploy** — no manual refresh needed

### Moderation & Federation
- Role-based: admin 👑, mod 🛡️, verified ✦, donor 💎
- Kick/ban, invite codes, auto-lockdown when no mods online
- **Phase 1 federation** — anyone can host; servers discover each other
- Single binary, zero dependencies, under 10 minutes to self-host
- Verified servers that adopt the [Humanity Accord](accord/humanity_accord.md) earn highest trust

---

## Architecture

| Component | Technology |
|-----------|-----------|
| Server | Rust (axum + tokio) |
| Storage | SQLite (rusqlite) |
| Transport | WebSocket + WebRTC |
| Client | Plain HTML/CSS/JS — no build step |
| Identity | Ed25519 (signing) + ECDH P-256 (encryption) |
| Desktop | Native binary (Rust/wgpu + egui GUI) |
| Hosting | nginx + systemd |
| Layout | Cargo workspace |

### Codebase Structure

```
Humanity/
├── server/                    ← Rust relay server (axum/tokio, SQLite)
│   └── src/
│       ├── main.rs            ← Entry point, routing, axum setup
│       ├── relay.rs           ← WebSocket handler (~5800 lines, message routing)
│       ├── handlers/          ← Extracted relay helpers
│       ├── storage/           ← SQLite domain modules (17 files)
│       └── api.rs             ← HTTP REST API endpoints
├── native/                    ← Rust desktop client (egui GUI + wgpu game engine)
│   ├── src/                   ← Engine source (gui, renderer, terrain, ship, assets, systems)
│   └── crates/                ← 19 sub-crates (core, modules, persistence)
├── web/                       ← Web interface (browser + WebView)
│   ├── chat/                  ← Chat client (app.js, crypto.js, chat-*.js)
│   ├── pages/                 ← Standalone pages (tasks, maps, settings, etc.)
│   ├── activities/            ← Game + real-world tools (gardening, download, etc.)
│   └── shared/                ← shell.js, events.js, theme.css
├── data/                      ← Hot-reloadable game data (CSV, TOML, RON, JSON)
├── assets/                    ← All shared media (icons, shaders, models, textures, audio)
├── docs/                      ← All documentation (design, accord, history, website)
│   └── accord/                ← Humanity Accord (civilizational principles)
└── SELF-HOSTING.md            ← Production server setup guide
```

The client uses **no build step** — plain `<script src="">` tags load modules in dependency order. All modules share global scope (no ES modules). When in doubt about a function's location, `grep` the web/ directory.

### Adding a New Page

1. Create `yourpage.html` at the repo root (copy any existing page as template)
2. Add `<script src="/shared/shell.js" data-active="yourkey"></script>` in `<head>`
3. Add `navTab('/yourpage', 'icon.png', 'Label', 'yourkey')` to `shared/shell.js`
4. Add a mobile drawer entry to `shared/shell.js`
5. Add nginx route: `location = /yourpage { try_files /yourpage.html =404; }` on the server
6. Deploy: `scp yourpage.html server:/var/www/humanity/`

---

## Security

- Server-side Ed25519 signature verification on every message
- E2E encrypted DMs (ECDH P-256 + AES-256-GCM) — server never sees plaintext
- Encrypted user data sync — profile, settings, follows encrypted at rest
- Fibonacci rate limiting + new-account slow mode
- Content Security Policy, HSTS, TLS 1.2+ only
- No IP logging
- Per-session upload tokens with magic-byte validation
- HMAC-SHA256 webhook verification
- Non-root systemd service with hardened sandboxing
- Full audit: [`SECURITY_AUDIT.md`](SECURITY_AUDIT.md)

---

## Transparent AI Development

This project is pioneering **fully transparent AI development**. Our AI assistant Heron 🪶 (named after Hero of Alexandria) operates with complete openness — public memory files, no black box, every decision documented.

→ [See live AI memory](memory/) · [AI workspace rules](AGENTS.md)

---

## Host Your Own Server

Anyone can run a Humanity relay. No permission needed.

```bash
git clone https://github.com/Shaostoul/Humanity.git
cd Humanity
cargo build --release -p humanity-relay
./target/release/humanity-relay
```

Put it behind nginx with TLS (Let's Encrypt is free). People connect with their existing keypair — no migration needed.

→ **[Full self-hosting guide](SELF-HOSTING.md)** — nginx config, systemd, federation, admin commands

Want verified federation status? Publicly adopt the [Humanity Accord](accord/humanity_accord.md) and contact [@Shaostoul](https://x.com/Shaostoul).

---

## Get Involved

- 💬 **Chat first** — [united-humanity.us/chat](https://united-humanity.us/chat) — no account needed, just a username
- 💜 **Discord** — [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC)
- 📦 **GitHub** — [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)
- 📖 **New contributors** — start with [ONBOARDING.md](ONBOARDING.md)

**Writers, designers, developers, educators, translators** — or just someone who cares. Show up and ask what needs doing.

**Donate** — Every dollar goes toward development and hosting.
→ [GitHub Sponsors](https://github.com/sponsors/Shaostoul)

---

## Links

🎥 [YouTube](https://youtube.com/@Shaostoul) · 📺 [Twitch](https://twitch.tv/Shaostoul) · 🟢 [Rumble](https://rumble.com/user/Shaostoul) · 𝕏 [X/Twitter](https://x.com/Shaostoul) · 📷 [Instagram](https://instagram.com/shaostoul) · 🔵 [Bluesky](https://bsky.app/profile/shaostoul.bsky.social) · 🟠 [Reddit](https://reddit.com/user/Shaostoul)

---

## License

Public domain — [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/). No permission required. No attribution required. This belongs to humanity.
