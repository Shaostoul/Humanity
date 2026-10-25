# In-App Ops: design

> **Status:** design + living backlog (2026-05-20). Codifies the "GUI-first configurability (no-CLI-required)" non-negotiable rule from CLAUDE.md and tracks the CLI debt to pay down.
>
> **2026-07-24: the registry EXISTS.** `data/admin/ops_registry.json` now holds
> the complete server-owner action surface (70 actions from a 5-agent
> code-verified sweep: 32 native-app, 1 web, 2 chat-command-only, 16
> config-file, 19 vps-shell), plus `planned` (the 12 ranked in-app gaps, which
> SUPERSEDE the table below as the canonical debt list) and `vps_only` (the 9
> actions that genuinely stay on the host shell, each with its reason). The
> native Server Settings page renders it as the "Admin map" section so a
> first-time owner always knows what is possible in-app vs what needs SSH; the
> web ops page mirrors it. Update the JSON when an admin surface changes - the
> pages render whatever is there. (This is the READ half of the north star;
> the invoke-through-one-endpoint half below is still the destination.)
>
> **2026-10-22: the invoke half EXISTS too.** `data/admin/actions.json` is the
> action registry (every admin action as data: id, label, description, role,
> inputs, danger level, how it runs), built into the relay, listed at
> `GET /api/admin/actions` and run at `POST /api/admin/actions/run` (signed
> with the caller's Dilithium3 key, never a bare token). The Alert channels
> editor (the first write panel) and the Federation panel run on it in both
> apps, beside an "Every admin action" list and the admin log. Section "The
> action registry (built 2026-10-22)" below is the reference; "The relay to
> system bridge (designed, not built)" holds fail2ban, relay control and
> secrets. The Admin map above stays: it is where EVERY owner task lives,
> shell-only ones included; the action registry is what can be RUN.

## The principle

Operator, verbatim (2026-05-20):

> "We want to make as many options/settings/configs accessible via the app. I'd prefer, for my sake and for the tech illiterate, that we avoid typing on the command line as much as possible. I don't mind if a button is effectively just running the console command. The easier it is for people to setup/use/modify the better. As an added benefit it is easier for you, Claude, to work the app since you'll be able to perfectly engineer always knowing what commands are available to you."

Three constituencies, one design:
1. **The operator**, prefers GUI; shouldn't need a terminal to run their own relay.
2. **Tech-illiterate users**, the accessibility mission (billions of people, most non-technical).
3. **AI agents**, a discoverable, structured in-app action surface lets an AI *enumerate* capabilities + invoke them correctly, instead of guessing shell commands. This is a force-multiplier for AI-assisted operation + development.

A button that just shells out to the equivalent command is acceptable. The requirement is **reachability from inside the app**, not reimplementation.

## North star: the admin action registry

Every admin/ops action is an entry in a data-driven registry (`infinite-of-x`: actions are data, not scattered hardcoded handlers). Each entry declares:

```
{ id, label, description, category, params:[...], danger_level, executor }
```

- The **GUI** (native Server Settings + web admin) renders the registry into panels/buttons/forms automatically, add an action, it appears, no per-action UI plumbing.
- An **AI agent** can fetch the registry (a read endpoint), see every action + its params + danger level, and invoke them through one authenticated "run action" endpoint. This is how an AI "perfectly knows what commands are available."
- **Danger levels** gate confirmation: `safe` (one click), `caution` (confirm), `destructive` (type-to-confirm, matches the operator's per-action-consent preference for prod-destructive ops).

This is the destination. We build toward it incrementally, each panel below is a step, and once 2-3 exist, factor out the registry so the rest fall out cheaply.

## Audit: what requires the CLI today (the debt)

Most of this is the TIER 0/1 ops work (built fast under incident pressure, all VPS-side shell/config). Each row: current CLI reality → in-app home.

| Capability | CLI today | In-app home | Notes / needs |
|---|---|---|---|
| **Alert channels** (ntfy/Discord/Telegram/webhook) | edit `data/alert-channels.secrets.json` over SSH | Server Settings → ADMIN → Alert channels | **SHIPPED 2026-10-22, both apps** (`src/gui/pages/server_settings/alerts.rs`, the admin page's panel in `web/pages/admin-console.js`): list, add, change, on or off, remove, and a test that says what the other end answered, as the registry's `alerts.*` actions (`src/relay/admin/alerts.rs`) on the same file the alert script reads. Secrets shown only by host and last characters. No email channel (the relay holds no mail settings; an email service's webhook works as a Webhook). |
| **Backups**: status, manual backup, restore | `humanity-backup-db`, `backup-relay-from-vps.ps1`, manual `cp` restore | Server Settings → Backups (+ ties into device-mesh) | **SHIPPED 2026-10-22 (continuous backup, `src/relay/backup_stream/`, `docs/admin/continuous-backup.md`), native AND web** through one endpoint, `POST /api/admin/backups`: the backups folder's list and Back up now (which also sends a fresh full copy to the destination), continuous backup's status (where, behind by, restorable from and to, size, history, key fingerprint), the **Restore drill** (restore into a scratch file, compare with the live database, never touching it), the destination editor (folder, S3-compatible bucket, another HumanityOS server; tested before saving; secrets never sent back), and whose backups this server keeps (allowlist with a size limit each). Still a shell step: putting a restored file in place (`HumanityOS --restore-backup` then a swap at restart); an in-app "restore on next restart" (type-to-confirm) is the next piece. |
| **Federation peers**: add/trust/defederate | raw `add_federated_server` / `set_server_trust_tier` storage calls | Server Settings → Federation | **SHIPPED on both apps, on the registry since 2026-10-22** (`src/gui/pages/server_settings/federation.rs`, the admin page's Federation panel; `federation.*` in `src/relay/admin/federation.rs`, which `POST /api/admin/federation` now hands its actions to as well): peers with trust, paused or linked and for how long, last heard from; add by address or by key (the 3,904-character key; both apps had still asked for 64 hex characters), trust, **pause and resume** (new: `storage/federation_pause.rs`, the peer and its trust kept while federation stops), re-check key, remove with confirmation, connect; which channels travel, as boxes; this server's own key with Copy. Spec: `docs/design/federation-activation.md` Phase 1. |
| **fail2ban**: view banned IPs, unban | `fail2ban-client status` / `set ... unbanip` over SSH | Server Settings → Security/Firewall | needs the relay to system bridge: **designed 2026-10-22, not built** (section "The relay to system bridge" below). Read-mostly + an unban button. |
| **Relay control**: restart, status | `systemctl restart humanity-relay` | Server Settings → System | the relay can't restart itself cleanly via its own process; **designed 2026-10-22, not built** (same section): a `relay.restart` action through the bridge. |
| **Health/system view**: uptime, disk, version, watchdog state, cert expiry | SSH + `df`/`certbot`/`journalctl` | Server Settings → System/Health | mostly READ. **SHIPPED**: web v0.287.0; native v0.720.0 (Server Settings → admin → "System health": status / deployed build / uptime / messages / peers via the public /health + /api/stats, worker-thread fetch + Refresh). Remaining depth: disk / cert / watchdog state need the small `/api/admin/system` signed-read endpoint. |
| **Secrets rotation**: API_SECRET, WEBHOOK_SECRET, VAPID | edit `.env` over SSH + restart | Server Settings → Security | sensitive; "rotate" buttons that generate + write + restart. Destructive-confirm. WEBHOOK_SECRET also needs the GitHub-side update (can't fully automate). **Designed 2026-10-22, not built** (the bridge section below): the new value is made and written by the root-owned helper and never passes through the relay, a log or a URL. |
| **Admin roster**: who's admin (ADMIN_KEYS) | edit `.env` + restart, or `set_role` | Server Settings → Roles (partly exists) | the roles editor exists; surface ADMIN_KEYS sync there. |
| **Play journal**: reading what the game recorded (2026-10-24) | the journal folder's files, or the dev MCP's `dev_play_journal` | Dev page > Journal | **SHIPPED 2026-10-24**: Dev > Journal (`src/gui/pages/dev_journal.rs`) reads it in the app (each kind, the marks with their pictures, Mark this moment, Open the folder, Clear); marking is F8 too. |
| **Disk-guard / retention knobs** | env vars on the systemd unit | Server Settings → System | thresholds (warn/crit %, keep-counts) as editable settings. |
| **Where a headless relay listens** (`PORT`, and `BIND_ADDRESS` since 2026-10-03: every interface by default, `127.0.0.1` for this machine only) | edit `.env` + restart | Servers page → Config | **The in-app node already has it:** Servers → Host a node on this PC → "Who can connect" ("Devices on my network" = `0.0.0.0`, "Only this computer" = `127.0.0.1`), saved with the node and restored by autostart. A headless relay (the VPS, a LAN box) still needs the `.env` edit, and the edit needs a restart, so this rides on the same sudo-gated restart bridge as Relay control. Listed in the in-app Admin map (`data/admin/ops_registry.json`, "Choose who can connect"). |
| **The call forwarder** (`TURN_PORT`, `TURN_BIND`, `TURN_PUBLIC_HOST`, 2026-10-09, `docs/admin/call-forwarder.md`), and opening its UDP port in the firewall | edit `.env` + restart; the firewall rule by `HUMANITY_OPEN_CALL_PORT=1` in `node.env` and a provision run | Server Settings > System | the settings could ride the same sudo-gated restart bridge as Relay control; the firewall rule is the operator's decision and stays a deliberate step. Status worth showing in-app now: whether the forwarder is listening (the relay knows) and how many allocations it holds. |
| **Deploy / version** | `git`/CI/`just` | Ops page (web has one) | mostly informational in-app; actual deploy stays CI (a "deploy" button would need careful auth). |
| **Homepage flavor** (per-node front door, v0.1141) | `ln -sfn /var/www/humanity/home/<flavor>.html /var/www/humanity-site/index.html` over SSH (a LINK, so it follows every update; a copy is frozen the day it is made: public.guide ran an August copy until 2026-10-12) | Server Settings → Website | a relay endpoint (admin-Dilithium-authed) that lists `web/home/` flavors + current state and writes/removes the override (a link for a stock flavor, a file for the node's own page); UI = flavor picker with preview links, and a "stock (follows updates)" or "your own page" label on the current state. The nginx seam is already data-shaped (presence of one file), so the endpoint is a thin file write. |
| **nginx log retention** (privacy sweep 2026-08-23: rotate 14 → 2 + history purge) | `sed` on `/etc/logrotate.d/nginx` over SSH | Server Settings → System | a retention-days setting the relay applies via the sudo-gated system bridge (same seam as fail2ban control). Until then any change is a documented SSH edit. |
| **The shared world's realism rules: Realistic carrying weight** (BUG-136, 2026-10-04) | none: there is no server-side rule at all | Server Settings → Game rules | Forgiving or Realistic carrying (`Settings > Gameplay > Carrying weight`, `AppConfig::carry_realistic`) is each player's OWN choice today, also in the shared world, so a player who picks Forgiving carries any load at full speed and the server has no say. Not a shell gap but a missing rule: a server setting (admin) that requires a mode in its world, sent to clients on join and shown on the Settings switch as "set by this server". Body heat (`body_heat_realistic`) and Illness (`illness_realistic`, BUG-162, 2026-10-05) are in the same position. The operator's MMO direction (2026-10-04: no cheated inventories in server worlds) is the reason it matters. |
| **The shared world's realism rules: Realistic death, and how long a pack stays** (2026-10-04, `docs/design/death-and-your-pack.md`) | edit `data/world/death.ron` and restart the game | Server Settings → Game rules (the mode); Settings → Gameplay (the time, for a single-player home) | Simplified or Realistic death (`Settings > Gameplay > Death`, `AppConfig::death_realistic`) is each player's own choice, also in the shared world, like carrying weight above. The pack's time of play, its warning, reach and the shore search are one data file read at startup, so changing the hour a pack stays needs a file edit and a restart. Needs: the mode as a server rule that requires it in its world, and the keep time as a setting (a server's for its world, the player's for their own home). A lootable pack on a server is a separate, open decision (the design note lists what it would need). |
| **Recording a clip** (movie mode, v0.1435.0) | `just clips`, or dropping `debug/record_request.json` | Studio page → Record | a Record button beside Go Live that records the live view frame-exact to a file (the engine side, `engine::movie`, already takes the request); shot presets from `scripts/clips.json` would need to move to `data/`. |
| **Backup encryption key custody** (`data/backup.key`, 2026-08-23; with continuous backup also `data/backup-destination.secrets.json`) | operator copies them off-box by hand | Server Settings → Backups | **Half done 2026-10-22:** the panel DISPLAYS the key's fingerprint and says which two files to keep off the machine (`HumanityOS --backup-drill` prints the fingerprint of the copy it uses, so a kept copy can be checked). Still missing: a one-click "download key copy" (admin-authed) with a keep-this-safe warning; no key, no restore. |
| **Alerts for continuous backup** (2026-10-22) | the relay sends "behind", "refused" and "caught up" through `scripts/humanity-alert.js` and `data/alert-channels.secrets.json`, the same channels the watchdog uses; the channels file is edited over SSH | Server Settings → ADMIN → Alert channels | **The channels are edited in-app since 2026-10-23** (the Alert channels panel, both apps, on the same file the script reads). Still open: the relay's own alerts still go out through `scripts/humanity-alert.js`, so a desktop-hosted node (no `scripts/` folder beside it, often no `node`) sends none. The relay now holds a sender of its own, `relay::admin::alerts::deliver` (the panel's test send uses it, every kind the script knows); continuous backup's "behind", "refused" and "caught up" can call it in place of the script. |
| **Removing a single backup copy** (2026-10-03: "Back up now" keeps the newest 10 by stamp; a copy stamped in the future by a clock that was once ahead pauses that pruning until real time passes it) | delete the file in `backups/` by hand | Server Settings → Backups | a per-row remove button on the Backups list (admin, confirm), so neither the retention doc nor the log has to send an admin to a shell. |
| **How fast a person may move aboard, and how far away someone is still in view** (increment 4 of `docs/design/ship-homes-and-logistics.md`, 2026-10-04) | edit `data/ship/shared_world.ron` and restart the relay | Server Settings → Game rules | The relay's speed check (`on_foot_mps`, the jitter margin, the banked seconds, the slack, how often a correction is sent, and since the review of increment 4 how often a declared jump may come, `jump_every_s` and `jump_burst`) and its delivery radii (`in_view_m`, `out_of_view_m`) are one data file, read by the relay at startup (the game's rig tooling reads it too). An admin who wants a bigger view for a small ship, or a tighter one for a crowded server, needs a shell today. Needs: the two radii as server settings (only the relay uses them), and the speed limits left in data (they must cover the fastest legitimate walk, which a test checks against the status effects and gear: an admin slider below it would correct honest players). |
| **A data folder's ship file older than the app** (the review of increment 4 of `docs/design/ship-homes-and-logistics.md`, P6, 2026-10-04) | move `<data folder>/blueprints/ship_structure.ron` aside by hand (the app then uses its own copy) | Settings → Data, and the refusal sentence itself | An installed game writes the ship file into its data folder on its first run and never refreshes it, and the updater swaps only the exe. After an update that changes the ship, every server running the new one refuses this game as "a different ship". The game now says so in one sentence naming the file, when the server's ship is the one built into the app. **Since 2026-10-12 (BUG-163 part 2) an update replaces a ship file nobody edited**, so the sentence is reached only when the file was edited (kept: Settings → Data → "Data folder" lists it with "Use the game's copy", which moves it aside, never deletes it) or the folder is one the game leaves alone. Still needs: the sentence (`engine/home_plot.rs` `other_ship_sentence`) saying so instead of "updating the app does not replace" it, and a button beside it that does what "Use the game's copy" does and reconnects. |
| **A data folder older than the app** (BUG-163, 2026-10-05; design: `docs/design/data-folder-updates.md`) | none since 2026-10-12 (before: read `logs/run.log` for `[built-in data copy]` lines, and rename the folder by hand so the next start writes a fresh one) | Settings → Data → "Data folder" | **BUILT 2026-10-12 (BUG-163 part 2, `src/gui/pages/settings/data_folder.rs`).** Every start brings each data file nobody edited up to the app's copy and keeps each edited one. The block shows the version the folder holds, what the last update brought up to date and could not write, each kept file with "Use the game's copy" (moves yours aside as `<name>.yours-<version>`, never deletes) and "Show the file", and each file this run did not use with why. Not offered in-app: putting a set-aside `.yours-` file back (rename it by hand), and a button to check for edits between updates (the kept list is worked out at each update). |
| **Ship plots and districts: the fields the panel does not show yet** (increment 1a of `docs/design/ship-homes-and-logistics.md`, 2026-10-03) | edit `data/blueprints/ship_structure.ron` by hand | Construction → Plots / Districts (Dev mode) | **Mostly in-app:** the Plots panel moves, resizes, adds and removes plots, picks the door's zone, its corridor and door size and the plot offline play uses, and shows why a plot would not load; Districts adds, moves, resizes and removes. Still file-only: a plot's `kind` (only one home design exists, `homestead`), `parent` and `neighbourhood` (both unused until block plots and neighbourhoods arrive), a district's label and `room_type`. Also missing, though not a CLI gap: districts no longer draw their build-mode wireframe, and the home's corridor mouth still draws a handle that can no longer be grabbed; both are drawn in `src/lib.rs`, outside 1a's file list. |
| **A space heater's thermostat** (BUG-155, 2026-10-05: the heater warms the air it stands in, `src/systems/farming/heat.rs`) | edit `heat_setpoint_c` on the `heater` in `data/machines/home.ron` (or `home_solo.ron`) and restart | the heater's walk-up card, and the Construction page's machine detail | Every heater holds 24 C, the catalog's game choice; a real one has a dial. Needs: a setpoint per placed heater, saved with the household's machine rows (a field beside `screen_source` on the instance), set from a slider on its card, read by `heat::heaters` in place of the catalog's, and an off position. |
| **How many building pieces the shared world holds** (ship homes increment 5, 2026-10-05, `docs/design/ship-homes-increment-5-plan.md`: at most 512 pieces in one frame, a plot or one of the ship's shared spaces; 512 kept by one builder; 4,096 on one ship) | change `MAX_PIECES_PER_FRAME`, `MAX_PIECES_PER_OWNER` and `MAX_PIECES_PER_WORLD` in `src/systems/construction/shared.rs` and rebuild both the relay and the game: they are compile-time constants, not even a data file | Server Settings → Game rules | The relay refuses a build past them (`frame_full`, `owner_full`, `world_full`), and the sentence the player then reads quotes the numbers from the same constants (`Reason::words` in that file), so a setting has to reach the game too: the welcome could carry the server's numbers beside its `ranks`. Needs: the three as server settings (admin), checked by the relay and sent in the welcome. How big they should be is an open question for the operator (`docs/PRIORITIES.md`, operator choices): twelve plots at 512 each (6,144) already exceed the ship's 4,096. |
| **Listing or taking down building pieces by who built them** (ship homes increment 5, 2026-10-05; not built) | stop the relay, run SQL on its database (`SELECT` or `DELETE ... FROM world_pieces WHERE owner_did = '<did:hum:...>'`), start it again: the relay reads that table only when it starts. The builder's `did:hum:` comes from their public key (`plot_owner_id`, `src/relay/storage/plots.rs`), and the app shows each person only their own (the Identity page) | Server Settings → ADMIN → Homes on the ship (native `src/gui/pages/game_admin.rs`, beside Release plot; on the web, the chat's Game Admin window) | In the app today: an admin or the owner takes down any ONE piece in the game (F, the `take_down_any` rank); releasing a plot takes down everything on it, whoever built it; erasing an account takes down that account's pieces everywhere. Missing: a game ban leaves the banned player's pieces standing, and nothing lists what one person built (on other plots under a household permit, or in the ship's shared spaces with the `can_edit_ship` rank). Needs: a list of pieces by builder (how many, in which frames), named by the public key an admin already uses for a ban or a plot release, and "Take down all of this person's pieces" (destructive, type to confirm), offered after a ban. Admin-only: no player is ever told who built what (`docs/design/shared-building.md`, status). |

## What's ALREADY in-app (the model to follow)

Server Settings (native `src/gui/pages/server_settings.rs` + web) already does a lot the right way: roles editor, channels grid, banned-users panel, muted-users panel, server-policy matrix, PQ/sharing toggles. These prove the pattern, admin-authed WS messages (`ServerSettingsUpdate`, `set_user_role`, `BannedListRequest`/`Unban`, etc.) + a rendered panel. New ops panels extend exactly this.

Also: the theme system is the gold standard, `theme_editor_coverage` test ENFORCES that 100% of theme tokens are editable in-app. The action registry should eventually get the same kind of coverage test ("every admin action is reachable in the GUI").

## Build order (recommendation)

1. **System/Health dashboard (read-only)**, lowest risk, fastest, immediately replaces a bunch of SSH. Live uptime/disk/version/watchdog/cert in Server Settings. Proves the read path + a small `/api/admin/system` endpoint. **Done** (web v0.287.0, native v0.720.0).
2. **Alert channels editor**, freshest debt; self-contained; first WRITE panel (relay reads/writes the watchdog's config file). Includes a "send test alert" button. **Done 2026-10-22, both apps, on the registry.**
3. **Backups panel**, status + "back up now" + restore. Ties into device-mesh Phase B. **SHIPPED 2026-10-22** with continuous backup (both apps, one endpoint, every action in `data/admin/ops_registry.json`); the attended in-app restore remains. Its endpoint is listed in the action registry by its route (`backups.*` entries); running its actions through `POST /api/admin/actions/run`, and so into the admin log, is still to do.
4. **Federation panel**, per `federation-activation.md` Phase 1. **Done 2026-10-22, both apps, on the registry, with pause and the channels.**
5. **fail2ban + relay-control + secrets**, these need the sudo-gated relay→system bridge; do them together once that bridge exists. **Designed 2026-10-22, not built** (below).
6. **Factor out the action registry**, once 3-4 panels exist, extract the common shape so remaining actions are data entries, and add an AI-facing "list actions" + "run action" endpoint + a coverage test. **Done 2026-10-22** (below).

Each step is its own increment; each pays down real CLI debt and demonstrably advances the no-terminal-required goal.

## Relay Control Center (operator ask 2026-07-13, "all three as tabs")

The operator wants **one main-menu button for everything about the one-or-more relays he owns, from his one PC**:

> "Maybe we should create a dedicated relay or server button on the main menu for everything I could ever want to do to the one or more relays I own from my one PC?"

This ELEVATES the ops surface: instead of ops being buried inside `Server Settings` (which is scoped to the currently-connected server), promote it to a top-level `GuiPage::Relays` page that treats each relay the operator administers as a first-class, switchable entity. He chose the full build: **three tabs**.

### Shape

```
Main menu → "Relays"
┌─────────────┬──────────────────────────────────────────┐
│ MY RELAYS   │  [ Health ] [ Control ] [ Config ]        │
│ ● united-…  │                                            │
│ ○ localhost │  (tab content for the selected relay)      │
│ + Add relay │                                            │
└─────────────┴──────────────────────────────────────────┘
```

- **Left rail — my relays.** Lists saved servers where the operator's role is admin/owner, each with a live health dot (green up / red down / grey unknown), the display name, and version. `+ Add relay` registers another (URL + it inherits the operator's identity for auth). Selecting one focuses the tabs on it. Single-relay operators just see one row — the multi-relay model costs nothing when there's one.
- **Tab 1 — Health (read).** Reuse the EXISTING System-health fetch (the Server Settings → admin "System health" path, native v0.720.0: status / deployed build / uptime / messages / peers via public `/health` + `/api/stats`, worker-thread fetch + Refresh). Add disk / cert-expiry / watchdog-state once the signed `/api/admin/system` read endpoint exists (tracked above).
- **Tab 2 — Control (act).** Restart relay (via the existing `service_control`/`service_state` WS path + admin-Dilithium auth), tail recent logs, and surface + embed the WATCHDOG (the VPS `humanity-relay-watchdog` timer): show its up/suspect/healing/down state and last check. Where an endpoint does not yet exist (logs, watchdog read), the button shows the honest CLI fallback (`just logs`, `systemctl status humanity-relay-watchdog`) rather than pretending — and that gap is logged here, never silently accepted (the GUI-first rule).
- **Tab 3 — Config (edit).** The GUI-first config surface: server name, roles, channels, announcements, policy/limits — all of which ALREADY exist as admin-authed editors in `server_settings.rs`. Tab 3 renders/hosts those same editors for the selected relay (reuse, do not reimplement), so "Config" is the one place to change how a relay behaves without a terminal.

### Increment plan

1. **Page shell + main-menu entry + left rail + tabs + all three tabs live.** **SHIPPED v0.846.0** (`src/gui/pages/relay_control.rs`, `GuiPage::RelayControl`, "Relays" nav entry, `paint_server` icon). Left rail lists saved servers + the connected relay with health dots; a new signed `/api/admin/stats` fetch (native's first — `net::identity::pq_sign_chat` over "admin_stats") drives a rich Health tab (version, uptime, users, messages, DB/upload size, disk %, watchdog state, backup age). Control tab shows the watchdog chip + honest CLI-fallback cards for restart/logs (no endpoint yet) + a Services hand-off. Config tab deep-links the existing `server_settings.rs` editors. Snapshots: `relay_control_health`, `relay_control_actions`.
2. **Tab 2 (Control) depth**: build the missing `/api/admin/system`-style restart + log-tail endpoints (sudo-gated relay→system bridge) so restart/logs work in-app instead of the CLI fallback.
3. **Tab 3 (Config) depth**: host the `server_settings.rs` editors inline (currently a deep-link jump) so config for a *non-connected* relay is editable in place.
4. **Multi-relay depth**: persist the relay registry (today `chat_servers` is ephemeral + role-less), per-relay admin auth, public-health fallback for relays where the operator isn't admin, switch-and-act across several relays.

This is the natural convergence point for the CLI-debt table above: every row's "in-app home" becomes a panel under this page's Config or Control tab, and once several exist the action-registry factoring falls out.

### Hosting a node from inside the app (SHIPPED)

Running a server at all was the largest remaining CLI-only step, and it was not
in the debt table above because it sits before every row in it: `HumanityOS
--headless`, or a systemd unit on a rented box. Someone who just wants their
family on their own server had to open a terminal before any of the panels
above applied to them.

`src/gui/pages/host_node.rs` closes that, rendered as a section of the Relays
page. There is only one binary and the `native` feature already includes
`relay`, so the desktop app links the whole server and starting one is just
`crate::relay::run_relay()` on its own thread with its own tokio runtime,
configured through the same `PORT` / `DATABASE_PATH` / `SERVER_NAME` /
`BIND_ADDRESS` env knobs the systemd unit reads. Stop resolves a oneshot that wins a `select!` against
the serve future, which drops the listener and frees the port.

What the surface shows: running or not, the port, who can connect ("Devices
on my network" or "Only this computer", the relay's `BIND_ADDRESS`), the
database file, the loopback and LAN addresses to hand to a friend, uptime, and
a Start / Stop pair. State is honest rather than optimistic: the port is
test-bound (a network node on the wildcard address AND loopback, because
Windows lets a 0.0.0.0 bind succeed past a 127.0.0.1 squatter; a node for this
computer only on loopback alone, so it never raises the Windows firewall
prompt) and the database folder created before the thread exists,
the thread body is wrapped in `catch_unwind`, and the node is only reported as
Running once `GET /health` actually answers.

Two deliberate limits, stated in the UI rather than hidden: it serves only while
HumanityOS is open (a machine that should serve unattended still wants the
headless binary plus a service), and reaching it from outside the local network
still needs a router port forward, which no app can do for the user.

Remaining depth: the local node has no in-app "restart", it is stop then start;
and it is not registered in `data/admin/ops_registry.json` yet, so the Admin map
does not list it.

## The action registry (built 2026-10-22)

**The data.** `data/admin/actions.json`: categories, roles, danger levels,
reserved prefixes, the signing recipe with a worked example, and one entry
per action:

```
{ id, label, description, category, role, danger, confirm_word?, inputs: [...],
  run?: "here", chat?: "/command {input} ...", route?: "POST /api/..." }
```

`role` is `admin` or `mod` (the claim alone is `member`). `danger` is `read`
(changes nothing, not logged), `safe`, `caution` (the apps ask once) or
`destructive` (the apps ask for the `confirm_word` typed, and the run
endpoint refuses without it, so an AI must send it on purpose). An input is
`text`, `secret` (drawn as a password box), `number`, `bool` or `choice`,
with `required`, `hint`, `choices`, `max_len`, `min` and `max`. How it runs:
`"run": "here"` through the one endpoint; `chat` is the chat command that
does the same (the only way, for an action not yet moved); `route` a signed
endpoint of its own.

**The relay** (`src/relay/admin/mod.rs`) builds the file in with
`include_str!`, so what a server runs is what its build says and an edit on
its disk changes nothing. `GET /api/admin/actions` serves it to anyone (the
same data as the open-source file, nothing secret, and an AI can learn a
server's surface before it holds a key), with each action's `destructive`
spelled out. `POST /api/admin/actions/run` takes `{key, timestamp, sig,
action, inputs, confirm?}`, `inputs` being the JSON TEXT the signature's
hash covers; the purpose signed is `admin_action\n<id>\n<BLAKE3 of the
inputs text>` (`run_purpose`, the same words in `web/shared/admin-actions.js`
`adminRunPurpose`, both held to the file's example). The checks, in order:
the action exists and runs here; the signature (fresh 5 minutes, not from
the future, used once); the role (`role_refusal`); the confirm word
(`confirm_refusal`); the inputs against the entry (`check_inputs`: nothing it
does not take, required present, kinds and limits, no control characters);
then the runner. `RUNNERS` pairs each `"here"` id with its function.

**The admin log** (`storage/admin_log.rs`, table `admin_log`): every change
through the registry, and through `POST /api/admin/federation`, with when,
the action, a one-line summary and whether it worked. The admin is a keyed
fingerprint (the erased-accounts secret, domain `hum/admin-log/v1`), never
the key; the log is shown with the names of the people who hold a role now,
and a row whose admin is gone reads "a former admin". A summary never holds
a token, an address or the member an action was about. Newest 2,000 rows
kept. Read through the `log.recent` action.

**The apps.** Native: `src/gui/pages/server_settings/` (`admin_api.rs` the
signed requests on worker threads, `alerts.rs`, `federation.rs`,
`all_actions.rs` with the log), state in `gui/state_types/admin_console.rs`.
Web: `web/shared/admin-actions.js` (the rules, tested in Node) and
`web/pages/admin-console.js` on `/admin`. "Every admin action" is drawn from
the server's own list in both: an entry added to the file appears with no
screen change. A `chat` action is sent as a chat command by the desktop app
and copied for the person to paste by the website (the admin page has no
chat of its own).

**Runs here today (24):** `alerts.list/add/update/set_enabled/remove/test`,
`federation.list/add/add_key/trust/pause/resume/recheck/remove/connect/channel`,
`log.recent`, and `members.kick/mute/unmute/ban/unban/mod/unmod` (the very
function the chat commands run, `handle_mod_command`, so its own checks hold).
**Listed with their chat command:** every other admin chat command
(channels, categories, lockdown, invite, the claim, verify and donor badges,
names, wipes, reports, pins). **Listed with their route:** `/api/admin/stats`,
`/api/data-files`.

**Coverage, in three tests.** (1) `src/relay/admin/tests.rs`
`every_action_that_runs_here_has_a_runner_and_every_runner_an_action`: the
runner table and the file agree both ways. (2) `tests/admin_action_coverage.rs`
(in `just lints` and CI): every `/api/admin/` route and every chat command the
relay answers is in the registry, or is named there as a member command with
its reason; and every `chat` and `route` an entry names exists. (3)
`the_registry_holds_together`: ids, categories, roles, danger, confirm words,
inputs and templates are consistent.

**The Backups panel** (continuous backup, built alongside, 2026-10-22). It
shipped with an endpoint of its own, `POST /api/admin/backups`
(`src/relay/backup_stream/api.rs`), signed the same way over
`admin_backups\n<action>\n<SHA-256 of the params>`, with the actions
`status`, `back_up_now`, `restore_drill`, `test_destination`,
`set_destination`, `host_allow` and `host_remove`. The registry lists each as
a `backups.*` entry with `"route": "POST /api/admin/backups"` (the coverage
lint holds the route to them), so "Every admin action" and an AI see them;
the panel itself is its own (`server_settings/continuous_backup.rs`,
`web/pages/backups-admin.js`). Moving them to run through
`POST /api/admin/actions/run` is one step: each becomes `"run": "here"` with
a line in `RUNNERS` calling the function the endpoint calls now (signature
`fn(Arc<RelayState>, Caller, Inputs) -> Result<Ran, Refusal>`), the panels
switch to the run endpoint, and the admin log, the role check and the inputs
check come with it. Until then its changes are not in the admin log. The
WebSocket `backup_list_request` / `backup_now` are named in the next step
below.

**What is left.**
- The coverage lint does not yet read the admin messages on the signed-in
  socket (`server_settings_update`, `channel_update`, `role_upsert`,
  `set_user_role`, `banned_list_request` and `unban`, `muted_list_request`,
  `game_ban` and `game_unban`, `game_release_plot`, `service_control`,
  `backup_now`, `admin_erase`, report decisions, the fleet ledger and the
  world clock, the Files page's admin messages). Each wants an entry (moved to
  `"run": "here"` or listed with a `ws` way to run), then the lint reads the
  socket dispatch in `relay.rs` the way it reads the chat arms.
- The rest of the chat-only actions move to `"run": "here"` one at a time,
  each needing its body lifted out of `relay.rs`'s socket loop into a function
  both can call (the moderation ones already were).
- The website could send a chat action through `/shared/relay-session.js`
  instead of copying it.

## The relay to system bridge (designed 2026-10-22, not built)

fail2ban, restarting the relay and rotating its secrets need root on the
host; the relay runs as the unprivileged `humanity` user. Not built, because
it can only be finished by installing a sudoers rule and a root-owned
program on the live servers, which is the operator's call and was out of
bounds for this increment. The design, so it is built the same way when it
is:

**One helper, an exact allow-list.** A root-owned program
`/usr/local/sbin/humanity-ops` (in the repo as `scripts/humanity-ops`,
installed `root:root 0755`, never writable by `humanity`), and one sudoers
drop-in naming it with each exact argument list, the pattern
`scripts/sudoers.d/humanity-relay-services` already uses for four
`systemctl` lines:

```
humanity ALL=(root) NOPASSWD: /usr/local/sbin/humanity-ops fail2ban-status
humanity ALL=(root) NOPASSWD: /usr/local/sbin/humanity-ops fail2ban-unban *
humanity ALL=(root) NOPASSWD: /usr/local/sbin/humanity-ops relay-restart
humanity ALL=(root) NOPASSWD: /usr/local/sbin/humanity-ops rotate-secret API_SECRET
humanity ALL=(root) NOPASSWD: /usr/local/sbin/humanity-ops rotate-secret VAPID
```

- `fail2ban-status`: runs `fail2ban-client status` and `status <jail>` for
  the jails it finds, prints JSON (jail, banned addresses, counts).
- `fail2ban-unban <address>`: the helper itself checks the address is one
  IPv4 or IPv6 address (no ranges, no options, nothing starting with `-`)
  before `fail2ban-client unban <address>`; the wildcard in sudoers is only
  safe because of that check, and the helper's test proves it refuses
  `-x`, `1.2.3.4;id`, `1.2.3.0/24` and an empty string.
- `relay-restart`: `systemctl restart humanity-relay`, started detached
  (`systemd-run --on-active=2`) so the relay answers the request before it
  is stopped.
- `rotate-secret <NAME>`, for an allow-listed name only: makes the new value
  from `/dev/urandom`, writes `.env` through a temporary file and a rename,
  keeps the previous file as `.env.prev` (mode 0600), restarts the relay.
  The value never leaves the helper: not in its output, not in the relay's
  memory, not in a log or an address. `WEBHOOK_SECRET` is left out: it must
  also change on GitHub, which an app cannot do, so its panel says so.

**The relay side** follows `src/relay/services.rs`: `sudo -n` (fails at
once, never hangs on a prompt), `std::process::Command` with an argument
vector and constant strings (no shell), the subcommand chosen from a
compile-time table by exact match, the address re-checked in Rust before it
is passed. Each becomes a registry action (`security.fail2ban_list` read,
`security.fail2ban_unban` caution, `relay.restart` caution, `security.rotate_secret`
destructive with a confirm word), so it is admin-only, signed, logged
(address unbans log "unbanned one address", never which), and listed for an
AI like the rest. The relay's log records the action and whether it worked,
never the output.

**What it takes to finish:** the helper and its tests, the sudoers lines,
the Rust table and four runners, an install step in the deploy pipeline
reviewed by the operator, and one attended run on a test server before the
live ones. Until then the Admin map keeps these as SSH tasks.

## Guardrails

- **Auth**: every admin action is Dilithium-authed (the relay's `verify_dilithium_signature` path), admin/owner role required. Never expose an unauthenticated ops endpoint.
- **Danger gating**: destructive actions (restore, wipe, secret rotation, defederate) require type-to-confirm in the GUI, matches the operator's standing rule of explicit per-action consent for prod-destructive ops.
- **The CLI stays too**: GUI-first does NOT mean GUI-only. The shell scripts remain (for recovery when the relay is down, for the BUS-FACTOR successor, for automation). The GUI is the primary path; the CLI is the fallback. Both stay in sync via shared scripts where possible (e.g., a "back up now" button shells out to the same `humanity-backup-db` the timer uses).
