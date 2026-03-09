# Foundation Decisions v1 (Locked)

Status: **active baseline**
Updated: 2026-03-08
Owners: Shaostoul + assistant

This document is the authoritative lock for initial architecture and product policy.
If future discussions conflict with this file, this file wins until explicitly revised.

---

## A) Engine + Platform

1. Renderer/API strategy
- v1 target: **Vulkan-first**.
- Architecture should remain backend-extensible later.

2. Minimum performance target
- Stable **30+ FPS** on **GTX 1060** class hardware.

3. Base install size policy
- No hard cap yet in pre-production.
- Track growth continuously; decide hard cap when profiling baseline is stable.

4. Asset quality packaging
- Support hardware-tiered content profiles:
  - low,
  - mid,
  - high/latest.

5. OS support
- Primary development target now: **Windows 10**.
- Planned support: Linux (actively testable), macOS later when test hardware exists.

---

## B) Gameplay Slice + Simulation

6. First playable vertical slice
- **Home + Systems + Quests loop**.
- Rationale: establish homestead baseline before transit/public/asteroid loops.

7. Combat policy for v1
- **Strictly off** in first loop.
- Tone target: peaceful farming/homesteading progression.

8. Travel model baseline
- **Real-time travel** as long-term baseline.
- FTL is future technology, not v1 foundation.

9. Hosting model sequence
- Start: client-hosted (offline + P2P multiplayer).
- Expand later: dedicated authoritative servers.

10. Quest proof model (v1)
- Keep proof/rank validation **simple** at first.
- Start with completion + basic quality checks.
- Advanced certification proof systems come later after stability.

---

## C) UX + Navigation

11. Header order (locked for now)
- **H, Private, Public, Ops, Utility**.

12. Menu interaction model
- Keep **dropdowns** across devices for consistent interface behavior.

13. H Dashboard top priorities
- Character stats,
- Skills,
- Character inventory (excluding home inventory),
- Quests.

14. Systems page direction
- Prefer cleaner base structure:
  - tree + data emphasis,
  - avoid visually noisy graph-first layouts.

15. Docs/wiki strategy
- Keep markdown as source-of-truth.
- Use same docs on disk and in-game.
- Reuse markdown-derived snippets/tooltips/help to avoid duplication and save space.

---

## D) Data + Updates + AI Runtime

16. Source-of-truth distribution
- Project website/distribution endpoints are primary source-of-truth.
- GitHub and other hosts can be fallback mirrors.

17. Offline usability
- Maximize offline functionality.
- Online requirements should be minimized for resilience.

18. Hot reload
- Hot reload should be enabled as much as safely possible.
- Provide option to disable hot reload for save/server stability.

19. Analytics/telemetry default
- **Off by default**.
- Explicit user opt-in required.

20. AI runtime policy
- AI **off by default**.
- Support pluggable local and cloud AI options.

---

## E) Governance, Moderation, Monetization

21. Age/content approach
- Broad accessibility with content-tiered gating.
- Do not require risky third-party identity verification for normal use.

22. Moderation rollout
- Start manual-first moderation.
- Add assisted moderation later:
  - rate limits,
  - keyword/behavior/tag signals,
  - duplicate report clustering.

23. External launch fallback
- If embed route is restricted by provider policy/DRM/ToS, launch externally.
- Maintain legal/compliance-safe integration paths.

24. Data retention default
- Local-only by default.
- Cloud backup optional and opt-in.
- MMO/server mode should store only data necessary for operation.

25. Red lines (locked)
- No pay-to-win.
- No manipulation monetization.
- No mandatory ID verification for normal access.
- No default data extraction.
- No dependency lock-in for core usability.
- No deletion trap for paid/durable content to best ability.
- Investigate user-friendly refund handling where practical.

---

## Change Control

Any change to this document requires:
1. explicit human approval,
2. written rationale,
3. date + editor note.

This is intentionally strict to prevent drift across sessions and `/new` restarts.
