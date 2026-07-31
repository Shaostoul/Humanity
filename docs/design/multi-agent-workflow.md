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
| `fidelity-expert` | no | "Why does this not look real?" Names the specific missing cue and the technique, grounded in real-world behaviour. Never proposes lowering quality. |
| `perf-expert` | no | Makes the SAME image cheaper, at one instance and at infinite-of-x scale. Measures before proposing (v0.1067: the particle loop was memory-bound, not compute-bound, which was the opposite of the assumption). |
| `domain-writer` | **yes** | Owns one domain's files, never the shared wiring. |
| `integrator` | **yes** | Applies WIRING REQUESTS to shared files one at a time, verifying between each. The only agent permitted to touch `lib.rs` and friends. |

### Fidelity and performance are a PAIR, held in tension

The operator's rule is maximum quality first, then tune performance toward it, and
never trade fidelity away to buy frames. So these two roles are deliberately opposed
and deliberately ordered:

- `fidelity-expert` asks **"what specific cue is missing that makes this read as
  fake?"** and proposes the technique that supplies it. It never proposes cutting
  quality.
- `perf-expert` then asks **"how do we get that exact image for less?"** Its output is
  same-image-less-cost. An optimisation that changes the appearance is reported as a
  fidelity regression, not accepted as a win.

If a quality target genuinely cannot be afforded, that is escalated to the operator as
a decision, never resolved silently by degrading the result.

`perf-expert` covers two distinct axes, because they have different answers:
**one instance** (fragment ALU, texture bandwidth, overdraw, vertex count) and
**N instances, the infinite-of-x question** (draw submission and instancing, culling,
LOD, impostors, shared work, memory layout). The live example of the second: water
shells still go through the classic per-object path at roughly 640 draws worst case.

**Eight read-only to two writers is the intended ratio.** The scarce resource in this
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

- **`orchestrator`** - conflict scheduling across several domains at once (see below).
  Only earns its keep when there are enough parallel domains to schedule; a single
  domain pass does not need one, which is why `domain-pass.js` has no orchestrator
  agent in it.
- **`accessibility`** - the mission is explicitly tech-illiterate-first and nothing
  checks it. The tofu-glyph lint and the font-size setting that silently reset every
  launch are both symptoms of that gap. Currently no automated coverage at all.
- **`perf-warden`** - a scheduled regression check rather than an on-demand analyst.
  `just perf-sweep` and `just perf-diff` already exist and vantages already carry
  `perf_floor_fps`, so this is mostly wiring rather than new tooling.

### Content agents (data-only, safest to automate)

These own data files and never touch `src/`, so several can run at once with almost no
collision risk. This is the cheapest real throughput available.

| Agent | Owns | Why it matters |
|---|---|---|
| `lexicographer` | `data/glossary.json` | 201 terms. Mission is explicitly tech-illiterate-first; every undefined word is where someone gets stuck and leaves. |
| `botanist` | `data/plants.csv` | 134 species on a 27-column schema already sourced from USDA/FAO/extension services. Someone may plant a real garden based on this, so a wrong number teaches someone to fail at growing food. |
| `homestead-engineer` | `data/home_outline.json`, `data/self_sufficiency/`, the homestead design docs | Mass-and-energy balance for the solo homestead. Get one resident exactly right, then N is mostly arithmetic. |

### Animations: NOT an agent yet, and why

There is currently **no 3D animation system in this repo**. No skeletal rig, no glTF
animation support, no animation data, no keyframe or pose code. The apparent hits are
UI easing inside `src/gui/`.

So an "animations agent" would not be populating or improving a domain, it would be
designing and building a subsystem from nothing. That is a different job with a
different shape, and a `domain-writer` pointed at an empty domain will invent an
architecture nobody reviewed. **The right first step is a design pass** (`challenger`
plus the built-in `Plan`) answering: skeletal or vertex, authored in Blender or
procedural, how it interacts with the instancing the renderer already relies on, and
what it costs at forest scale. Once a system exists and has owned files, a normal
`domain-writer` covers it and no special agent is needed.

## The three mechanisms available

1. **Subagents** (`.claude/agents/*.md`). Named roles with their own system prompt,
   model and tool set. Invoked on demand. Ten defined, see the roster above.
2. **Workflow scripts** (`.claude/workflows/*.js`). Deterministic JS orchestration:
   fan out, pipeline, barriers, judge panels, loop-until-dry. This is the automated
   part; control flow is code, not model judgement. Two exist: `visual-sweep.js`
   (capture the vantages, judge each against its golden spec) and `domain-pass.js`
   (the fidelity-then-performance pass described below).
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

### Verified, not assumed (2026-07-30)

Tested against a real worktree on this repo rather than trusting the theory:

| Property | Result |
|---|---|
| Edit in worktree shows in main tree | **No.** Main stayed clean. |
| `git add -A` inside a worktree touches main's index | **No.** Fully contained. |
| Merge back to main | Clean, `ort` strategy, correct diff. |
| Cost to create | ~2,600 files checked out, a few seconds. |

So the dangerous primitive (`git add -A`) becomes *safe* inside a worktree, which is
exactly the property automation needs. The probe was reverted and the worktree and
branch removed; nothing was left behind.

The caveat that makes this bearable: worktrees share one `.git`, so a branch checked
out in one cannot be checked out in another. Give each agent its own branch name.

The counterpart requirement: a workflow using worktrees must **never** invoke
`just clean-worktrees`, which force-deletes worktrees and branches with no check for
unmerged work and has already destroyed completed, review-approved agent work.

## The first real workflow: `domain-pass.js`

Improves one visual domain end to end, with fidelity and performance in tension.

```
Workflow({ scriptPath: ".claude/workflows/domain-pass.js",
           args: { domain: "clouds",
                   owns: ["src/renderer/clouds.rs", "assets/shaders/pbr/40-clouds.wgsl"],
                   vantages: ["ocean-storm-low", "limb-400km"] } })
```

Five phases, and the ordering is the point:

1. **Prior art** - `historian` gates the whole run. If the pass already shipped and
   there is no remaining gap, the workflow STOPS rather than spending a fleet
   rebuilding it. Cheapest agent, guarding the most common waste.
2. **Analyse** - `fidelity-expert` and `perf-expert` in parallel. Both read-only, so
   they cannot corrupt anything and cannot corrupt each other.
3. **Decide** - `challenger` sees both reports and returns the single concrete task,
   or RECONSIDER, which stops the run before any code is written.
4. **Implement** - ONE `domain-writer`, in its own worktree. Single writer by design:
   this workflow improves one domain, and parallel writers only pay off across
   *different* domains.
5. **Verify** - `runtime-verifier` (does it still boot and enter the world) and
   `critic` (could the verification have failed) in parallel.

**It does not commit or merge.** The writer leaves work staged in its worktree and the
workflow returns a report. Landing it is the operator's call, because agent work here
has been wrong in ways that passed every local check.

Requires a built release exe and a GPU: two phases drive the real game.

## Custom agents load at SESSION START (found the hard way, 2026-07-30)

The first `domain-pass` run failed with:

    agent type 'historian' not found. Available agents: claude, claude-code-guide,
    Explore, general-purpose, Plan, statusline-setup

That list is exactly the built-in set from the start of the session, before
`.claude/agents/` existed. The definitions were on disk and correctly formed; they
were simply not discovered, because **the agent registry is built when the session
starts and is not reloaded when you add files mid-session.**

So the sequence for anyone setting this up is:

1. Write the `.claude/agents/*.md` definitions.
2. **Restart the Claude Code session.**
3. Then invoke the workflow.

Two smaller findings from the same shakedown, both now fixed in the script:

- `args` can arrive as a JSON **string** rather than an object. `domain-pass.js` now
  parses it either way instead of relying on the caller to get it right.
- The argument guard fired correctly and cost nothing: 0 agents, 15 ms. A workflow
  that validates its inputs before spawning anything is worth the few lines.

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
