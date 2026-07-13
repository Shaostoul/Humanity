# UI Surface Audit — punch list (2026-07-13)

Multi-agent sweep of every native (egui) + web UI surface (~109: 32 native pages,
24 native modals, 32 web pages, 21 web-chat modals). Verdict: **~92 production-grade**;
the real work is (1) rescuing built-but-unrouted surfaces, (2) fixing surfaces that
*look* done but are stubs/dead-routed, (3) collapsing duplicate profile/identity
editors before they drift, (4) small aesthetic cleanup in the security-critical
web-chat identity/seed modals. Dev/debug tooling (Files/Bugs/Testing/Dev/Ops/agents/
admin, F1-F4 overlays) is PERMANENT — orphaned dev pages get an in-app entry point,
never deletion.

Work top-to-bottom. Check items as they ship.

## 1. Mechanical quick wins
- [x] **Malformed CSS in web-chat identity/seed/backup modals** (`chat-profile.js`): `1var(--…)` and `var(--x)var(--y)` (no space) silently dropped by the browser → cramped security modals. Fixed 2026-07-13 (v0.844.1).
- [x] **Voice-modal View-profile wrong-arg bug** (`chat-voice-modal.js:255`): `requestViewProfile(key)` → `requestViewProfile(name, key)`. Fixed 2026-07-13.
- [ ] **Passphrase-import modal fully hardcoded hex** (`chat-ui.js:696` `showPassphraseModal`): only chat modal never tokenized; swap literals for `var(--bg-secondary/--border/--text-muted/--danger/--radius)`.

## 2. Delete confirmed dead code (matches 2026-07-05 orphan-deletion precedent)
- [ ] Native **Create Channel modal** (`chat.rs` `draw_create_channel_modal` + `show_create_channel_modal` + dispatch): `show_create_channel_modal` never set true since v0.187; channel creation lives in Server Settings. Safe delete.
- [x] Web **Key-Rotation modal** (`crypto.js` `openKeyRotationModal`/`doKeyRotation`): no caller + targets a relay route DELETED in Inc5b (v0.265) + replaces local key regardless of server ack (desync risk). **Deleted v0.845.2** — removed the whole cluster (openKeyRotationModal / doKeyRotation / _sendRotationToRelay / _storeRotatedIdentity, crypto.js tail) + the stale chat-profile.js doc block. Traced zero live callers first. In-app key replacement lives in the native Settings "Replace Identity" flow.

## 3. Wire surfaces that look done but are stubs
- [ ] Native **Identity page** (`identity.rs`): `identity_lookup_pending` never consumed; cards print literal `GET /api/v2/…` strings. Consume the flag + resolve the DID.
- [ ] Native **Recovery page** (`recovery.rs`): `recovery_lookup_pending`/`recovery_guardian_pending` never consumed. Call `GET /api/v2/recovery/setup/{did}`.
- [ ] Native **chat mute action** (`chat.rs:1769` `TODO: implement mute`): send the WS moderation message like the other mod actions.
- [x] Native **User-Profile "Watch Stream" button** (`chat.rs`): empty handler. **Removed v0.845.0** — the native roster carries no per-user stream URL and there's no native stream viewer, so it was a false affordance; the "Live" status dot already signals streaming. Re-add when an in-app viewer exists.
- [ ] Verify **BugReport submit** (`bugs.rs`): confirm reports persist to the relay, not just an in-session Vec.

## 4. Rescue orphaned, fully-built surfaces
- [ ] Native **Civilization page** (`civilization.rs`): live-relay community dashboard with NO route. Wire in as the Humanity tab's "Community" content (richer than the local 3-tile scoreboard).
- [ ] Native **Notes** + **Calendar** (`notes.rs`, `calendar.rs`): full working apps reachable only via quest links / boot-page. Give them a shared nav/section entry (a Tools/productivity cluster).
- [ ] Web **admin.html** + **agents.html**: wired dev dashboards, URL-only. Add drawer/ops.html entry points (dev tooling stays — GUI-first needs an in-app path).
- [ ] Web-chat **Seed-phrase reveal + Encrypted-backup**: reachable only from onboarding step-4; onboarding falsely promises "always available under Profile → Seed Phrase" (`chat-onboarding.js:415`). Add a chat identity/Security menu entry.

## 5. Collapse duplicate profile/identity editors (drift risk)
- [ ] Native standalone **Profile page** (`profile.rs`) vs the **Real "Profile" tab** (`real.rs`): diverge (standalone keeps a Streaming section). Repoint `server_settings.rs:238` "Open Profile" + onboarding `/profile` links at `GuiPage::Real`, retire standalone (keep `draw_section_content`).
- [ ] Web **Edit-Profile modal** (`chat-profile.js`) vs standalone **/profile page** (`profile.html`): sidebar link opens the page, `/profile` command opens the modal. Pick one canonical editor.
- [ ] Web **Restore-Identity-file modal** vs **Restore-from-Seed modal**: fold file-restore into the seed modal's encrypted-file tab; share the 24-word validation with Login Seed Recovery (`chat-ui.js:756`).
- [x] Native **User-Profile modal** (`chat.rs draw_user_modal`): the one chat modal hand-rolling its own `egui::Window` (no backdrop, no click-outside, ~14 hardcoded colors). **Ported to `widgets::dialog` v0.845.0** — themed backdrop + click-outside-to-close + title bar, avatar badge with hued ring, tokenized `widgets::Button` variants throughout (Send DM primary / Call + Follow columns / Moderation / Admin), a shared `send_mod_action` helper (was 6× copy-pasted JSON), and a snapshot test (`snapshot_user_profile_modal`). Zero hardcoded `Color32` literals remain. **(The modal the operator asked to improve — doubled as merge + polish.)**

## 6. Placeholder / parity gaps
- [ ] Web **crafting.html**: bare "Coming soon" box on a PRIMARY nav tab while native has a full Crafting page. Build a recipe browser mirroring native, or an honest desktop hand-off.
- [ ] Web **civilization.html Sim mode**: hardcoded fake colony stats (47 colonists, 78% morale…) — the fake-data pattern the operator deleted 2026-07-05. Wire to real save state or show an honest empty state.
- [ ] Web **resources.html Sim guides**: dead `#anchor` links; and `realResources`/`simResources` (~180 lines) belong in a `data/` JSON (Infinite-of-X).
- [ ] **Library/Accord naming**: web "Library" tab → `/resources` (links), but the built Accord viewer (`accord.html`) has no nav entry. Native "Library" shows Accord docs. Reconcile.

## 7. Aesthetic tokenization sweep (quick items first)
- [x] Native User-Profile modal → tokens (done with #5, v0.845.0).
- [ ] `passphrase_modal.rs` + `main_menu.rs draw_storage_chooser`: hardcoded `Color32`/font sizes → `bg_card()`/`success()`/`theme.font_size_*`.
- [ ] Planet info tooltip (`lib.rs:18699`): 13-arm hardcoded name→resources match → source from `data/solar_system/`; theme the frame.
- [ ] `calendar.html` / `chat-onboarding.js` / `showUserContextMenu` literal colors → tokens.
- [ ] View-Profile card + `market-app.js`/`trade-app.js`/`admin.html` heavy inline styles → CSS classes; `web.html DEFAULT_SITES` + resources literals → `data/` JSON.

## 8. Larger builds (tracked, not launch-blocking)
- [ ] Studio capture/encode/stream backend (must pump in the engine loop, not gated on the page).
- [ ] In-app browser webview (R&D, own lightweight browser per operator).
- [ ] Optional relay/vault sync for localStorage-only web pages (calendar, notes, bookmarks).
