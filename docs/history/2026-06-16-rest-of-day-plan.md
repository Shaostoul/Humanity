# Rest-of-day plan (2026-06-16)

Produced from an 8-angle survey workflow (construction, home-sim, web/UX, bugs/debt, gameplay,
security/ops, novel-ideas, mission-strategy) plus a synthesis pass, then the two load-bearing
findings were verified by hand. This is a decision-ready menu for the operator + the safe solo work
already in motion.

## Headline
Run the multiplayer co-presence scoping (the operator's #1 stated goal, read-mostly, zero-risk) and
flag the operator-only release-signing blocker; in parallel ship a stack of safe solo wins (signing
health-check, lint/doc cleanups, the pure-Rust construction de-risking spike).

## P0 (verified) -- desktop auto-update is silently dead
Every release from v0.421.0 through v0.469.0 carries only `data-manifest-*.json` + the platform
binaries, and NO `release-manifest.json.sig.json`. Verified by `gh release view v0.469.0/v0.468.0/
v0.466.0 --json assets`. The v0.421+ desktop updater runs in enforce mode and only offers SIGNED
releases, so it has offered NOTHING for ~48 releases. **Fix is one operator-only command:**
`export HUMANITY_SIGNING_PASSPHRASE=... && just sign-release v0.469.0` (needs the passphrase +
`release-signing-key.enc`; an AI/CI cannot sign). Signing the latest release carries all prior
changes to desktop users. This is the single biggest launch blocker.

## The #1-goal gap (verified) -- multiplayer co-presence has never been wired on the client
The relay's authoritative game world is LIVE (`handle_game_join` at
`src/relay/handlers/msg_handlers.rs:3276`, with `game_join`-first guards throughout). The native
client's WebSocket handler is CHAT-ONLY: it correctly filters `__game__:` / `__sync_data__` out of
the chat display (`src/lib.rs:4264`) but there is NO path that routes game traffic INTO the world.
`NetSyncSystem` + `NetClient` exist (`src/net/sync.rs`, `src/net/protocol.rs` with
`Join`/`PlayerJoined`/`PositionUpdate`) but are referenced ZERO times in `lib.rs`. There is also
protocol drift: client `net::protocol::NetMessage` ({type:Join}) vs the relay's envelope
({type:game_join}). Net: no two humans have ever co-occupied the world, and STATUS.md's multiplayer
checkmarks are self-flagged unverified pre-v0.132. See `docs/design/first-playable.md` for the
file:line gap list.

## Recommended for the rest of today (ranked)
1. **Sign v0.469.0** (operator-only, S, the launch blocker). Also diagnose whether `just sign-release`
   was erroring or simply never invoked.
2. **Multiplayer co-presence scoping** (solo, M, read-mostly): the gap doc + a PRIORITIES TIER entry.
   Converts the #1 goal from a wish into a scoped backlog. (See first-playable.md.)
3. **Construction structural-solver de-risk spike B1** (solo, L, pure Rust, relay-safe): node-beam +
   flood-fill island detection in a new `src/systems/construction/solver.rs`, tested in `#[cfg(test)]`.
   The design doc names this the #1 thing to prototype FIRST; retires the biggest construction-arc risk.
4. **Wire the dead web a11y/i18n/glossary modules** (solo, M): they are loaded by ZERO of 38 web pages
   yet the landing publicly promises high-contrast/colorblind/reduced-motion + 5 languages + glossary.
   Visible-but-inert toggles are worse than none. Load from `shell.js`; wire `settings-app.js`.
5. **Battery state-of-charge (live-home-sim 1b)** (solo, M, pure ECS): turns the static "2.8 days
   autonomy" into a live draining number. Greenfield (no Battery component exists).
6. **Signing health-check + free lint delist** (solo, S): `scripts/check-release-signing.js` wired into
   `just status` so the P0 can never recur silently; delist `alert.rs` from theme_token_lint (0 real
   violations).
7. **Construction editor papercuts** (solo, S): lock Tab/I/E/R out of editor mode (input leaks into the
   reveal-peek/inventory while orbiting); per-room height DragValue; web nav active-tab fixes.

## Full inventory by theme
### Launch readiness / security-ops (highest real-world stakes)
- SIGN v0.469.0 (operator). Diagnose why signing lapsed. Add `check-release-signing.js` gate (solo).
- GitHub branch+tag protection on `main` (deploy auto-pushes to the live relay with no approval gate).
- Overdue monthly cargo-audit (never run, ~4wk overdue). Quarterly independent code review due (~200
  releases since the v0.266 PQ review; bounded diff v0.421..HEAD).
- Backup-restore drill never run + Litestream activation (VPS). Off-box "whole-VPS-down" monitor (solo).
- Secrets-rotation log + BUS-FACTOR "if I disappeared" checklist both empty (operator sit-down).

### Multiplayer / First Playable (the stated near-term goal)
- Co-presence scoping -> `docs/design/first-playable.md` (done this session). Then wire the smallest
  position-broadcast slice as a follow-on if the gap is small.
- Reconcile client `NetMessage` with the relay `game_*` envelope. Instantiate `NetSyncSystem`/`NetClient`.

### Construction editor arc (operator's active hands-on flow)
- De-risk spikes B1 (structural solver) + B2 (framing generator on an off-axis wall) -- both pure Rust.
- Multistory v0.470.0: A1 `level:i32` on RoomConfig+ConstructionRoom + `story_height`; **A2 (HARD
  prereq) make `find_shared_edges` (fibonacci.rs:819) Y/level-aware** or stacking phantom-cuts doors
  into floors; A3 level-selector UI; A4 numeric level stepper. DEFER floor slabs/stairs, SVG cutouts,
  curved aperture, click-to-place, data-driven keybindings.

### Live home simulation (single-player depth, pure-ECS solo wins)
- 1b Battery SoC (top solo pick). 1c Home-page summary reads live PowerStatus. 1e register
  PlumbingSystem (watch the 12 m MAX_DRAW_DIST vs the spread layout). 1d per-machine live stats.
- Arc2 vitals HUD bars (solo) + death/respawn (operator difficulty fork). Arc4 enclosed-space
  atmosphere couples to construction boundaries -- do AFTER editor multistory.

### Web/UX parity + accessibility
- Wire dead a11y/i18n/glossary (top web pick). Expand i18n past ~37 strings/language. Fix nav
  active-tab mismatches. Add the 3 inert mobile-drawer group CSS rules. `crafting.html` is a stub
  behind a promoted nav tab. Studio nav vs nginx redirect disagree.

### Bugs / tech-debt / doc hygiene
- theme_token_lint: delist alert.rs; migrate row.rs + lib.rs blues to accent(). STATUS.md header stale
  (v0.416.0). PAGES.md stale vs v0.469.x. Reconcile signing "ACTIVATED" docs vs the lapsed practice.
- Baseline is healthy: git clean, relay build green, 0 broken doc links, all 4 lints pass.

## Novel ideas (not currently planned)
1. **FederationBundle** -- one signed-object bundle/verify primitive that travels over ANY medium (USB,
   QR, LoRa, sneakernet). The architecture's biggest latent superpower; 4 other ideas snap onto it.
2. **Skill Passport** -- portable, offline-verifiable, post-quantum proof-of-competence (W3C-VC but
   federated + PQ). Direct anti-poverty infrastructure for the unbanked/undocumented.
3. **Build-It-For-Real** -- BOM -> local sourcing -> group-buy button on every survival system. Poverty
   is often a minimum-order-quantity problem; turn "here's how" into "4 neighbors split the spool."
4. **Offline Survival Codex** -- the chemistry/biome/ET/NPK/DLI corpus bundled + printable + with
   built-in calculators, working on a borrowed laptop with zero internet.
5. **Schema-driven in-app "Make-a-Thing" editor** for ALL 23 infinite-of-X schemas -- a teacher in
   Kenya adds local crops with no code. The keystone hedge against founder-dependence.
6. **Time-Bank** -- log/exchange hours-of-help as the on-ramp economy before any money exists.
7. **Generation-Ship co-op pilot** -- the shared life-support habitat as the first multiplayer scenario
   (selfishness literally kills the colony; the "altruism is an engineering requirement" thesis, played).
8. **AI Mentor "Sage"** -- a first-class NPC citizen with its own identity + a scripted-fallback decision
   tree (ships now with zero LLM dependency; the reference implementation of "how an AI participates").
9. **Disaster Mode** -- aim the disaster/weather/hydrology/atmosphere systems at REAL preparedness
   (location-personalized prep plan). A non-exploitative growth hook, news-relevant every storm season.
10. **Open sensor protocol** -- ingest a $5 ESP32's pH/EC/temp into the live home-sim. The moment the
    game becomes the control panel for your real homestead.
11. **Mission Pulse** -- a federated, privacy-preserving, aggregate-only real-world impact dashboard
    (each server publishes signed counts, no per-user data leaves home). The honest scoreboard for the
    grant pitch.
12. **Continuity Capsule** -- self-sovereign succession via Shamir-split seed across trusted contacts.
    Generalizes BUS-FACTOR to every citizen.

## Operator-gated decisions (your calls when you're back)
- **Sign releases** (THE blocker). Diagnose why signing lapsed.
- GitHub branch+tag protection on main; Litestream + backup-restore drill; secrets-rotation +
  BUS-FACTOR checklist.
- FORK: F/M/Tab camera -- implement third-person + orbit toggle to match `keymaps.ron:17-19`, or delete
  the stale lines?
- FORK: death/respawn severity + creative-mode default (currently ON, suppresses survival).
- FORK: is the home pressurized (rotating-ship canon) or Earth-ambient? Gates the atmosphere arc.
- FORK: web landing -- sidebar-hub (mirror native Humanity 4-section) or clean marketing scroll?
- FORK: Studio canonical web target (/chat live-streaming vs /download vs a future studio page).
- FORK: multistory next slice depth -- ship A1+A2+A3+A4-numeric as v0.470.0, or also the 3D
  vertical-drag-to-restack gesture?

## Watchouts
- RELAY-BUILD GATE: every Rust change must pass `cargo check --features relay --no-default-features`
  before push. Native-gate any new GUI/render/persistence module. (B1/B2 are feature-agnostic by design.)
- NEVER `cargo fmt`. NEW THEME TOKEN requires wiring into settings.rs same-change (prefer reusing
  accent()). Windows LNK1318: use `cargo test --features native --lib`. Commit with `-F <file>`, never -m.
- MULTISTORY landmine: `find_shared_edges` is pure-XZ -- A2 (Y-aware) is a HARD prereq.
- Untracked files fail fresh CI -- `git status --short` and stage new .rs/.ron before push.
- Scope creep: pick ONE slice. Don't start in-world machine placement (it IS the operator's live editor).
- Don't over-trust the surveys: the two load-bearing claims were verified; STATUS.md checkmarks are
  self-flagged unverified pre-v0.132 -- the co-presence work must MEASURE, not assume.
