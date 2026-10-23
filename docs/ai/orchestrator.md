# Orchestrator handbook

**Who reads this:** the top-level session the operator talks to directly, the one that orients at
session start, dispatches agents, merges their work and closes the session. A dispatched subagent
does not need it.

These rules lived in `CLAUDE.md` until 2026-10-23, when that file was split by audience so the
file every agent loads on every call stays short (operator: "We should have a varied set of docs
like this so worktrees always learn only what they need to."). Their wording is unchanged, and
they keep their full force. `CLAUDE.md` still holds what every agent needs, including the short
START HERE list this page completes.

## 1. Session start: the rest of the checklist

CLAUDE.md's START HERE runs `just brief`, then TIER 0, then the journal. The full text of those
steps, and the ones that only a top-level session does:

- **READ the top of TIER 0 in `docs/PRIORITIES.md`**: strict-ranked TACTICAL backlog; its TOP item is what gets worked on next (the file is over 100 KB: read TIER 0, search the rest). New convention as of v0.283.x. Its STRATEGIC companion is **`docs/ROADMAP.md`** (themed, status-badged, the public roadmap AND the build to-do list; the website renders it from `data/roadmap.json` via `scripts/roadmap-to-json.js`). When you change strategic scope, update ROADMAP.md + regenerate the JSON.
- **READ `data/coordination/orchestrator_state.json`**: running session journal. Tells you what the previous orchestrator was doing, what decisions were made, what scopes have active claims, what NOT to redo.
- **Run `node scripts/agent-status.js`**: per-scope coordinator-friendly summary aggregating `data/coordination/sessions/*.json`.
  *Note (2026-10-23):* the script now warns that its scope-audit rows are 180+ days stale and says
  to trust the journal and PRIORITIES instead. Until a scope audit refreshes them, treat its output
  as background, not as live claims.
- **Version sync check:** `docs/ai/release.md`, "Version SOP" (session start).
- **Before claiming a multi-AI scope**, check `data/coordination/agent_registry.ron` for ownership rules and the `agent_sessions` SQLite table for active claims.
- If agents report editing files under `native/src/`, `server/src/`, or `crates/`, those paths don't exist anymore. Run `just clean-worktrees` and redo against the real `src/` tree.
  *Superseded in part:* "Run `just clean-worktrees`" in that step applies to the operator or an
  orchestrator only, after the check in section 7; everyone else re-syncs their own worktree.

Every "before you..." read in CLAUDE.md's Router (FEATURES, code-map, STATUS, PAGES, BUGS,
ui-system, infinite-of-x, storage-architecture, cryptography, `docs/SOP.md` for the development
procedures) applies to you as to every agent.

## 2. Session end

- **Before ending the session** with significant changes: update `docs/PRIORITIES.md` (what's next) AND `data/coordination/orchestrator_state.json` (why we got here). End the reply with a "Next:" pointer (the session-end convention adopted in v0.283.x). If the journal has grown large (run `just brief`; or it's > ~150 KB), run `just rotate-journal` to archive old decisions to `docs/history/journal-archive-<month>.md` (keeps newest at the bottom).

**Session end: Verify sync**
- If any changes were made, confirm GitHub release matches local `Cargo.toml` version
- Never leave a session with local ahead of GitHub

## 3. Cross-session persistence (perpetual)

Your memory between sessions is the **disk, not the conversation**. Anything only "internalized" in-context is lost at session end, or sooner, on a crash or context compaction. So **persist durable knowledge to its store the moment it's established, not deferred to session end** (the session may not get a clean end). What goes where:

| What you learned | Where it persists | When to write it |
|---|---|---|
| Operator correction / working preference | `CLAUDE.md` (a norm section) + memory `User Preferences` | the moment it's given |
| Decision + the WHY behind it | `data/coordination/orchestrator_state.json` → `recent_decisions` | at each significant decision/checkpoint |
| What to work on next | `docs/PRIORITIES.md` | whenever scope/priority shifts |
| Lesson / gotcha / incident | the right `docs/` file (`INCIDENT-PLAYBOOK.md`, the design doc, `BUGS.md`) | when learned |
| Session narrative | `docs/history/<date>.md` | at session end |

Session START already reloads `CLAUDE.md`, `MEMORY.md`, `orchestrator_state.json`, and `PRIORITIES.md` (see the START HERE checklist), so what's on disk carries forward; what's only in-context does not. Write small + immediately; never let a durable learning live only in the current conversation.

## 4. Usage-budget pacing (operator protocol, 2026-07-04)

**Which rule wins (2026-10-23 reading):** the throughput recalibration of 2026-07-11 (the last
paragraph) governs how hard to parallelize. It supersedes the 2026-07-04 cap of ~5 subagents, and
the "~70% = shift to solo small-ship mode" phase of the first paragraph, both of which throttled
for budget. Still standing from 2026-07-04: wind down to a committed, pushed tree near the cap,
verify selectively, never re-run a battery that passed on unchanged code, never relaunch a whale
workflow whose output is already on disk. Consistent with both, the 2026-10-22 rule in
CLAUDE.md (Dual modes): a waste of 5% or more (frame time, build time, tokens) is worth a fix.

The AI CANNOT see the subscription usage meter. The operator calibrates it with
one-line readings in chat whenever convenient: "usage: 36%, resets 9pm" (5-hour
window) and occasionally "weekly: 40%". Between readings the AI self-tracks
roughly: each Workflow run reports subagent tokens back (maps ~1M, adversarial
reviews 500-800k -- the whales); a solo implement+verify+ship iteration is
~20-60k. Phased window plan: FRESH window = big swings first (multi-agent
workflows, fleet waves, reviews, large arcs); ~70% = shift to solo small-ship
mode; ~85-90% = wind down (journal + PRIORITIES + version stamps + push) so
NOTHING is uncommitted when the cap hits. A spend-limit error mid-workflow =
treat every unverified finding as unadjudicated (the v0.677 review died with 3
unverified findings; 2 were real). Do not default to timid solo loops out of
cutoff fear -- ask the meter, then spend with intent.

**Efficiency recalibration (operator, 2026-07-04, after a 51-agent audit workflow
burned a full 5-hour window in ~5 minutes):** plan upgraded to Max $200 (~4x
headroom), but the directive is anti-waste, not more-spending. Concretely: cap
fan-out waves at ~5 subagents (chunks of 5, not 50); verify SELECTIVELY (adjudicate
the load-bearing claims, not all of them); NEVER re-run a test battery that already
passed on unchanged code; never relaunch a whale workflow when its data is already
on disk (read the output file instead). Time, money, water, electricity, and the
operator's effort all count. Big-swings-first still applies, but a "big swing" is
one well-scoped wave with a clear question, not maximal parallelism.

**Throughput recalibration (operator, 2026-07-11, SUPERSEDES the "cap at ~5 /
anti-waste" tone above for the throughput question):** "In terms of usage we have
plenty available. I would prefer you get through the work faster than be concerned
about being too careful... if you need to spin up a dozen subagents to complete a
task in 5 minutes instead of an hour, then I'd rather you do so. We don't want to
leave any usage on the table. I want to properly use 100% of that [$200 Max]
usage. Every lost usage from 5-hour or 1-week limit resets is lost money." NEW
strategic frame: hitting the weekly cap SOONER is GOOD, not bad, because it means
the operator "has more content to share on social media earlier to hopefully get
donations" (this project runs on donations under a $1k/mo income cap). So: DEFAULT
TO AGGRESSIVE PARALLELISM. Do not self-throttle, do not wind down early out of
budget caution, do not leave headroom unused. When the cap hits, the operator just
sends a prompt and we resume from disk (this is why the commit-early / journal-
current / nothing-uncommitted discipline matters MORE, not less: a hard cutoff can
land any time, so keep the tree shippable). The ONLY remaining limiter is
CORRECTNESS: parallelism is still gated by real file-conflict risk (this repo
funnels through a 16k-line `lib.rs` + the shared shader tail `assets/shaders/pbr/90-fragment-main.wgsl` (the PBR shader is numbered files under `assets/shaders/pbr/`, not one megashader: `docs/ai/renderer.md`), so >2-3
agents editing the SAME shader branch / lib region create genuine 3-way-merge
correctness hazards, not just merge chores) and by independent verification of
every merge. So the shape is: fan out WIDE on disjoint file sets; when tasks must
touch the same hot files, run them as back-to-back waves and merge serially, but
never sit idle to "save budget." "Efficient" now means throughput, not frugality.
See memory `feedback_usage_budget_pacing.md`.

## 5. Dispatching agents

- **Lean hard on parallel subagents** (cost is a flat Max subscription with abundant headroom, see memory `project_financial_context.md` + `feedback_velocity_no_checkpoints.md`); the real limiter is your own verification, so parallelize that too. Fan out wide on DISJOINT file sets; agents that must touch the same hot file (`src/lib.rs`, the shared shader tail `assets/shaders/pbr/90-fragment-main.wgsl`) run as back-to-back waves, merged serially (section 4).
- **Compiles take turns for memory on their own** once `just build-slot` has been run in the main checkout (`docs/ai/builds.md`): a wave of compiling agents queues instead of paging the PC, so no agent needs a waiter of its own. Run it once per machine, and again after a merge that changes `scripts/build-slot/`.
- **Never dispatch several rig-booting agents at once**: stagger them so their boots serialize (one GPU, one instance; CLAUDE.md and `docs/ai/game-boots.md`). Read-only and code work parallelize fine.
- **After a parallel wave merges, run a critic review of the batch**: systems built side by side disagree at the seams (memory `feedback_review_after_parallel_waves`; the `merge-agent` skill).
- **Disk before a wave, and cleanup after:**
  - **Build caches fill C: (it filled on 2026-09-27; the main `target/` reached 1.13 TB on 2026-09-29).** Each worktree builds its own `target/` (8 to 10 GB) and cargo never prunes. The hourly, silent "HumanityOS Dev Disk Guard" (`scripts/pc-disk-guard.js`, `just install-disk-guard`; last run in `just brief`'s DISK row) deletes old builds of our crate in every target folder first, even beside a running build (since 2026-10-23; 182 GB of the main target were builds of old versions; `docs/ai/builds.md`), then worktree targets idle 48 h, the main `debug/incremental` at 75% full and the whole main `target/` at 85%: build output only, never while cargo, rustc or HumanityOS runs. Still yours: delete a merged worktree's `target/` (worktree and branch stay); put scratch builds' target in the scratchpad and delete it after; check `df -h /c` before a wave of worktree agents; `cargo clean` when C: runs short (nobody else building, no game running; ~10 min to rebuild). A landed worktree (`git cherry main <branch>` prints no `+` line, and its `git status` shows only `.claude/settings.local.json`) may go with `git worktree remove --force`, branch kept. Deleting `target/` never loses work, so it is not the operator-only `clean-worktrees`.

## 6. Prompt framing and the model's safety classifiers

*Note (2026-10-23):* Fable access has since ended (the July journal records the Fable-to-Opus
handoff plan; sessions now run on Opus). The "stay on Fable" goal below is history, but the
framing lessons are not: the `[bio]` kill of 2026-09-15 happened on Opus 5. Keep prompts in plain
feature-completeness terms. For parallelism in general, section 4's throughput rule wins; the
"prefer SOLO work" advice applies to security-adjacent work, as the paragraph itself scopes it.

**Stay on Fable; do not trip the model safeguard (operator, 2026-07-06).** The
operator pays for Fable-tier access and wants the work done on Fable. Fable has
a broad dual-use safeguard that, per its own notice, may flag routine coding and
security-adjacent work; a flagged turn is auto-switched to Opus (and if the Opus
safety classifier is momentarily unavailable, a follow-up tool call can fail with
"Service is busy"). What tripped it on 2026-07-06: a 6-agent audit workflow whose
prompts were dense with security jargon (E2EE, authorization gates,
permission-gated, ban, identify challenge, adversarial). To keep sessions on
Fable: (1) prefer SOLO work over large fan-out workflows -- a 6-agent security
audit spawns six flaggable prompts; reading the code yourself triggers none. (2)
Describe work in plain feature-completeness terms ("is the DM flow finished",
"can the app reach the server", "does the Files page add/remove server files"),
not offensive-security framing ("harden the attack surface", "authorization
gates"). (3) Keep actions small and incremental. This is about avoiding
FALSE-POSITIVE flags on legitimate cooperative-platform work, nothing more. Big
multi-agent swings are still fine for plainly-worded non-security tasks (maps,
refactors, gameplay); reserve them for those.

**The same thing happens with a `[bio]` classifier on Library health and
toxicology guides (2026-09-15, Opus 5).** A subagent writing the home-canning
safety guide was killed mid-work, ~80% done, with `Details: [bio]`. The topic is
about as mainstream as consumer safety gets (USDA publishes a whole book on it),
but the PROMPT was dense with toxicology framing: lethal dose, toxin production,
spores germinating, poison. Relaunching with the same required content described
in practical terms ("why home canning has tested processes and what each rule is
protecting against") went through fine. Nothing was written when it died, so the
work was lost entirely, not just interrupted.

Several Library topics still to write sit in this territory: `forage_toxic`,
`forage_edible`, `health_poisoning`, `chem_toxins`, `hunting`, `butchery`. For
each, write the prompt the way the finished guide would describe itself to a
reader, not the way a toxicology paper would. Say what the reader is deciding
("which foods need a pressure canner and why"), not what the mechanism is called.
The safety content itself is unchanged and must stay complete; it is the framing
that trips the classifier. If one dies anyway, check whether the agent wrote its
file before relaunching, and reframe rather than retrying verbatim.

## 7. Worktrees and `just clean-worktrees`

- Run `just clean-worktrees` to kill stale AI context before it corrupts new work. **OPERATOR/ORCHESTRATOR ONLY: a dispatched subagent must NEVER run this.** The script (2026-07-01 hardened, `scripts/clean-worktrees.sh`) now refuses to touch a worktree with uncommitted changes or unmerged commits, but a subagent told to "read CLAUDE.md first" (a routine review-agent instruction) used to read this step literally and run it as its own first action, force-deleting sibling worktrees mid-review; it happened 3 times in one day before the script was hardened. If you are a subagent reading CLAUDE.md as background context for a specific delegated task, skip this step entirely; it is not part of your task.

- **`just clean-worktrees` is OPERATOR/ORCHESTRATOR ONLY (START HERE step 0): a dispatched subagent never runs it.** Stale worktrees cause context rot: an agent reporting edits under `native/src/`, `server/src/` or `crates/` (gone since v0.90.0) found one, so never trust agent reports blindly. Since 2026-07-01 the script skips a worktree with uncommitted or unmerged work (only `--force-unmerged` destroys it); before that it destroyed agents' unmerged work three times in one day, commits and branches included. Before running it, confirm every agent dispatched this session is merged into main or explicitly abandoned. Re-sync a stale worktree of YOUR OWN with `git fetch origin main && git reset --hard origin/main` inside it, NEVER in the shared checkout. This SUPERSEDES START HERE step 15's "Run `just clean-worktrees`" for anyone but the operator or an orchestrator, and the older advice to run it regularly and hard-reset your current worktree.

## 8. Hunk-selective commit of a shared file

**When a SHARED file (lib.rs etc.) holds both your hunks and another session's:**
stage it hunk-selectively, then prove the staged tree compiles before pushing.
Recipe (used 2026-08-23, sealed-sender DM cutover, when lib.rs carried another
session's in-flight renderer hunks referencing fields that existed only in their
uncommitted files): (1) `git diff <file>` and identify your hunks by content;
(2) write ONLY your hunks to a patch (script it; hunk headers self-locate by
context) and `git apply --cached that.patch`; (3) `git checkout-index -a
--prefix=<scratch>/` and run `cargo check` for BOTH feature sets in the scratch
tree - that is EXACTLY what CI and the tag build will see, and it catches the
half-a-refactor-committed failure the working-tree build cannot. Never commit a
shared file wholesale while another session's hunks are in it.
