# Night loop results (2026-07-01, ~6.5 hours unattended)

> Companion to [`2026-07-01-night-loop-plan.md`](2026-07-01-night-loop-plan.md)
> (the mission, safety rules, and running backlog written at the start of the
> night). This file is the after-action summary, written when the loop
> reached a natural stopping point: both explicit priorities complete, the
> entire stub-completion backlog closed or correctly reclassified, and a
> self-review pass caught and fixed the one real regression introduced
> during the night. Read this first if you're catching up in the morning;
> the plan doc has the full blow-by-blow if you want more detail on any
> item below.

## Bottom line

12 cycles, 24 commits, versions v0.640.1 -> v0.650.0, all pushed to `main`
and green on CI (Verify + Deploy to VPS + Deploy to GitHub Pages) as each
cycle landed. Zero use of `Write` on an existing file (Edit only, per the
operator's standing fear about accidental large-file clobbering). Zero
worktree-exe network prompts (all live testing used a local loopback relay,
never production). 4 real bugs found and fixed (3 pre-existing, 1
introduced by this session's own earlier cycle and caught by a later
cycle's self-review). 656 lib tests pass (up from ~630 at the start of the
night), all gains proven via revert-and-retest, not just "test passes."

## What shipped, in order

**Priority #1 -- chat feature completeness (cycles 1-4, v0.641.0-v0.644.0):**
- **BUG-041** (v0.641.0): every group chat member saw themselves as group
  admin -- the client had no field to receive the server's real per-group
  `role`, so a hardcoded `true` stood in for it. Added `ChatGroup.role`,
  wired the `group_list` handler to parse it, extracted `is_group_admin`.
- (v0.642.0): the native DM-notification-preferences toggle was a
  client-side-only no-op; wired it to fetch/persist against the relay's
  real `notification_prefs` table.
- (v0.643.0): group channel voice-icon click was a no-op; wired it to the
  relay's `voice_room` join/leave protocol, and the relay's join handler
  gained a REAL group-membership check (`is_group_member`) where it
  previously granted access to anyone.
- (v0.644.0): onboarding's "Connect" button unconditionally claimed
  success regardless of whether the server was reachable; now does a real
  background-thread `/health` check. Also corrected a stale doc comment
  claiming ban/mute were unimplemented (they were already fully built).

**Priority #2 -- livestreaming verification (cycles 5, 11, v0.645.0,
v0.648.2):**
- **BUG-043** (v0.645.0): `viewer_peak` was fed the LIVE viewer count at
  leave/stop time -- a number that only ever decreases after a join, so by
  the time a stream ended the persisted peak was usually far below the
  real maximum (proved live: 2-viewer peak would have recorded 0). Fixed
  with an explicit `ActiveStream::peak_viewers` high-water mark.
- Stream start, stream chat (real-time + persisted), and stream end were
  all verified live and found correct, no fix needed.
- (v0.648.2, cycle 11): live-verified the WebRTC signaling relay
  pass-through (`stream_offer`/`answer`/`ice`) with 3 bot connections --
  confirmed correct unicast routing (no leakage to a bystander),
  server-authenticated `from` (not client-spoofable), no self-echo. What's
  left needs a real browser/str0m peer or the live production relay (the
  actual WebRTC media handshake, and the client-side scene-management UI)
  -- out of scope for the loopback harness, noted below for the operator.

**Priority #3 -- broader stub-completion sweep (cycles 6-10,
v0.645.1-v0.648.1):**
- (v0.645.1, docs-only): corrected 4 stale "NOT registered" claims in
  FEATURES.md (Weather/Atmosphere/Skills/Quests were all actually
  registered and ticking).
- **BUG-044** (v0.647.0, cycle 8): spoiled food was tracked (a per-slot
  timer correctly aged and flagged items) but never had any gameplay
  consequence when eaten -- a player could eat fully-spoiled food forever
  with full nutrition and zero risk. Fixed: spoiled food now grants only
  25% nutrition and always triggers food poisoning.
- (v0.646.0, cycle 7): `ecs::cosmos::body_position_in_system_meters` was a
  `DVec3::ZERO` stub; wired it to the real, already-shipped Kepler
  propagator in `src/cosmos.rs` for the `"sol"` system. No live caller
  exists yet (Phase 3/4 of the cosmos architecture aren't built), so this
  is progress banked for later, not a user-visible change today.
  Discovered in the process: `src/systems/navigation/orbital.rs`'s own
  Kepler stub is dead code, superseded by the same `cosmos.rs` module.
- (v0.648.0, cycle 9): the Cosmos/Maps page's "Track" button was a
  disabled stub; implemented continuous camera-follow of a body's live
  orbital position (vs the existing "Focus" one-shot snap-to). Also
  confirmed `src/systems/skills/learning.rs`'s practice-hours `Skill` is
  dead code (superseded by the real XP-based `SkillSystem`), and found
  `src/gui/pages/maps.rs` (591 lines) is ALSO fully dead code --
  `GuiPage::Maps` has forwarded to `cosmos::draw` since v0.203.2. Fixed the
  stale file-path pointers in FEATURES.md/PAGES.md.
- (v0.648.1, cycle 10, docs-only): re-audited the plan doc's own
  "needs a design decision" bucket (8 files) and found ALL of them are
  ALSO dead scaffolding with zero external callers -- not awaiting a
  decision at all. See "Left for the operator" below for the full list.

**Cycle 12 -- self-improvement pass (v0.649.0, v0.650.0):**
- A web/frontend TODO sweep turned up nothing actionable (the only hit was
  a Tauri-era dead-code TODO in `shell.js`, guarded behind a
  `window.__TAURI__` check that's never true since Tauri was deprecated).
- Dispatched an independent adversarial-review agent over the whole
  night's diff before stopping. **It found a real bug in this session's
  own BUG-044 fix**: the spoiled-food slot lookup used a forward search
  (first matching slot) while `Inventory::remove_item` actually consumes
  from the LAST matching slot backward -- so a fresh stack and a spoiled
  stack of the same item in different slots could silently defeat the
  whole fix (eating the spoiled one while the check inspected the fresh
  one, or vice versa). Fixed with a matching reverse search + a new
  multi-slot regression test, proven via revert-and-retest. The other 6
  reviewed areas (chat role, group voice membership, main-menu health
  check, viewer_peak, ecs::cosmos wiring, Cosmos Track toggle) were
  confirmed correct.
- Also fixed a stale v0.283.0 comment in `lib.rs` claiming native has no
  WebRTC stack -- it does, shipped in the v0.485-495 arc; found while
  cross-referencing STATUS.md during the cycle-11 verification.

## Bugs found (BUGS.md has full detail on each)

| # | What | Found in | Fixed in |
|---|------|----------|----------|
| BUG-041 | Every group member saw fake admin status | pre-existing | v0.641.0 |
| BUG-042 | Onboarding Connect always claimed success | pre-existing | v0.644.0 |
| BUG-043 | Livestream viewer_peak fed the wrong count | pre-existing | v0.645.0 |
| BUG-044 | Spoiled food had zero consequence | pre-existing | v0.647.0, follow-up fix v0.649.0 |

BUG-044 is the interesting one: it was found AND fixed this session, then
the FIX ITSELF had a bug, caught by a later self-review cycle in the same
night. Left as one BUGS.md entry with both the original root cause and
the follow-up documented together.

## Left for the operator (real decisions, not guessed at)

Two genuine product questions, logged in
`data/coordination/orchestrator_state.json`'s `open_questions_for_human`,
NOT force-built or force-deleted:

1. **`SkyRenderer` (`src/renderer/sky.rs`)** -- fully dead code (never
   instantiated). The mothership's real sun lighting already uses an
   astronomically-correct Earth-Sun vector. Does `SkyRenderer` still have
   an intended role (a ground/planet-surface exploration mode with a
   visible sky, distinct from the windowless mothership interior), or is
   it vestigial and safe to delete?
2. **Chat "Mute Server" button** (`chat.rs`) -- a TODO stub. There's no
   OS-level desktop notification system (no toast/sound) anywhere in the
   native client yet, and no per-channel/server unread tracking either
   (only per-DM). Wiring "mute" to a bare flag with nothing to consume it
   would be a hollow no-op. What should mute actually suppress once
   notification infrastructure exists, and should that infrastructure be
   its own feature first?

One low-risk cleanup opportunity, NOT executed (deleting across 6
subsystems in one unattended sweep felt like a bigger, more visible single
action than anything else done tonight -- deliberately left for a morning
decision rather than done unilaterally):

3. **11 files (~250 lines) of confirmed-dead scaffolding**, zero external
   callers each, safe to delete whenever convenient: `src/systems/ai/
   autonomy.rs`, `src/systems/construction/{blueprint,csg}.rs`,
   `src/systems/logistics/{mod,shipping,cargo}.rs`,
   `src/systems/navigation/{mod,galaxy,system,surface}.rs` (`orbital.rs`
   already covered above), `src/physics/{fluid,collision}.rs`,
   `src/systems/psychology.rs`, `src/input/{mod,bindings}.rs`. Also
   `src/gui/pages/maps.rs` (591 lines, separately documented in
   FEATURES.md/PAGES.md since those docs needed fixing regardless of
   whether the file itself is ever deleted) and
   `src/systems/skills/learning.rs`. None of these need a design decision
   -- they're just dead, unlike items 1-2 above.

One thing that needs a real browser/production relay to finish verifying,
not attempted tonight per the no-worktree-network-permission /
no-production-testing safety rules:

4. **The actual WebRTC media handshake for livestreaming** (SDP
   negotiation succeeding, ICE connectivity, audio/video frames flowing)
   and the client-side scene-management UI (`chat-voice-streaming.js`). The
   relay-side signaling routing is now proven correct (cycle 11); what's
   left needs a real 2-browser round-trip against the live relay.

## Process notes for the next unattended run

- The `bot_`/`bot_secret` fastpath + `scripts/ws-test-client.js` (built
  cycle 2) turned out to be reusable infrastructure for the rest of the
  night -- worth keeping for future protocol-level verification without
  needing the full Dilithium identify handshake.
- The "is this a real gap or dead/superseded code" question came up
  constantly and the discipline of grepping for real callers BEFORE
  building anything paid off repeatedly: 6 of the ~13 backlog items
  investigated turned out to be dead code, not real gaps (`orbital.rs`,
  `learning.rs`, `maps.rs`, and the entire 8-file "needs a design
  decision" bucket from cycle 10). Building any of those would have been
  pure wasted effort producing duplicate/unused code.
- Revert-and-retest caught real issues every time it was applied, but it
  has a blind spot: it only proves a test catches the SPECIFIC bug it was
  written against. It did NOT catch BUG-044's slot-index bug, because the
  original test only exercised a single-slot scenario. The adversarial
  review agent (cycle 12) is what actually caught that one -- a genuinely
  different set of eyes constructing a scenario the original author didn't
  think to test. Recommend making an end-of-window adversarial review a
  standing part of future long unattended runs, not just an ad-hoc idea.
- The journal (`orchestrator_state.json`) crossed the ~150 KB rotation
  threshold once (cycle 7) and was rotated cleanly via `just
  rotate-journal` -- worth checking journal size proactively in future
  long runs rather than waiting for it to become unwieldy.

## Verification discipline held all night

Every code change: both `cargo check` feature sets (native + relay),
`cargo test --features native --lib` (test count watched to only ever
increase), all 5 standalone lints, `check-doc-links.js` where docs
changed, and revert-and-retest for every new regression test. No shortcuts
taken to save time; the operator's "correctness is NOT sacrificed for
speed" standing instruction was followed throughout.
