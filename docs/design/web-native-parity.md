# Web ⇄ Native Parity — rebuild plan

> **Status:** active (2026-05-20). Operator directive: the native app is the canonical/parent design; the web chat is the old/broken UI and must be rebuilt to mirror it. This is the systematic divergence map + migration order the rebuild executes against. **Incremental, component-by-component — the web chat stays usable throughout** (no big-bang rewrite).

## Approach

- **Native (egui) leads; web (HTML/CSS) mirrors.** Match native's structure + UX + layout. Web MAY render richer visuals where egui can't (emoji, gradients, video) but must NOT *diverge* in structure or behavior. egui's constraints define the shape; web fills it in faithfully.
- **Theme is already shared** — `web/shared/theme.css` is generated from `data/gui/theme.ron` (same tokens native reads). So colors/spacing/radii/fonts are already aligned; the divergence is **layout + component structure + which elements exist**, not palette.
- **Keep it working.** Migrate one component at a time behind the existing markup; never leave the web chat broken between increments.

## Divergence map (web current → native target)

| Area | Native (target) | Web (current) | Action |
|---|---|---|---|
| **Left rail nav** | Stacked **collapsible sections** all visible at once: scratchpad row → DMs → Groups → Servers→channels (`draw_left_panel`, each section has a collapse caret) | **Tabbed** — Servers / Groups / DMs tabs, only one visible at a time (`#sidebar-tabs`) | **#1 priority.** Replace tabs with stacked collapsible sections matching native's order + collapse behavior. Biggest feel change. |
| **Studio placement** | (none in chat yet — to be added per studio-streaming.md) | Embedded in the LEFT sidebar (metrics bar + pills + voice channels) | Remove from left; relocate to the RIGHT rail top per studio-streaming.md (that's Track S, but the *removal* from left happens here). |
| **Right rail** | Friends (collapsible) + Members (collapsible) (`draw_right_panel`) | "People & Streams": Friends / Groups / United-Humanity | Align section set + naming + collapse behavior to native. Streaming viewer widgets are Track S. |
| **Channel header** | `# general | General discussion` with the lock/cog affordances native uses | Similar but older styling | Match native's header layout + affordances. |
| **Message rows** | Native bubble/row style, timestamp pill (Þ), reactions inline in the pill, reply/quote, context menu | Older message styling (`messages.css` 16K) | Match row layout, timestamp-pill, inline reactions. Largest CSS surface. |
| **Composer** | Input at bottom, search/pin/help/Send affordances | Similar | Align affordance set + styling. |
| **Top nav** | Text-labeled page buttons (Profile/Identity/…/Settings) in tiers | Icon-only top bar | Decide: match native's labeled tiered nav (the `shell.js` nav already exists app-wide — align the chat page's to it). |

## Migration order

1. **Left rail: tabs → stacked collapsible sections. ✅ ALREADY DONE (verified 2026-05-20).** The static `index.html` still has the tab markup, but `chat-ui.js::initUnifiedLeftSidebar` (called at runtime via `setTimeout(..., 0)`) rewrites it into stacked collapsible `.unified-section`s in native's order (DMs/Groups/Servers) with header counts + carets, and the CSS forces all panes visible in unified mode (`.sidebar-unified-left .sidebar-tab-content { display:block !important }`). The web left rail already matches native's structure. **Lesson: the static HTML misled the audit; the runtime DOM is the truth — always verify the rendered state, don't rebuild from the markup.**
   - **Studio relocation (1b) — DONE v0.287.4.** The studio panel (`#stream-studio-panel`) was still in the left rail; native puts streaming on the right. `relocateStudioToRightRail()` (chat-ui.js) now moves it to the TOP of `#right-sidebar` at runtime (above the people list), preserving every control's id/handler. The full right-rail studio *widget redesign* (compact widget + modal + viewers) is Track S; this step just gets it onto the correct side.
2. **Right rail align to native. ✅ DONE (v0.287.5 + v0.287.6).** Dropped the "People & Streams" overarching title (native has none). `renderUnifiedRightSidebar` now builds Friends + a full server-member list (count matches native; was de-duping → "(1)" vs native "(3)"); removed the right-rail Groups section (native keeps Groups left-rail only).
3. **Message rows + timestamp pill + inline reactions** — the biggest CSS surface; do after the rails so the frame is right first.
4. **Channel header + composer affordances.**
5. **Top nav alignment** (chat page → the shared `shell.js` nav).
6. **Sweep**: spacing/scale audit against native; remove dead CSS from the old design.

Each step is its own increment + its own version bump; the web chat is usable after every step.

## Guardrails

- **Don't regress working features.** The web chat has real, mature functionality (DMs, groups, voice/WebRTC, search, pins, reactions, profiles). Parity is about matching native's LOOK + STRUCTURE, not deleting capability. Anything web has that native lacks (voice, streaming) stays — native catches up separately.
- **Theme tokens only.** No hardcoded colors in the rebuilt CSS — use the `theme.css` vars (the web equivalent of native's theme-token rule).
- **Verify visually.** UI parity is judged by the operator on a real build (screenshots), like the prior server-settings styling work — ship incrementally so each step can be eyeballed.

## Relationship to Track S (studio/streaming)

The left-rail studio REMOVAL happens here (parity); the right-rail studio widget + viewers + persistent stream + privacy guard are Track S (`studio-streaming.md`). They interlock at the rails, so do parity step 1 (left rail) + step 2 (right rail) before Track S's S1 (right-rail studio widget) lands in the cleaned-up frame.
