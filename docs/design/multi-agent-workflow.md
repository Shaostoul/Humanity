# Multi-agent workflow

> Written 2026-07-30 after the operator asked whether the manual "three Claude chat
> windows" setup could become an automated workflow with specialised roles (plants,
> weather, enemy AI, friendly AI, orchestrator, critic, challenger).
>
> Short answer: yes for the advisory roles today, yes for data-owning domains with
> small changes, and only with real care for code-writing domains. The constraint is
> not agent count. It is **file collisions**.

## The three mechanisms available

1. **Subagents** (`.claude/agents/*.md`). Named roles with their own system prompt,
   model and tool set. Invoked on demand. Defined here: `challenger`, `critic`,
   `domain-writer`.
2. **Workflow scripts** (`.claude/workflows/*.js`). Deterministic JS orchestration:
   fan out, pipeline, barriers, judge panels, loop-until-dry. This is the automated
   part; control flow is code, not model judgement.
3. **Worktree isolation** (`isolation: "worktree"` on an agent call). Each agent gets
   its own git worktree, so parallel edits cannot collide in the working tree.

## The actual constraint: shared files, not agent count

Domain work in this repo separates cleanly. Verified 2026-07-30:

| Domain | Owns (disjoint) |
|---|---|
| weather / clouds | `src/systems/{weather,weather_events,atmosphere,hydrology,disasters}.rs`, `src/renderer/{clouds,cloud_noise,atmosphere}.rs`, `assets/shaders/pbr/{30-atmosphere,40-clouds}.wgsl` |
| plants | `data/{plants.csv,plants_visual.ron}`, `src/renderer/{plant_mesh,tree_mesh}.rs`, `src/systems/{ecology.rs,farming/}` |
| creature AI | `data/{creatures.csv,npcs.ron,npc/}`, `src/systems/ai/{behavior,flow_field}.rs` |
| combat | `src/systems/combat/{damage,effects}.rs` |

The shaders are already split into seven numbered files and AI/combat into modules,
so these domains genuinely do not overlap. **But they all funnel through the same few
wiring files:**

    src/lib.rs                        ~17,500 lines
    src/gui/mod.rs                    ~6,800 lines
    src/config.rs
    assets/shaders/pbr/90-fragment-main.wgsl
    Cargo.toml

That is where three parallel agents corrupt each other. Not by editing the same
domain, but by all appending to the same `match` arm or the same init function in
`lib.rs` and producing a three-way merge that compiles into nonsense, or does not
compile at all.

**So the rule is: domain agents never edit wiring files.** They return a WIRING
REQUEST (file, anchor text, exact insertion, reason) and the orchestrator applies
those serially, one at a time, verifying between each. Parallel where the files are
disjoint; serial through the bottleneck. This is encoded in
`.claude/agents/domain-writer.md`.

## What is safe to automate, in order

**Tier 1: advisory roles, read-only. Safe today, any number in parallel.**
`critic` and `challenger` have no write tools. They cannot corrupt anything, they
parallelise perfectly, and they address the two failure modes this repo actually has:
confident-but-false claims, and approaches adopted without being chosen. Start here.

**Tier 2: data-owning domains. Low risk.**
`data/plants.csv`, `data/creatures.csv`, `data/chemistry/*`. Separate files, no
wiring, validated by `just validate-data`. Several `domain-writer` agents can run
concurrently across these with almost no collision risk. This is the cheapest real
throughput win.

**Tier 3: code-writing domains. Needs the wiring discipline.**
One `domain-writer` per domain, disjoint owned paths, wiring requests returned rather
than applied. Merge serially. Verify after each merge, not once at the end.

**Tier 4: renderer and shader work. Serialise it.**
CLAUDE.md already warns that more than two or three agents in the same shader branch
creates genuine correctness hazards, and renderer changes cannot be verified by
`cargo check` alone (device limits and bind-group mismatches only fail at runtime,
which is how ten releases shipped unbootable). Run these one at a time.

## What the orchestrator is actually for

Not "assigning work". Its real job is **conflict scheduling**:

1. Read `data/coordination/lanes.json` and `agent_registry.ron` for ownership.
2. Batch agents whose owned paths are disjoint; run those in parallel.
3. Serialise anything that touches a shared wiring file.
4. Apply wiring requests one at a time, verifying between each.
5. Stage by name (`just mine`), never `git add -A`, and commit per domain so each
   change keeps its own rationale.

## Hard-won constraints any workflow must respect

- **All sessions share ONE working directory** unless worktrees are used. There is no
  per-session state on disk, so ownership cannot be auto-detected. It must be declared.
- **`just ship` commits staged-only** as of 2026-07-30, specifically because blanket
  adds swept other sessions' in-flight work into unrelated commits three times in one
  day. Do not reintroduce `git add -A` into any automated path.
- **`just clean-worktrees` is destructive and operator-only.** It force-deletes
  worktrees and branches with no check for unmerged work; it has destroyed completed,
  review-approved agent work. A workflow using `isolation: "worktree"` must never
  invoke it.
- **Verification must be able to fail.** The recurring defect in this repo is a check
  that could not have detected the problem: pages marked "both" that read different
  data, settings that "saved" but had no config field, tests that pass vacuously.
  Every automated verify step should be sanity-checked by breaking the thing on
  purpose once and confirming the check goes red.
- **Do not auto-merge to main.** Agent work in this repo has been wrong in ways that
  passed local checks. The orchestrator proposes; a human or a verify gate approves.

## Cost shape

Advisory agents are cheap and mostly read. Domain writers are moderate. The expensive
part is verification, not generation, and verification is what this repo actually
needs more of. Budget accordingly: it is better to run three writers and five critics
than eight writers.
