# Hot Reload Tiers & Reload Protocol

## Purpose
Define what can be updated live vs what requires restart so update behavior is predictable and safe.

---

## Tier 1 — Live Hot Reload
Safe to apply immediately while app is running:
- markdown/docs content
- UI labels/translations/themes
- non-critical feature flags
- visualization presets/layout rules

Mechanism:
- fetch bundle -> verify -> apply -> publish in-app event (`content:updated`).

---

## Tier 2 — Module Reload
Requires module/session reload but not full app restart:
- plugin modules
- renderer adapters
- non-core integration connectors

Mechanism:
- pause module -> swap bundle -> re-init -> restore state snapshot.

---

## Tier 3 — Restart Required
Binary/runtime changes:
- core engine updates
- protocol internals
- security-critical updates

Mechanism:
- staged download + verify + "apply on restart" prompt/policy.

---

## Compatibility Rules
- Each bundle declares min/max runtime compatibility.
- Incompatible bundle is skipped and logged.
- Runtime never partially applies incompatible sets.

---

## Rollback Rules
- Keep previous applied bundle per tier.
- On failed apply, revert automatically.
- Surface rollback event in logs/status UI.

---

## Developer Workflow
1. Mark change tier in release metadata.
2. Publish signed bundle/package.
3. Runtime decides apply strategy by tier.
4. Observe success/failure telemetry.

---

## User Experience
- Tier 1: seamless updates, subtle "content updated" indicator.
- Tier 2: module reload toast, no app close.
- Tier 3: clear restart prompt with defer option.
