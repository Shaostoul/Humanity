# Game modes: what ships, what is designed, what is missing

> **Audit, 2026-09-15.** Every claim below was checked against code, not docs.
> "Modes" in this project are not one ladder: they are six independent axes that
> different documents each call a "mode". This file is the map of all six, so the
> next session does not re-derive it. When you change a mode, update the row here.

## The six axes

A player's situation is the product of six choices, not one:

| Axis | Question it answers | State |
|---|---|---|
| 1. Powers | What am I allowed to do (cheats, scope)? | **shipped** |
| 2. Difficulty | How hard does the sim push back? | **shipped as loose knobs**, designed as a ladder, not connected |
| 3. Authority | Who owns the world I am in? | offline shipped, shared world wired but never live-tested |
| 4. Custody | Who owns my character's progression? | designed only |
| 5. Engagement | How do I meet each need (grow it, buy it, automate it)? | designed only, zero code |
| 6. Purpose | Why am I in this world (play, learn, practice, prepare)? | **no axis exists at all** |

Axis 6 is the whole gap. Everything under "should exist, not planned" below is an
axis-6 mode.

---

## 1. Powers: Normal / Creative / Dev (SHIPPED)

`src/config.rs:87` (`PlayMode`), shipped v0.799. The single ladder every cheat
and scope gate hangs off, resolved through a `Capability` truth table rather than
mode comparisons:

| Capability | Normal | Creative | Dev |
|---|---|---|---|
| `HomesteadEditing` (build your own home) | yes | yes | yes |
| `FreeResources` (skip inventory and consumption) | no | yes | yes |
| `DevTools` (Dev page, spawn, teleport, FTL, creature editor) | no | no | yes |
| `ShipStructureEditing` (zones, corridors, whole ship) | no | no | yes |

- UI: Settings > Gameplay radio buttons, `src/gui/pages/settings.rs:3342`.
- Honesty: a CREATIVE / DEV tag paints on the HUD whenever the mode is not
  Normal (`src/gui/pages/hud.rs:163`), so screenshots cannot lie.
- Dev tools additionally require the `Theme.cheats_enabled` kill-switch. Both.
- **The default is still `Dev`** (the operator builds the mothership in-game
  pre-launch). `config.rs`'s `default_is_dev_pre_launch` test is the tripwire to
  flip it to `Normal` at launch. Do not delete that test.
- Multiplayer honesty: in a shared world the relay is authority on shared state,
  so Dev tools still function; per-player server-enforced permissions are the
  follow-up.

## 2. Difficulty: three loose knobs (SHIPPED), a ladder (DESIGNED, ORPHANED)

Shipped, in Settings > Gameplay, born from an operator field report about being
killed by wolves and dying of thirst too fast:

- `hostile_wildlife` on/off. Off removes predators immediately.
- `vitals_drain` 0.0 to 3.0. 1.0 is normal (roughly half an hour from full to
  empty), 0 pauses survival needs entirely.
- `home_variant` family / solo, which home design loads.

Designed but never built: `docs/game/difficulty_fidelity_matrix.md` defines a
five-rung ladder (Baby/Creative, Easy, Medium, Hard, Realistic) mapped to
per-system fidelity levels L0 to L3 across eight domains, session-authoritative.
It calls for a `FidelityPreset` enum. **No such enum exists; `grep FidelityPreset
src/` is empty.**

**The live gap:** the ladder and the knobs have never been wired to each other. A
preset that sets `hostile_wildlife` and `vitals_drain` today (and per-system
fidelity later) is the smallest useful piece of work on this page, and it is the
difference between "difficulty exists" and "a new player can find it".

Note that `vitals_drain = 0` is, accidentally, the peaceful mode. It is a slider
buried in Settings with no name.

## 3. Authority: solo offline (SHIPPED), shared world (WIRED, UNPROVEN)

- **Solo offline** is the default and is real: local `WorldSave`, saves, settings
  and identity on disk, progress persists between sessions (v0.381).
- **Relay co-presence** shipped its client wiring in v0.472. `copresence_active`
  and `copresence_solo` live on `GuiState`; two clients join the VPS world, stream
  position, and render each other. It has **never had a two-player live test** and
  is paused behind the graphics arc (ROADMAP "Right now" item 3).
- `docs/game/session_modes.md` (offline / host-P2P / join-P2P / dedicated) is an
  **alternate direction, not the plan.** The architecture being built is
  relay-authoritative. That doc carries its own correction banner. Do not build
  from it without a deliberate decision to change direction.
- `docs/design/two_timeline_offline_model.md` is marked Accepted: a local timeline
  that is always writable plus a canonical online timeline with explicit merge
  rules. Accepted, unimplemented.

## 4. Custody: open / closed / hybrid (DESIGNED)

`docs/design/characters-and-servers.md`, the Diablo II open-vs-closed Battle.net
model, grounded in this codebase. The split invariant is worth memorising: **a
field is self-custodiable if and only if forging it grants no competitive
advantage.** Identity and cosmetics are always self-custodial (seed-derived
Dilithium key, signed `character_v1` object). Progression is owned per server
policy: open trusts the save you bring, closed makes the relay the sole writer,
hybrid runs both lists side by side. Partly pre-built server-side
(`PlayerProgress`, and `handle_game_join` already re-seeds a returning player from
the server row and never from client-sent stats).

## 5. Engagement: five modes per domain (DESIGNED, ZERO CODE)

`docs/design/engagement-modes.md`: one simulation depth, and the player picks
**per domain** how they meet that need. Direct production / Trade / Automation /
Cooperative / Background. This is the project's stated answer to the accessibility
problem (do not force everyone through electrical engineering; do not shallow the
sim for everyone). `grep -i engagementmode src/` returns nothing. It is entirely
unbuilt, and it is the most load-bearing unbuilt design in the repo.

## 6. Purpose: NOTHING EXISTS

There is no axis that answers "why am I in this world". Every mode under "should
exist" below is one.

---

## Planned elsewhere (roadmap, not yet built)

- `[next]` **Real/fake save flag plus real-life-first boot.** Every save's houses
  and characters carry a real or fake flag; the app boots to real life and the sim
  loads only for a fake-flagged save or an explicit opt-in.
- `[planned]` **First Playable guided first day.** `data/quests/tutorial.ron`
  exists; the mode-shaped wrapper does not.
- `[future]` **Disaster Mode.** Aim the disaster / weather / hydrology /
  atmosphere systems at real preparedness, with a location-personalised prep plan.
- `[future]` **Generation-Ship co-op.** A shared life-support habitat where
  selfishness literally collapses the colony. Called "the first mission-shaped
  multiplayer scenario", but there is no scenario mechanism underneath it.
- `[future]` **Armory firing range and arena, combat expeditions, non-LLM tactical
  PvP** driven by self-play. Explicitly gated behind the peaceful survival /
  construction / economy loop working first.
- `[future]` **Boot straight into Play, and VR.**
- Captured, not built: the **turret defense minigame**
  (`docs/game/turret-defense-minigame.md`, 2026-06-13; the `defense_turret` marker
  machine is already placed in `data/machines/home.ron`).

---

## Should exist, not planned

Ranked by value per unit of work. Each names what already exists to build on.

### A. A data-driven scenario format (cheapest structural win)

Generation-Ship co-op, Disaster Mode, the guided first day, a classroom lesson
world and any community challenge are all the same shape: **start conditions,
constraints, goal, difficulty preset**. Today each implies bespoke code. Under
Infinite-of-X, anything that can exist more than once is a data file, so this is
`data/scenarios/*.ron` plus a loader, and it carries five roadmap items at once.
Write this before writing any individual scenario.

### B. Classroom / cohort mode (highest mission value)

The mission is teaching real survival skills. 143 curriculum topics, 46 documents,
six Library gates, a quest engine, guilds and governance all ship or are shipping.
Nothing lets one person run a world for N learners: no roster, no lesson plan, no
visible progress, no assessment. The roadmap's only teacher mention is that a
teacher can add local crops via a data schema. This reuses guilds, quests and the
curriculum rather than needing new simulation, and it is the clearest adoption and
funding story the project has.

### C. AI-citizen play mode (biggest stated-principle gap)

CLAUDE.md declares AI agents first-class citizens of HumanityOS.
`docs/design/ai_interface.md` bounds AI to explanation, navigation, analysis and
authoring, which is explicitly **not playing**. There is no headless world client
and no action API into the sim, so an AI cannot run a homestead beside you. Two
payoffs: it makes "accounts for all humans and all AI" real rather than
aspirational, and an agent that actually plays finds gameplay bugs that no
snapshot rig or probe sweep can reach.

### D. Spectator / observer, with replay

`docs/design/action_log.md` already defines a canonical action log "used for
deterministic replays" and `docs/design/cosmos-architecture.md` calls out
deterministic playback. Nothing consumes either. A spectator buys teaching (watch
the instructor), streaming (the Studio and Watch surfaces already exist), operator
verification without a second GPU boot (which matters under the one-instance
rule), and bug reports that arrive as a replay file instead of prose.

### E. Peaceful (no-fail) and Hardcore (permadeath), as named choices

Health hits 0 and nothing happens today: death, respawn and healing are all in the
designed closure, not the code. When the death-and-recovery loop lands, both ends
of that axis should be a deliberate named choice made at world creation, not an
afterthought. No-fail is an accessibility-mission requirement (children, disabled
players, anyone using this as a life tool). Hardcore matters because stakes are
the teaching mechanism. Right now the peaceful mode exists only as an unnamed
slider at 0.

### F. Shared-household co-op (one homestead, several characters)

The family / solo home variant ships and the operator's own context is
family-central. There is no mode where two or three people share ONE homestead
with separate characters, separate inventories and a shared power / water / food
budget. It is much smaller than full MMO co-presence, and it is the honest test of
the Generation-Ship co-op thesis at a scale of three instead of ten billion.

### G. Practice-then-do-it-for-real

`docs/design/two-realities.md` says the real side is the priority and under-built,
and `docs/design/education_model.md` says understanding emerges through action,
consequence and reflection. No loop closes that: learn it in the sim, the app asks
you to do it in real life, you confirm and log it, the real-side tracker updates.
Disaster Mode is the nearest planned thing but it is a prep plan, not a loop. This
is the mode that makes the educational claim falsifiable.

### H. Difficulty presets wired to the shipped knobs

See axis 2. Not really a new mode, just the missing wire, and the smallest item on
this page.

## Related docs

- `docs/design/engagement-modes.md`, the five per-domain engagement modes
- `docs/design/characters-and-servers.md`, open / closed / hybrid custody
- `docs/design/two-realities.md`, why there is no Real/Sim toggle
- `docs/game/difficulty_fidelity_matrix.md`, the designed difficulty ladder
- `docs/game/session_modes.md`, an alternate authority model, NOT the plan
- `docs/design/gameplay-loop-map.md`, the tier stack (stale in its current-state half)
