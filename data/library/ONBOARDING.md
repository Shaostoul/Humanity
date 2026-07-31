# Humanity: The Fuller Story

You've read [getting-started.md](getting-started.md). This is the longer version, the
two layers, the Humanity Accord, and where the project actually stands today.

If you want to build or contribute rather than just use it, this is the wrong door,
go to **[docs/contributor/00-START-HERE.md](../contributor/00-START-HERE.md)** instead.
It has the real architecture, file map, and a "first day in this repo" walkthrough kept
current with the code (this document is not, by design, it is for users, not builders).

---

## What Is This Project?

**Humanity** is an open-source platform with two interconnected layers:

### Layer 1: HumanityOS (the real-world platform)

A communication and life-management system you actually own. Think Discord + Notion +
life-OS, but:
- No accounts, your identity is a cryptographic key that lives on your device
- No tracking, no ads, no central authority
- Federated, anyone can host a server; users keep their identity across all servers
- Public domain (CC0), no permission required to use, fork, or deploy

What's live right now: chat channels, end-to-end encrypted DMs, voice/video calls,
streaming, follow system, project boards, marketplace, asset library, inventory
tracking, skills, maps, calendar, dashboard, and more. See
**[docs/STATUS.md](../STATUS.md)** for the full, currently-accurate feature inventory
and **[docs/ROADMAP.md](../ROADMAP.md)** for what's coming next; both are living
documents updated every release, unlike the snapshot below.

### Layer 2: The simulation (the game)

A free game teaching practical skills, homesteading, agriculture, building, health,
survival. The game uses the same data layer as the platform. Skills you develop in the
game reflect real-world capability. The game is how people learn to use the tools for
real. Toggle between the two with the Real/Sim switch (see `CLAUDE.md`'s "Real/Sim
toggle" section if you're curious why it isn't called "Game").

### The Humanity Accord

A living document of civilizational principles, non-negotiable ethical foundations that
all servers must adopt to earn verified status. Think of it as the constitution.
Everything in this project must conform to it. Read it at
**[docs/accord/humanity_accord.md](../accord/humanity_accord.md)**.

---

## Why Does It Exist?

In 2017, Michael Boisson (Shaostoul) nearly died. That experience stripped away the
noise and left one clear answer: help people become capable of helping themselves.

Poverty is not just lack of money, it's lack of capability. People trapped in systems
they can't understand, knowledge they can't access, skills they never learned. The
solution is education, tools, and community built at civilizational scale.

Everything here is public domain. This belongs to everyone, present and future.

---

## The State of the Project

The platform is **live and actively used** at
[united-humanity.us](https://united-humanity.us). It ships new releases continuously
(hundreds shipped since early 2026), so any specific feature count or file layout
written here would be stale within days. For a snapshot that stays accurate, read:

- **[docs/STATUS.md](../STATUS.md)**, what's built, partial, or planned, feature by feature
- **[docs/ROADMAP.md](../ROADMAP.md)**, the public strategic roadmap
- **[docs/PAGES.md](../PAGES.md)**, every page in the app (native + web) with its purpose

What's solid and working: chat, DMs, voice/video, streaming, the full communication
layer; post-quantum cryptographic identity and end-to-end encryption; federation
(server discovery, trust tiers); construction/home-building; farming and skills;
inventory and marketplace.

What's actively being built: check the top of **[docs/PRIORITIES.md](../PRIORITIES.md)**,
that is the single ranked source for "what's happening right now."

---

## The Identity System

Understanding this unlocks the whole platform.

Every user's chat identity is a **Dilithium3 / ML-DSA-65 keypair** (FIPS 204,
post-quantum), derived deterministically from a BIP39 24-word seed phrase generated on
first use:
- **Private key**, never leaves your device
- **Public key**, your identity; also your "user ID" (a Dilithium3 hex string; the app shows the shorter `did:hum:...` form for display)

Every message is signed with the private key. The server verifies the signature before
accepting the message. This means no passwords, no accounts, the server cannot
impersonate you, and you own your identity completely.

For encrypted DMs, a **Kyber768 / ML-KEM-768 keypair** (FIPS 203, post-quantum) handles
key exchange, deriving an AES-256-GCM key so the server never sees DM content.

This is a summary. The canonical, always-current crypto inventory (with exact algorithm
names, file locations, and activation status) lives in the "Cryptography" section of
**[CLAUDE.md](../../CLAUDE.md)**, read that before quoting any algorithm in your own
writing.

---

## The Accord (Read Before Proposing Changes)

The [Humanity Accord](../accord/humanity_accord.md) defines what this project must
never do. It's short. Read it.

Non-negotiable prohibitions include anything involving sexual violence, child
exploitation, slavery, political coercion, and a handful of others. Every server that
joins the network must adopt it to reach verified status.

The Accord isn't ideology, it's a minimal floor that allows people from radically
different backgrounds to cooperate.

---

## Want to Help Build It?

Good work speaks for itself. Start at
**[docs/contributor/00-START-HERE.md](../contributor/00-START-HERE.md)**, it has the
real, currently-maintained architecture map, module layout, and a first safe task to
try. This document intentionally stops here, anything more technical belongs there, not
in the user-facing folder.

---

## Getting Help

- **Chat**, [united-humanity.us/chat](https://united-humanity.us/chat), real-time, best for quick questions
- **Discord**, [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC), longer-form discussion and voice chat
- **GitHub Issues**, for bugs and feature proposals: [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)

Don't overthink it. Show up, ask questions, start somewhere small.
