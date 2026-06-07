# Homes as Profiles

> Status: **direction adopted 2026-06-07** (operator proposal). Model + first
> increments defined here; the deeper save/multiplayer/real-device work is
> sequenced, not yet built. This doc is the single source of truth for the
> "home" concept; update it as the model firms up.

## The idea (operator, 2026-06-07)

Make each **home a save profile**. One shared interface (the homestead: the
inventory/container tree, garden, crafting, vitals, map) renders every home.
What differs between homes is a single **kind** toggle that says what the home
is *for*:

1. **Real home** — bound to real-world monitoring/automation hardware. The app
   becomes the control + monitoring layer for an actual autonomous homestead
   (e.g. real aeroponics at the operator's house). This is the payoff of the
   project's north star, and it is built **last**.
2. **Server home** — a home you play on a multiplayer server (the relay). Shared
   world, other players, server-authoritative state.
3. **Offline home** — local single-player. **The first offline home is special:**
   it is the first homestead we design to be **100% self-sustaining if at all
   possible**, and it doubles as the **gamified blueprint** every other home can
   copy.

Homes are built from **designs** (blueprints). The **first design is the
Fibonacci homestead** (the spiral-deck layout the renderer already generates),
the homestead we are trying to get working first.

## Why this is the right model

It dissolves the Real-vs-Play tension we had been fighting. Instead of a
top-level page firewall (separate "Real" and "Play" tabs), the real-vs-game
distinction becomes a **property of the home you are in**. The homestead UI is
identical everywhere; the *kind* decides what it connects to:

- offline home -> a local simulation you can experiment in freely,
- server home  -> a shared world on a relay,
- real home    -> a dashboard wired to physical sensors and actuators.

This is exactly the north star made concrete: you design your ideal
self-sustaining homestead **once** (as a gamified, safe-to-fail simulation),
and that same design is simultaneously (a) the game's first level, (b) your
real-life build plan and parts list, and (c) a shareable template for anyone.
"Build it in the game, get the parts list, build it for real, then let the app
run it" is one continuous arc through one interface.

## How it maps to what exists today

| Concept here | Existing code it builds on |
|---|---|
| A home (save profile) | `saves/` named save slots (profile, inventory, farm, quests, skills, world) in the local data dir; `src/persistence.rs` |
| The homestead contents/layout | the `Place` container spine (`data/places/seed.json` -> `GuiState.places`), rendered as the inventory `tree_list` |
| A "design" / blueprint | the ship/room layout system (`src/ship/`, RON layouts) + the Fibonacci spiral-deck generator referenced in the renderer |
| Server home | the server-authoritative relay multiplayer (already partially live: avatars, position sync, shared quests, world persistence in `src/relay/storage/game_persistence.rs`) |
| Real home (last) | the device-mesh + in-app-ops direction (`docs/design/device-mesh.md`, `docs/design/in-app-ops.md`); a real home surfaces a monitoring/automation dashboard instead of (or beside) the sim |

So most of the pieces exist; the new work is the **home as the top-level unit**:
a save gains a `kind` and a `design`, and there is a place to pick/create homes.

## The model

```
Home (= save profile)
  ├ name            "Fibonacci Homestead", "Riverbend Server", "My Real Aeroponics"
  ├ kind            Offline | Server | Real
  ├ design          blueprint id this home was built from (e.g. "fibonacci")
  ├ (Server)        relay/server reference
  ├ (Real)          device/monitoring binding   ← LAST
  └ world state     the existing save payload (inventory, garden, places, …)

Design (= blueprint, infinite-of-X data, not code)
  ├ id              "fibonacci"
  ├ name            "Fibonacci self-sustaining homestead"
  ├ layout          how the structure generates (spiral decks / rooms)
  └ systems         the real-world systems it targets (water, energy, food, air)
                    → each maps to a parts list (3D-print / buy / trade)  ← north star
```

The **Play** button (added v0.377) enters the 3D world. In this model, Play
enters the world **for the currently selected home** — so the same button does
the right thing whether you are in an offline sim, joined to a server, or (later)
looking at your real homestead.

## Open design questions (operator's call)

These are genuine forks, recorded so we decide them deliberately rather than by
accident:

1. **Can a home change kind, or is kind fixed at creation?** The powerful version:
   you design an **offline** home, build it for real, then **promote it to a Real
   home** so the same save becomes your live dashboard. That makes the sim->real
   arc a single object's lifecycle. (Leaning: allow promotion offline -> real.)
2. **Design vs. home separation.** A *design* is a reusable template; a *home* is
   an instance built from one. Many homes can share the Fibonacci design. Confirm
   we want that two-level split (vs. a home just *being* a design).
3. **Where does home-select live?** Candidates: the title screen (MainMenu already
   has Play/Settings/Quit), and/or an in-app "Homes" surface. A first home is the
   default so nothing breaks for a single-home user.
4. **What does a Real home show?** Likely the homestead tree with live sensor
   values on the relevant container nodes (tank levels, power, temps) plus
   controls, instead of simulated values. This is the in-app-ops dashboard.

## Improvements adopted (operator-approved 2026-06-07)

The operator approved these refinements ("I like your suggestions") and chose to
build **offline single-player first**, deferring all multiplayer/server/Real work
until offline is solid.

1. **Base character = the existing cryptographic identity.** Don't invent a second
   "you": your Dilithium key / DID / signed profile (name, bio, avatar) IS your base
   character. Servers reference it; they can't forge or overwrite it without your
   signature.
2. **A home's "kind" is *who owns the truth*** -- Offline = you (local save), Server
   = the server, Real = physical sensors (reality, mirrored). Conflicts resolve
   toward whoever is authoritative; promotion (offline -> real) is natural.
3. **The blueprint carries its bill of materials from day one** (not a later step).
   Systems -> components -> {3D-print / buy / trade}. The sim spawns from it; the
   real build list comes from the same tree. **Realized v0.379** (the Home page
   aggregates the BOM).
4. **Closure / self-sufficiency score** -- turn "100% self-sustaining" into a number
   per loop (water in >= out, energy made >= used, food grown >= eaten). Same metric
   sim or real. **Started v0.379** (demand totals + self-sufficiency kit); the exact
   output-vs-demand score needs per-component generation-capacity data (next layer).
5. **Designs are forkable, signed, CC0** -- publish a homestead design, others fork /
   improve / share back. The blueprint becomes a commons (= the mission).
6. **A Real home is a digital twin**, not just a dashboard -- the sim runs
   predictively on live sensor data ("battery dies in 6h", "tank dry in 3 days").
7. **Progressive disclosure** -- one base character + one offline home with zero
   selector friction until you create a second. Protects "I only want one profile".

## Sequenced path (offline-first; refine with operator)

Real-life integration stays **last** but shapes every step (each game system is
designed as a real buildable system with a parts list). Multiplayer/server/Real are
**deferred** (operator 2026-06-07) until offline single-player is solid.

1. **Play button** — dedicated FPS-mode entry. **DONE v0.377.** (Was: Esc only,
   no on-screen indicator.)
2. **Home model + home-select** — saves gain `kind` + `design`; a place to
   create/pick a home; default first home so single-home users are unaffected.
   **NEXT (offline increment 2).**
3. **Offline Fibonacci design** — **browsable Design view DONE v0.379**: the existing
   `data/blueprints/fibonacci_homestead.ron` is surfaced on the Home page
   (`pages/homes.rs`) with its bill of materials, power/water demand, and a
   self-sufficiency summary, by build scale (Solo/Family/Community/Colony).
   Remaining: spawn the world from the design (playable) + the closure-score layer.
4. **Server homes** — bind a home to a relay; reuse the server-authoritative
   multiplayer already in place.
5. **Parts list from design** — each design's systems enumerate the real parts
   to 3D-print / buy / trade (the bridge artifact between game and reality).
6. **Real homes (LAST)** — bind a home to real monitoring/automation devices; the
   homestead UI becomes the live control/monitoring dashboard.

## Characters (the other profile axis)

A **home** is a place/world (above). A **character** is *you* -- your identity,
biography, and look. The operator wants exactly **one base character** and never
to re-enter a biography per server (2026-06-07):

- **One base character = your real self.** Identity and biography are typed
  **once** and owned by you. The Profile page's selector (added v0.378) is where
  you pick which character view you are looking at; today it has a single entry,
  "Base".
- **Servers store an augmented version.** Joining an MMO server saves *that
  server's* augmented copy of you (look, and whatever that world adds), but your
  **base is always preserved and shared**, so you keep the same identity/look
  across servers unless a server deliberately augments it.
- **Shared vs. locked inventory (Diablo II model).** A *set* of characters can
  share one inventory/stash (the "offline / open Battle.net" feel). A character
  can also be toggled to a **locked** inventory, isolated both locally and on
  servers (the "closed / ladder" feel). A server may require you to start a fresh
  character with only **its** starter gear.

So the two axes compose: a **character** (who you are: a shared base + per-server
augmentation) enters a **home** (offline / server / real). The character carries
your identity in; the home/server decides what it augments and which inventory
rules apply.

### Open questions (characters)

1. **What exactly does a server augment?** Look only, or gear/stats too? (Leaning:
   cosmetic + server-granted gear; base appearance is the fallback.)
2. **Inventory-sharing scope.** Is the shared stash per-account, per-character-set,
   or per-home? (Leaning: an explicit "character set" the player groups.)
3. **Who can lock an inventory?** The player (self-imposed challenge) and/or the
   server (enforced ruleset)? (Leaning: both; the server rule wins.)

## Relationship to other docs

- North star (real-world bridge): recorded in `data/coordination/orchestrator_state.json`
  (2026-06-06 "REAL-WORLD-BRIDGE NORTH STAR") and `docs/PRIORITIES.md`.
- Device mesh / real devices: `docs/design/device-mesh.md`.
- In-app ops (GUI-first admin/control surface): `docs/design/in-app-ops.md`.
- Storage / saves / signed objects: `docs/design/storage-architecture.md`.
- Infinite-of-X (designs are data, not code): `docs/design/infinite-of-x.md`.
