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

## Sequenced path (proposal, refine with operator)

Real-life integration stays **last** but shapes every step (each game system is
designed as a real buildable system with a parts list).

1. **Play button** — dedicated FPS-mode entry. **DONE v0.377.** (Was: Esc only,
   no on-screen indicator.)
2. **Home model + home-select** — saves gain `kind` + `design`; a place to
   create/pick a home; default first home so single-home users are unaffected.
3. **Offline-first Fibonacci design** — the first blueprint as data
   (`data/designs/…`), the 100%-self-sustaining reference homestead, playable
   single-player.
4. **Server homes** — bind a home to a relay; reuse the server-authoritative
   multiplayer already in place.
5. **Parts list from design** — each design's systems enumerate the real parts
   to 3D-print / buy / trade (the bridge artifact between game and reality).
6. **Real homes (LAST)** — bind a home to real monitoring/automation devices; the
   homestead UI becomes the live control/monitoring dashboard.

## Relationship to other docs

- North star (real-world bridge): recorded in `data/coordination/orchestrator_state.json`
  (2026-06-06 "REAL-WORLD-BRIDGE NORTH STAR") and `docs/PRIORITIES.md`.
- Device mesh / real devices: `docs/design/device-mesh.md`.
- In-app ops (GUI-first admin/control surface): `docs/design/in-app-ops.md`.
- Storage / saves / signed objects: `docs/design/storage-architecture.md`.
- Infinite-of-X (designs are data, not code): `docs/design/infinite-of-x.md`.
