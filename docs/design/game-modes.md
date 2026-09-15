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

## Decision pending: split the powers axis in two (2026-09-15)

> **Operator question:** "Creative mode and dev mode are pretty much the same
> thing, can you define a clear difference and why it'd be better to keep both
> instead of just simplifying creative into a single dev mode?"

The honest answer is that the current three-rung ladder **conflates two different
axes**, which is exactly why the two modes feel redundant. Separate them and the
difference becomes obvious:

| Axis | Question | Who decides | Scope |
|---|---|---|---|
| **Mode** | Do resources deplete for ME? | the player, freely | personal, affects nobody else |
| **Rank** | What may I touch that OTHERS share? | the server operator | authority, must be granted |

`FreeResources` is purely the first. `DevTools` and `ShipStructureEditing` are
purely the second. Today all three are bundled into one self-selected client-side
setting, which produces a real honesty hole the code already admits to
(`src/gui/pages/settings.rs:3374`): *"in a shared world the relay is the
authority on shared state, so Dev tools keep working for now; per-player
server-enforced permissions are the follow-up when real players arrive."* On a
shared server today, anyone can simply pick Dev and edit the whole ship.

**Recommendation: keep two things, but redraw the line.**

- **Mode stays a player setting, and shrinks to what it always really was:**
  Survival (resources deplete) vs Creative (they do not). Both scoped to what
  you own. Nothing here needs server permission because nothing here affects
  anyone else.
- **Dev stops being a mode and becomes a RANK capability.** Whole-ship structural
  editing, entity spawning, teleport and FTL are authority, not preference.

This is not new machinery. The relay already has a **data-driven roles table**
(`src/relay/storage/roles.rs`, schema at `storage/mod.rs:1229`) with per-role
capability columns (`can_stream`, `can_upload`, `can_voice`, `can_image_share`,
`can_file_share`) and five seeded built-ins (unverified, verified, donor, mod,
admin). It has **no build-related capability yet**, so this is adding columns of
exactly the shape that already exists: `can_edit_ship`, `can_spawn`,
`can_teleport`.

It also directly delivers the operator's own requirement from the same message:
*"With the construction editor normal play will have edit area confined to their
default zone. Outside the ship requires dev or some other 'rank' that allows
editing outside homes."* That sentence IS this split. The client-side
`Capability` truth table stays as the local gate; the server-side role becomes
the authority that grants it.

Nothing is lost for solo or self-hosted play: the operator is the owner role and
holds every capability automatically.

**Not yet implemented.** `PlayMode` below is still the shipped reality.

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

### C. AI-citizen play mode (BACKBURNERED by the operator, 2026-09-15)

> **Operator decision:** an LLM-driven AI player is deferred in favour of a
> populated ship. "If we can efficiently simulate 500 simple AI humans in one
> mothership sector that live their lives and actually do stuff throughout the
> ship visually that'd be amazing... Once we have the base framework running
> right, when we add the LLM AI player then it'll work right." The reasoning is
> sound and worth preserving: an LLM agent added to a working crowd framework is
> one more inhabitant, whereas an LLM agent added to nothing is a special case
> that has to invent the whole embodiment layer itself. The architecture is in
> [crowd-simulation.md](crowd-simulation.md); this entry stays here as the rung
> that comes AFTER it.

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

### F. Co-op, sized for a DOZEN (operator requirement, 2026-09-15)

> **Operator decision:** "For the shared household co-op I immediately have at
> least a dozen people that would want to play on whatever world I do. MMO or
> something more private with only a handful of players. So whatever co-op game
> mode we make has to support at least a dozen."

So the design target is **12+ players in one world**, not the 2-3 the
shared-household framing implied. What that changes:

- **The hosting story already exists.** Host-a-node runs a full relay from inside
  the app (`src/gui/pages/host_node.rs`), one binary, `run_relay()` on a thread.
  A private world for a dozen friends is a friend hosting, and the isolation unit
  is the relay PROCESS, not a world id. The schema has a `world_id` primary key
  (`storage/mod.rs:674`) but exactly one world is ever constructed, and there is
  no in-relay private-world gate on `game_join` beyond the ban list: anyone who
  can identify on the relay can enter the world.
- **Raw fan-out is not the problem.** 12 players at 15 Hz is 2,160 socket-sends
  per second at ~228 bytes, about 493 KB/s of relay egress. That is nothing.
- **What actually breaks first was a real bug, now fixed.** The per-socket
  forwarder was `while let Ok(msg) = broadcast_rx.recv().await`, which treats a
  lagged receiver exactly like a closed channel: the loop ended, the
  `tokio::select!` aborted the read task, and the socket was torn down, evicting
  the user from the game world AND chat. At 2 players the 256-slot buffer is ~8
  seconds of slack so it never fired; at a dozen it is ~1.4 seconds, which an
  ordinary TLS stall exceeds. Fixed 2026-09-15 (`recv_skipping_lag`, with three
  tests, the first of which proves the lag condition is real).
- **Still open before a dozen people log in:** game broadcasts go to every
  connected socket including chat-only users, because `RelayMessage::System` has
  no delivery filter; every player spawns at the identical point `[0,1,0]` and
  will stack inside each other; `NEW_ID_MAX_PER_IP` is 5 new keys per IP per hour,
  which will refuse the sixth friend onboarding from one household or VPN; and
  nothing but position is actually networked, so the homestead, machines and
  inventory are private client-local state that cannot be shared or conflict.
- **Players and crowd NPCs are the same pipeline.** A player is an inhabitant
  whose brain is a human. Every piece of interest management, instanced drawing
  and interpolation built for 500 NPCs is what a dozen players need too. See
  [crowd-simulation.md](crowd-simulation.md): this is one arc, not two.

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
