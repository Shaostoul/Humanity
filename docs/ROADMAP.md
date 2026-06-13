# HumanityOS Roadmap

> **This is the single source of truth for where HumanityOS is going.** It is both
> the public roadmap (so anyone can see what we are building and why) and the build
> to-do list (the maintainers work the items here in order). When the to-do list
> changes, this file changes, and the public roadmap at
> [united-humanity.us/roadmap](https://united-humanity.us/pages/roadmap.html)
> updates with it. One list. Everyone sees the same thing.

**Mission:** end poverty and unite humanity, by giving every person (and every AI)
free tools to meet their own needs: water, energy, food, shelter, knowledge, and a
voice. Not a startup. Infrastructure for civilization. Everything here serves that.

## How to read this

Every item carries a status tag:

| Tag | Meaning |
|-----|---------|
| `[done]` | Shipped and live. You can use it today. |
| `[building]` | Actively being worked on right now. |
| `[next]` | Immediately queued, starts as soon as the active work lands. |
| `[planned]` | Designed and committed, not started yet. |
| `[future]` | On the long horizon. Real, but not scheduled. |

Versions in parentheses (like `v0.342`) mark when something shipped. The full
release history lives in `git log` and `docs/history/`.

This roadmap is **strategic** (the themes below). The day-to-day "what gets touched
this hour" detail lives in the maintainers' journal
(`data/coordination/orchestrator_state.json`).

---

## Right now

The active queue, strict-ranked. The top item is what is being worked on next.

1. `[building]` **Documentation refactor.** Audience-first structure (separate clear
   paths for users, admins, AI, and contributors), an em-dash-free content sweep,
   and this roadmap becoming the one canonical to-do list. In progress this session.
2. `[next]` **Next gameplay arc (operator's pick):** either the garden plot-types
   registry (soil / sand / pots / trays / direct-sow as data-driven growing methods)
   or the First Playable arc (walk-up crafting stations, a 3D heads-up vitals
   display, death and respawn, a guided first day).
3. `[next]` **Security sprint tail.** Sign each published release; ship the
   member-directory opt-out toggle; enable GitHub branch protection.

---

## Survival and self-sufficiency

The heart of the mission: in-game systems that map to real-world systems a person can
actually build, then a parts list to build them, then (last) the app monitoring and
automating the real thing.

- `[done]` Crafting loop, mine to refine to craft, with a 2-tier ore to ingot to
  alloy chain (v0.329 to v0.333).
- `[done]` Cooking, nutrition, and vitals: eat, decay, conditions, well-fed (v0.330).
- `[done]` Gardening: plant, water, harvest, with a Garden panel (v0.331).
- `[done]` Drone and asteroid mining: commission, trip, mine a finite asteroid,
  deliver (v0.332).
- `[done]` Skills and XP, with tech-unlock gating and a quest chain (v0.340 to v0.342).
- `[done]` Aeroponic towers end to end: curated configs, a compatibility check, a 3D
  placeholder, the inventory tree, plant to grow to harvest, and a real-world parts
  list (v0.382 to v0.394).
- `[done]` Seed economy: survival mode consumes seeds, a starter grant, and a harvest
  that returns seeds so the loop sustains itself (v0.398 to v0.399).
- `[done]` Homestead designs: the Fibonacci self-sufficient blueprint as a browsable
  design, plus a self-sufficiency model (coupled energy / water / food / waste loops)
  (v0.379 to v0.380).
- `[done]` Save and load: offline progress (inventory and skills) now persists between
  sessions (v0.381).
- `[next]` Garden plot-types registry: generalize "tower" into a data-driven growing
  method (aeroponic / soil / sand / pot / raised-bed / direct-sow / trays), each
  moddable, none hardcoded.
- `[planned]` First Playable arc: persistence depth, a 3D vitals HUD, walk-up
  stations, death and respawn, and a guided first day.
- `[future]` Real-hardware control layer: bind a home to real monitoring and
  automation hardware (for example real aeroponics, solar, water), the north star
  where the game becomes the control panel for your actual homestead.
- `[future]` Parts-list-to-real: for every buildable system, a refined bill of
  materials you can 3D-print, buy, or trade for.

## Identity and privacy

You own your identity and your data. No accounts, no passwords, no tracking. Even the
server operator cannot read your private messages.

- `[done]` Post-quantum identity: a Dilithium3 / ML-DSA-65 key derived from a BIP39
  24-word seed (which is also your Solana wallet). The canonical crypto details live
  in the Cryptography section of `CLAUDE.md`.
- `[done]` End-to-end encrypted DMs: pure Kyber768 / ML-KEM-768 to BLAKE3-KDF to
  AES-256-GCM. The relay stores only ciphertext.
- `[done]` Encrypted vault (PBKDF2 600k both clients) with three auto-unlock modes.
- `[done]` BIP39 24-word recovery and social recovery, so losing a device never locks
  you out forever.
- `[done]` Proof-of-possession at connect: a signed challenge-response before the
  relay binds your identity (closes identity spoofing).
- `[done]` Release signing: a hybrid Ed25519 + Dilithium3 signature on every release
  (both must verify), so a compromised GitHub or a stray tag can never push code to
  your machine (v0.418 to v0.421).
- `[next]` Member-directory opt-out: a privacy setting so you can join a server
  without appearing in its public member list, with a user-facing toggle.
- `[future]` Multi-device key sync: authorize a second device by QR or short code over
  an encrypted channel (part of the device mesh).
- `[future]` STARK selective disclosure: prove a fact about yourself without revealing
  the underlying data.

## Communication and federation

Talk, meet, and organize without a gatekeeper. Servers are meeting places, not owners
of your identity, and your identity is portable across all of them.

- `[done]` Chat: channels, threaded replies, reactions, pins, search, and direct
  messages.
- `[done]` Voice and video calling on the web (WebRTC peer-to-peer with screen share).
- `[done]` Peer-to-peer groups: signed objects, end-to-end encryption, and a group
  that behaves like a channel, on both web and native (Phases 1 and 2).
- `[done]` Signed profile replication: your profile gossips between servers, so there
  is no single home server to lose.
- `[building]` Web-to-native parity: rebuilding the web chat view to mirror the native
  app one-to-one (Track W).
- `[next]` Federation activation: an admin UI to add and trust peer servers, a second
  operator-run relay, end-to-end federation testing, then vetted third-party peers.
- `[planned]` Peer-to-peer groups Phases 3 to 5: relay-independent transport (a group
  survives even if its home relay dies) and serverless discovery.
- `[planned]` Native voice and streaming: the WebRTC transport that native currently
  lacks (web users have voice today; native users are observer-only).
- `[planned]` Calendar with RSVP: events, recurring schedules, group calendars,
  reminders.
- `[future]` Mobile clients for Android and iOS.

## Governance and economy

Turn groups into cooperatives. Coordinate work, trade fairly, and govern by a
constitution rather than a promise.

- `[done]` The Humanity Accord: a CC0 constitution that binds the platform, readable
  in-app.
- `[done]` Tasks and mission control: a Fibonacci-scoped board for coordinating work
  at every scale.
- `[done]` Marketplace: listings, images, reviews, and seller ratings.
- `[done]` Reputation: peer-endorsed skills signed with your identity key.
- `[planned]` Group governance: proposals, ranked-choice voting, and quorum rules,
  with every vote signed and tamper-evident.
- `[planned]` Native trade UI completion (the page exists; the trade events need
  wiring).
- `[future]` Learning paths: complete a module, get peer-endorsed, unlock the next.
- `[future]` AI agent governance: evolve Article 14 of the Accord from documented
  intent into enforced rules with appeals as more AI participants join.

## The simulation

A 3D educational world where you learn real survival and production skills by doing.
The game teaches the homestead; the homestead is real.

- `[done]` Engine: a wgpu PBR renderer, an ECS, and 40-plus game systems wired in.
- `[done]` World: icosphere planets with level-of-detail, voxel asteroids, and a
  ship-at-origin starting world.
- `[done]` The full production, survival, and progression sandbox (the gameplay-loop
  arc, v0.329 to v0.342).
- `[building]` UI consolidation: a slim top navigation, universal reusable widgets,
  the Humanity Mission Dashboard, and the in-app Library.
- `[planned]` First Playable: walk-up stations, a 3D vitals HUD, and a guided first
  day (shared with the survival theme above).
- `[future]` Game-world depth: more content, characters, and deeper coupled systems.
- `[future]` Real-terrain world generation from real elevation data (USGS / SRTM /
  Copernicus) keyed to a place's latitude and longitude.
- `[future]` VR support and a "boot straight into Play" mode.

## Accessibility and onboarding

The mission requires that anyone, of any age or background or ability, can use this.
This layer is not optional.

- `[done]` Accessibility modes: high-contrast, colorblind, and reduced-motion.
- `[done]` Glossary overlay on the web (150-plus plain-language term definitions).
- `[done]` Localization in 5 languages (English, Spanish, French, Japanese, Chinese).
- `[done]` The Mission Dashboard: makes "what I am doing" and "what we are doing
  together" instantly clear to anyone.
- `[building]` Three-audience onboarding: clear, separate getting-started guides for
  standard users, server admins, and AI agents (this refactor).
- `[next]` A tooltip on every interactive element, in plain language.
- `[planned]` A "first 5 minutes" guided tour for brand-new users.
- `[planned]` Localization expansion: Arabic, Hindi, Portuguese, Russian, German,
  Swahili, and more.
- `[planned]` A native glossary widget to match the web.
- `[planned]` A full WCAG 2.1 AA accessibility audit with fixes.

## Infrastructure and sovereignty

The platform must survive disasters, censorship, and the maintainer stepping away.
Every operator gets the same sovereignty tools, not just the original.

- `[done]` One unified binary: full desktop app, or headless relay server, from the
  same code.
- `[done]` Resilience: off-site 3-2-1 backups, SQLite corruption recovery, a liveness
  watchdog with self-heal, and multi-channel alerting.
- `[done]` Abuse resistance: fail2ban, per-key rate limits, signed-request anti-replay,
  and per-author submission quotas (the security sprint, v0.420 to v0.422).
- `[done]` Distribution: GitHub plus a self-hosted Forgejo mirror plus a BitTorrent
  seeder.
- `[building]` In-app ops console: bring every admin action into the app (the GUI-first
  mandate), paying down the CLI-only debt.
- `[planned]` Device mesh: a "My Devices" dashboard, backup designation, restore flow,
  and direct LAN sync, so your own devices back each other up.
- `[planned]` Litestream continuous replication (roughly 1-minute recovery point).
- `[planned]` GitHub branch protection and required signed commits on `main`.
- `[future]` LoRa mesh radio integration (needs hardware on hand).
- `[future]` Distribution beyond GitHub: Codeberg, Software Heritage, a WinGet
  manifest, and IPFS.

---

## Recently shipped

Newest first. For older history see `docs/history/` and `git log`.

- `v0.422` Vault-sync anti-replay; release signing activated and independently verified.
- `v0.421` Release signing ACTIVATED: hybrid Ed25519 + Dilithium3, fail-closed updater.
- `v0.420` Security sprint: relay rate limits, per-author object quotas, gossip-flood
  caps.
- `v0.418 to v0.419` Signed-manifest auto-updater and signed local-build launcher.
- `v0.416` The relay-build fix (CI had been red for 25 releases) plus retired-page
  cleanup.
- `v0.409 to v0.414` The universal nested-expandable widget, inline item cards, and
  Mining and places on universal rows.
- `v0.400 to v0.404` The inventory one-panel spreadsheet redesign.
- `v0.398 to v0.399` The seed economy loop.
- `v0.382 to v0.394` Aeroponic towers, end to end.
- `v0.379 to v0.381` Homestead designs and the first working save and load.
- `v0.329 to v0.342` The gameplay-loop arc: the full survival, production, and
  progression sandbox.

---

## How this roadmap stays honest

- **It is the to-do list.** Maintainers pull the next item from "Right now." When scope
  shifts, this file is updated in the same change, and the website re-renders from it.
- **Status tags are kept truthful.** `[done]` means shipped and live, not "coded but
  not deployed." If something regresses, its tag moves back. We would rather under-claim.
- **The website mirrors this file** via `scripts/roadmap-to-json.js` (which generates
  `data/roadmap.json`) and `web/pages/roadmap-app.js` (which renders it). Edit this
  file, regenerate, and everyone sees the change.
- **Want something on here?** Open an issue or a discussion, or (for contributors)
  propose an edit to this file. The roadmap is CC0 like everything else.
