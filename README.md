# Humanity

**Wholesomely aiding humanity's betterment.**

An open-source cooperative project to end poverty through education and technology. Not charity — capability. Public domain. Built by volunteers. For everyone.

🌐 [united-humanity.us](https://united-humanity.us) · 💬 [Chat](https://united-humanity.us/chat) · 📦 [GitHub](https://github.com/Shaostoul/Humanity) · 💜 [Discord](https://discord.gg/9XxmmeQnWC)

---

## What's Live

### Humanity Chat
A real-time communication platform built on cryptographic identity. No accounts, no tracking, no analytics.

**Identity & Privacy**
- Ed25519 cryptographic identity — keys stored in your browser, never on our server
- Multi-device key linking — use the same identity across devices
- Key backup, export, and import
- No IP logging, no analytics, no tracking
- 18+ by-entry confirmation

**Communication**
- Channels with admin-created rooms and descriptions
- Direct messages between users
- @mentions with highlighting and notifications
- Emoji reactions (persistent, synced across sessions)
- Reply/quote system with collapsible blocks
- Message editing and deletion
- Image sharing with lazy-loaded placeholders
- Browser push notifications and notification sounds
- Typing indicators and unread markers

**Community**
- User profiles with bio and social links
- Unique pixel-art identicons per user
- Client-side user blocking
- Report system with rate limiting
- Pin system — server pins (mod/admin) and personal pins (local)

**Moderation**
- Role-based system: admin 👑, mod 🛡️, verified ✦, donor 💎
- Kick/ban with instant WebSocket disconnection
- Auto-lockdown when no mods online
- Invite codes for controlled access

### Hub
A tabbed interface with sections for Chat, Reality, Fantasy, Streams, and Debug — the foundation for everything we're building.

### Shared Design System
Centralized styles (`theme.css`) and navigation shell (`shell.js`) shared across all pages. Download page with OS auto-detection.

---

## Security

- Server-side Ed25519 signature verification on every message
- Fibonacci rate limiting + new-account slow mode
- Content Security Policy (CSP), HSTS, TLS 1.2+ only
- No IP logging — we don't store what we don't need
- Per-session upload tokens with magic-byte validation
- HMAC-SHA256 webhook verification
- Non-root systemd service with hardened sandboxing

---

## Architecture

| Component | Technology |
|-----------|-----------|
| Server | Rust (axum + tokio) |
| Client | Single-file HTML/JS |
| Identity | Ed25519 |
| Storage | SQLite |
| Transport | WebSocket |
| Layout | Cargo workspace |

The client is a single HTML file with no build step — open it in a browser and it works. The server is a Rust binary that handles WebSocket connections, message persistence, identity verification, and file uploads.

---

## What We're Building

### The Humanity Accord
Civilizational principles for cooperation at scale — across cultures, distances, and generations. A living, revisable framework.
→ [Read the Accord](accord/humanity_accord.md)

### The Humanity Network
Federated communication built on cryptographic identity. No central servers owning your data. Your identity is portable across all servers.
→ [Design specs](design/network/) · [Federation spec](design/network/server_federation.md)

### Project Universe
A free game teaching practical skills — homesteading, agriculture, building, health, survival. Learn to provide for yourself and your community.

---

## Host Your Own Server

Anyone can run a Humanity Network server. No permission needed.

1. Clone: `git clone https://github.com/Shaostoul/Humanity.git`
2. Build: `cargo build --release -p humanity-relay`
3. Run: `./target/release/humanity-relay`
4. Put it behind nginx with TLS (Let's Encrypt is free)
5. Share your URL — people connect with their existing keypair

→ **[Full self-hosting guide](SELF-HOSTING.md)** — production setup, nginx config, systemd, federation, admin commands

Want verified status? Contact [@Shaostoul on X](https://x.com/Shaostoul). Publicly adopt the [Humanity Accord](accord/humanity_accord.md) for highest trust tier.

→ [Federation & trust tiers](design/network/server_federation.md)

---

## Get Involved

- 💬 **Chat:** [united-humanity.us/chat](https://united-humanity.us/chat) — no account needed
- 💜 **Discord:** [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC)
- 📦 **GitHub:** [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)
- 📖 **Docs:** [shaostoul.github.io/Humanity](https://shaostoul.github.io/Humanity)

**Contribute** — Writers, designers, developers, educators, translators. Check the issues or show up and ask what needs doing.

**Donate** — Servers and infrastructure cost money. Every dollar goes toward development and hosting.
→ [GitHub Sponsors](https://github.com/sponsors/Shaostoul) · [Ko-fi](https://ko-fi.com/shaostoul) · [Patreon](https://www.patreon.com/c/Shaostoul)

---

## Links

- 🎥 [YouTube](https://youtube.com/@Shaostoul) · 📺 [Twitch](https://twitch.tv/Shaostoul) · 🟢 [Rumble](https://rumble.com/user/Shaostoul)
- 𝕏 [X/Twitter](https://x.com/Shaostoul) · 📷 [Instagram](https://instagram.com/shaostoul) · 🔵 [Bluesky](https://bsky.app/profile/shaostoul.bsky.social)
- 🟠 [Reddit](https://reddit.com/user/Shaostoul) · 👤 [Facebook](https://www.facebook.com/2571477392923654) · 🎮 [Steam](https://steamcommunity.com/id/Shaostoul)

---

## Desktop App

A native desktop app is available for Windows, macOS, and Linux — built with [Tauri v2](https://tauri.app/). It wraps the web client in a native window, so updates are instant without needing to update the app.

**[Download the latest release →](https://github.com/Shaostoul/Humanity/releases/latest)**

To build from source, see [`desktop/README.md`](desktop/README.md).

---

## License

This work is released into the **public domain** under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

No permission required. No attribution required. This belongs to humanity — present and future.
