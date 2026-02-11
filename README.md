# Humanity

**Wholesomely aiding humanity's betterment.**

---

## What is this?

Humanity is a cooperative project to end poverty through education and technology. Not charity — capability. We build open-source tools that help people learn to provide for themselves and their communities.

The premise is simple: life is finite and sacred. We exist to exist. That means removing barriers to living — poverty, ignorance, isolation — is the work. Not because of ideology, but because it's practical. People who can feed themselves, build shelter, stay healthy, and communicate freely don't need to be saved. They need tools and knowledge. That's what we're building.

This project started in 2019 when Michael Boisson, after a near-death experience in 2017, decided to stop asking "what should I do?" and start building what actually matters. Everything here is open source, public domain, and built by volunteers. No venture capital. No shareholders. No exit strategy. Just people building for people.

---

## What's live now

**[chat.united-humanity.us](https://chat.united-humanity.us)** — The Humanity Network chat is live.

**Identity & Privacy:**
- No accounts, no tracking, no analytics, no IP logging
- Ed25519 cryptographic identity — keys stored in your browser, never on our server
- Signed messages with verification badges
- 18+ only (by-entry confirmation)

**Communication:**
- Multiple channels with admin-created rooms
- Direct messages between users
- @mentions with highlighting and notifications
- Emoji reactions (persistent, synced across sessions)
- Reply/quote system with collapsible blocks
- Message editing and deletion
- Image sharing with lazy-loaded placeholders
- Browser push notifications + unread indicators
- Typing indicators

**Community:**
- User profiles (bio + social links)
- Unique pixel-art identicons per user
- Client-side user blocking (invisible to blocked user)
- Report system with rate limiting
- Full user list with online/offline status

**Moderation & Security:**
- Role-based system: admin 👑, mod 🛡️, verified ✦, donor 💎
- Kick/ban with instant WebSocket disconnection
- Auto-lockdown when no mods online + manual lockdown mode
- Invite codes for controlled access during lockdown
- Fibonacci rate limiting + new-account slow mode
- Upload validation (magic bytes + 500MB disk cap + per-user FIFO)
- WebSocket origin checking, CSP headers, HSTS, TLS 1.2+

**Pins:**
- Server pins (admin/mod) — survive message wipes
- Personal pins (per-user, stored locally) — only you see them
- Collapsible pin bar under channel header

We're building the communication layer first. If people can't talk to each other freely and privately, nothing else matters. Come say hello.

---

## What we're building

### The Humanity Accord
Civilizational principles for how humans can cooperate at scale — across cultures, distances, and generations — without domination, exploitation, or violence as default tools. Not a manifesto. A living, revisable framework.
→ [Read the Accord](accord/humanity_accord.md)

### The Humanity Network
A federated communication protocol built on cryptographic identity. No central servers owning your data. No accounts. Ed25519 means you prove who you are with math, not with a password stored on someone else's computer. Anyone can host a server. Servers are meeting places, not gatekeepers — your identity is portable across all of them. Trust is tiered: verified servers that adopt the Humanity Accord rank highest.
→ [Design specs](design/network/) · [Federation spec](design/network/server_federation.md)

### Project Universe
A free, open-source game teaching practical skills — homesteading, building, agriculture, health, survival — so anyone, anywhere, can learn to provide for themselves and their community. Think Minecraft meets real-world education. The game won't replace doing the real thing, but it can teach you how before you need to.

---

## Architecture

This repository is organized by a strict authority stack. Higher layers govern lower layers. This prevents drift over time — principles stay principles, specs stay specs, and code serves both.

```
accord/   → Human-facing civilizational principles (highest authority)
design/   → Technical constraints, schemas, system specifications
data/     → Canonical structured data that must validate against schemas
engine/   → Deterministic simulation implementation (Rust)
```

Lower layers may not contradict higher layers. If two files disagree, the higher layer is correct. This structure exists so the project can grow without losing coherence.

---

## Tech stack

| Component | Technology |
|-----------|-----------|
| Language | Rust |
| Identity | Ed25519 |
| Hashing | BLAKE3 |
| Encryption | XChaCha20-Poly1305 |
| Serialization | CBOR |
| Transport | WebSocket relay |
| Storage | SQLite |

---

## Host your own server

Anyone can run a Humanity Network server. No permission needed.

1. Build the relay: `cargo build --release -p humanity-relay`
2. Put it behind nginx with TLS (Let's Encrypt is free)
3. Share your URL — people connect with their existing keypair

Want verified status? Contact [@Shaostoul on X](https://x.com/Shaostoul). Publicly adopt the [Humanity Accord](accord/humanity_accord.md) for highest trust tier.

→ [Federation & trust tiers](design/network/server_federation.md)

---

## Get involved

**Chat with us** — The fastest way to get involved. No account needed.
→ [chat.united-humanity.us](https://chat.united-humanity.us)

**Join the Discord** — Longer-form discussion, community, and coordination.
→ [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC)

**Contribute** — Writers, designers, developers, educators, translators. Check the issues or just show up and ask what needs doing.
→ [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)

**Donate** — This project is built by volunteers, but servers and infrastructure cost money. Every dollar goes toward development and hosting.
→ [GitHub Sponsors](https://github.com/sponsors/Shaostoul) · [Ko-fi](https://ko-fi.com/shaostoul)

---

## License

This work is released into the **public domain** under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

No permission required. No attribution required. This belongs to humanity — present and future.

---

## Links

### Project
- 🌐 **Website:** [united-humanity.us](https://united-humanity.us)
- 💬 **Humanity Chat:** [chat.united-humanity.us](https://chat.united-humanity.us)
- 📖 **Docs:** [shaostoul.github.io/Humanity](https://shaostoul.github.io/Humanity)
- 📦 **GitHub:** [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)

### Video
- 🎥 **YouTube:** [@Shaostoul](https://youtube.com/@Shaostoul)
- 📺 **Twitch:** [twitch.tv/Shaostoul](https://twitch.tv/Shaostoul)
- 🟢 **Rumble:** [rumble.com/user/Shaostoul](https://rumble.com/user/Shaostoul)

### Social
- 𝕏 **X / Twitter:** [x.com/Shaostoul](https://x.com/Shaostoul)
- 📷 **Instagram:** [instagram.com/shaostoul](https://instagram.com/shaostoul)
- 🔵 **Bluesky:** [shaostoul.bsky.social](https://bsky.app/profile/shaostoul.bsky.social)
- 🟠 **Reddit:** [reddit.com/user/Shaostoul](https://reddit.com/user/Shaostoul)
- 💜 **Discord:** [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC)
- 👤 **Facebook:** [facebook.com/Shaostoul](https://www.facebook.com/2571477392923654)

### Gaming
- 🎮 **Steam:** [steamcommunity.com/id/Shaostoul](https://steamcommunity.com/id/Shaostoul)
- 🎯 **Nexus Mods:** [nexusmods.com/profile/Shaostoul](https://www.nexusmods.com/profile/Shaostoul)
- 🕹️ **itch.io:** [shaostoul.itch.io](https://shaostoul.itch.io)

### Support
- ❤️ **GitHub Sponsors:** [github.com/sponsors/Shaostoul](https://github.com/sponsors/Shaostoul)
- ☕ **Ko-fi:** [ko-fi.com/shaostoul](https://ko-fi.com/shaostoul)
- 🎭 **Patreon:** [patreon.com/Shaostoul](https://www.patreon.com/c/Shaostoul)
