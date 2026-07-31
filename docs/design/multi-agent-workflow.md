# Multi-agent workflow

> Written 2026-07-30 after the operator asked whether the manual "three Claude chat
> windows" setup could become an automated workflow with specialised roles (plants,
> weather, enemy AI, friendly AI, orchestrator, critic, challenger).
>
> Short answer: yes for the advisory roles today, yes for data-owning domains with
> small changes, and only with real care for code-writing domains. The constraint is
> not agent count. It is **file collisions**.

## The roster, and why each role exists

Every role below is justified by a documented failure in this repo, not by a generic
notion of what a team should have. If a role cannot be tied to a real recurring cost
here, it is not worth its tokens.

| Agent | Writes? | The failure it exists to catch |
|---|---|---|
| `runtime-verifier` | no | v0.782-784 shipped 3 unbootable releases; v0.1029-1038 shipped 10 that panicked on world entry. `just verify` is entirely static and passes on all of them. |
| `doc-truth` | no | PAGES.md claimed parity that did not exist for months; two comments claimed persistence with no backing field; PRIORITIES marked shipped work as NEXT. Agents are told to trust these files. |
| `historian` | no | CLAUDE.md carries seven separate "never rebuild what exists" rules. That much process implies the failure is common and expensive. |
| `migration-guard` | no | BUG-046 took the live relay down ~25 min. Fresh-state tests all passed; only the existing database failed. |
| `critic` | no | Confident-but-false findings, and verification that could not have detected the problem. |
| `challenger` | no | Approaches adopted by default rather than chosen. |
| `domain-writer` | **yes** | The only writer. Owns one domain's files, never the shared wiring. |

**Six read-only to one writer is the intended ratio.** The scarce resource in this
repo is not code generation, it is trustworthy verification: nearly every expensive
incident was work that looked verified and was not. Prefer three writers and five
checkers over eight writers.

### Deliberately NOT added

- **researcher / planner / explorer.** Claude Code ships `Explore`, `Plan` and
  `general-purpose` built in. Duplicating them adds maintenance for nothing.
- **security auditor.** The 51-agent audit of 2026-06 closed the code surface, and
  CLAUDE.md records that dense security-jargon prompts tripped a model safeguard.
  Low marginal value against a real cost. Revisit before launch, not now.
- **test-writer.** Tests written by the same pass that wrote the code inherit its
  blind spots. `critic` checking whether a test *could have failed* is worth more.

### Worth adding when automation actually starts

- **`integrator`** - the counterpart to `domain-writer`. Applies WIRING REQUESTS
  serially, verifies between each, stages by lane, commits per domain so each change
  keeps its own rationale. Without it, wiring requests pile up with nobody to apply
  them. This is the first gap to fill when going automated.
- **`orchestrator`** - conflict scheduling (see below). Only useful once there are
  enough parallel domains to schedule.
- **`accessibility`** - the mission is explicitly tech-illiterate-first and nobody
  checks it. The tofu-glyph lint and the font-size setting that silently reset are
  both symptoms. Currently no automated coverage at all.
- **`perf-warden`** - `probe-sweep.js` already captures fps per vantage, so a
  regression check is mostly wiring rather than new tooling. The water arc (16-18 fps
  in ocean views versus 30-40 over land) is TIER 0 right now.

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

## Worktree isolation is a REQUIREMENT for automation, not an option

Evidence, gathered the hard way on 2026-07-30. `just ship` was changed that day to
commit staged-only, specifically to stop sessions sweeping each other. **A sweep
happened again immediately afterwards**, because the fix only covers `just ship`: a
session that runs `git add -A` directly, or `just ship-all`, bypasses it entirely.

That is not a gap to patch. It is structural:

- All sessions share one working directory, one git index, one `cwd`.
- There is therefore **no per-session state on disk** to attribute a change to.
- A pre-commit hook cannot help, because it cannot tell WHO staged a file. The
  "commit spans 3+ lanes" heuristic in `scripts/lanes.js` catches the obvious case and
  missed this one, because one session's legitimate work plus another's files still
  read as a single lane.
- Documentation (CLAUDE.md) only works for a session that reads and follows it. Four
  sweeps in one day, one of them after the rule was written, is the measurement.

**Conclusion: a shared checkout is workable for a human driving 2-3 chat windows and
paying attention. It is not workable for automated agents.** Before turning on real
automation, domain agents must run with `isolation: "worktree"`, which gives each its
own working directory and index. Then attribution is free, `git add -A` inside an
agent is harmless, and merges become explicit rather than accidental.

The counterpart requirement: a workflow using worktrees must **never** invoke
`just clean-worktrees`, which force-deletes worktrees and branches with no check for
unmerged work and has already destroyed completed, review-approved agent work.

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
