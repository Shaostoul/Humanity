# HumanityOS Roadmap: April 2026

Snapshot after v0.91.2 landing-page rewrite and v0.91.3 onboarding work. Captures the next wave of planned work so it does not get lost between sessions.

## Context

The landing page is now plain-English and benefit-led. First-time visitors can answer "what is this?" in ten seconds. The next user-experience gap is: once they click in, the chat (the foundation of the platform) has visible regressions and the rest of the pages need a coherent long-term plan.

This doc covers five tracks:

1. Chat rework (highest priority, user-reported bugs)
2. Onboarding and quest system (now live at `/onboarding`, plus future expansion)
3. Page strategy (consolidation, additions, rethinks)
4. Universal widget system (started with the help modal, extend)
5. Quest-chain framework beyond onboarding (gamified learning for every feature)

Priority order at the end.

---

## 1. Chat rework (native desktop app, `src/gui/`)

Seven issues from field testing on v0.91.1. All appear to be in the egui rendering code of the native client. The web chat at `web/chat/` may or may not share the same regressions and needs a separate pass.

### P0 blockers

**DM decryption failure.** Every message in a DM thread renders as `[encrypted - decryption failed]`. The user was already mid-conversation with a trusted peer when this started, suggesting a key or envelope format mismatch, not a missing key. Investigation path:
- Check the DM crypto path in `src/relay/core/` (shared keygen, ECDH derivation).
- Confirm the native client is using the same ECDH + AES-256-GCM envelope as the web client.
- Check whether recent refactors changed the ciphertext framing (version byte, nonce position, associated data).
- Compare a captured ciphertext against what the web client produces for the same plaintext.
- Likely root cause candidates: key-rotation certificate not being honoured by the native client; nonce derivation diverged; envelope header bytes shifted.

**Settings cogs inert.** The gear icons next to server headers (e.g. `united-humanity.us`), groups, and channels do nothing on click. Either the handler was not wired up after the chat refactor, or the hit-test region is wrong. Fix: grep for the cog-icon rendering in `src/gui/pages/chat*.rs` and confirm the click closure is installed; confirm it dispatches to the correct context menu builder.

**Text selection broken.** Users cannot click-drag to select message text for copy. Egui's default `Label` widget is non-selectable; messages must render through `egui::Label::new(...).selectable(true)` or use `ui.code` / `ui.text_edit_multiline` in read-only mode.

### P1 UX: message layout spec

Current: userbox, timestamp, and message text render in a broken arrangement that does not wrap around the userbox.

Correct behaviour (from the user's spec):

- The **userbox** (avatar + name, rectangular) anchors to the **top-left** of a message group.
- **Message text wraps** around the userbox. Top-left of the text anchors to the **top-right of the userbox**; subsequent lines **indent** to clear the userbox until the text overflows past its bottom, then flows back to the left margin.
- **Timestamp is an inline prefix** on every message line: `12:34 UTC · Hello there`. The separator is the **interpunct** character `·` (U+00B7).
- Every new message from the same sender still gets its own `timestamp · text` prefix, even if it visually sits to the right of the same userbox (same sender sending several short replies in a row).
- If the userbox is taller (e.g. the name wraps), the text to the right of it gets correspondingly more lines before it needs to flow underneath.

Implementation: probably a custom egui layout that allocates the userbox region first, then lays out the text with a per-line indent that depends on whether the current y-cursor is above the userbox bottom.

### P1 UX: click-zone isolation

Currently, clicking the empty space to the right of a userbox (but within the userbox row) still opens the profile modal. It should only fire when the user clicks the userbox itself.

Fix: the click response should be installed on the userbox's exact rect, not the parent row. Likely a case of calling `ui.interact(row_rect, ...)` instead of `ui.interact(userbox_rect, ...)`.

### P2 enhancements

**Right-click context menu with role-colored sections.** On right-click over a message, show a context menu with three visually distinct sections:

- **Red** — user actions (copy text, reply, quote, view profile, DM)
- **Green** — moderator actions (delete message, mute user, pin, timeout) — only visible if caller has mod role on the current channel
- **Blue** — admin actions (ban, remove from server, view IP if logged, audit trail) — only visible if caller is server admin

Egui supports context menus via `response.context_menu(|ui| { ... })`. The section colors become `ui.colored_label` or a tinted background on each sub-group. Role gating keys off the existing role system in `src/relay/storage/roles.rs`.

**Inline image embeds.** Upload URLs (`/uploads/123_image.jpg`) currently render as plain text. They should:
- Detect image extensions (`.jpg`, `.png`, `.webp`, `.gif`).
- Render as a clickable thumbnail inline with the text.
- Click expands to full-size inline (not a modal), keeping the text flow around it.
- The same wrap-around rule as userboxes applies: surrounding text flows around the image, indenting to clear it.

**Inline widget framework.** The goal beyond images: arbitrary widgets can appear inline with chat text and the text flows around them. Likely needs:
- A widget registry (`WidgetKind::Image | ::Vote | ::Poll | ::Scheduler | ::Embed | ...`).
- A chat-layout primitive that treats widgets as "floats" (like CSS `float: left`) and flows text around their bounding box.
- Per-widget render functions that egui can call during the message layout pass.

---

## 2. Onboarding system

Shipped in v0.91.3:

- `/onboarding` page — data-driven quest chains, four core concepts, core-pages overview, CTAs to chat and accord.
- `data/onboarding/quests.json` — three quest chains (Welcome, Explore, Going Deeper), fifteen total steps. Progress tracked in `localStorage` under `humanity_quest_progress`.
- Universal help modal (`window.hosHelp`) — registerable, reusable across pages.
- Help icon next to the Real/Sim toggle — first built-in help topic.

### Next onboarding improvements

- **Link from landing page** — add a "Start here" hook in the "Where do you fit?" card so first-time visitors land on `/onboarding` before the chat. (Done in v0.91.3.)
- **Auto-suggest on first visit** — existing `onboarding-tour.js` is a step-through overlay. Decide: keep the overlay tour (short, contextual) AND the onboarding page (long, reference), OR collapse one into the other. Recommendation: keep both; the overlay for first-session nudges, the page for permanent reference.
- **Native-app parity** — the same onboarding page should render inside the native desktop app. Options: (a) embed the web page in a webview inside native; (b) render the quest-chain data in egui directly. Option (b) is more consistent with the rest of the native UI. Either way, the data file is the source of truth.
- **Wire up quest completion to real actions** — right now, ticking a step only toggles a localStorage flag. Later, "Set up your Profile" could auto-complete when the user saves a profile, "Send your first message" when they hit send, etc. Requires a small event bus (`hos.emit('quest:complete', 'welcome:profile')`) wired into each target page.

---

## 3. Page strategy

Today's nav exposes around 20 pages: Chat, Wallet, Donate, Profile, Civilization, Tasks, Inventory, Maps, Market, Projects, Settings, Download, Ops, Bugs, Dev, plus page-only destinations like Home, Dashboard, Calendar, Notes, Data, Web Browser, Roadmap. That is too many for a first-time visitor to parse.

### Most-used (protect, polish)

Ranked by expected usage:

1. **Chat** — the foundation. Everything else is secondary to talking to people.
2. **Profile** — identity discovery. People click names to learn who they are talking to.
3. **Wallet** — money attention is strong.
4. **Tasks** — daily coordination.
5. **Market** — economic activity.

These five deserve frictionless polish first. Any regression here is felt immediately by every active user.

### Missing pages to add

- **In-app help / docs** — today the only help is external GitHub links. A `/help` page that indexes the Humanity Accord, glossary, onboarding, common how-tos, keyboard shortcuts, would be high-leverage. Could reuse the glossary overlay as a component.
- **Events feed** — "what is happening right now on the network": recent messages across public channels, new listings, trending tasks, upcoming calendar events. A dashboard-style feed, separate from the private dashboard.
- **Global search** — one search bar that finds messages, users, listings, tasks across all accessible scopes.
- **Community / server directory** — finding federated servers is currently invisible. A page that lists known servers with their descriptions, member counts, rules snapshot, and a join button would help new users find their tribe.
- **Notifications inbox** — already partially implemented in-chat but deserves its own page for reviewing missed activity across all contexts.

### Consolidate / rethink

- **Home ≈ Dashboard** — overlapping concepts. Pick one (recommend: `Dashboard` is clearer and already implemented). Delete or redirect `Home`.
- **Ops + Dev + Bugs** — three developer-facing tabs in the blue group. Merge into a single `Dev` tab with sub-routes. Keep the blue group small.
- **Web Browser + Data** — these feel like utilities, not pillars. Move under a `Tools` page, alongside Calculator, Calendar, Notes.
- **Civilization** — vague name. Either rename to make the purpose obvious, or merge into Maps / Dashboard depending on current content.
- **Projects overlaps Tasks** — a "project" is already a filterable attribute on tasks. Projects as a top-level tab may be redundant. Audit: if Projects is doing something Tasks cannot, keep it; otherwise fold in.
- **Inventory only matters in Sim mode** — hide non-Sim-relevant pages when Real is active, and vice versa. The nav already has color groups; add a context-gated class.

### Final recommended nav

- **Identity (red):** Chat, Wallet, Donate
- **Life (green, context-sensitive):** Profile, Tasks, Market, Maps, Calendar, Notes, Inventory (Sim only)
- **System (blue):** Settings, Download, Dev (merged Ops/Dev/Bugs), Help

Ten top-level tabs, plus the brand and the Real/Sim toggle with help icon. Fits on a single row at desktop width and collapses cleanly on mobile.

---

## 4. Universal widget system

Precedent set in v0.91.3 with the help modal:

- Registered in `shell.js` as `window.hosHelp.register(id, title, content)`.
- Displayed with `window.hosHelp.show(id)` or any `[data-help-id="..."]` button.
- Styled with theme CSS variables so settings-page theming applies automatically.

### Extend this pattern to

- **Confirmation dialogs** — `window.hosConfirm(title, body, onConfirm)` replacing ad-hoc `confirm()` calls. Same styling, same dismissal pattern.
- **Toast notifications** — transient feedback ("Task saved", "Copied to clipboard") with a single `window.hosToast(msg, {type: 'success'|'error'|'info'})` call.
- **Contextual menus** — dropdown context menus that use the same theme vars and z-index stack as help.
- **Inline tooltips** — hover-triggered text bubbles for terms and icons (partially covered by the existing tooltip logic at the bottom of `shell.js`).

Each new universal widget should:
- Live in `shell.js` (or a split `web/shared/widgets.js` once shell.js gets too big).
- Use theme CSS variables only, no hard-coded colors.
- Expose a `window.hosX(...)` API with a clear, minimal surface.
- Degrade gracefully if called before the shell finishes initialising.
- Be documented in this file or in `docs/design/widgets.md` so pages know what is available.

### Settings-page integration

The end goal is a Settings page section ("Appearance") that can override these widget styles globally: accent color, card radius, border style, motion preference. Because every widget already reads from `--accent`, `--bg-card`, `--radius` etc., the settings page only needs to override those CSS custom properties on `:root` to restyle every widget.

---

## 5. Quest-chain framework beyond onboarding

Today's quest data file is focused on onboarding. The same format works for any feature area.

Proposed extensions:

- **`data/quests/tasks.json`** — "Master the kanban board" (create, assign, label, comment, close).
- **`data/quests/marketplace.json`** — "Your first trade" (create a listing, get a review, message a buyer).
- **`data/quests/wallet.json`** — "Take custody" (back up seed, send first payment, receive payment, stake).
- **`data/quests/sim/farming.json`** — "Raise your first crop" (plant, water, fertilize, harvest).
- **`data/quests/sim/construction.json`** — "Build your first room" (place foundation, walls, door, furnish).
- **`data/quests/contributor.json`** — "Become a contributor" (read onboarding, open an issue, submit first PR).

Each quest chain would show:
- On the relevant feature page as an optional sidebar ("Quests for this page").
- On the onboarding page as the master list.
- In a future `/quests` index page aggregating everything.

Progression hooks: when a player completes a quest chain, it could unlock a badge on their profile, a small amount of sim currency, or just a satisfying confetti. Mechanically cheap, emotionally effective.

Integrates with the sim/game system cleanly: a "quest" in sim mode is the same data structure as a "tutorial" in real mode.

---

## Priority order

1. **Chat P0 fixes** (DM decryption, settings cogs, text selection) — blocks daily use, start next.
2. **Chat P1 layout + click-zone** — high visibility, moderate scope, single native rebuild.
3. **Page consolidation** (merge Ops/Dev/Bugs, pick Home or Dashboard, rename Civilization if needed) — web-only, fast, reduces first-visit overwhelm.
4. **Link onboarding from landing + native** — small edit to landing, then figure out native embed.
5. **Help system expansion** — register topics for Real/Sim (done), plus Wallet, Identity, Federation, Sim mode, Humanity Accord.
6. **Chat P2 enhancements** (right-click menu, image embeds, inline widget framework) — longer project, build after P0/P1 ship.
7. **Confirmation + toast widgets** — quick wins, replace ad-hoc `confirm()` and `alert()` across the codebase.
8. **Feature quest chains** (tasks, marketplace, wallet, sim) — incremental, one JSON at a time.
9. **Missing pages** (Help index, events feed, global search, community directory, notifications inbox) — prioritize Help first, others in rough order of user friction.

Things explicitly NOT on this list right now:
- Bigger 3D / game-world work. Still scaffolded, not the top blocker.
- Native mobile app. Web-on-phone is the stop-gap.
- Map rework. Parked behind the higher-priority chat and nav items.
