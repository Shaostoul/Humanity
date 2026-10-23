# CLAUDE.md passages retired on 2026-10-23

On 2026-10-23 `CLAUDE.md` was split by audience: what every agent needs stayed, and the rest moved
to `docs/ai/orchestrator.md`, `docs/ai/release.md`, `docs/ai/game-boots.md`,
`docs/ai/renderer.md`, `docs/ai/storage.md` and `docs/ai/code-patterns.md` with its wording
intact. The passages below were history or superseded rather than standing rules, so they are kept
here verbatim instead. The whole file as it stood before the split is
`git show 4f2d6903a:CLAUDE.md`.

## The docs layout note (START HERE, v0.422.x to 2026-10-23)

Superseded by `docs/README.md`, the router, which carries the folder map. Kept for its record of
what was deleted and corrected on 2026-06-30.

**Docs layout (audience-first as of v0.422.x):** `docs/README.md` is the router. Audience folders: `docs/user/` (using the app), `docs/admin/` (self-hosting + ops runbooks: SELF-HOSTING, release-signing, forgejo, torrent, mirrors), `docs/ai/` (AI participation + the numbered 05-AI-ONBOARDING; note as of 2026-06-30 the AGENTS/BOOTSTRAP/OPERATING_CONTRACT files were deleted, they were a generic OpenClaw personal-assistant template that never matched this project, not real coding-agent docs, use CLAUDE.md itself for that), `docs/contributor/` (the numbered 00-09 sequence minus 05, plus ENGINE_REFERENCE/development_loop/validate_data; 00/01/03/04 were corrected 2026-06-30 to the real single-crate reality, matching 02-ARCHITECTURE.md and 06-SOURCE-OF-TRUTH-MAP.md; 08/09 kept as aspirational content-planning docs with a note that "module" means a feature domain inside `src/`, not a Cargo crate). Topic folders unchanged: `design/`, `network/`, `game/`, `accord/`, `reference/`, `history/` (dated/superseded plans + vision essays archived here), `website/` (Jekyll). Operational gates stay at root (PRIORITIES, ROADMAP, STATUS, FEATURES, PAGES, BUGS, SOP, SYNC, INCIDENT-PLAYBOOK, BUS-FACTOR, SECURITY-CADENCE, HEALTH-DASHBOARD, index, knowledge_manifest; SECURITY_AUDIT.md is superseded and archived under `history/`). The AI onboarding moved to `docs/ai/onboarding.md` (was `docs/ai-onboarding.md`). Verify doc links with `node scripts/check-doc-links.js` (must stay at 0 broken).

## The pages deleted in the 2026-07-05 trim

From the "Forever development" norm, which still stands in CLAUDE.md without this list: "TRIM
genuine throwaway fluff, dead code, broken pages, and redundant tech-demo pages (e.g. deleted
2026-07-05: the audit fake-data page, the dashboard "Coming Soon" stub, the ai-usage orphan, the
old activities hub, a second half-built gardening game, and game.html which was just another
"download the app" page, the game IS the downloaded native app)."
