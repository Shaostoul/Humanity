# Crowd simulation: how a mothership holds a population

> **Operator direction, 2026-09-15.** "If we can efficiently simulate 500 (or
> whatever is a doable amount on our weak hardware) simple AI humans in one
> mothership sector that live their lives and actually do stuff throughout the
> ship visually that'd be amazing. Eventually we want a mothership that houses
> billions of simple AI crew. Once we have the base framework running right, when
> we add the LLM AI player then it'll work right."
>
> The LLM-driven AI player is explicitly BACKBURNERED in favour of this. The point
> of building the crowd framework first is that an LLM agent later becomes one more
> inhabitant in a system that already works, rather than a special case.
>
> This doc is the architecture. The measurement protocol for the first crowd test
> lives in [npc-crowd-stress.md](npc-crowd-stress.md), whose inventory table was
> corrected the same day and should be read alongside this.

## The trap to avoid

"Simulate 500 agents" is a dead end at 501. Anything that ticks every inhabitant
every frame, or streams every inhabitant's position to every client, has a hard
ceiling set by whichever bill saturates first, and no amount of optimisation moves
a ceiling that is structural.

The operator arrived at the escape independently on 2026-07-01, before seeing the
research that reached the same place (recorded in
[mothership-superstructure.md](mothership-superstructure.md) "Open questions"):

> "We don't literally have to render everything and do physics for everything,
> most things are just simple calculations like going to x to spend x time then go
> to do this for x time, at any given time everything is being done by a certain
> percentage of people."

That is the whole architecture. Written out, it is three tiers.

## The three tiers

| Tier | Scope | Count | Cost model | What it answers |
|---|---|---|---|---|
| 1. Aggregate | whole ship | unbounded (billions) | O(zones), arithmetic only | Do the ship's numbers close? |
| 2. Roster | loaded sector | thousands | O(1) per query, evaluated lazily | Who is this person, and where are they right now? |
| 3. Embodied | what you can see | hundreds | O(visible) per frame | What is walking past me? |

**Tier 1 (aggregate).** A `population: u64` per zone plus shift fractions, feeding
per-capita demand into the resource model. This is the layer that makes "ten
billion" an honest number rather than a rendering fantasy: it costs the same
whether the zone holds 500 people or 500 million, because the cost scales with
zone count, not headcount. It reuses the aggregation pattern the utility trio's
`PowerCircuit` / `PlumbingCircuit` islands already implement one tier down.

**Tier 2 (roster).** Individual identities for the loaded sector: name, role, home
room, work room, shift. The critical property is that a roster member's state is
**derived, not stored**. `where_is(agent_id, t)` is a pure function of the agent's
id, the timetable, and the clock. Nothing ticks. Nobody integrates. Ask where
someone is at any time, past or future, and get an answer in constant time.

This is the layer that makes a crowd feel like people rather than traffic. The
person you bump into has a name, a job, and a bunk in a sector you will never
visit, because all of that is computable from their id and a seed.

**Tier 3 (embodied).** Roster members inside the player's volume get promoted to
real ECS entities with a transform, collision, animation and a draw call. They are
demoted when the player leaves. Promotion state comes from `where_is()`, so there
is no "the world froze while you were away" artefact: the schedule never stopped,
because the schedule was never running.

**The property that matters:** per-frame cost scales with what is VISIBLE, not
with population. A billion-inhabitant ship costs the same per frame as a
500-inhabitant one. 500 is not a limit, it is just how many happen to be promoted.

## The consequence for networking: derive, do not stream

Today crew NPCs are simulated on the relay and broadcast to every client (see the
bills below). At 500 that is a firehose, and it is the single hardest bill to pay.

If tier 2 is a pure function, the relay does not send positions at all. It sends
the **seed and the timetable**, once. Every client computes the identical crowd
from the identical inputs. Only DEVIATIONS need syncing: an NPC a player talked
to, one that got interrupted, one that a player's action displaced.

The network cost of an ambient crowd becomes approximately zero, which is the only
way 500 inhabitants work over a modest VPS. This is not an optimisation to add
later; it is the reason to shape tier 2 as a function in the first place.

**This is also the thing that blocks reuse of the current chore system.** Today a
chore is a running timer: the relay holds `remaining` in a JSON value and does
`remaining -= dt` every tick (`src/relay/handlers/game_state.rs:1161`). That is
stateful integration and it cannot be derived. Converting chores from
"timer counting down" to "timetable entry with a start and an end" is the central
refactor of this arc, and everything else follows from it.

## Players are just embodied agents

A player is an inhabitant whose brain happens to be a human. If tier 3 is built
properly, the dozen co-op players the operator wants are twelve more tier-3
entities in the same pipeline: the same interest management, the same instanced
draw, the same interpolation.

This is why the crowd arc and the co-op arc are one arc and not two. Every piece
of interest management built for 500 NPCs is what a dozen players also need, and
the reverse.

## What exists today

Verified against code 2026-09-15. Good news first, because there is more of it
than the docs suggest.

- **The "live their lives" loop already works.** Crew pick a chore, walk to the
  room, dwell, pick the next one. `data/npc/crew.ron` (who they are) and
  `data/npc/chores.ron` (what they do, 14 chores across 6 roles) are already
  data-driven and infinite-of-x compliant since v0.937. The chore label is
  already synced and already shows as "Botanist Yara: Inspecting the hydroponic
  racks".
- **Appearance variety is already modelled.** The `Appearance` component
  (`src/ecs/components.rs:749`) carries body type, height scale, skin tone, hair
  style and colour, eye colour, and there is a 14-row cosmetics catalogue with
  per-slot colours. Hash an agent id into that and 500 people look like 500
  people, using the same component the player uses rather than a fork.
- **A proven instanced draw path already ships.** Grass is a genuine single
  instanced draw (`src/renderer/mod.rs:4658`, `0..self.grass_n`) on the same
  pipeline the classic loop uses, with a 48-byte per-instance record
  (`src/renderer/mesh.rs:110-118`) and a growable buffer that rebuilds every
  frame. Terrain additionally runs real multi-draw-indirect
  (`src/renderer/patch_arena.rs:144`, submitted at `mod.rs:4611`) with a
  per-draw fallback, because the feature is requested as an intersection with
  adapter support and can never fail a boot.

  **Precise correction to the record:** `mothership-superstructure.md:129` says
  "the renderer's instancing path is confirmed dead code." That is still true of
  `Renderer::render_instanced` (`src/renderer/mod.rs:5266`), which has zero
  callers and is unusable anyway since it clears the surface and owns a whole
  frame. It is NOT true of the engine: v0.1091 shipped the live grass path
  above. The conclusion that doc draws (aggregate population) stands; only its
  renderer premise is stale.

  **Gotcha for whoever implements this:** `src/renderer/mesh.rs:731-751` pins the
  48-byte stride with a test that also asserts `PatchInstance`
  (`patch_arena.rs:125`) is the same size, because both write the same slot. Grow
  the record for NPC data and you must update the patch arena in the same commit,
  or every batched terrain patch after the first renders at another patch's
  anchor. An upright crowd probably does not need to grow it: position, local up,
  drawn height, yaw and a packed 8:8:8 colour are already in there, and yaw plus
  up is a sufficient orientation for a walking human.
- **A shared clock exists.** `GameTime` accumulates `elapsed_seconds` as f64.
  (Caveat: it accumulates by `+= dt`, which drifts between clients. A derived
  crowd needs the relay authoritative on the clock, or a tick count rather than a
  float accumulator.)
- **Private worlds are already possible.** Host-a-node runs a full relay from
  inside the app (`src/gui/pages/host_node.rs`); the isolation unit for "a world
  for my dozen friends" is the relay process, not a world id.

## What does not exist

Also verified. These are the real work, and none of them are hinted at by the
"crew NPCs work" surface.

- **No pathfinding of any kind.** Both `src/systems/ai/flow_field.rs` and
  `src/systems/ai/behavior.rs` are 26-line dead stubs with zero call sites and
  non-existent data files, despite a doc comment claiming flow fields support
  "million-agent navigation". Crew walk in straight lines through walls today,
  and that is documented in `data/npc/chores.ron` as an accepted first-step
  limitation. At 6 crew it reads as a quirk. At 500 it is the whole scene.
- **No character animation of any kind.** No skeletal or skinned pipeline, no
  GLTF animation import. NPCs and remote players both draw as a two-primitive
  body-and-head marker (`src/lib.rs:8795` teal players, `src/lib.rs:8833` amber
  crew). 500 inhabitants today are 500 sliding markers, and "actually do stuff
  visually" is exactly the part that needs this.
- **No spawn path to 500.** Crew spawn one per matching room type
  (`src/relay/handlers/game_state.rs:503`), so the starter ship caps near six.
  There is no `npcs:N` knob, though the `lights:N` precedent
  (`src/engine/ipc.rs:522`) is the pattern to copy.
- **No interest management anywhere.** Every client receives every NPC update.

## The frame budget, measured 2026-09-15

There was no interior vantage cost capture on disk (all 577 were cloud and
altitude ladders), so one was taken: `home-clock-noon` / `home-clock-night`,
v0.1312.2, 2560x1387, real GPU timestamp queries, operator's live graphics
settings mirrored, panics 0. Capture in `.probe-rig/sweeps/20260915-202858/`.

| GPU pass | ms | | CPU stage | ms |
|---|---:|---|---|---:|
| `gpu.scene` | **12.619** | | `cpu.scene` | 1.474 |
| `gpu.transparent` | 5.072 | | `cpu.celestial` | 0.820 |
| `gpu.stars` | 0.967 | | `cpu.system.ai` | **0.017** |
| everything else | 0.18 | | everything else | 1.08 |
| **GPU TOTAL** | **18.84** | | **CPU TOTAL** | **3.39** |

**Read this carefully, because it reframes the whole question.** That is the
interior deck with the SIX crew that ship today. It is already over a 16.6 ms
budget before a single extra inhabitant exists. The blocker at 60 fps is not the
crowd; it is 17.7 ms of unattributed `gpu.scene` plus `gpu.transparent`.

30 fps is the honest target, and that is this project's own bar rather than a
lowered one: 48 of 343 vantages carry a `perf_floor_fps`, the range is 5 to 30,
and not one of them is 60. At 30 fps there is roughly 14.5 ms of GPU and 30 ms of
CPU headroom to spend on a crowd.

Two caveats stated plainly. The rig runs backgrounded at a 30 fps cap, so the GPU
may be downclocking and 18.84 ms should be read as an upper bound on true busy
time; relative pass ranking is unaffected. And `home-clock-noon` is the homestead,
not a mothership sector, because no interior sector exists to measure yet.

### What 500 costs, against that budget

Per-draw cost is anchored on this repo's own measurement, from the terrain
batching work (`patch_arena.rs:16-21`): 2,049 draws at 15.8 ms versus 8,718 at
25.8 ms in the same scene, which is **1.50 us per bare `draw_indexed`**.

| Item | Today | One instanced draw | Basis |
|---|---|---|---|
| Draw submission | 1,000 draws, **3.0-6.0 ms** | 1 draw, **~0.02 ms** | est. on measured 1.50 us |
| Object uniforms | 1,000 `Mat4::inverse`, 256 KB staging | one 24 KB write | estimated |
| Crowd raster | 174k tris, 1-3 ms | same | estimated |
| AI tick | **2.5-11 ms** (O(n squared)) | 0.2-0.5 ms with a grid | est.; measured 0.017 ms at n=6 |
| Animation | **0, does not exist** | +0.2-0.5 ms + bone buffer | no system to measure |
| Nameplates | 0.4-1.0 ms | same, reorder two tests | estimated |

The measured 0.017 ms AI tick at six NPCs is exactly the kind of number that
must not be extrapolated: at n=6 the quadratic term is invisible.

**Threading will not save the AI tick, and the repo already proved it.**
`particles.rs:25-28` records that rayon across six cores bought only 1.25x on this
CPU, and `particles.rs:214-223` sets `PAR_THRESHOLD = 50_000` because at 10k the
parallel path measured SLOWER than serial (0.319 ms versus 0.168 ms): fork/join
costs more than a working set that already fits in cache. 500 agents is two orders
of magnitude below that. Fix the algorithm with a spatial grid and stop allocating
a `String` per decision; keep it on one core.

## The bills, structural

Each of these is a structural ceiling, not a constant factor, and each is ranked
by what breaks first rather than by how bad it sounds.

1. **The join handshake breaks around 200 crew.** `game_welcome` sends every
   entity's full components JSON including `dialog[]` and `greetings[]` at roughly
   640 bytes each, against a 128 KB WebSocket message cap
   (`src/relay/mod.rs:1217`). This one bites before any rendering cost does.
2. **Broadcast overrun disconnected clients.** 500 travelling NPCs produce 500
   messages per half-second window into a 256-slot channel. **Fixed 2026-09-15**
   (`recv_skipping_lag`): a lagged socket now skips to the present instead of
   being torn down. Before the fix, falling behind evicted the user from the game
   world and chat alike.
3. **Client receive is O(messages x NPCs).** Each NPC update does a full linear
   ECS scan to find its entity (`src/net/sync.rs:300-318`), with no id index. At
   500 that is roughly 250,000 comparisons per broadcast window.
4. **1000 unbatched draw calls per frame.** Each NPC pushes two `RenderObject`s
   (body + head) with no frustum or distance test on the push side
   (`src/lib.rs:8851`), each drawn as `draw_indexed(.., 0..1)`
   (`src/renderer/mod.rs:4579`). The object cap is 16384 so the cap is not the
   wall; draw submission cost is, on a renderer that already spent an arc getting
   4x on exactly that.
5. **1000 string allocations per frame** rebuilding nameplate labels
   (`src/lib.rs:18361-18377`), before any distance test runs.
6. **Relay tick does JSON tree-walks under one global write lock**
   (`game_state.rs:1037-1167`): roughly 10k JSON walks and 20k string allocations
   per second at 500 NPCs, serialised behind a lock the 20 Hz sim also wants.
7. **`AISystem` is O(n-squared)** with multiple all-pairs loops and no spatial
   index (`src/systems/ai/mod.rs:400-491`). **Not on the crew path today** (crew
   lack `Health` so they fail its query), but it becomes the bill the moment
   crowd NPCs get `AIBehavior`. Do not put them on it without a spatial index.
8. **Every player spawns at the identical point** `[0.0, 1.0, 0.0]`
   (`msg_handlers.rs:3246`), so a dozen joiners stack inside each other.
9. **Relay and client do not share a world.** The relay simulates a multi-deck
   ship; the client renders the flat homestead. Remote Y is clamped to a constant
   to stop crew floating in the sky (`src/net/sync.rs:33`). Co-presence today is
   floating XZ avatars over private unshared worlds. A crowd cannot be placed
   correctly until this is resolved, and it is upstream of everything else here.

## Build order

Each rung is separately shippable and separately measurable. The order is chosen
so that every rung's cost is observable before the next rung depends on it.

0. **Reconcile the two worlds** (bill 9). Until relay and client agree on what the
   world IS, crowd placement is undefined. Nothing below is meaningful first.
0.5. **Attribute the interior frame.** 17.7 of the measured 18.84 ms is
   unexplained, and it is unrelated to NPCs. One rig run settles the biggest
   question: capture `home-clock-noon` at a quarter of the pixel count. If
   `gpu.scene` falls about 4x it is fill-bound (and the fix is a depth prepass or
   front-to-back sorting, neither of which exists: the loop at `mod.rs:2808-2840`
   does no sorting and there is no prepass); if it barely moves it is vertex or
   state bound. Noon and night measured 12.619 and 12.626, identical to 0.06%,
   which already argues the cost is lighting-invariant. Do this before optimising
   anything interior, crowd included.
1. **The `npcs:N` spawn knob plus `[npc-diag]` counters**, local-only, copying the
   `lights:N` pattern. Counters before content, so even the first wander rung is
   measured. This is the protocol already specified in npc-crowd-stress.md.
2. **Instanced crowd rendering.** One draw call for bodies, one for heads, with
   per-instance colour and height from `Appearance`. This simultaneously fixes
   bill 4 and delivers visual variety, since per-instance data is exactly what
   both need. The grass path is the template.
3. **The timetable refactor.** Chores stop being countdown timers and become
   schedule entries; `where_is(agent, t)` becomes a pure function. This is the
   keystone: it unlocks tier 2, kills the network bill, and makes promotion and
   demotion trivial. Do not attempt tiers before this.
4. **Promotion and demotion** on a volume boundary, with interest management. Now
   bills 1, 3 and 6 stop scaling with population at all.
5. **Navigation.** A real flow field or navmesh per deck, replacing both dead
   stubs. Crowds that walk through walls read as broken no matter how many there
   are, and flow fields are the correct technique precisely because their cost is
   per-cell, not per-agent.
6. **Animation.** A skinned pipeline with a walk, an idle, and a work cycle, and
   an impostor rung for distance. Last because it is the largest content-pipeline
   investment and every rung above is visible without it. Note that skinning and
   instancing only coexist if poses ride a bone-matrix storage buffer; storage
   buffers are available (`wgpu::Limits::default()`) but per the v0.782 incident
   that must be proven by a real boot, not by a passing test.

   The impostor rung has a head start and a gap. `src/lod_registry.rs` already
   parses `data/lod/categories.ron`, which has `animal_small` and `animal_large`
   rows, and `src/renderer/billboard_bake.rs` already bakes side-on sprites into
   an atlas for trees. But **every row in that file has `billboard_m: 0.0`** and
   the only consumer anywhere is the `tree` row, so no card or billboard stage
   exists for creatures at all. A humanoid impostor is the existing baker aimed at
   a different mesh. Convenient coincidence: past roughly 40 m an impostor is
   indistinguishable from a 348-triangle box-and-sphere, and 40 m is already where
   `CREW_NAME_DIST` retires the nameplate, so the two culls agree for free.

## The scaling ladder

| Visible | What it needs | Verdict |
|---|---|---|
| **500**, one sector | instanced draw + spatial-grid AI + a net HashMap | comfortable at 30 fps; blocked at 60 by the unattributed 17.7 ms, not by NPCs |
| ~5,000 | + humanoid impostors + frustum and distance culling + tick budgeting | reachable, still 1-2 draws |
| ~50,000 | + GPU culling writing `IndirectArgs` (the engine already has this shape) + GPU animation | reachable on the 4070, not on the platform floor |
| billions | tier 1 aggregate | never a rendering problem |

Two things bite before the draw does. **Simulation caps around 8,000 agents on one
core** even after the O(n squared) is fixed, at roughly 2 us per agent in a 16.6 ms
budget, and threading cannot buy past that until ~50k (above). And **the CPU work
that FEEDS the instance buffer** is the real trap: the grass draw scales
beautifully while the grass HARVEST that fills it measures 16-31 ms
(`src/terrain/grass.rs:702`), runs inline on the frame thread every 4 m of camera
movement, and produces "one doubled frame every few seconds of walking". The
documented fix (time-slice it) is unbuilt. A crowd harvest must not repeat that.

Everything in the first two rungs helps WEAK hardware most, which matters given
the platform floor: the current cost is CPU submission (a low-clock CPU pays
1.50 us per draw harder) and unmanaged overdraw (a small GPU pays fill harder).
Collapsing 1,000 draws into 1 is a pure win on integrated graphics. Only the GPU
culling rung is 4070-specific.

## Open questions

- **Where does tier 2 live, relay or client?** Deriving it on both is the whole
  point, but somebody must own deviations. Probably: relay owns the seed, the
  timetable and the deviation list; clients derive everything else.
- **What is the promotion volume?** A zone, a room, a radius, or a portal-visible
  set. The zone system already partitions the ship, which argues for zone plus
  adjacency, matching the rendering LOD strategy already proposed in
  mothership-superstructure.md.
- **Do inhabitants persist?** A roster derived from a seed is free but forgets
  everything. A roster with saved state costs storage per person. Likely: derived
  by default, with a small promoted set of "known" inhabitants who persist once a
  player interacts with them.
- **How much does an inhabitant matter?** If crowd members consume resources in
  the tier-1 aggregate, the player's actions can starve them, which is the whole
  Generation-Ship co-op thesis. If they are scenery, the ship is a diorama. This
  is a taste call and it is the operator's.

## Related docs

- [npc-crowd-stress.md](npc-crowd-stress.md), the measurement protocol and the
  corrected inventory of what exists
- [mothership-superstructure.md](mothership-superstructure.md), zones and the
  aggregate population reasoning
- [game-modes.md](game-modes.md), where this sits among the engagement axes
- [infinite-of-x.md](infinite-of-x.md), why rosters, shifts and schedules are data
