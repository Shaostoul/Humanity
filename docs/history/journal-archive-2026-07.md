# Orchestrator journal archive -- 2026-07

Decisions rotated out of `data/coordination/orchestrator_state.json` (oldest first within each batch; newest overall is in the live journal). Source of truth for "why we did X" once it ages past the live tail. See also git log + the GitHub releases.

## 2026-07-02

**Decision:** OPERATOR DECIDED economy Phase 2 vehicle pipeline: BOTH models, staged. Stage 1: big end-products craft as an oversized "kit" ITEM (lives in home stock, not backpack; tradeable through the existing market) that deploys to spawn the real Vehicle entity -- reuses the whole existing crafting/storage/market chain for easy testing. Stage 2: factories gain the ability to SPAWN the physical vehicle in the world when a job finishes (finished-goods pad). Stage 3: TRANSPORT -- a purchased tank/mecha/spaceship physically travels from where it was built (factory/fleet) to the buyer, and the player can FOLLOW or TAKE OVER driving/piloting the transport.

**Why:** Operator 2026-07-02: "I like the idea of having both... The in inventory idea allows us to test a lot of things real easily. The spawning the vehicle gives us the ability to spawn physical objects in the world after a job finishes. Then add in the transport. It would be cool if the player could follow or take over the transport of whatever they bought." Long-term vision logged same message: the line between game objects and real buildable objects blurs -- an in-game house/car/spaceship should eventually correspond to real, buildable, working designs.


## 2026-07-02

**Decision:** Declined the operator's .rar preference for release archives; shipped .zip alongside .tar.gz instead, with exe-first for Windows.

**Why:** RAR is proprietary: no open-source tool can CREATE it (CI cannot legally produce it) and Windows 10 (our stated minimum) cannot open it natively -- both directly against the no-corporations/no-catch ethos and the friction goal. Zip opens natively everywhere and WinRAR (the operator's preference) handles zip fine, so the operator personally loses nothing.


## 2026-07-02

**Decision:** Session wrap on operator instruction after the spend limit killed wave 3: salvaged all partial agent work as WIP commits pushed to origin branches; completed + shipped the essentially-finished saffron parser fix as v0.670.0 (710 tests); wrote docs/history/2026-07-01-afternoon-loop-results.md as the durable session record.

**Why:** Operator: "wrap up what was done and ship so it is saved in case anything happens to this session." Durability rule: pushed branches survive anything -- session loss, worktree accidents, machine failure.


## 2026-07-02

**Decision:** Fleet mode complete for waves 1+2: 8 releases shipped in one evening (v0.663.0 through v0.669.0), all agent branches reviewed + merged + re-verified on main before release, every Rust merge re-running the full suite (final: 709 tests).

**Why:** Operator directive to use the remaining weekly allowance developing the whole app in parallel. Isolated worktrees + commit-to-branch + orchestrator-merges kept the clean-worktrees disaster class impossible while 4+ agents built simultaneously.


## 2026-07-01

**Decision:** Entered fleet mode on operator directive: 4 parallel worktree implementation agents (web governance voting+KAT, web laws mirror, NPC task-AI, homestead data gaps 3-4) with strict file-disjointness from the main tree's uncommitted economy-automation diff. Economy Phase 1 implemented on main: AutoRefine machines (data-driven auto_recipe in home.ron: smelter->smelt_iron, new workbench->craft_hammer) acting on the home inventory, drone standing orders (Keep mining checkbox -> auto_mine_order -> Deliver-arm relaunch), and scaled_dt so all economy timers respect time_scale.

**Why:** Operator: 92% of the weekly allowance left with ~24h to reset; wants the whole app developed in parallel, explicitly asked for many subagents. Economy Phase 1 is the operator's living-ecosystem vision -- the full_chain_drone_ore_becomes_a_hammer_untouched test proves one drone commission becomes a finished tool with zero interaction.


## 2026-07-01

**Decision:** Shipped v0.658-v0.660 (Studio mic meter + help_modal adoption; Donate real server-funding fetch; native Governance page fully live with Dilithium-signed vote_v1/proposal_v1 submission via the in-crate ObjectBuilder). Adopted a review-before-commit discipline for substantive diffs: a 2-lens adversarial Workflow ran on both the Donate and Governance changes BEFORE committing.

**Why:** Operator re-enabled Fable 5 + ultracode mid-loop and asked for maximum capability. The review workflows proved their cost immediately: Donate review caught a real money-routing bug (stale server-A donation addresses displayed as server-B's), Governance review caught 6 defects including cross-server stale-proposal voting (an orphan vote stored on the wrong server with a false success message) and a ~17-minute fetch pin. All fixed + regression-locked before the code ever reached main.


## 2026-07-01

**Decision:** Shipped Phase A of the self-sustaining homestead design (v0.656.0/0.656.1): authored data/machines/home_solo.ron per docs/design/homestead-solo-design.md's exact BOM, and built the home_variant selector (AppConfig field + SettingsState mirror + machines::home_ron_path() touching all 5 real MachineHome::load call sites + a Settings -> Data -> Home Design radio UI) since MachineHome::load was hardcoded to home.ron everywhere with no variant mechanism.

**Why:** Operator asked for a dedicated homestead design pass ("designing a fully fledged self-sustaining homestead") to establish the honest one-person baseline before scaling to infinite. The design doc (produced by a 3-research+1-synthesis Workflow) found ~90% of the BOM already exists as data; implementing it required discovering and fixing the missing loader-variant gap first, otherwise the new file would be inert.

