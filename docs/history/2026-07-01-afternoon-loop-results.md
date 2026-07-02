# Afternoon/evening loop results (2026-07-01 -> 02, operator awake, fleet mode)

> Companion to `2026-07-01-afternoon-loop-plan.md` (the backlog this executed).
> Session arc: single-orchestrator loop through v0.662, then FLEET MODE on the
> operator's directive ("spin up many subagents... developing the whole app, not
> just one part at a time") -- parallel isolated-worktree agents, each committing
> to its own branch, the orchestrator reviewing/merging/releasing each. The
> session ended when the account hit its spend limit mid-wave-3; everything
> completed before that is shipped, everything partial is preserved on pushed
> branches (nothing lost).

## Shipped releases (15 in this loop, all tagged + published + pushed)

| Version | What |
|---|---|
| v0.656.0/.1 | Homestead Phase A: `home_solo.ron` (one-person self-sufficient home) + the Settings "Home Design" Family/Solo selector (`home_variant` plumbing -- `MachineHome::load` was hardcoded to home.ron everywhere) |
| v0.657.0 | Homestead gaps #1-2: real edible mushrooms (oyster/shiitake/button) in plants.csv + tank fish (tilapia/channel_catfish) in creatures.csv |
| v0.658.0 | Studio real mic meter (`net::voice::mic_level`) + FIRST-EVER help_modal adoption (3 topics; the native help system had zero call sites until then) |
| v0.659.0 | Donate page fetches the CONNECTED server's real funding info; fake "$350/$1000" bar removed. Adversarial review caught a money-routing bug pre-commit (stale server-A addresses shown as server-B's) -- fixed + regression-locked |
| v0.660.0 | Native GOVERNANCE LIVE: real proposal feed + weighted tallies + Dilithium-signed vote_v1/proposal_v1 built with the in-crate ObjectBuilder. Review found 6 defects (cross-server stale-proposal voting, a ~17-min fetch pin, ...) -- all fixed + locked (7 tests incl. relay-storage round-trip) |
| v0.660.1 | version stamp (build-game) |
| v0.661.0 | Laws category filter chips (the data's own `categories`, surfaced for the first time) + BASE/REAL as real bordered chips |
| v0.662.0 | Humanity Mission Dashboard visual pass: heart-badge hero + accent rule, tiered card weights (accent CTA / quiet essays), shared stat cards with Civilization |
| v0.663.0 | ECONOMY AUTOMATION PHASE 1: AutoRefine machines (smelter auto-smelts, new workbench auto-crafts) + drone standing orders ("Keep mining") + game-time economy clocks. THE test: `full_chain_drone_ore_becomes_a_hammer_untouched` -- one commission, zero interaction, ore becomes a tool. Review found 5 defect classes (creative-mode item printer, same-tick duplication TOCTOU, mid-batch despawn loss, full-inventory grinder, standing-order dormancy) -- all fixed + locked (11 tests) |
| v0.663.1 | Web Laws mirror (fleet agent): /laws with jurisdictions, chips, search; shared logic in laws-logic.js; 42 assertions |
| v0.664.0 | Homestead gaps #3-4 (fleet agent): crop_nutrition.ron (85 crops), component_outputs.ron + location.ron, feature-neutral loaders + calibrated pure math (potato test 0.96x of the design claim) |
| v0.665.0 | WEB GOVERNANCE VOTING REAL (fleet agent): browser-built Dilithium-signed vote_v1 over the pre-existing KAT-locked canonical-cbor.js; `just vote-kat` proves JS==Rust byte-for-byte; a frozen browser-built submission verifies through the actual relay wire path |
| v0.666.0 | NPC CREW CHORES (fleet agent): 14 data-driven chores, deterministic rotation, travel + dwell, synced + rendered natively -- the agent discovered crew were NEVER rendered before; this made them visible AND purposeful. PERSIST_KEY v8->v9 |
| v0.667.0 | "What one home cannot close" panel (fleet agent): the operator's core pedagogical ask -- 5 traded loops grounded in the game's own recipes; "The gap is not a failure. It is civilization." |
| v0.668.0 | Grow-light power meter (fleet agent): green/amber/red vs real headroom ("this is why the garden uses the sun") + REAL BUG FIX: batteries were counted as 48 kWh/day phantom demand EACH (home meter read ~395 instead of ~11 kWh/day) |
| v0.669.0/.1 | Studio Program/Preview split (fleet agent): stage scenes, Cut to Program, edits target preview; 7 state tests + reviewed snapshot. v0.669.1 = the operator's exe build stamp |
| v0.670.0 | Saffron fractional-yield fix (fleet agent WIP completed by orchestrator): yields f32 + probabilistic harvest rounding + a zero-drop guard over all of plants.csv (the row-resilient loader had silently eaten saffron for months) |

Test suite: 659 -> **710 lib tests** over the loop, all green at every release.
Every substantive orchestrator diff got a 2-lens adversarial review workflow
BEFORE commit (Donate, Governance, Economy) -- each review caught real bugs.

## Fresh exe for the operator

`v0.669.1_HumanityOS.exe` (also `HumanityOS.exe` refreshed) -- has everything
through the Studio split. The v0.670 saffron fix is relay-safe data/parser work;
next `just build-game` picks it up. The relay redeployed via CI, so crew NPCs +
web voting are live server-side.

## Cut off mid-flight (preserved, NOT lost -- all pushed to origin)

The spend limit ended wave 3 mid-work. State of each:

1. **`worktree-agent-a168793b96ce96490` (saffron fix)** -- was essentially
   complete; orchestrator verified + merged + shipped as v0.670.0. DONE.
2. **`worktree-agent-aac73bcf603eec50f` (crew chore-label nameplates)** -- WIP
   commit pushed (175 insertions: hud.rs draw path + lib.rs populate +
   gui/mod.rs field + FEATURES note). Unverified/incomplete; resume by checking
   out the branch, finishing per the v0.666.0 agent's follow-up spec (labels
   already sit on RemoteNpc.name/.activity), and running the full battery.
3. **`worktree-agent-a8ca55ebf1a29a385` (studio.rs theme-token migration)** --
   WIP commit pushed (167 insertions: theme.ron tokens + theme.rs accessors +
   settings.rs editor rows + theme.css regen). Unverified; the goal is removing
   studio.rs from theme_token_lint's LEGACY_OFFENDERS -- verify that lint
   passes before merging.
4. **`agent-ac6aa0b2dcc171a4f` (computed food loop)** -- produced NOTHING
   substantive before cutoff; no branch worth keeping. The task (compute the
   Home page's food numbers from crop_nutrition.ron) is still open and fully
   specified in PRIORITIES.md Active Focus.
5. **Snapshot QA sweep agent** -- rendered snapshot PNGs in its worktree but
   never wrote its report. No unique artifacts; re-run when capacity returns.

## Remaining backlog (non-gated)

- Finish + merge the two WIP branches above (nameplates, studio theme).
- Computed food loop from crop_nutrition.ron (not started).
- Snapshot QA sweep (re-run; then triage findings).
- Stray "Unaligned" label in UI snapshots (operator started a separate session
  for it -- task_30ff8cfe).
- Studio real transport (multi-cycle, loop-plan item #3).
- Web governance proposal-CREATION form (voting shipped; creation is native-only).
- Live-browser click test of web voting post-deploy (agent recommendation).
- pq-identity.js dead legacy pqSign/pqVerify (stale noble arg order + CDN
  import, zero callers) -- delete when convenient.

## Gated on the operator (do not start without their input)

- Donate payment-method list (Patreon discrepancy vs server-config.json).
- Mute Server scope/sequencing.
- Dead-code cleanup deletion (~15 files + maps.rs).
- Economy Phase 2: is a truck an Item or a Structure?

## Process notes (what worked)

- **Adversarial review before commit caught real bugs every single time it
  ran** (money-routing, cross-server vote misdirection, creative-mode item
  printer, duplication TOCTOU). Worth the tokens on every substantive diff.
- **Isolated worktrees + commit-to-branch + orchestrator-merges** let 4+ agents
  build in parallel with zero lost work and zero clean-worktrees risk; two
  trivial conflicts (PAGES.md, a version-stamp Cargo.lock) in nine merges.
- **Agents kept finding real pre-existing bugs while building features**:
  never-rendered crew NPCs, battery phantom demand, the saffron row drop, the
  utility-meter inflation. Fresh eyes over old code pay.
- **Spend limits end sessions abruptly**: the WIP-commit-and-push salvage at
  the end is the pattern -- worktree branches pushed to origin survive anything.
