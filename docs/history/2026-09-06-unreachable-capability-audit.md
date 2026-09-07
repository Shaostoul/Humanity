# Built but unreachable: capability audit, 2026-09-06

Read-only sweep of five layers (v1 REST API, v2 REST API, WebSocket protocol, native egui
pages, storage and configuration) hunting for functionality that is finished server-side and
has no path to it from any user interface. Every finding was then handed to a second agent
whose only instruction was to find the caller the first one missed, since that is the likely
failure mode. Only survivors are listed.

**28 confirmed.** By value and wiring cost: 1 high/small, 3 high/medium, 10 medium/small,
9 medium/medium, 1 low/small, 4 low/medium. By layer: v1 API 8, storage and config 6,
native GUI 5, WebSocket 5, v2 API 4.

**Two findings are unadjudicated,** not listed below: their verifier calls were rejected by a
model safeguard and never ran. They were "issue a vouch or skill endorsement" (v2 API) and
"redeem a friend code in the web client" (WebSocket). The first is almost certainly real, since
the storage layer sweep found the same capability independently and it was confirmed.

**The pattern worth naming:** the relay is substantially further along than either client.
Much of what reads as unbuilt is built and simply has no button.

Ordered by value, then by how cheap the wiring is.

---

## HIGH value

### 1. Keeping the personal profile you type in - body and clothing measurements, pronouns, location, website, private notes, interests, social links

**Wiring cost:** small | **Layer:** native-gui

**Already built.** The editors are all built and rendered: src/gui/pages/profile.rs:113-140 (height, weight, eye colour, blood type, hair x4, neck, shoulders, chest, waist, hips, thighs, inseam, shoe/shirt/pants size), :159-161 (pronouns, location, website), :188 (private notes), :291 (interests Add), :487 (social links Add). The relay accepts most of these: RelayMessage::ProfileUpdate at src/relay/relay.rs:1013-1033 takes pronouns, location, website, socials and banner_url, and ProfileData at :1040-1058 sends them back. AppConfig already persists Vec-shaped GuiState (src/config.rs:368 saved_servers, :774 donate_addresses).

**No path to it.** grep -c "profile_height|profile_pronouns|profile_interests|profile_social_links|profile_private_notes" src/config.rs returns 0 - no profile text field is written to config.json. A per-field scan of GuiState showed profile_height/weight/eye_color/blood_type/hair_*/neck/shoulders/chest/waist/hips/thighs/inseam/shoe_size/shirt_size/pants_size/pronouns/location/website/private_notes each appear exactly ONCE outside src/gui/mod.rs, at their profile.rs text box. The one Save path, src/gui/pages/profile.rs:257-265, sends only bio + avatar_url + privacy and hardcodes "socials": "{}" (line 259, same at src/gui/pages/privacy.rs:115 and :291) - so saving from native also wipes socials set from web. Inbound, src/lib.rs:16469-16480 reads only name/bio/avatar_url from profile_data and drops pronouns/location/website/socials/banner; it also writes the server bio into state.profile_bio (the Identity section) while the Save button ships state.profile_network_bio, so the round trip does not close. web/chat/chat-profile.js:96-110 sends the full set.

**What wiring means.** Add the profile text fields to AppConfig (src/config.rs from_gui_state / apply_to) so they survive restart, add pronouns/location/website/socials to the json! payload at src/gui/pages/profile.rs:257, and extend the profile_data arm at src/lib.rs:16469 to read the fields the relay already sends.

### 2. A server member directory: a paginated, searchable list of everyone on the server, plus a per-person card showing their name, role, join date, bio, avatar, location, how many marketplace listings they have, and their average seller rating. It even has a working privacy opt-out that both clients already expose as a checkbox.

**Wiring cost:** medium | **Layer:** api-v1

**Already built.** src/relay/api.rs:1475 get_members (paginated + search + total count), :1495 get_member_by_key (full card: listing_count, avg_rating, review_count, bio, avatar_url, location), :1526 get_member_count. Routed at src/relay/mod.rs:985-987. Storage in src/relay/storage/members.rs:179/228/265, with the directory opt-out as shared SQL at members.rs:26 and a passing test suite at members.rs:349-392 proving an unlisted member is hidden from the list, the single lookup, the count and the search. src/relay/features.rs:209 explicitly names /api/members* as part of "the floor every node provides".

**No path to it.** grep -rna "api/members" over all of web/ and src/ returns exactly three hits, and all three are COMMENTS, not calls: web/chat/chat-profile.js:107 and web/pages/profile.html:1224 ("the member-directory opt-out ... hides you from /api/members") and src/gui/pages/profile.rs:215 (same sentence). grep for "members" combined with fetch in web/pages/*.js, web/pages/*.html and web/chat/*.js returns only /api/guilds/{id}/members and /api/v2/groups/{id}/members, which are different endpoints. grep "/members" in src/gui/ returns only guilds.rs. No client anywhere calls /api/members, /api/members/count or /api/members/{key}. The result is that both clients ship a privacy toggle governing visibility in a directory that no user can open.

**What wiring means.** One new page (or a tab on the existing Civilization page) that calls GET /api/members?limit=&offset=&search= and renders the rows, with a click through to GET /api/members/{key} for the person card. The native mirror is the same fetch in a new src/gui/pages/members.rs following the pattern already used by src/gui/pages/guilds.rs. Nothing on the server needs to change, and the privacy opt-out already works.

### 3. Photos on marketplace listings. Sellers can attach up to five images to a listing, ordered, with the owner-or-moderator checks already written.

**Wiring cost:** medium | **Layer:** api-v1

**Already built.** src/relay/api.rs:1794 add_listing_image (resolves the caller's key from an upload token or key param, verifies they own the listing or are admin/mod, then stores), :1831 get_listing_images, :1845 delete_listing_image (same ownership check). Routed at src/relay/mod.rs:996-997. Storage in src/relay/storage/marketplace.rs:228-330: add_listing_image enforces the max of 5, get_listing_images orders by position, delete_listing_image, reorder_listing_images, and update_listing_images_json which keeps the listing row's images field in sync so the WebSocket listing broadcast already carries them.

**No path to it.** grep -rna for "listings/.*images", "/images'", "/images\"" and "/images`" across web/ and src/ (excluding src/relay/) returns nothing at all. grep "images" in web/pages/market-app.js returns nothing; grep "images" in src/gui/pages/market.rs returns nothing. Neither client adds, deletes, reorders, or even DISPLAYS a listing image, despite the images field riding along in every listing_new / listing_updated frame they already parse. The marketplace is therefore text-only in practice, and reorder_listing_images has no route at all.

**What wiring means.** In the listing create/edit flow, after the existing POST /api/upload succeeds, POST the returned URL to /api/listings/{id}/images (this is exactly what the handler's own doc comment describes as the intended flow). Then render the images array in the listing card and detail modal in web/pages/market-app.js and src/gui/pages/market.rs, with a delete control for the owner. A route for reorder_listing_images would need adding if drag-to-reorder is wanted.

### 4. Issue a credential about another person (a vouch, a verified-human attestation, a skill endorsement, a graduation, an employment record), revoke one you issued, withdraw one issued about you, or dispute a bad issuer. The relay recognizes eight credential schemas, indexes them, applies revocations and withdrawals in place, scores issuers up and down from disputes, and feeds all of it into the trust score. No client can create a single one of these objects - both clients ship a read-only credential viewer for credentials that can never exist.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** src/relay/storage/credentials.rs:109 index_credential, :150 revoke_credential, :168 withdraw_credential. The schema allowlist is src/relay/storage/signed_objects.rs:279-281 (vouch_v1, verified_human_v1, skill_endorsement_v1, graduation_v1, employment_v1, role_v1, member_v1, account_age_v1), with :289 applying revocation_v1 -> revoke_credential and :301 applying withdrawal_v1 -> withdraw_credential, and :271 routing dispute_v1 into src/relay/storage/issuer_trust.rs:182 index_dispute -> :114 issuer_trust_bad. Read surface exists and is wired: src/relay/api_v2_credentials.rs behind /api/v2/credentials (src/relay/mod.rs:1063-1064), consumed by web/shared/pq-identity.js:77-81 and src/gui/pages/identity.rs. Trust weighting: src/relay/storage/trust_score.rs:22 W_VCS = 0.30, the largest single weight.

**No path to it.** grep -rl over web/, src/gui/, src/net/ for each schema name returns NONE for verified_human_v1, skill_endorsement_v1, graduation_v1, employment_v1, role_v1, account_age_v1, revocation_v1, withdrawal_v1 and dispute_v1; vouch_v1 hits only web/shared/pq-identity.js:154, which is a doc comment describing the shape, not a builder. grep -rn 'postObject|buildAndSign' over web/pages and web/chat shows the only signed objects any client actually posts are proposals and votes (web/pages/governance.html:510, :682) and P2P group objects (web/chat/chat-groups-p2p.js), plus market provider_v1/offering_v1 and the read-only recovery pages. Consequence in the code: grep -rn 'issuer_trust_good' across all of src returns only its definition at src/relay/storage/issuer_trust.rs:62 and its own unit tests, so issuer trust never rises above the NEUTRAL_TRUST 0.5 constant in production, and the 30%-weighted VC sub-score is computed from an always-empty index.

**What wiring means.** Add per-schema payload builders next to the existing voteV1Payload/modActionV1Payload in web/shared/pq-object.js (and Rust twins beside src/net/api_v2.rs submit_signed_object), then an "issue a credential about this person" action on the profile/identity pages and a "revoke" button on the credentials you issued, both posting to the /api/v2/objects endpoint that already exists. The signing, canonical CBOR encoding, indexing, revocation application and issuer scoring are all already built and tested; what is missing is a form. This is also the fix that makes the trust score at GET /api/v2/trust/{did} return something other than near-zero.

---

## MEDIUM value

### 1. Propose any of the nine kinds of decision the governance system actually knows how to judge ,  schema additions, treasury grants, juror-pool lotteries, Accord interpretations, federation floor policy, civilization schema registration ,  each with its own authored quorum and pass threshold.

**Wiring cost:** small | **Layer:** api-v2

**Already built.** data/governance/proposal_types.ron authors nine types (ids at :17 parameter_change, :25 schema_add, :33 treasury_grant, :41 juror_pool_lottery, :49 local_rule, :60 accord_amendment, :68 accord_interpretation, :76 federation_floor_policy, :84 schema_civilization_register), each with its own quorum_fraction, pass_threshold, scope and duration. The relay honours all nine: src/relay/api_v2_governance.rs:163 loads ProposalTypeRegistry and looks up p.proposal_type, then :168 computes the quorum and pass verdict from THAT type's authored thresholds. The shipped file is guarded by a test (api_v2_governance.rs:'shipped_proposal_types_parse_and_are_sane') that asserts at least five types parse.

**No path to it.** Both clients hardcode the same three. src/gui/pages/governance.rs:54 is `const PROPOSAL_TYPES: [(&str, &str); 3]`, consumed by the type dropdown at :507-509 with an explicit `.min(2)` clamp. web/pages/governance.html:541 mirrors it with a three-entry const, and its own comment at :536-539 states the reason outright: 'The relay's full authored set is data/governance/proposal_types.ron (9 kinds, each with its own quorum and pass thresholds), but no endpoint serves that file to a browser, so both clients offer the same curated three. If a proposal-types endpoint ever lands, both clients should read it instead.' grep -rn 'proposal_types' src/relay/mod.rs returns nothing ,  there is no route. So six of the nine governance decision types can be tallied but never created from any UI.

**What wiring means.** One new route, GET /api/v2/proposals/types, returning the already-parsed ProposalTypeRegistry (the loader exists and is called on every tally), and replacing the two hardcoded three-entry constants with a fetch. This also fixes an Infinite-of-X violation: the project's own rule says a list that can grow lives in a data file, and here the data file exists while both clients ignore it.

### 2. Bug triage: marking a submitted bug report as in progress, fixed, wont fix, or duplicate. Both clients already display and filter by these statuses; nothing can set one.

**Wiring cost:** small | **Layer:** api-v1

**Already built.** src/relay/api.rs:4427-4455 update_bug_status. It verifies the caller actually holds the key (verify_signed_actor with purpose "bug_status", api.rs:4275), then requires role admin/mod/moderator, then validates the status against [open, in_progress, fixed, wont_fix, duplicate] before calling Storage::update_bug_status. Routed at src/relay/mod.rs:1014 as PATCH /api/bugs/{id}.

**No path to it.** grep -rna "bug_status" and "PATCH.*bugs" over web/ and src/ (excluding src/relay/) returns one hit only: web/pages/bugs-app.js:167, which is the /vote endpoint, not the status one. grep "status" in web/pages/bugs-app.js returns only form-status, filter-status and a status badge render (lines 11, 14, 108, 114, 136, 143) - read paths. grep "status" in src/gui/pages/bugs.rs returns status_label/status_color renderers (lines 205-220, 468-469) and a local status_message string for form feedback; there is no status control. grep "bug" in web/pages/admin-app.js returns nothing. So a project whose stated rule is that everything must be doable from inside the app has an in-app bug tracker that can accept reports and votes but can never close one without hand-editing SQLite.

**What wiring means.** A status dropdown on each bug card, shown only when the signed-in user's role is admin/mod, that signs the "bug_status" purpose (the same helper both clients already use for bug votes and account export) and sends PATCH /api/bugs/{id}. One control in web/pages/bugs-app.js and one in src/gui/pages/bugs.rs.

### 3. Renaming, recoloring, re-scoping, or deleting a task project. The buttons exist in the web Tasks page and are wired to endpoints that reject them.

**Wiring cost:** small | **Layer:** api-v1

**Already built.** src/relay/api.rs:1304 create_project, :1351 update_project, :1402 delete_project - all three call check_api_auth (api.rs:1309, :1357, :1407), which requires an Authorization: Bearer header matching the server's API_SECRET env var and fails closed when it is unset (api.rs:36-50). Routed at src/relay/mod.rs:993-994. The correct per-user path also exists and is complete: src/relay/handlers/msg_handlers.rs:2805 handle_project_update and :2856 handle_project_delete both do a real ownership check (Storage::update_project / delete_project with is_admin derived from the caller's role) and send a Private error message back when the caller is not the owner.

**No path to it.** web/pages/tasks-app.js:1010-1013 sends PATCH /api/projects/{id} with headers of only Content-Type - no Authorization - so check_api_auth returns 401 every time and the catch at :1048 shows "Error: ...". web/pages/tasks-app.js:1064 sends DELETE /api/projects/{id} with no headers at all, same 401. grep -rna "project_update|project_delete|project_create" over web/ and src/gui/ returns four hits (tasks-app.js:268, :274, :283 are inbound broadcast handlers, and :1027 sends project_create); nothing ever sends project_update or project_delete. grep "apiKey" in tasks-app.js returns three hits, all at lines 309-338, i.e. an admin-key text field that exists only in the task-create modal - the project modal has no such field. So the two finished ownership-checked WebSocket handlers have no caller, and the project edit and delete buttons are dead.

**What wiring means.** Change the two fetch calls in web/pages/tasks-app.js to taskWs.send({type:'project_update',...}) and {type:'project_delete', id}, matching the project_create call twelve lines above them. The relay side already exists and already checks ownership. Optionally drop the API_SECRET gate from the three REST project handlers, or keep them as the bot-only path and document them as such.

### 4. Removing your encrypted vault backup, or your stored system profile (your OS, CPU, GPU, RAM and display), from the server without deleting your whole account.

**Wiring cost:** small | **Layer:** api-v1

**Already built.** src/relay/api.rs:2616 vault_sync_delete - verifies a Dilithium3 signature over "vault_sync", enforces a five-minute freshness window, and checks a replay nonce (state.auth_nonce_fresh) before calling Storage::delete_vault_blob. src/relay/api.rs:2740 system_profile_delete - same signature and freshness checks over "system_profile", calls Storage::delete_system_profile (src/relay/storage/system.rs:30). Routed at src/relay/mod.rs:1023-1027 and :1090-1094.

**No path to it.** grep -rna "vault_sync|vault/sync|vaultSync" over web/ and src/ shows web/pages/settings-app.js:1821 (PUT) and :1840 (GET) and nothing else - there is a Sync button and a Restore button, no Remove button. grep "me/system" returns web/chat/chat-profile.js:1334 (PUT) and :1363 (GET) only. grep -rn "delete_vault_blob|delete_system_profile" across all of src/ outside storage returns only the two api.rs call sites. Worse for the second one: src/relay/storage/account.rs:193-211 lists every table the self-service account erase clears, and system_profiles is NOT among them (vault_blobs is, at :198). So the system profile a user uploads is not removed even by "erase my entire account", and the endpoint written to remove it has no button in either client.

**What wiring means.** A "Remove cloud backup" button next to the existing Sync/Restore pair in web/pages/settings.html, and a "Remove system profile from server" button next to the existing sync control in web/chat/chat-profile.js, each signing the same purpose string the PUT already signs and issuing a DELETE. Separately, add system_profiles to the erase list in src/relay/storage/account.rs so full erasure actually erases it.

### 5. Viewing someone's full skill sheet - every skill they have with its real-world XP, in-game XP, and level.

**Wiring cost:** small | **Layer:** api-v1

**Already built.** src/relay/api.rs:2492 get_user_skills returns the complete per-skill list (skill_id, reality_xp, fantasy_xp, level) for a public key. Routed at src/relay/mod.rs:1022. The data is genuinely there: skills are written over the live socket by handle_skill_update (src/relay/handlers/msg_handlers.rs:80, upsert_skill), and endorsements and peer verification of skills are implemented alongside it (msg_handlers.rs:95-171).

**No path to it.** grep -rna "skills/" across web/ and src/ returns web/pages/profile.html:1660 calling /api/skills/search, plus three references to the static data file data/skills/xp_curve.json, and nothing else. No client ever requests /api/skills/{user_key}. The consequence is that you can search the server for "who has carpentry at level 3 or above" but cannot then open that person and see what else they know, which is the obvious next click and the whole point of a skills directory in a mutual-aid platform.

**What wiring means.** Make each result row in the existing skill-search results on web/pages/profile.html link to a panel that calls GET /api/skills/{user_key} and lists the skills; mirror it in the native profile page. It pairs naturally with the member card from the member-directory finding, which is the same public-key-addressed view.

### 6. Create and delete channels from the native Server Settings channel spreadsheet. The page's own doc comment advertises a 'full channel spreadsheet (create / edit / delete inline)' and it draws a '+ New channel' row with name/description fields and a Create button, plus a Delete action per row. The server does implement both operations - just under different message names.

**Wiring cost:** small | **Layer:** websocket

**Already built.** Server side: src/relay/relay.rs:3756 (`/channel-create <name> [--readonly] [desc]`) and :3845 (`/channel-delete <name>`), both admin-gated and both rebroadcasting a fresh channel_list. Client side: src/gui/pages/server_settings.rs:1946-2004 draws the '+ New channel' row and on click sends `{"type":"channel_create","name":...,"description":...}`; :2038-2047 sends `{"type":"channel_delete","channel_id":...}`. Both then set server_settings_status to a success-sounding string ('Channel X creation requested.').

**No path to it.** `grep -rn "channel_create" src/ web/ --include=*.rs --include=*.js` returns exactly ONE line in the whole repo: src/gui/pages/server_settings.rs:1996, the send site. There is no `channel_create` variant in the RelayMessage enum (the 141 serde renames extracted from src/relay/relay.rs:617-2065 contain channel_update but not channel_create/channel_delete) and no `Some("channel_create")` arm in the only raw-JSON dispatch in the relay (src/relay/relay.rs:3356, the sole `raw.get("type")` match - confirmed by `grep -rn 'get("type")' src/relay/` which returns just that one site plus its comment). Unrecognised frames fall through to `_ => {}` after logging 'RelayMessage deserialization failed' (src/relay/relay.rs:3475). `grep -n "pub fn send" -A 12 src/net/ws_client.rs` confirms the client transport is a raw passthrough that does not rewrite the type. Same for channel_delete: only src/gui/pages/server_settings.rs:2042 mentions it. Note the native CHAT page does it correctly - src/gui/pages/chat.rs:6230 sends the `/channel-delete` slash form - so delete works there and silently fails on the Server Settings page.

**What wiring means.** Either change the two send sites to emit the slash-command form the server already accepts (`/channel-create <name> <desc>` and `/channel-delete <id>`, exactly as src/gui/pages/chat.rs:6230 and web/chat/chat-ui.js:1866 do), or add `channel_create`/`channel_delete` arms next to the existing `RelayMessage::ChannelUpdate` handler. Also stop reporting success optimistically before the server confirms.

### 7. Browse the server's member directory: a paginated, searchable roster of everyone who has joined, with each member's join date, last-seen, role, plus (over REST) their listing count and seller rating. It honours a per-user 'unlisted' opt-out that both clients already expose in their privacy UI.

**Wiring cost:** small | **Layer:** websocket

**Already built.** WebSocket: src/relay/relay.rs:1533-1550 (MemberListRequest{limit,offset,search} / MemberListResponse{members,total,target}), routed at src/relay/relay.rs:6003-6005, handled at src/relay/handlers/msg_handlers.rs:1856-1879 (caps limit at 200, returns a matching total for pagination, targets the reply privately). Also MemberJoined/MemberLeft broadcasts. Storage: src/relay/storage/members.rs - join_server writes a row on join (:39-55), and MEMBER_DIR_JOIN/MEMBER_DIR_VISIBLE (:22-27) apply the directory opt-out consistently across list, count and single-member queries. REST twins: src/relay/api.rs:1474 GET /api/members, :1494 GET /api/members/{key}, :1525 GET /api/members/count.

**No path to it.** `grep -rn "member_list" web/ src/gui/ src/net/ src/lib.rs` returns nothing (the only near-miss is voice_channel_list, excluded). Nothing sends member_list and nothing handles member_list_response, member_joined or member_left. The REST twins are equally unused: `grep -arn "api/members" web/ src/` returns only three COMMENTS (web/chat/chat-profile.js:107, web/pages/profile.html:1224, src/gui/pages/profile.rs:215) that all describe the opt-out; the only /members fetches in the codebase are the unrelated /api/guilds/{id}/members (src/gui/pages/guilds.rs:73) and /api/v2/groups/{id}/members (web/chat/chat-groups-p2p.js:331). Sharper still: the opt-out is a first-class onboarding choice (src/gui/pages/privacy.rs:34 directory_unlisted, :107 writes {"directory":"unlisted"}) but it only filters this unreachable directory - the roster users actually see, full_user_list, is built from src/relay/storage/pins.rs:340-352 which selects straight from registered_names with no opt-out filter at all.

**What wiring means.** A members panel that sends member_list with limit/offset/search and renders the response rows (name, role, joined, last-seen), reusing the existing paginated total. Native has no members page at all; web could add one beside the chat roster. Wiring it is also what makes the advertised 'hide me from the directory' privacy setting mean something - or, at minimum, apply the same MEMBER_DIR_VISIBLE filter to the full_user_list roster people actually see.

### 8. An operator kill-switch and per-role permission for live video: a server-wide 'video streaming' master toggle plus a per-role 'stream' capability, both editable in the native Server Settings GUI. Composed as effective_can_stream = master AND role.can_stream.

**Wiring cost:** small | **Layer:** websocket

**Already built.** Enforcement: src/relay/handlers/msg_handlers.rs:2427-2452 (handle_stream_start reads get_server_settings().video_streaming_enabled and role_def(role).can_stream, and refuses with a targeted Private naming which gate failed). Storage: src/relay/storage/mod.rs:1069 (video_streaming_enabled column), :1235 + src/relay/storage/roles.rs:29 (can_stream per role). GUI: src/gui/pages/server_settings.rs:2360 (the master checkbox), :2421 and :2465 (the per-role 'stream' checkbox in the roles editor and the new-role row), :2740 (sent via server_settings_update).

**No path to it.** handle_stream_start is only invoked from the `RelayMessage::StreamStart` arm at src/relay/relay.rs:6087, and `grep -rn "stream_start|stream_stop|stream_offer|stream_answer|stream_ice|stream_viewer|stream_chat|stream_info|stream_set_external" web/ src/gui/ src/net/ src/engine/ src/lib.rs` returns 0 lines - no client can ever reach that gate. Live video actually ships over a different, later path that consults neither setting: src/gui/pages/studio.rs:6 publishes to wss://<relay>/ws/live/pub, authenticated in src/relay/live.rs:200-240 purely by a Dilithium signature over 'live_publish' plus a registered name - its own module doc at src/relay/live.rs:47 states 'no admin role' is involved, and `grep -n "can_stream|video_streaming" src/relay/live.rs` returns nothing. So an owner who unticks 'video streaming' in Server Settings changes nothing, and a role with 'stream' unticked can still publish.

**What wiring means.** Move the two-line permission check from handle_stream_start into src/relay/live.rs's authenticate() (it already resolves the publisher's key to a registered name, so get_role / role_def / get_server_settings are all one call away), and return the same refusal text over the existing error frame the publisher socket already sends. That makes the two GUI controls real without touching the transport.

### 9. Delete a task from the shared task board. The server implements it with admin/mod authorization and broadcasts the removal to every connected board, and the web board already knows how to render that broadcast.

**Wiring cost:** small | **Layer:** websocket

**Already built.** Protocol: src/relay/relay.rs:1379 (task_delete) and :1385 (task_deleted). Routing: src/relay/relay.rs:5965. Handler: src/relay/handlers/msg_handlers.rs:1247-1273 (role check for admin/mod, db.delete_task, broadcasts TaskDeleted, or a targeted 'Task not found.'). The receive half is already wired on the client: web/pages/tasks-app.js:257-261 filters the deleted id out of allTasks and re-renders, and closes the detail overlay if it was open.

**No path to it.** `grep -rn "task_delete" web/ src/gui/ src/net/ src/lib.rs` returns exactly one line - web/pages/tasks-app.js:257, the `task_deleted` RECEIVE handler. Nothing sends the request. `grep -an "delete|Delete" web/pages/tasks-app.js` shows delete UI only for PROJECTS (deleteProject at :1060-1076, via REST DELETE /api/projects/{id}) and for the unrelated local quest list; there is no delete control for a task. The native Tasks page is not a client for this at all - `grep -n "ureq|ws_client|client.send|http" src/gui/pages/tasks.rs` returns nothing, it is purely local state. The REST twin is not a human path either: src/relay/api.rs:1166-1174 DELETE /api/tasks/{id} is documented 'via bot API (requires API_SECRET auth)', so an ordinary admin in a browser cannot use it. Net effect: a task created on the board can never be removed by anyone without shell access to the server's API secret.

**What wiring means.** One delete control in the task detail overlay in web/pages/tasks-app.js that sends `{type:'task_delete', id}` (mirroring the existing task_update send at :682), shown only when the viewer is admin/mod. The server authorization, the broadcast and the client re-render already exist.

### 10. Setting your own presence status (online / away / busy / do not disturb) so other members see it

**Wiring cost:** small | **Layer:** native-gui

**Already built.** Relay: RelayMessage::SetStatus at src/relay/relay.rs:1255, handled at :5922-5937 - validates the four statuses, persists via db.save_user_status, updates the in-memory map and rebroadcasts the peer and full user lists. Native already CONSUMES the result: src/gui/pages/chat.rs:2695-2696 colours the presence dot for away/busy/dnd, and :5759-5760 labels it "Away" / "Do Not Disturb" in the user modal.

**No path to it.** grep -rn "set_status" over src/gui/ src/net/ src/engine/ src/lib.rs returns 5 hits, ALL of them an unrelated private helper fn set_status in src/net/voice.rs:153,210,214,225,565. The WS message is never constructed. Native's only DND control is src/gui/pages/settings.rs:2185-2226, which is local quiet-hours for notifications and rides update_notification_prefs, a different message. web/chat/chat-ui.js:2224-2238 has setMyStatus/clearMyStatus sending the real message.

**What wiring means.** A four-option selector next to your own name in the chat sidebar that does ws.send(json!({"type":"set_status","status":s,"text":""})). The rendering side is already finished.

### 11. Look up anyone's identity card from inside the desktop app ,  their DID, their trust score with all six sub-scores, every credential issued to them, and whether they are a human or an AI agent ,  plus, on the Recovery page, who your recovery guardians are and whose shares you are holding. The relay answers all five of these live today.

**Wiring cost:** medium | **Layer:** api-v2

**Already built.** Server side, all working: src/relay/api_v2_did.rs:57-80 (resolve_did), src/relay/api_v2_trust.rs:26-41 (get_trust_score, 5-min cache, ?fresh=true), src/relay/api_v2_credentials.rs:76-98 (list_credentials), src/relay/api_v2_ai.rs:18-40 (get_ai_status), src/relay/api_v2_recovery.rs:26-66 (setup + shares-held-by). Client side, the buttons exist: src/gui/pages/identity.rs:85 ('Look up' sets state.identity_lookup_pending = true), src/gui/pages/recovery.rs:50 ('Look up' sets recovery_lookup_pending), src/gui/pages/recovery.rs:94 ('Show shares' sets recovery_guardian_pending). The native client already has a proven blocking-GET helper it could reuse verbatim: src/gui/pages/governance.rs:143 `let get = |url: &str| ureq::get(url).timeout(6s)...`, which is how the native Governance page reads /api/v2/proposals today.

**No path to it.** grep -rn 'recovery_lookup_pending|recovery_guardian_pending|identity_lookup_pending' src/ returns exactly 9 lines: three declarations (src/gui/mod.rs:4679, 4738, 4742), three initializations to false (src/gui/mod.rs:5824, 5847, 5849), and three assignments to true (identity.rs:85, recovery.rs:50, recovery.rs:94). Nothing ever READS them. grep -n 'ureq|http|fetch|reqwest' src/gui/pages/identity.rs src/gui/pages/recovery.rs returns nothing at all ,  neither page contains an HTTP client. Instead identity.rs:113, :137, :160, :184 print the strings 'GET /api/v2/did/{did}', 'GET /api/v2/trust/{did}', 'GET /api/v2/credentials?subject={did}', 'GET /api/v2/ai-status/{did}' into detail_row widgets as documentation, and recovery.rs:62 does the same for 'GET /api/v2/recovery/setup/{holder_did}'. So three buttons in the shipped desktop app do nothing when clicked. Web is the only client that calls these: web/pages/identity.html:169,188 and web/pages/recovery.html:120,148 via web/shared/pq-identity.js.

**What wiring means.** Copy the six-line `get` closure from src/gui/pages/governance.rs:143 into identity.rs and recovery.rs, spawn it on a worker thread the way governance.rs:151-190 already does, deserialize into structs mirroring the JSON the handlers return, and replace the detail_row 'Server endpoint' rows with the actual values. No server change, no new endpoint, no design work ,  the response shapes are already fixed and the web page proves them.

### 12. Editing a guild you own: its name, description, icon, or color.

**Wiring cost:** medium | **Layer:** api-v1

**Already built.** src/relay/api.rs:3897 update_guild - loads the existing guild, falls back to the current value for each unset field, validates the name length, and calls Storage::update_guild which returns false unless the caller is the owner (mapped to 403 "Only the guild owner can update"). Routed at src/relay/mod.rs:1008 as the PATCH arm alongside GET and DELETE.

**No path to it.** grep -na "api/guilds" in web/pages/guilds.html returns seven calls (lines 339, 345, 351, 357, 368, 377, 386): list, get one, members, create, join, leave, invite. grep -rn "api/guilds" in src/gui/pages/guilds.rs returns eight (lines 33, 38, 73, 358, 474, 484, 492, 567): the same set plus the owner's DELETE at :474. grep -rna "PATCH|patch(" across both files returns nothing. So a guild owner can create a guild and can destroy it, but can never fix a typo in its name - the only recourse is delete and recreate, which loses every member.

**What wiring means.** An "Edit guild" form reachable from the owner block that already renders the Delete Guild button (src/gui/pages/guilds.rs:471, and the equivalent in web/pages/guilds.html), sending PATCH /api/guilds/{id} with owner_key and the changed fields. The native page's spawn_guild_action helper already takes a method and body, so it needs a PATCH arm and a form.

### 13. Group channels into named, ordered categories (the collapsible sidebar sections every chat app has). Full CRUD exists: create, rename, delete a category, assign a channel to one, and every client is already shipped the category list and each channel's category on connect.

**Wiring cost:** medium | **Layer:** websocket

**Already built.** Storage: src/relay/storage/misc.rs:547-615 (create_category, delete_category, rename_category, set_channel_category, list_categories with positions); table at src/relay/storage/mod.rs:814-820; src/relay/storage/channels.rs:117-133 (list_channels_with_categories). Wire format: src/relay/relay.rs:551-554 (ChannelInfo.category_id / category_name), :573-577 (CategoryInfo{id,name,position}), :735 (ChannelList.categories). The server populates it on every connect and every rebroadcast: src/relay/relay.rs:2954-2955. Commands: src/relay/relay.rs:4856 /category-create, :4882 /category-delete, :4902 /category-rename, :4922 /channel-category.

**No path to it.** `grep -arn "category_id\|category_name\|\.categories" web/chat/*.js src/gui/pages/chat.rs src/gui/mod.rs` returns only three hits in web/chat/chat-ui.js:2106-2123, which are the unrelated slash-command PALETTE's own categories - no client reads the channel-list categories. The native client actually decodes it and then drops it: src/lib.rs:16111 parses `category_name` into ChatChannel.category (field declared at src/gui/mod.rs:1657), and `grep -c "category" src/gui/pages/chat.rs` returns 0, so the sidebar that draws the channel list never reads the field. For the admin side, `grep -rl "/category-create\|/category-delete\|/category-rename\|/channel-category" src/gui/ web/` returns nothing at all - these four are the only commands in the whole slash surface with zero GUI wrapper anywhere (compare /server-add, /verify, /lockdown, /wipe, which src/gui/pages/server_settings.rs all wraps in buttons).

**What wiring means.** Native: group the sidebar by the ChatChannel.category value it already holds (src/gui/pages/chat.rs channel list drawing) - the data arrives, nothing else is needed to display it. Web: read msg.categories and each channel's category_name in the channel_list handler. Admin: a small panel (or a column in the existing Server Settings channel spreadsheet) that sends the four existing commands, matching how that page already wraps /server-add and friends.

### 14. Looking up a DID and seeing its trust score, credentials and AI status from the native Identity page - the page's entire advertised purpose

**Wiring cost:** medium | **Layer:** native-gui

**Already built.** Server: src/relay/mod.rs:1061-1072 registers GET /api/v2/did/{did}, /api/v2/credentials, /api/v2/trust/{did}, /api/v2/ai-status/{did}. Trust is fully computed in src/relay/storage/trust_score.rs:70 get_trust_score / :155 compute_trust_score_inner (vouch entropy, activity diversity, account age, stake, sigmoid, 0.95 cap) and is already load-bearing - src/relay/storage/governance.rs:154 weights votes by it. Native already has the exact fetch pattern it needs: src/gui/pages/governance.rs:144-180 does blocking ureq GETs on a worker thread into a state channel.

**No path to it.** grep -rn "v2/trust|v2/credentials|v2/ai-status|v2/did" over src/gui/ src/net/ src/lib.rs returns exactly four hits, all string LITERALS printed as UI text: src/gui/pages/identity.rs:113, :137, :160, :184 render rows like "Server endpoint | GET /api/v2/trust/{did}". identity.rs is in the set of pages with no ureq:: call at all. The "Look up" button at src/gui/pages/identity.rs:85 sets state.identity_lookup_pending = true; grep -rn identity_lookup_pending over src/ returns only the struct field (src/gui/mod.rs:4679), the Default (:5824) and that one write - nothing ever reads it. The web mirror does the real thing (web/pages/identity.html doLookup -> window.HumOS.pq.enrichDid, helpers at web/shared/pq-identity.js:64,77,88,114).

**What wiring means.** Give identity.rs the governance.rs treatment: on identity_lookup_pending, spawn a thread doing four ureq GETs against state.server_url, push results through an mpsc into new GuiState fields, and render them in the four cards that currently print endpoint names. The cards, layout and copy already exist.

### 15. Notes and Calendar events surviving a restart

**Wiring cost:** medium | **Layer:** native-gui

**Already built.** Both pages are complete: src/gui/pages/notes.rs (list, editor, title, bold/italic toolbar, word count, and a Saved / Unsaved changes indicator at :176-186) and src/gui/pages/calendar.rs (month grid, day selection, Add Event at :294-310 with title, time and colour). The persistence mechanism exists and is used by ~200 other GuiState fields: crate::config::AppConfig::from_gui_state(state).save() - src/gui/pages/onboarding.rs:179 calls it after ticking a single checkbox, exactly the pattern needed here.

**No path to it.** grep -c "notes|cal_events" src/config.rs returns 0. grep -rn "GuiNote|GuiCalendarEvent" over src/ (minus snapshots) returns only the struct definitions at src/gui/mod.rs:1282,1294, the two Vec fields at :3065,:3071, and the two push sites at src/gui/pages/notes.rs:52 and src/gui/pages/calendar.rs:303. Nothing in src/persistence.rs or src/lib.rs touches them. The Saved indicator at notes.rs:176-186 is derived purely from a timestamp delta, so it prints Saved in green two seconds after the last keystroke while nothing has been written anywhere. The web mirror does persist (web/pages/notes-app.js:71 saveNotes -> localStorage 'hos_notes_v1').

**What wiring means.** Add notes: Vec<GuiNote> and cal_events: Vec<GuiCalendarEvent> to AppConfig with #[serde(default)], copy them in from_gui_state/apply_to, and call AppConfig::from_gui_state(state).save() after edits (debounced) in notes.rs and after the push at calendar.rs:303.

### 16. Publishing a vouch for another member, and declaring yourself an AI agent bound to a human operator - the two identity objects the trust score and AI-status system read

**Wiring cost:** medium | **Layer:** native-gui

**Already built.** Server indexes and consumes them: src/relay/storage/trust_score.rs:175-180 counts distinct issuers of vouch_v1 VCs as the primary trust input; src/relay/storage/ai_status.rs:84 index_subject_class, :110 index_controlled_by, :181 enforces that an ai_agent must carry a controlled_by_v1 binding; src/relay/storage/governance.rs:432 uses subject_class to exclude AI from vote weight; src/relay/storage/signed_objects.rs:279 and src/relay/transport/lora.rs:206-213 route these types. Native already has the complete publish path: src/gui/pages/governance.rs:230-259 (ObjectBuilder + payload_cbor + sign with the Dilithium keypair) and :283-296 post_signed_object -> POST /api/v2/objects.

**No path to it.** grep -rn "vouch_v1|subject_class_v1|controlled_by_v1" over src/ and web/ (excluding .md) shows the only constructions are inside relay test helpers (src/relay/storage/credentials.rs:295, issuer_trust.rs:300, ai_status.rs:210,220, trust_score.rs:334, governance.rs:432). In src/gui/ the strings appear only as descriptive prose on the Identity page (src/gui/pages/identity.rs:125, :172-173). In web/ vouch_v1 appears once, as a doc-comment example at web/shared/pq-identity.js:154. No client can create either object, so the trust graph has no way to gain edges and an AI participant cannot become compliant with the rule the Identity page states.

**What wiring means.** Reuse governance.rs's builder/post helpers with a different object_type: a "Vouch for this person" button on the chat user modal (src/gui/pages/chat.rs:5870-5881 already has the follow/unfollow buttons and the target key), and an AI-declaration section on the Identity or Settings page that posts subject_class_v1 plus controlled_by_v1.

### 17. Post a moderation action as a signed, publicly verifiable object instead of an untraceable server command, and let a space publish who its moderators are. The relay checks the signer against the space's own signed policy, applies the mute/ban/role change, and records whether the authority was the space's declared policy or just this server's role table - so a moderated user can later prove what was done, by whom, and under what rule. Nothing in either client can produce one of these objects.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** src/relay/storage/moderation.rs is 961 lines and complete: :132 current_space_policy (resolves the space's signed space_policy_v1), :229 apply_mod_action gated on object_type == "mod_action_v1" at :230, returning a ModOutcome that distinguishes Declared authority from the ServerRole fallback. It is dispatched on every incoming signed object at src/relay/storage/signed_objects.rs:236, after the Dilithium signature is already verified. The client-side encoders are written too, byte-mirrored to the Rust: web/shared/pq-object.js:169 MOD_ACTIONS, :181 modActionV1Payload (validates action, target_kind, and requires a reason "because an unexplained moderation action is what the appeals requirement exists to prevent"), :220 buildModActionV1, :242 spacePolicyV1Payload, :268 buildSpacePolicyV1.

**No path to it.** grep -rn 'buildModActionV1|buildSpacePolicyV1|modAction|spacePolicy' over all of web/ excluding web/shared/pq-object.js returns zero hits - the two exported builders have no importer. grep -rn 'mod_action_v1' over web/ and src/ excluding src/relay/storage/moderation.rs hits only pq-object.js's own definitions; 'space_policy_v1' likewise appears only in pq-object.js and moderation.rs. The moderation buttons that do exist take the unsigned path instead: src/gui/pages/chat.rs:5912-5951 call send_mod_action at :6022, which sends the plain WS message {"type":"mod_action"} handled at src/relay/relay.rs:5992 - no signed object, no appeal trail. The module's own header says the point was to stop being "an encoder nobody calls"; the Rust half stopped, the UI half did not start.

**What wiring means.** On the web side: import buildModActionV1 from /shared/pq-object.js in the chat moderation menu, collect the reason the payload already requires, and POST the result to /api/v2/objects the same way web/pages/governance.html:510 already posts a proposal. A one-time Server Settings form calling buildSpacePolicyV1 publishes the moderator set. Native needs the equivalent Rust payload builder next to the existing src/net/api_v2.rs submit_signed_object. The verification, authority resolution, and application logic are all already running.

### 18. A points-and-levels reputation record per member, with a per-event history (helpful message, task completed, trade completed, review given, reported, verified by a moderator) and a server leaderboard. The tables, the level maths, the history query and the leaderboard query are all written, and two REST endpoints serve them. Nothing ever awards a point, and no client ever reads the result.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** src/relay/storage/reputation.rs:32 points_for_event (the six event types and their point values), :48 add_reputation_event (inserts the event and upserts the running score+level), :93 get_reputation, :121 get_reputation_history, :150 get_reputation_leaderboard; level_from_score at :26. Tables at src/relay/storage/mod.rs:2161 reputation and :2168 reputation_events. Endpoints src/relay/api.rs:4069 get_reputation (returns the record plus its last 20 events) and :4109 get_reputation_leaderboard, routed at src/relay/mod.rs:1017-1018.

**No path to it.** grep -rn 'add_reputation_event' across all of src returns exactly one line, its own definition at src/relay/storage/reputation.rs:48 - no caller anywhere, not even in tests, so the tables are never written. grep -rn 'points_for_event' likewise returns only its definition. On the client side, grep for '/api/reputation' over web/, src/gui/ and src/net/ returns zero hits; the five files that contain the word "reputation" are prose (src/gui/pages/identity.rs:23,126; src/gui/pages/settings.rs:371; src/gui/pages/humanity.rs:351; web/pages/settings-app.js:2035) and the game's unrelated per-entity quest reputation in src/relay/handlers/game_state.rs. The gap is visible inside the trust score too: src/relay/storage/trust_score.rs:255 hardcodes `let reputation_score = 0.0;` with the comment "leave at 0 unless the legacy reputation table happens to be", while W_REPUTATION at :27 is 0.10 - so a tenth of every trust score is permanently zero because nothing feeds this table.

**What wiring means.** Call add_reputation_event at the six places the point table already names - they all exist as handlers today (message accepted, task marked complete in src/relay/storage/board.rs, trade filled in src/relay/storage/trading.rs, review created in src/relay/storage/reviews.rs, report filed, /verify issued) - then show the score on the profile card and add a leaderboard view fed by the finished GET /api/reputation/leaderboard. Bridging did -> pubkey hex at src/relay/storage/trust_score.rs:255 then also un-zeroes the reputation sub-score.

### 19. Turn a whole capability off on your own server: chat, game, market, vault backup, uploads, tasks, voice, live video, federation, push. The relay enforces these hard, refusing the feature's endpoints with HTTP 403 and publishing the manifest so clients hide what they cannot use. Choosing them is a hand-edit of data/server-config.json plus a restart, over SSH. The Server Settings page has a Services table that looks like it does this but only flips the softer database toggles.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** data/server-config.json "features" block (ten keys, all defaulting true). Parsed at src/relay/relay.rs:279 via crate::relay::features::Features::from_config, with a startup log at :286-292 that literally tells the operator to "Edit the `features` block in data/server-config.json to change this". Enforcement is the feature_gate middleware at src/relay/mod.rs:67-90, which maps a request path to a feature and returns 403 {error: feature_disabled} - applied inside security_headers so the refusal carries the headers too. The manifest is published on GET /api/server-info so clients can hide disabled features.

**No path to it.** This is a server-owner control with no in-app path, and it is not one of the known gaps: scanning every action in data/admin/ops_registry.json for /feature|capabilit|manifest|server-config/i returns only the SOFT-gate entry ("Services: feature gates and daemon start/stop", which the entry itself describes as server_settings_update checkboxes), the server name/description entry, the capacity-knobs entry and the donations entry. The hard manifest appears in no action, in no also_available line, and in none of the ten planned gaps. Cross-checking the other direction, every column of the server_settings table (src/relay/storage/mod.rs:1059-1085) does have a native editor in src/gui/pages/server_settings.rs - so the soft gates are covered and the hard ones are the hole. The registry's stated purpose is that "an AI can enumerate the full action surface from this one file"; ten enforced kill switches are missing from it.

**What wiring means.** Add the ten checkboxes to the existing Server Settings ADMIN panel and have an admin-signed endpoint write the features block back into data/server-config.json, with a "restart to apply" note - the same shape as the already-planned boot-default config editor for max_connections/max_history/donations, and the natural place to do both in one pass. Nothing on the enforcement side changes: Features::from_config, the 403 middleware and the /api/server-info publication already work.

---

## LOW value

### 1. Show how much signed content a relay actually holds ,  total objects, or objects of one type ,  so an operator or a prospective member can size up a server before joining it.

**Wiring cost:** small | **Layer:** api-v2

**Already built.** src/relay/api_v2_objects.rs:498-513 (count_objects), backed by Storage::count_signed_objects, with an optional object_type filter (CountObjectsQuery at :494). Routed at src/relay/mod.rs:1053.

**No path to it.** grep -rn 'objects/count' across web/, src/, scripts/, tests/ and docs/ returns only the route line and the handler's own two comments (src/relay/api_v2_objects.rs:7 and :492). It has no wrapper in web/shared/pq-identity.js (which wraps sixteen other v2 calls), so not even the JS bridge exposes it. The place it belongs is already built and already displayed: GET /api/stats (src/relay/api.rs:207-217) returns total_messages, connected_peers and version ,  messages but no signed objects, which is where the marketplace listings, proposals, groups and credentials all live.

**What wiring means.** Either fold the count into the existing /api/stats response, or add one line to the Ops and server-info surfaces that already render stats. It is the cheapest orphan in this layer and it is the one number that would tell a person whether a federated server has anything on it.

### 2. Down-weight credentials from issuers this server has learned to distrust, and let a member formally dispute a credential that was issued about them. Trust drops rather than issuers being banned, which is the Accord's non-domination rule expressed in code.

**Wiring cost:** medium | **Layer:** api-v2

**Already built.** src/relay/storage/issuer_trust.rs (fully implemented, seven tests): issuer_trust_good at :62, issuer_trust_bad at :114, the reader issuer_trust at :167, index_dispute at :182 which applies a 0.02 penalty at :211 and refuses self-disputes (test at :337). It is actively being written in production: src/relay/storage/signed_objects.rs:286 credits every issuer 0.005 on each valid VC ingested, and :273 routes dispute_v1 objects into index_dispute.

**No path to it.** Nothing reads the matrix and nothing can file a dispute. grep -rn '\.issuer_trust(' src/ returns five lines, ALL inside #[cfg(test)] in issuer_trust.rs itself (:236, :277, :285, :327, :349) ,  the getter has no production caller. grep -n 'issuer' src/relay/storage/trust_score.rs returns only vc_index issuer_did lookups (:175-178); the Phase 2 trust score does NOT consult issuer trust, so the module docstring's claim at :13 that low issuer trust 'down-weights their VCs in Phase 2 trust scores' is not implemented. And no client can produce the input: grep for 'dispute_v1' across web/ and src/gui/ returns nothing, so index_dispute can only ever fire on an object no shipped client knows how to make. There is no /api/v2 route exposing issuer trust at all ,  grep -n 'issuer' src/relay/mod.rs returns nothing.

**What wiring means.** Three connected pieces: a read route so a client can see how this server rates an issuer, a 'dispute this credential' action that builds a dispute_v1 signed object (the ingest path already handles it), and one term added to the trust-score computation so issuer trust actually modulates VC weight the way the module says it does. Today the table accumulates a score on every credential and no code path, user or server, ever asks it a question.

### 3. A reputation system: points for helping, completing tasks, completing trades, giving reviews, being verified by a moderator (and negative points for being reported), a level every 50 points, a per-user event history, and a server leaderboard.

**Wiring cost:** medium | **Layer:** api-v1

**Already built.** src/relay/storage/reputation.rs is complete: points_for_event (:31, the six event types and their values), level_from_score (:25), add_reputation_event (:48, inserts the event and upserts the total in one transaction), get_reputation (:93, returns a zeroed default for a user with no history), get_reputation_history (:121), get_reputation_leaderboard (:150, score DESC, score > 0). Tables and indexes at src/relay/storage/mod.rs:2161-2181. Endpoints at src/relay/api.rs:4070 get_reputation (score, level, and the last 20 events) and :4110 get_reputation_leaderboard (joins display names from server_members). Routed at src/relay/mod.rs:1017-1018.

**No path to it.** grep -rna "api/reputation" across web/ and src/ returns nothing outside src/relay/. grep -rni "reputation" in web/ returns two hits, both prose: web/pages/identity.html:110 and web/pages/settings-app.js:2035. In src/gui/ it returns four hits, all prose in humanity.rs, identity.rs and settings.rs. The write side is equally unwired: grep -rn "add_reputation_event" across the whole of src/ returns zero hits outside its own definition in storage/reputation.rs, so no task completion, trade fill, review, or moderator action ever awards a point. Both halves of the loop are missing their call sites while the entire middle is built and indexed.

**What wiring means.** Two things. First, call state.db.add_reputation_event at the five places the point table already names: the task-completed branch in msg_handlers, the trade fill handler in api.rs:2195, handle_review_create in msg_handlers.rs:1975, the report/moderation path, and the moderator verify path. Second, display it: a reputation line on the member card (see the member-directory finding, which is the natural home) and a leaderboard section on the Civilization page, which already fetches aggregate server stats.

### 4. A live dashboard of which AI agents are working on which parts of the project: who holds a claim on a scope, when they last checked in, whether they are working, paused, blocked or done, plus a human override to mark a scope active/passive/blocked. Today the operator reads this by running `node scripts/agent-status.js` in a terminal. The web navigation already has an "Agents" menu entry, and it leads to a page that does not exist.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** Storage: src/relay/storage/agent_sessions.rs:43 agent_claim_scope (with the 30-minute stale-claim yield rule), :87 agent_heartbeat, :110 agent_release_scope, :132 agent_get_session, plus agent_list_sessions; table at src/relay/storage/mod.rs:1969 CREATE TABLE agent_sessions. API: src/relay/api_v2_agents.rs serves GET /api/v2/agents/status (aggregating the runtime table with data/coordination/agent_registry.ron, data/coordination/sessions/*.json and data/coordination/overrides.ron), GET /api/v2/agents/sessions, and POST /api/v2/agents/override with Dilithium admin auth and scope-id validation at :48; routed at src/relay/mod.rs:1085-1087. Its own header at line 18 says "The dashboard page (/agents) hits these endpoints".

**No path to it.** grep -rn '/api/v2/agents' over web/, src/gui/ and src/net/ returns zero hits. `find web -iname "*agent*"` returns nothing - there is no agents.html. Yet web/shared/shell.js:1119 renders mobileLink('/agents', 'Agents') inside the Developer-and-operator group, and shell.js:311 has an active-tab rule for paths starting /agents, so the nav ships a link to a 404. On the native side the only hits for "agents" under src/gui/pages/ are in mod.rs and settings.rs, neither of which fetches these endpoints. Four of the five public functions in agent_sessions.rs have no caller outside the module, so no agent ever claims a scope or heartbeats either.

**What wiring means.** Build the page the nav already links to: web/pages/agents.html rendering the single GET /api/v2/agents/status response as a table of scope, holder, state, last heartbeat and notes, with a status dropdown posting to /api/v2/agents/override. One endpoint, one table, no server work. Separately, the claim/heartbeat/release functions need a caller if agents are meant to register at all - today they are declared but nothing checks in, so the runtime column would render empty until scripts/agent-status.js writes through the relay instead of straight to disk.

### 5. A record of what has been streamed on a server: title, category, who streamed it, when it started and ended, peak viewer count, and the stream's chat log. The persistence layer, the viewer-peak tracking and the chat capture are all implemented and unit-tested. The live-video feature that actually shipped records none of it, so no server has any stream history and no page can show one.

**Wiring cost:** medium | **Layer:** storage-and-data

**Already built.** src/relay/storage/streams.rs is a complete module: create_stream, end_stream (stamping ended_at and MAX(viewer_peak)), update_stream_viewer_peak, store_stream_chat, get_recent_streams. Tables at src/relay/storage/mod.rs:1528 streams and :1541 stream_chat. The write path is fully handled: src/relay/handlers/msg_handlers.rs:2462, :2504, :2615, :2651, driven by nine WebSocket variants defined at src/relay/relay.rs:1910-2008 (stream_start, stream_stop, stream_offer, stream_answer, stream_ice, stream_viewer_join, stream_viewer_leave, stream_chat, stream_info_request, stream_set_external) and dispatched at src/relay/relay.rs:6087-6114. There are passing tests at msg_handlers.rs:4380-4441 proving the lifecycle records correctly.

**No path to it.** grep -rn 'stream_start|stream_stop|stream_offer|stream_answer|stream_ice|stream_viewer_join|stream_chat|stream_info|stream_set_external' over web/ and src/ excluding src/relay/ returns zero hits - no client sends any of these, so nothing is ever written. The read side is equally unused: the only callers of get_recent_streams are inside the #[cfg(test)] module beginning at src/relay/handlers/msg_handlers.rs:4330. The streaming that did ship is a different, in-memory plane: src/relay/live.rs holds streams in a RwLock<HashMap> (:124-141) and touches the database only for db.name_for_key at :233, served over /ws/live/pub, /ws/live/sub/{stream} and GET /api/live and consumed by src/gui/pages/studio.rs, src/gui/pages/watch.rs:228 and web/chat/chat-live.js:67. When a live stream ends, everything about it is gone.

**What wiring means.** Call the four existing storage functions from the live plane instead of from the abandoned protocol: create_stream when src/relay/live.rs:288 inserts into the registry, update_stream_viewer_peak as subscribers attach, end_stream at :335-338 when it is removed. Then a "past streams" list on the Watch page reads get_recent_streams. Decide first whether the stream_* WebSocket variants and their handlers should be deleted rather than kept alongside - keeping two stream protocols is what produced the orphan.
