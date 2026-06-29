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

1. `[building]` **Multiplayer co-presence + the character selector.** Co-presence
   CLIENT WIRING SHIPPED (v0.472): two players share the VPS world, stream position, and
   see each other as avatars; pending a two-player test. Remaining: nameplates, the
   world-snapshot prefill, and the CHARACTER LAUNCHER (the Play button becomes a launcher
   with character select + homes + a default to skip it; self-custodial LOCAL vs
   server-authoritative SERVER characters, open / closed / hybrid like Diablo II). Design
   in `docs/design/characters-and-servers.md`.
2. `[building]` **First Playable / live home sim depth.** Battery state-of-charge SHIPPED
   (v0.473: the banks now charge/discharge with the solar swing, live HUD readout).
   Remaining: walk-up stations, a 3D vitals HUD, death and respawn, a guided first day,
   and letting battery discharge prevent load-shedding.
3. `[next]` **GitHub branch + tag protection on `main`** (deploy auto-pushes to the live
   relay with no approval gate) and the backup-restore drill.

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
- `[future]` Open sensor protocol: a simple JSON-over-HTTP spec so a 5-dollar ESP32
  feeds your real pH / EC / temperature / water readings into the live home sim, the
  exact moment the game becomes the control panel for your actual homestead.
- `[future]` Disaster Mode: aim the disaster / weather / hydrology / atmosphere systems
  at real preparedness, a location-personalized prep plan computed from the codex and
  your battery days-of-autonomy.
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
- `[planned]` Self-custodial vs server-authoritative characters: your identity and look
  are always yours; a server holds your in-world progression only when you choose its
  closed realm (open vs closed Battle.net). Designed in
  `docs/design/characters-and-servers.md`.
- `[future]` Multi-device key sync: authorize a second device by QR or short code over
  an encrypted channel (part of the device mesh).
- `[future]` Skill Passport: a portable, offline-verifiable, post-quantum proof of
  competence (W3C-style credential, but federated and PQ), direct infrastructure for
  the unbanked and undocumented.
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
- `[done]` Native voice: mic capture, Opus over WebRTC, and per-channel voice rooms
  that interoperate with the web client for live two-way audio, with RNNoise noise
  suppression and selectable transmit modes (open mic, push-to-talk, voice-activated,
  push-to-mute).
- `[next]` Native voice polish: per-peer volume and mute controls, web transmit-mode UI
  parity, an in-process WebRTC test harness for CI, and graceful relay restart so
  deploys do not drop active calls.
- `[planned]` Native streaming and video: the screen-share and video transport native
  still lacks (web users have video today).
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
- `[future]` Time-Bank: log and exchange hours of help as the on-ramp economy before
  anyone has money, so the poorest can participate from day one with skills and time.
- `[future]` Build-It-For-Real: a structured bill of materials with local sourcing and
  a neighbor group-buy button on every survival system, because poverty is often a
  minimum-order-quantity problem, not a knowledge one.
- `[future]` Mission Pulse: a federated, privacy-preserving, aggregate-only impact
  dashboard (each server publishes signed counts, no per-user data leaves home).
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
- `[done]` UI consolidation: a slim top navigation, universal reusable widgets, the
  Humanity Mission Dashboard, and the in-app Library.
- `[done]` In-app construction editor: build the homestead from inside the app: a
  top-down plan, a 3D astral camera + room grab, a three-column editor, doors and
  windows as placed objects you add / move / resize on still-solid walls, and
  multistory with level-aware adjacency so stacked rooms never cut doors into floors
  (v0.463 to v0.471).
- `[done]` Structural de-risk spike: a pure node-beam solver (load routing, cascade
  failure, disconnected-island detection) so the structural-integrity pass is proven
  before it is wired to geometry (v0.471).
- `[done]` Construction editor v2, the homestead-builder rebuild (v0.532 to v0.603): a
  fixed outer box plus freely-placed interior walls (rooms emerge by flood-fill),
  data-driven wall materials / thickness / surface layers, mitred corners, doors and
  windows with animated styles (swing / slide / iris / rotate / fold / energy / nanowall
  / fixed) plus locks and control panels, per-home lights, a unified single-line object
  browser, move / duplicate gizmos, undo / redo, a CAD dimension overlay, a construction
  console (the AI act surface) + live JSON home introspection (the AI read surface), and
  first-person wall / door collision.
- `[done]` Structural pieces (v0.583 to v0.592): a data-driven registry of buildable
  stairs, ramps, ladders (climb), elevators (ride), teleporters, train rail, decks, and
  roads as a curved node graph; walk on them in first person.
- `[done]` Home power sim + buildability (v0.437 to v0.606): machines are live ECS power
  entities (generators / consumers / batteries) that load-shed by priority and charge /
  discharge with the solar swing; a design-time buildability validator (power source,
  energy balance, wiring, conduits, power circuit).
- `[building]` Utility wiring, no magic transmission (v0.604 to v0.608): power, water,
  air, and data travel through rated cables and pipes (real AWG / ampacity / voltage-drop
  physics); machines declare IN / OUT ports by utility. SHIPPED: the data model + cable
  registry + physics, machine ports + the Conduits buildability check, the Power-circuit
  connectivity check, runtime per-island power-flow gating, and a live water / plumbing sim
  coupled to power (powered pumps fill the cistern; cut the power and it drains). Next: a
  wire-A-to-B gizmo and the room-temperature superconductor upgrade mission. Design in
  `docs/design/utility-wiring.md`.
- `[next]` Multiplayer co-presence + the character / server model: two players in one
  world on the VPS; self-custodial local characters vs server-authoritative ones (open
  vs closed Battle.net); the Play launcher with character select, homes, and a default.
- `[planned]` First Playable: walk-up stations, a 3D vitals HUD, and a guided first
  day (shared with the survival theme above).
- `[future]` Generation-Ship co-op: a shared life-support habitat where selfishness
  literally collapses the colony, the first mission-shaped multiplayer scenario.
- `[future]` AI Mentor "Sage": a first-class NPC citizen with its own identity and a
  scripted-fallback decision tree (ships with zero LLM dependency, pluggable later).
- `[future]` Game-world depth: more content, characters, and deeper coupled systems.
- `[future]` Real-terrain world generation from real elevation data (USGS / SRTM /
  Copernicus) keyed to a place's latitude and longitude.
- `[future]` VR support and a "boot straight into Play" mode.

## Accessibility and onboarding

The mission requires that anyone, of any age or background or ability, can use this.
This layer is not optional.

- `[done]` Accessibility modes: high-contrast, colorblind, and reduced-motion (now
  wired and applying on every WEB page too, not only native, with the Settings toggles
  actually taking effect, v0.471).
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
- `[future]` Schema-driven in-app "Make-a-Thing" editor: a form generated from each
  data schema so a teacher anywhere can add local crops, recipes, and lessons with no
  code (the keystone for community content and against founder-dependence).
- `[future]` Offline Survival Codex: the chemistry / biome / water-and-nutrient corpus
  bundled, printable, and with built-in calculators that work with no internet.

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
- `[done]` Release-signing pipeline verified end to end (a release dual-signed
  Ed25519 + Dilithium3, the fail-closed updater offering it) plus a `check-signing`
  health gate so an unsigned latest release can never again silently freeze auto-update
  (v0.470).
- `[done]` First dependency security audit (`cargo audit` on the cadence): three TLS
  certificate-validation advisories patched via a rustls-webpki upgrade (v0.470).
- `[building]` In-app ops console: bring every admin action into the app (the GUI-first
  mandate), paying down the CLI-only debt.
- `[planned]` Device mesh: a "My Devices" dashboard, backup designation, restore flow,
  and direct LAN sync, so your own devices back each other up.
- `[planned]` Litestream continuous replication (roughly 1-minute recovery point) and
  a quarterly backup-restore drill.
- `[planned]` GitHub branch + tag protection and required signed commits on `main`
  (deploy auto-pushes to the live relay today with no approval gate).
- `[future]` FederationBundle: one signed-object bundle that travels over any medium
  (USB, QR, LoRa, sneakernet), so identity and knowledge move with no internet at all.
- `[future]` Continuity Capsule: self-sovereign succession via a Shamir-split seed
  across trusted contacts, for every citizen, not just the operator.
- `[future]` LoRa mesh radio integration (needs hardware on hand).
- `[future]` Distribution beyond GitHub: Codeberg, Software Heritage, a WinGet
  manifest, and IPFS.

---

## Recently shipped

Newest first. For older history see `docs/history/` and `git log`.

- `v0.621` Telecom Stage 2: data routing -- machines demand internet (Mbps), a data
  connection picks its medium (Cat6 / fibre / WiFi) in the editor, and a Data-links
  buildability check sizes bandwidth + range + cautions when a wireless link's RF is near a grow.
- `v0.620` Telecom consequence: a powered WiFi router's RF now HARMS the grow -- the
  FarmingSystem drains crop health by the home RF level, so you run wired (Cat6/fibre,
  zero RF) to protect a sensitive crop. The operator's "the tradeoffs bite" example.
- `v0.619` Telecom / internet utility Stage 1: real data media (Cat6 ethernet, fibre,
  WiFi) with bandwidth / range / latency / RF-emission tradeoffs + a link-physics check.
  Next: emissions become enemy/player detection signatures + pheromones.
- `v0.618` Air life-support Stage 2: occupancy + a powered air recycler -- cut the power
  and the scrubbers stop, O2 falls, and the player suffocates (power -> air -> Vitals).
  All three life-support utilities (energy, water, air) now have real consequences.
- `v0.612 to v0.617` Build-editor backlog (multi-select, snap guides, lock-per-type), the
  cable picker + superconductor bulk-upgrade, a CI-lint fix, and AIR life-support Stage 1
  (the AtmosphereSystem is live: a sealed home air space + a Live air Home-page card).
- `v0.611` Water to food: the FarmingSystem reads the live cistern level -- a dry cistern
  stops auto-irrigation and the garden wilts, completing power to water to food (cut the
  power, the pump stops, the cistern drains, then days later the crops die).
- `v0.610` Water sim fix (adversarial review): the seed home now actually fills when
  powered + drains when cut (the pump was over-modelled; the towers were inert islands).
- `v0.608` Live water / plumbing sim coupled to power: cisterns store, powered pumps +
  purifiers fill them, fixtures draw, all per pipe island -- and cutting the power stops
  the water (the first power -> water consequence chain). Shown live on the Home page.
- `v0.604 to v0.607` Utility wiring: real copper-cable physics + a conduit registry,
  machine IN / OUT ports, the Conduits + Power-circuit buildability checks, a physically
  wired seed home, and runtime per-island power-flow gating (no magic transmission).
- `v0.583 to v0.603` Structural pieces (stairs, ramps, ladders, elevators, teleporters,
  train rail, decks, roads as a curved node graph) plus build-editor polish: a unified
  object browser, move / duplicate gizmos, a wall wireframe + dimension overlay.
- `v0.532 to v0.582` The home-construction rebuild: a fixed box plus interior walls
  (rooms emerge by flood-fill), wall materials / thickness / layers, mitred corners,
  doors / windows / locks / lights, the construction console, and live JSON introspection.
- `v0.512 to v0.517` The nested-container spatial inventory (person to pocket to wallet).
- `v0.601` Crash-safe logging: every log line tees to disk plus a panic hook, so a
  windowed crash leaves its cause in the app data logs folder.
- `v0.473` Battery state-of-charge: the home's battery banks now charge and discharge
  with the day/night solar swing, with a live HUD readout (so "autonomy" is a draining
  number, not a static string).
- `v0.472` Multiplayer co-presence: the client joins the relay's shared world over the
  existing socket, streams its position, and renders other players as moving avatars
  (the relay half was already done). Needs a two-player VPS test to confirm.
- `v0.471` Multistory editor (level-aware adjacency), the structural de-risk spike, and
  the web accessibility modes wired into every page.
- `v0.470` First `cargo audit` on the cadence (TLS advisories patched) and the
  release-signing pipeline verified end to end with a `check-signing` health gate.
- `v0.469` Website parity: the landing became the app Mission Dashboard, the header
  mirrors the app nav, the service worker went network-first, plus openings as placed
  objects in the construction editor.
- `v0.463 to v0.468` The construction editor arc: top-down plan, 3D astral camera and
  room grab, the three-column editor, and the door/window slide gizmo.
- `v0.437` Live home sim, increment 1: machines spawn as ECS entities and a live power
  budget (solar + electrical) drives a HUD readout.
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
