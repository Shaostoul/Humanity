# Increment 5b build plan: the household page, and walking into a permitted plot

Date: 2026-10-12. Read against main at 401d21114 (v0.1487.1). Sources: docs/design/ship-homes-increment-5-plan.md (section 4 "3B", section 6 question 3, "Decisions taken (2026-10-05)" item 3), docs/design/ship-homes-and-logistics.md (sections 2, 4 and 7, increments 2 to 5), docs/design/shared-building.md (as built), CLAUDE.md, docs/PRIORITIES.md, and the code cited below. Every line number was read today. Nothing was edited or booted.

## 0. In short

- **Increment 5 is built and released (v0.1464.0).** The relay already checks household permits on `game_build` / `game_unbuild` (src/relay/handlers/shared_build.rs:396-409). The game neither mints nor sends one: both its requests go out with `permit: None` (src/engine/shared_build.rs:1061, :1084).
- **A permit travels as a sealed DM note.** The text is `[[hum:permit:v1]]` followed by the contract's `Permit` JSON, which the DM's inner signature covers (the house's newer "marker plus JSON" notes, like `[[hum:offer:v1]]`).
  - The giver mints it with the existing `build_plot_permit` (src/relay/core/pq_crypto.rs:569-584).
  - It is sealed and sent with `control_puts` (src/engine/dm.rs:469-486), carrying the friend's pass so it gets past their gate.
  - It counts as given only on `dm_put_ok` (the 10l rule).
  - Its self-copy tells the giver's other devices.
  - The grantee keeps it in the per-(identity, server) encrypted DM store, from friends only, after checking it against the sender, this server's `did:hum` and their own `did:hum`.
- **One new relay message pair: `game_permit_check` / `game_permit_status`.** After each welcome, and every 5 minutes, the game asks whether each held permit works now (signature, date, and whether the giver still holds the plot). It answers only about the asker's own permits and stores nothing. A door opens only on the relay's "works". Without this, a stale permit would walk its holder into a new holder's home.
- **Walking in.** For plots this game holds a working permit for, a new never-saved `ShipStructure::open` field turns increment 2's render-only neighbour into a walkable one:
  - the street or Commons wall gets its corridor gap for walking;
  - the corridor's side walls and the home's walls collide;
  - the corridor's doors open for this player (`others_only` false);
  - the shipped design's own lights come on while the player stands on that plot or in its corridor, charged to nobody's meter.
  - Nothing changes for anyone else, and the meshes are not rebaked.
- **What a permit holder sees** is all the shared world knows of that home: the plot kind's shipped design, plus the pieces the server keeps on `plot:<id>`. Both are already in every game near the plot (neighbours.rs, and the 250/300 m piece interest). The holder's own edits, machines, garden and private pieces never leave their device (section 1.2).
- **The household page** is a "Household" card at the top of the Home page (GuiPage::Homes, the nav's "Home"). It has two parts:
  - **Your plot:** pick a friend, pick 7, 30 or 90 days (a data file), Give (with one confirm step); each standing permit has Renew.
  - **Plots you may build on:** what you hold, and whether it works.
  - It says plainly that a permit cannot be taken back before its end date.
- **Building there.** The gate opens a permitted plot to shareable pieces. F takes down only your own pieces there. The permit rides on each request.
- **The website** swallows the note (web/chat/chat-social.js `ingestDmControl`). It has no game, so it shows nothing.
- **Nobody is told who lives on which plot:** no game message changes, and a permit is told only to the friend its giver chose (section 3.11).
- **Waves:** five lanes in parallel after a one-builder Wave 0, two lanes after that, then the rig. No hot file goes over budget: lib.rs, msg_handlers.rs, net_route.rs and frame_ws_poll.rs are untouched, relay.rs changes one line, and gui/mod.rs gets room by moving two test modules into sibling files.
- **Proof:** `just verify-household` = `node scripts/verify-copresence.js --household`, with one game and one scripted friend on a throwaway relay:
  - the game gives a permit through its page, the friend builds on the game's plot with it, and the game renews it;
  - the friend gives the game a permit; the game's walk into her plot is blocked before it and goes through after;
  - the game sees her wall, lit, and builds and takes down there under the rules.

## 1. Today's code (file:line)

### 1.1 Plots, doors and neighbour shells
- **Plots.** Twelve homestead plots, pN at (0, 0, 99(N-1)), 55 x 3 x 89 m; p1's corridor runs to the Commons at z 40, p2 to p12 to First Street (data/blueprints/ship_structure.ron; ship-homes doc 1005-1020).
- **Where the home stands.** `ShipStructure` :509-535 (`home` is `#[serde(skip)]` :532-534, the precedent for a field that is never saved). `home_plot()` :1314, `home_is_away()` :1666. `ship_hash()` :1358-1361 hashes `ship_file()` :1930-1937, so skip fields never change the hash.
- **Who is a neighbour.** `neighbour_plots()` ship_structure.rs:1681-1688: every plot but the home's, all of them for a guest. `plot_door_tube()` :1694.
- **Neighbour shells.** src/ship/neighbours.rs:
  - each neighbour is drawn as its kind's shipped design (`HomeDesign::built_in_ref`, ship_structure.rs:368-377, embedded data/homes/shipped/homestead.ron) with its door corridor (`make_view` :131-150);
  - render only: no machines, no rooms, so no lights, and no walking collision (module doc :16-24);
  - the view is cached per thread by `NeighbourKey` (own plot, plots, zone boxes) :73-125, and one shell is baked per design and door (`bake` :170-206, using `HomeStructure::bake_shell_groups`, home_structure.rs:1183: floors, interior walls, structures, ceiling, trim, no glass);
  - `draw_into` :238-244, `neighbour_mouths` :248-250, `segments(sight)` :256-273 (the home's walls with its door open, plus its corridor side walls).
- **Collision.** src/ship/wall_collision.rs:
  - `ship_wall_segments` :135-137 is walking: own zones, ship corridors, never a neighbour;
  - `ship_sight_segments` :148-150 adds the neighbours and their corridor holes, for sight only;
  - `everyones_walls` :159-161 is the rig's;
  - all three build in `ship_segments_impl` :177-208.
  - The hole a neighbour's corridor makes in First Street or the Commons is cut in the mesh only, so the wall still stops this player (neighbours.rs:18-20, test :344-365).
- **Doors.** src/ship/door_panels.rs: `PanelPlacement::others_only` :68-72; `ship_panel_placements` :81-102 appends the neighbour pairs last; `neighbour_panel_placements` :112-115; `door_actor_distance` :123-130 opens an `others_only` door for other players only.
- **Hull.** It wraps every neighbour plot opaquely (src/ship/hull.rs:331-348). Unchanged here.
- **Rebuild sites.** Colliders and lights are rebuilt in src/engine/home_meshes.rs `rebuild_homestead` :206-300 (walls :240-243, sight :244-247, auto and placed lights :255-262), `rebuild_door_panels` :2095-2120 and `home_lights` :2145- (placed lights of every zone not put away). The world load does the same at src/engine/world_load.rs:156-163, :172-179.
- **Guests.** A guest's home is put away and every plot is a neighbour's. `guest_stand` / `on_plot_ground` / `in_corridor` are at src/engine/home_plot.rs:394-425; `over_plot` is at :1141.

### 1.2 What the relay tells a game about other plots, and what a game can know of another player's home
- **The ship file.** It is identical on both sides: the welcome carries `ship` and a join with another hash is refused (src/relay/handlers/msg_handlers.rs:3281-3295). So every game knows every plot's id, kind, box and door corridor.
- **Who holds which plot.** Never sent:
  - the welcome's `home_plot` is the joiner's own (msg_handlers.rs:3289);
  - another player's entity carries only `name`, `appearance` and `key` (src/relay/handlers/game_interest.rs:74-75; `components_seen_by_others` :81-113, the increment 4 P7 rule).
- **The pieces the server keeps on that plot.** Every frame within 250 m comes into view and leaves past 300 m (relay shared_build.rs `rejudge` :202-217, `on_join` :699, `on_move` :710, `snapshot_messages` :614, `built_messages` :580). The game puts each frame's list into its world (src/engine/shared_build.rs `replace_frame` :655-678, `spawn_piece` :612-624). The pieces are drawn by src/engine/planet_build.rs:456 `push_render_objects` and are solid (src/engine/build_place.rs:747 `built_piece_segments`). So a neighbour's server-kept walls already stand, unseen, inside its closed shell in every game within 250 m.
- **Not known and not sent:**
  - the holder's own home design (their data/homes/<kind>.ron, which they may have edited offline in Dev);
  - their machines, animals and plants;
  - their private pieces (blueprints not marked `shared`, kept in their save);
  - their placed lights.
  - The operator decided on 2026-10-04 that server worlds start every player fresh and only appearance crosses (docs/PRIORITIES.md:171-172). That is not built (increment 6), so today the holder's own game still draws their offline home on the plot.
- **What is missing to draw the home "as the shared world knows it".** Nothing on the wire. A way in (walking collision, doors), light inside (a neighbour has no rooms, so no lights) and the permit wiring are missing.

### 1.3 Permits today
- **Contract.** src/systems/construction/shared.rs:
  - `PERMIT_MAX_DAYS = 90` :84-88;
  - the `Why` codes and words for permits :289-351 (`PermitExpired`, `PermitNotFromHolder`, `PermitBad`, `PermitTooLong`, `NotYourPiece`);
  - `Permit {issuer, server, plot, grantee, expiry, sig}` :467-491 and `Ranks` :493-504;
  - `ToRelay` :562-594 (`permit` on Build/Unbuild);
  - `FromRelay` :596-659;
  - the `msg` type strings from :445.
- **Crypto.** src/relay/core/pq_crypto.rs:
  - `plot_permit_preimage` :527-529;
  - `build_plot_permit` :569-584, whose minting refuses what the check refuses;
  - `verify_plot_permit` :594-612, with one day of slack on "too long";
  - tests :1274-1395 (pinned preimage, one server only, at most 90 days).
- **Relay checks.** src/relay/handlers/shared_build.rs:
  - `PieceBook.server_did` :106-108 is loaded in `load_pieces` :244-248;
  - `Asker` :283-301, `ranks_of` :314-316;
  - `may_build` :325-349 and `may_remove` :358-388;
  - `permit_lets_in` :396-409: the signature over the relay's own facts first, then `plot_holder`;
  - field caps `PERMIT_ISSUER_MAX_LEN` / `PERMIT_SIG_MAX_LEN` :83-86 and `read_request` :824-858;
  - `handle` :795-802; relay.rs:3792-3796 routes `game_build | game_unbuild | game_pieces_request` there.
  - The end-to-end relay test is src/relay/features.rs:4151 `a_household_permit_lets_its_holder_build_and_take_down_only_their_own`.
- **Game side.** src/engine/shared_build.rs:
  - `gate` :122-182 (comment :136: "Household permits (increment 5b) will open that row"), `gate_here` :186-204;
  - `take_down_gate` :215-232 (5b comment :219);
  - `SharedBuild` :294-333;
  - `on_game_message` :451, `on_welcome` :459, `tick` :468, `is_ours` :493-495, `receive` :511-565;
  - `welcome` :893-907, `leave` :915-936, `tick_core` :939-970, `send_next` :1036-1098, `probe_json` :1203-1220.
  - E's and F's routes: src/engine/build_place.rs `press_build_on` :138-143, `gate_here` call :351, `placing_hint` :538-560, `take_down_entity_on` :642-653, `place_at` :178, `take_down_piece` :664.
- **The scripted walker mints permits.** scripts/second-player.js: `permit` command :223-225 and :1140-1147, `parsePermitJson` :1085, `mintPermit` :1320, `didFor` :1360-1361, the relay's did :1329-1337. CLAUDE.md:525 still says the game neither mints nor sends permits.

### 1.4 DM control notes: built, sealed, ingested, verified, stored
- **Shapes.** src/net/dm_pq.rs:
  - `DmInner {from, to, ts, text, sig_b64, cert}` :181-202; `cert` is not covered by `sig`, but `text` is;
  - the markers `CTL_FOLLOW`, `CTL_UNFOLLOW`, `CTL_FRIEND_CERT`, `CTL_BLOCK`, `CTL_UNBLOCK`, `CTL_CHOICE` :204-218;
  - `DM_AES_DOMAIN = "hum/dm-aes/v1"` :46;
  - `seal` :83-109 (ML-KEM-768, then BLAKE3 derive_key, then AES-256-GCM), `open` :113-142;
  - `build_signed_inner_ext` :347-383 (Dilithium3 over `hum/dm/v2\n{from}\n{to}\n{ts}\n{text}`, padded to `DM_PAD_BUCKETS` [256, 1024, 4096, 16384] :386);
  - `seal_v2` :390, `parse_verify_inner` :421-443.
- **Ingest.** src/engine/dm.rs `ingest_dm` :223-303 screens in this order:
  1. block :230;
  2. choice notes :235;
  3. the three controls :238-244;
  4. contact requests :248;
  5. group reports :255;
  6. the reach screen `file_if_refused` :258;
  7. device-to-device file notes :264;
  8. only then `store.insert`.
- **Acting on controls.** `ingest_control` :317-432: a pass is verified with `verify_friend_cert` against `store.pass_server()` at :397-398; the echo from my other devices is handled at :362-395.
- **Sending.** `control_puts_at` :469-486 builds the recipient copy, which carries `friend_cert` = the pass THEY gave me, plus the self-copy. `send_held` :557-570 adds a random `ref` and holds the self-copy until `dm_put_ok` (10l). `mint_pass` :575-595.
- **Server identity.** `note_server_did` :702-712 records `identify_challenge.server_did` (frame_ws_poll.rs:96).
- **Answers.** src/net/put_answers.rs (`new_ref` :30-34, `PendingPut` :47-63, `Answer` :66-91). src/engine/put_answer.rs `on_frame` :39-48 is reached from the pump's catch-all chain at frame_ws_poll.rs:1544; `expire` :51-55.
- **Store.** src/net/dm_store.rs: one encrypted file per (identity, server URL) (`load` :278-312, `save` :315-330); `pass_server` / `set_pass_server` :461-481, which clears every pass when the server's identity changes; `is_friend` :455-457.
- **Website.** The desktop's controls are mirrored in web/chat/crypto.js:904-914. The website swallows notes in web/chat/chat-social.js `ingestDmControl` :1022-1035, called from web/chat/app.js:1089 (live `dm_new`) and :1164 (mailbox `dm_batch`), after `blockScreenDm` and before `reachScreenDm`. An unknown `[[hum:...]]` text falls through to `hosDmStore.insert` and is shown as a message. app/web/ is regenerated from web/ by scripts/bundle-web.js.

### 1.5 Friends on the desktop
- **Friendship is a mutual follow** in the DM store (`following` and `followers` sets, dm_store.rs:68-73, `is_friend` :455-457), mirrored into `GuiState::chat_friends`. That mirror is filtered to members online on this server (src/gui/mod.rs:711-714; `refresh_social_mirrors` dm.rs:887-903).
- **Names** come from `dm_display_name` dm.rs:70-87.
- **A DM can be sealed** only to someone whose Kyber key the app has seen (`peer_kyber_keys`, filled from the member list, frame_ws_poll.rs:313-317, :348-351).
- **Contact requests.** src/engine/reach.rs: `contact_request_in` :195-232, `accept_request` :355-378, which stores their pass, follows back and sends ours.

### 1.6 Which page the household page hangs off
- **The Home page.** `GuiPage::Homes` (src/gui/mod.rs:466-470, "server/real homes come later"), "Home" in the nav (src/gui/pages/escape_menu.rs:169-176), dispatched at src/gui/dispatch.rs:67, drawn by src/gui/pages/homes.rs `draw` :207-250.
- **Precedent:** shared-world sections live inside existing pages, as Inventory > The fleet does (src/gui/pages/inventory.rs:1682-1686, `fleet_ledger::draw_section`).
- **No new `GuiPage` variant.** gui/mod.rs is at 4,799 of 4,800 lines, and `page_registry_lint` would need docs/PAGES.md.
- **Patterns to follow:** page-local draft state in a `thread_local!` (homes.rs:62, inventory.rs:37); headless page tests (src/gui/pages/fleet_ledger_tests.rs:224-247); snapshots (src/gui/ui_snapshots.rs:1640-1660).

### 1.7 Constraints
- **Line budgets** (tests/file_size_ratchet.rs:376-417), measured today:

  | File | Lines | Budget |
  |---|---|---|
  | src/lib.rs | 16,476 | 16,700 |
  | src/gui/mod.rs | 4,799 | 4,800 |
  | src/relay/relay.rs | 6,322 | 6,400 |
  | src/relay/handlers/msg_handlers.rs | 4,653 | 4,660 |
  | src/engine/frame_ws_poll.rs | 1,698 | 1,800 |
  | src/gui/state_types.rs | 1,836 | 1,850 |

  gui/mod.rs must shed lines before `GuiState` can gain a field.
- **BUG-133:** a data file with a built-in copy is read through `embedded_data::load_data_or_embedded` (src/embedded_data.rs:429-470), which calls `note_builtin_copy`.
- **Lints:** theme tokens only (theme_token_lint); no em dashes (emdash_lint); account words (terminology_lint).
- **Rigs:** one GPU and one game at a time (machine guard). The other rig modes stay green: `--plots`, `--build`.

## 2. Stale words this increment corrects

| Where | Says | Now |
|---|---|---|
| docs/PRIORITIES.md:235 | "a plot's holder gives and withdraws build permits" | Gives and renews. No permit can be withdrawn (decision 3, pq_crypto.rs:499-503) |
| CLAUDE.md:525 | "The game itself neither mints nor sends one yet ... `just verify-shared-build` has no permit leg yet" | The game mints, sends, holds, checks (`game_permit_check`) and attaches them; proof `just verify-household` |
| ship-homes-and-logistics.md:418 | "A household member is simply a permit with no expiry" | No endless permits; 90 days at most, renewable |
| ship-homes doc section 7, increment 5 Files | "src/net/dm_pq.rs (permit building, beside build_friend_cert)" | Minting is in pq_crypto.rs; dm_pq.rs gets only the marker |
| increment-5 plan 1.6 and 3B | dm.rs:147-153/198-244/249-306, app/web/chat/crypto.js:783-809, chat-social.js:459 | dm.rs:223-303/317-432/469-486; edit web/chat/crypto.js:904-914 and chat-social.js:1022-1035 only (bundle regenerates app/web) |
| src/engine/shared_build.rs:136, :219-220; shared_build_tests.rs:265, :286 | "Household permits (increment 5b) will open that row"; "no permit until 5b" | Opened (section 3.8) |

## 3. The design

### 3.1 The permit note, exactly
- **Marker:** `pub const CTL_PERMIT: &str = "[[hum:permit:v1]]";` in src/net/dm_pq.rs beside `CTL_CHOICE`, mirrored in web/chat/crypto.js.
- **Text:** `[[hum:permit:v1]]` then, with nothing between, the contract's `Permit` serialized by serde_json in field order:
  ```
  [[hum:permit:v1]]{"issuer":"<giver's Dilithium3 key, hex>","server":"did:hum:<relay>","plot":"p2","grantee":"did:hum:<friend>","serial":"<32 hex>","expiry":1765400000,"sig":"<base64 over hum/permit/v2\n{server}\n{plot}\n{grantee}\n{serial}\n{expiry}>"}
  ```
  About 8.4 KB, so the inner pads to the 16,384 bucket. The relay's envelope limit is 131,072 (msg_handlers.rs:703).
- **Why the text and not `DmInner.cert`:** the text is covered by the DM's own signature, the way every note since 10j is.
- **Reading it:** `net::household::parse_note(text) -> Option<Permit>`.
  - It refuses any of: a non-hex issuer over 8,192 characters, a sig over 8,192, ids that are empty, over 128 or hold a line break, or a body over 16,384.
  - `is_note(text)` is "starts with the marker", so a malformed note is still never shown.
- **Who the desktop accepts it from** (`engine::household::ingest`). It is called in `ingest_dm` right after the controls :238-244, so after block and choice screening and before contact requests and the reach screen:
  - **From me** (`inner.from == me`): the self-copy of a permit I gave `inner.to`. It is recorded as given (section 3.2) after the same signature check.
  - **To me from a friend** (`store.is_friend(from)`), checked by `net::household::judge_arrival(p, sender, store.pass_server(), my_did, now_s)`:
    - `p.issuer == inner.from` (the DM's checked sender), else `NotFromItsIssuer` (a forwarded permit);
    - `p.server == pass_server`, else `OtherServer`;
    - `p.grantee == plot_owner_id(my key)`, else `NotForMe`;
    - then `verify_plot_permit(inner.from, pass_server, p.plot, my_did, p.expiry, p.sig, now_s)`, giving `RanOut`, `TooLong` or `DoesNotCheckOut`;
    - no server identity known yet gives `NoServerYet`. That cannot happen in practice: mail is fetched after `identify_challenge`.
    - Kept, it raises the notice in section 3.10 and asks for a relay check if in session.
  - **From anyone else:** dropped with one log line naming 12 characters of the key, never stored or shown. The page gives permits only to friends, and a stranger's permit would only fill the list.
- **Every case returns "consumed"**, so no permit note ever reaches `store.insert`.

### 3.2 What is stored per server
In `DmStore`'s encrypted body (one file per identity and server URL), new fields with `#[serde(default)]`:

```rust
permits_held: Vec<HeldPermit>,    // one per (from, plot): the later expiry wins
permits_given: Vec<GivenPermit>,  // one per (to, plot): the later expiry wins

pub struct HeldPermit { pub from: String /* giver's key hex = permit.issuer */, pub permit: Permit, pub at: u64 /* DM ts, ms */ }
pub struct GivenPermit { pub to: String /* friend's key hex */, pub plot: String, pub expiry: u64, pub at: u64 /* ms */, pub state: GivenState }
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GivenState { OnItsWay { reference: String }, Taken, Unconfirmed, Refused { reason: String } }
```

- **Limits:** at most 64 held and 256 given (constants in net/household.rs). Past the cap, the earliest-ended rows go first.
- **Pruning:** `prune_permits(now_s, ended_kept_days)` drops rows ended more than `ended_kept_days` (30) ago. It runs on every save that touches permits.
- **Server identity changes:** `set_pass_server` clears both lists with the passes, because they name the old identity.
- **On restart:** an `OnItsWay` row whose send is no longer in memory reads as `Unconfirmed`.
- **Session state** (never saved) lives in a new `GuiState::household: net::household::Household`:
  - `my_plot: Option<String>`: the plot this session's welcome gave, else None;
  - `verdicts: BTreeMap<PermitKey{from, plot, expiry}, Verdict{Checking | Works | Refused(Why) | Later}>`, with the time each was asked;
  - `usable: BTreeSet<String>`, `walkable: BTreeSet<String>`, `lit: Option<String>`;
  - `asked: BTreeMap<u32, (PermitKey, f64)>`, `last_req`, `next_check_at`;
  - `on_their_way: Vec<PermitPut{reference, peer, plot, expiry, days, self_copy: Value, at: Instant}>`;
  - `line: String`: the page's last result.
- **Lengths** come from a new data/ship/household.ron (`lengths_days: [7, 30, 90]`, `default_days: 30`, `ended_kept_days: 30`). It is read with `load_data_or_embedded` and its built-in copy is in embedded_data.rs.

### 3.3 Giving and renewing
`engine::household::give(gs, peer, days) -> Result<(), String>`, called from the page's Give button.

- **Refusals**, each with its sentence:
  - identity locked;
  - not connected;
  - no `pass_server`;
  - `gs.household.my_plot` is None;
  - `peer` is not a friend, or is blocked;
  - their Kyber key is not known;
  - a permit to them is still on its way;
  - `days` is not in 1..=90.
- **Mint:** `build_plot_permit(seed, pass_server, my_plot, plot_owner_id(peer), now_s + days*86400, now_s)`.
- **Send:** `control_puts(gs, peer, &note_text(&permit), None)` builds the copy for them, which carries `friend_cert` = the pass they gave me, plus a self-copy. Their copy gets a `ref` from `put_answers::new_ref()` and goes out at once. The self-copy is held in `on_their_way`.
- **Record:** `GivenPermit{state: OnItsWay}` is recorded and saved.
- **Answers** (`household::on_answer(gs, &Answer) -> bool`, the first line of `put_answer::on_frame`):
  - `dm_put_ok`: the row becomes `Taken` and the self-copy goes out;
  - `dm_put_refused {reason}`: the row becomes `Refused{reason}` and the self-copy is dropped;
  - no answer in 30 s (`household::expire`, called from `put_answer::expire`): the row becomes `Unconfirmed` and the self-copy is dropped.
  - Permits stay out of `pending_puts`, so they never hold up the pass sweep.
- **Renew** (`household::renew(gs, peer)`) is `give` again for the same friend and the same length (`round((expiry - at/1000)/86400)`, clamped to 1..=90), counted from today. It is refused with a sentence when the row's plot is no longer `my_plot`. The friend's app keeps only the later permit.

### 3.4 When a received permit is checked, and against what
1. **On arrival** (section 3.1): against the DM's checked sender, this store's server `did:hum`, my own `did:hum`, the clock, and the signature.
2. **By the relay, every session** (section 3.5): after each welcome; again 300 s after its last answer while in the shared world; at once when a new one arrives in session. Against the relay's own server identity, the plot as this ship has it, the asker's own `did:hum`, its clock, and `plot_holder` now.
3. **By the relay, on every `game_build` and `game_unbuild`**, as today. A refusal with a `permit_*` why also sets that permit's verdict.

### 3.5 The relay's permit check (new, exact)
Contract (shared.rs; `msg::PERMIT_CHECK = "game_permit_check"`, `msg::PERMIT_STATUS = "game_permit_status"`):
```rust
#[serde(rename = "game_permit_check")]  PermitCheck { req_id: u32, permit: Permit },
#[serde(rename = "game_permit_status")] PermitStatus { req_id: u32, plot: String, ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")] why: Option<Why>,
    #[serde(default, skip_serializing_if = "is_false")] again: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")] message: String }
```

Relay (`shared_build::permit_check`, a new arm in `handle`; relay.rs:3793's pattern gains `| "game_permit_check"`). Cheapest step first:
1. Rate: `perception_rate_allows(state, key, "permit")`, its own 200 ms bucket. Too soon gives `again`.
2. In the world (`Asker::in_world`), else `again`.
3. `read_request`, with the existing permit field caps. A bad shape gives `ok: false, why: permit_bad`.
4. The plot is a plot frame of this ship (`plot_frame_id(&p.plot)`, kind Plot), else `permit_bad`.
5. The asker's own plot gives `ok: true`.
6. `permit_lets_in(book, db, ship_id, plot, &who, &p, now_s)`. This rebuilds the words from the relay's facts and checks the signature BEFORE the holder, so a permit not made out to the asker is always `permit_bad`. It never says anything about who holds a plot.
7. `storage_error` gives `again`.

- **The answer** goes to the asker alone (`send_to`), with `message = why_words(...)`.
- **Log line, never a key:** `Game: permit for {plot} checked (ok)` or `(not_allowed/<why>)`.
- **Nothing is stored.**

Game (Wave 2A): `household::tick` sends checks at most one per 250 ms (`shared::SEND_INTERVAL_MS`). A check unanswered for 10 s is sent again. `on_status` sets the verdict: `again` sets `Later` and the check is asked again after 5 s.

### 3.6 Running out, renewing, and why nothing can be withdrawn
- **At expiry** (the game's own clock against `expiry`; the relay rules on builds by its own clock):
  - the plot leaves `usable`, so E on it refuses with `PermitExpired`'s words before anything is spent;
  - it leaves `walkable` only once the player is off that plot and out of its corridor (section 3.7), so nobody is ever walled in;
  - a build already sent is judged by the relay and refunded on `permit_expired`, as today;
  - pieces the friend built stay; the holder can take them down, the friend no longer can;
  - both lists say "ran out on <date>" for 30 days, then the row goes.
- **Renew** is one press on the holder's row (section 3.3).
- **Withdrawal is impossible.** The relay keeps no list of permits, so a permit cannot be ended before its date.
  - The page's own words (section 3.9): "There is no way to take a permit back before it runs out: the server keeps no list of permits, so it cannot cross one off. Give a short one when you are not sure, and renew it with one press."
  - A given row for someone the person has since blocked says the permit still runs to its date, and that the holder can take down anything they build.

### 3.7 Walking in: open plots
- **The field.** `ShipStructure` gains `#[serde(skip)] pub open: OpenPlots` with `OpenPlots { plots: BTreeSet<String>, lit: Option<String> }`: "plots this game may walk into besides its own, and the one it lights (ship homes 5b, engine/household.rs); never saved, never in the hash". It is not in `NeighbourKey`, so opening rebakes nothing.
- **Readers** (all existing call sites pick these up unchanged):
  - `ship_segments_impl`, walking path only: for each neighbour whose plot is in `open.plots`, add its zone cut to the zone it runs to and its `segments(false)` (the home's walls with its door open, windows solid, interior doorways open, and its corridor side walls);
  - `neighbour_panel_placements`: `mouth_pair(m, others_only = !open.plots.contains(plot))`;
  - `home_lights`: when `open.lit` names an open neighbour, add its design's placed lights (`design.body.lights`, about 35 for the homestead) offset by the plot origin. Zone lights only reach `lighting_watts` (ship_structure.rs:699), so no meter is charged.
- **Who decides.** `engine::household_world::apply_open(state)` runs once a frame after `shared_build::tick_core`:
  - `walkable = usable ∪ ({plot under the camera or in its corridor} ∩ last frame's walkable)`. The camera is used only while aboard (`home_plot::aboard`), through a new `home_plot::plot_ground_of(ship, p) -> Option<&Plot>`.
  - `lit = walkable ∩ {plot under the camera or its corridor}`.
  - When either differs from `ship.open`, it sets it and calls a new `home_meshes::open_plots_changed(state)`, which redoes `wall_colliders`, `rebuild_door_panels` (the count is unchanged, so open fractions are kept) and `room_lights`. Meshes, hull and sight are unchanged.
- **`usable`** = held permits whose plot is a plot of this ship (not my own), not run out, and whose verdict this session is `Works`.

| | Everyone else (no working permit) | The permit holder (working permit this session) |
|---|---|---|
| Shell | The kind's shipped design, opaque roof, no machines, no rooms | Same |
| Corridor doors (both ends) | Open for other players only | Open for this player too |
| Street or Commons wall at the corridor | Whole for walking (hole for the eye only) | Open |
| Corridor side walls, the home's walls | Sight only | Sight and walking |
| Light inside | None | The design's placed lights, while standing on the plot or in its corridor; nobody's meter |
| Server-kept pieces on `plot:<id>` | Drawn and solid when in view | Same, plus building and taking down your own |
| Air, room name on the HUD, machines | Ship's air, none, none | Same |

### 3.8 Building there
- **Gate.** `GateInput` gains `household: &'a net::household::Household` and `gate_here` passes `&gui.household`. For `FrameKind::Plot`, not the player's own, with a shareable piece:
  - in `usable` and `box_inside_frame` holds: `Shared(frame)`; outside the box: "a piece on someone else's plot must stand wholly on it";
  - a held permit whose verdict is `Checking` or `Later`: "your household permit for this plot is being checked with the server; try again in a moment";
  - a held permit that ran out, or whose verdict is `Refused(why)`: that `why`'s words;
  - no permit: today's `NotYourPlot` / `Guest`.
- **Take-down gate.** `take_down_gate(kept, ship, ranks, usable)`: another plot in `usable` gives `Ok` if `kept.mine`, else `Err(NotYourPiece)`.
- **Sending.** `send_next` sets `permit: household::permit_for(cx.gui, plot)` (the held permit with the latest expiry for that plot) on a Build to, or an Unbuild of a piece in, `plot:<id>` that is not the player's own.
- **Placing hint:** `kept = "with your household permit, kept by the server: anyone near sees it"`.

### 3.9 The household page (Home > Household), rows and words
A collapsible card at the top of `homes::draw`, inside a ScrollArea of at most 360 px, drawn before the design (and also when no design loaded). Theme tokens only: `text_primary`, `text_secondary`, `text_muted`, `accent`, `success`, `danger`.

**Intro** (muted): "A household permit lets a friend build on your plot on this server, and walk into your home, until the date you choose. Permits are kept on your devices; the server checks each one when it is used."

**Your plot**
- Locked: "Unlock your identity to see your household permits." Not connected: "Connect to a server to see its household permits."
- Not in the shared world: "Step into this server's shared world to give permits: the server says which plot is yours when you arrive."
- Guest: "You have no plot on this ship (the server had none free), so there is nothing to give a permit for."
- In session: "Your plot on this server: p2."
- One row per friend on this server (mutual follows from the DM store, so offline friends too, by `dm_display_name`, sorted). The row shows one of:
  - "Can build on your plot until 10 November 2026 (29 days left)." [Renew for 30 days]
  - "Sending the permit..."
  - "The server did not say whether Bo got the permit. Give it again to be sure." [Give again]
  - Refused: "Bo's settings do not take messages from you right now." (`reach`); "The server is busy; try again in a moment." (`rate`); "The server did not take it; try again." (other)
  - "Their permit ran out on 10 November 2026." [Give a new one]
  - "Bo has not been online on this server since you started the app, so a permit cannot be sealed for them yet."
  - Otherwise the lengths from the data file as choices ([7 days] [30 days] [90 days], the default picked) and [Give a permit].
- Confirm (inline under the row): "Give Bo a household permit for your plot p2 until 10 November 2026? Bo can build there and take down only what Bo builds. You can take down anything on your plot. A permit cannot be taken back before it runs out." [Give] [Cancel]
- A given row for an old plot: "For plot p3, which is no longer yours here: it does not work any more."
- A blocked person: "You blocked Bo. The permit you gave runs until 10 November 2026; you can take down anything Bo builds."
- No friends: "No friends here yet: people you follow who follow you back on this server are listed here."
- Note (muted): the no-take-back sentence from section 3.6.

**Plots you may build on**
- Row heading: "Ada's plot p1, until 10 November 2026 (29 days left)." Then the status line:
  - Works, in session (success): "The server says it works: plot p1's door opens for you, and you can build there and take down what you build."
  - "Checking it with the server..."
  - Not in session: "It works in this server's shared world."
  - Not from holder (danger): "It no longer works: Ada does not hold plot p1 any more."
  - Bad or too long (danger): "It does not check out. Ask Ada for a new one."
  - "Ran out on 10 November 2026. Ask Ada for a new one."
  - "Plot p1 is not on this ship any more."
- None: "Nobody has given you a permit on this server."
- Dates use `net::household::date_words(unix_s)` ("10 November 2026"), through `crate::relay::epoch_days_to_ymd` (src/relay/mod.rs:275).
- The page's draft (picked friend, picked length, the open confirm) lives in a `thread_local!` in pages/household.rs.
- Never written on the page: certificate, signature, key, token, revoke, verify.

### 3.10 Notices and crosshair words
- **Received:** "Ada gave you a household permit for plot p1 until 10 November 2026. Home > Household lists it."
- **A check says no:** "Your household permit for plot p1 no longer works: Ada does not hold that plot any more." (or that `why`'s words).
- **Crosshair:** section 3.8. Take-down refusals reuse shared.rs's `NotYourPiece` / `NotYourPlot` sentences.

### 3.11 Nobody is told who lives on which plot: how it holds
- **No game message changes for anyone else.** `game_built` and `game_pieces` still carry no builder, only `mine` to the builder. `components_seen_by_others` is untouched (game_interest.rs:81-113). The welcome still names only the joiner's own plot.
- **The only person who learns where the giver lives is the friend they chose to tell.** The note is sealed end to end; the relay sees one more `dm_put` to the friend's key, as for any DM.
- **The permit check answers the asker only, about permits made out to the asker.** The signature is checked over the asker's own `did:hum` before `plot_holder` is read, so "permit_not_from_holder" can only ever be learned about a real permit from that giver. It never names a holder, and the log line carries no key.
- **Third parties see nothing new.** A permit holder walking into a plot looks exactly like the holder walking home: in a third player's game that corridor door opens for any other player. Positions were always visible, and the relay never says why anyone is where they are.
- **The open set is local and never sent:** `ShipStructure::open` is `serde(skip)` and outside the hash.

### 3.12 The website
It has no game, so it holds and shows no permits (documented as the dual-UI exception).
- web/chat/crypto.js gains `const CTL_PERMIT = '[[hum:permit:v1]]';` beside `CTL_FRIEND_CERT`.
- web/chat/chat-social.js `ingestDmControl` gains a first check: `if (typeof inner.text === 'string' && inner.text.startsWith(CTL_PERMIT)) return true;`. It sits before the choice-note check, so the note is consumed live and from the mailbox (app.js:1089, :1164) before `reachScreenDm` and `hosDmStore.insert`. It is never stored, listed, notified or turned into a request.
- Known limit, in the docs: someone who reads mail only on the web for longer than the mailbox lifetime (30 days) never gets the permit on the desktop.

## 4. Questions only the operator can answer (each with a recommendation)
1. **Taking a permit back, Block included.** Today a permit runs to its date even after Block, because the server keeps no list (decision 3). Recommend: ship 5b as decided (the page says so, and a blocked person's row says when their permit ends). Make withdrawal the next household step: a permit that names a serial, withdrawn through the relay's existing withdrawal list as friendship passes have been since 10b (about a day; a v2 permit format).
2. **What a permit holder sees of the holder's home.** It is the start's design plus what was built through the server, not the holder's own offline edits, machines, garden or private pieces. Recommend: accept for 5b. The two become the same home once server worlds start every player fresh, which you decided on 2026-10-04 and which increment 6 builds.
3. **Lights while a friend visits.** The visited plot's lights are on only while the permit holder stands in it, and charged to no one's power meter. Recommend: accept. No one's bill should rise because a friend walks in; metering the ship's own lighting is already a logged follow-up.

## 5. Waves

**Ownership.** Each shared file belongs to exactly one wave:
- src/gui/mod.rs: Wave 0
- src/relay/relay.rs and src/relay/features.rs: 1C
- CLAUDE.md: 1A
- src/engine/ipc.rs, src/engine/home_plot.rs and src/gui/connections.rs: 2A
- Justfile and scripts/verify-copresence.js: 3A
- Nobody: src/lib.rs, msg_handlers.rs, game_state.rs, net_route.rs, frame_ws_poll.rs, config.rs

Edited by two waves in sequence, never in parallel:
- src/relay/handlers/shared_build.rs (0 stub, then 1C)
- src/engine/shared_build.rs (0 stub, then 2A)
- src/engine/household.rs and household_tests.rs (1A, then 2A)

**Budgets after all waves:**

| File | Lines | Budget |
|---|---|---|
| lib.rs | 16,476 (+0) | 16,700 |
| relay.rs | 6,322 (one line changed) | 6,400 |
| msg_handlers.rs | 4,653 (+0) | 4,660 |
| gui/mod.rs | about 4,684 (-121 moved, +6) | 4,800 |
| frame_ws_poll.rs | 1,698 (+0) | 1,800 |

**Rule for every test below:** it is red on the code before its wave with the named break, and its comment records the failure text, as every test here does.

### Wave 0: the contract and the room (one builder, merges first)
Files:
- src/gui/mod.rs: move `donate_funding_tests` (:198-273) and `ui_click_sound_tests` (:4749-4799) verbatim to new src/gui/donate_funding_tests.rs and src/gui/click_sound_tests.rs via `#[cfg(...)] #[path] mod`. Add `pub household: crate::net::household::Household` beside `fleet` (:2613) and its default (:4061).
- src/systems/construction/shared.rs: the two messages and msg constants.
- src/relay/handlers/shared_build.rs: a `read_request` arm for `PermitCheck` with the permit caps only.
- src/engine/shared_build.rs: a stub arm `FromRelay::PermitStatus { .. } => {}` in `receive`.
- src/net/dm_pq.rs: `CTL_PERMIT`, and its line in the `DmInner.text` doc.
- NEW src/net/household.rs and src/net/mod.rs: the note, `HeldPermit` / `GivenPermit` / `GivenState`, `PermitKey`, `Verdict`, `Household`, `judge_arrival`, keep and record rules, `prune`, `usable`, `date_words`, `Lengths` / `load_lengths`.
- src/net/dm_store.rs: the two fields, `permits_held`, `keep_permit`, `permits_given`, `record_given`, `set_given_state`, `prune_permits`; `set_pass_server` clears them.
- src/ship/ship_structure.rs: `OpenPlots` and the `open` field.
- NEW data/ship/household.ron; src/embedded_data.rs (const and match arm).

Tests:
- shared.rs `the_permit_check_and_its_answer_read_as_the_wire_says`: exact JSON, `why` / `again` / `message` absent when empty. Red with `req_id` renamed in PermitStatus.
- net/household.rs `the_permit_note_round_trips_and_a_bad_one_is_still_a_note`: red with the marker spelt `[[hum:permit]]`.
- net/household.rs `a_permit_is_kept_only_from_its_giver_for_me_on_this_server`: eight rows (good; forwarded; other server; other grantee; ran out; 92 days; bad signature; no server yet). Red with the issuer-equals-sender line removed: "a permit Cy forwarded from Ada was kept".
- net/household.rs `a_later_permit_replaces_and_an_earlier_one_changes_nothing`: red with the comparison reversed.
- net/household.rs `ended_permits_leave_both_lists_after_the_kept_days`: red with `prune` skipping the given list.
- net/household.rs `the_lengths_are_data_and_none_runs_past_90_days`: disk and built-in copies parse; each in 1..=90, sorted, default among them. Red with 120 added in a temp copy.
- dm_store.rs `permits_are_kept_per_server_and_a_new_server_identity_drops_them`: red with `set_pass_server` not clearing them.
- ship_structure.rs `the_open_plots_are_never_part_of_the_ship`: hash and ship file text equal with p1 open and lit. Red with the `serde(skip)` removed.
- The moved test modules run with the same counts (`cargo test donate_funding_tests ui_click_sound`).

### Wave 1 (five lanes in parallel)

**1A Permits travel (desktop DMs).**
Files:
- NEW src/engine/household.rs: `ingest`, `give`, `renew`, `on_answer`, `expire`, `friends(gs)` rows for the page, `permit_for`.
- NEW src/engine/household_tests.rs.
- src/engine/mod.rs.
- src/engine/dm.rs: one call in `ingest_dm` after :244.
- src/engine/put_answer.rs: `household::on_answer` first in `on_frame`; `household::expire` in `expire`.
- CLAUDE.md: the "Plot permits" row (the game mints, sends, holds, checks, attaches; the note; the website swallows; `just verify-household`) and the file map.
- NEW scripts/tests/fixtures/permit-note-v1.json: written by an ignored Rust test from fixed seeds. It holds the permit, the note text and one Rust-sealed envelope with its recipient seed.

Tests:
- `giving_seals_one_note_to_the_friend_and_counts_only_once_the_server_took_it`: two GuiStates with DM stores; one `dm_put` to Bo with `friend_cert` and `ref`, nothing `Taken` until `dm_put_ok`, then the self-copy goes. Red with `Taken` recorded at send.
- `a_received_permit_is_verified_and_kept_per_server` (the plan's name): red with `judge_arrival` bypassed (a forged permit kept).
- `a_permit_from_someone_not_a_friend_is_dropped_and_never_shown`: red with the friend check removed.
- `my_permit_echoed_from_my_other_device_is_listed_as_given`: red with the from-me arm missing.
- `giving_needs_the_plot_this_session_and_a_sealable_friend`: no plot, no Kyber key, blocked, one on its way, 0 or 91 days. Red with the `my_plot` check removed: "a permit was minted for a plot the server never gave".
- `renew_gives_the_same_length_counted_from_today`: red with the old expiry reused.
- `a_refused_or_unanswered_permit_says_so_and_sends_no_self_copy`: red with the self-copy sent on refusal.
- `ingest_dm_hands_a_permit_note_to_the_household_before_anything_is_stored`: red without the dm.rs call (the note is stored as a message).
- `the_rigs_permit_note_fixture_is_the_games`: red with the marker changed.

**1B Walking in (ship and render).**
Files:
- src/ship/neighbours.rs: open queries on `NeighbourView` (zone cuts, segments and mouths by plot; design lights by plot).
- src/ship/wall_collision.rs: the walking path.
- src/ship/door_panels.rs.
- src/engine/home_meshes.rs: `home_lights` adds `open.lit`; new `open_plots_changed`.

Tests:
- wall_collision.rs `an_open_plot_lets_its_permit_holder_walk_in_and_a_closed_one_does_not`: `resolve` walks from the Commons (70, 1.7, 40) to (40, 1.7, 40) on the ship assembled on p2. Open p1 ends within 0.1 m of the target; closed stops at x 65 or more. Red with the zone cut not added.
- door_panels.rs `an_open_plots_corridor_doors_open_for_this_player`: only p1's two pairs flip. Red with the flag left `true`.
- home_meshes.rs `the_open_plot_you_stand_in_is_lit_and_on_nobodys_meter`: lights grow by the shipped design's lit count; `lighting_watts` is unchanged. Red with the lights not added.
- neighbours.rs `opening_a_plot_rebakes_no_neighbour`: `views_made` unchanged. Red with `open` put in `NeighbourKey`.
- The existing neighbour, door and collision tests stay green: closed by default.

**1C The relay's permit check.**
Files:
- src/relay/handlers/shared_build.rs: `permit_check`, and the `handle` arm.
- src/relay/handlers/shared_build_tests.rs.
- src/relay/relay.rs: the :3793 pattern, the same line count.
- src/relay/features.rs.

Tests:
- shared_build_tests.rs `a_permit_check_answers_as_a_build_would`: truth table (works; ran out; too long; giver gave the plot back; another grantee; another server; another plot; your own plot). Red with `permit_lets_in` returning Ok.
- features.rs `the_relay_says_whether_a_permit_works_and_tells_nothing_else`, on a real relay:
  - Ann on p1, Bo on p2, Ann's permit for Bo works, and no row or message reaches Ann;
  - after an admin releases p1 it answers `permit_not_from_holder`;
  - two checks within 200 ms gives `again`, and so does a check before joining;
  - no `relay.log` line carries a key.
  - Red with the holder check skipped: "after Ann gave p1 back, Bo's permit still works".
- features.rs `a_permit_check_never_answers_about_someone_elses_permit`: Bo shows Cy's permit and gets `permit_bad`. Red with the holder read moved before the signature: "permit_not_from_holder told about Cy's permit".

**1D The website.**
Files: web/chat/crypto.js; web/chat/chat-social.js; scripts/tests/reach-web.test.js (a new numbered case in its existing harness).

Test: the marker equals `CTL_PERMIT` read from src/net/dm_pq.rs. A permit note from a friend, live and from the mailbox, is not stored, listed or notified. One from a stranger is not listed under Requests. Red with the chat-social.js line removed: "the permit note is stored as a DM".

**1E The rig's DM library** (writes now, merges after 1A for its fixture).
Files:
- NEW scripts/lib/sealed-dm.js: `signedInner` with the padding buckets, `seal` (noble `ml_kem768`, `blake3` derive_key "hum/dm-aes/v1", node AES-256-GCM), `open`, `verifyInner`.
- scripts/second-player.js:
  - keep `kyber_public` from `peer_list` / `peer_joined`;
  - open `dm_new` and log `dm: friend pass from <key12>`, `dm: follow from <key12>`, `permit received: {json}` and `permit checks out` (its own Dilithium check);
  - stdin `befriend <key>`: a contact request built with web/shared/friend-pass.js and web/shared/reach.js;
  - stdin `give-permit <key> <plot> <days>`: mints with `mintPermit` and sends with `friend_cert` = their pass.
- scripts/tests/second-player.test.js.
- NEW scripts/tests/sealed-dm.test.js.

Tests:
- sealed-dm opens 1A's Rust-sealed fixture and checks its inner signature. Red with the AES domain misspelt.
- Seal and open round trip; buckets equal dm_pq.rs's, read from the source.
- second-player parses `befriend` and `give-permit`, and its note text equals the fixture's. Red before each exists.

### Wave 2 (two lanes, after Wave 1 merges)

**2A The game's glue.**
Files:
- src/engine/household.rs and household_tests.rs: `session_began`, `session_ended`, `tick` (checks), `on_status`, `apply_open`, `probe_json`.
- src/engine/shared_build.rs:
  - `is_ours` gains `PERMIT_STATUS`;
  - `receive` calls `household::on_status`;
  - `welcome` and `leave` call `session_began` / `session_ended`;
  - `tick_core` calls `household::tick` with the outbox; `tick` calls `apply_open`;
  - the gate rows, the `take_down_gate` rows, and the permit attached in `send_next`.
- src/engine/shared_build_tests.rs.
- src/engine/build_place.rs: `usable` passed at :653; the hint.
- src/engine/home_plot.rs: `plot_ground_of`; `in_corridor` made `pub(crate)`.
- src/engine/ipc.rs:
  - the probe's `household` beside `shared_build` (:3758): `me` (full public key), `me_did`, `server`, `my_plot`, friends, given, held with verdicts, `usable`, `walkable`, `lit`, `open_lights`, and p1 to p12's corridor `mouths` (`others_only`, open fraction);
  - showcase verbs `{"page":"homes"}` (via `dispatch::page_from_id`) and `{"accept_request":"<key>"}` (`engine::reach::accept_request`).
- src/gui/connections.rs: a server switch resets `household` (:268-281).

Tests:
- `the_gate_opens_a_plot_you_hold_a_working_permit_for` (rows: works, checking, ran out, not from holder, outside the box). Red with `usable` ignored.
- `take_down_gate_lets_a_permit_holder_take_down_only_their_own`: replaces the "no permit until 5b" rows. Red with `mine` ignored.
- `a_build_on_a_permitted_plot_attaches_its_permit` (the plan's name) and `a_take_down_of_my_piece_there_attaches_it_too`: red without.
- `each_held_permit_is_checked_after_a_welcome_and_the_answer_opens_its_plot`: red with `on_status` ignored.
- `a_plot_stays_walkable_until_you_step_off_it`: red with `walkable = usable`.
- `leaving_the_shared_world_closes_every_plot_but_the_one_you_stand_in`.
- net_route.rs `the_permit_status_reaches_the_household`: red with `is_ours` not listing it.
- ipc.rs `the_page_and_accept_request_verbs_parse` and `the_probe_reports_the_household`.
- connections.rs `a_server_switch_forgets_the_households_session`.

**2B The page.**
Files:
- NEW src/gui/pages/household.rs and household_tests.rs.
- src/gui/pages/mod.rs.
- src/gui/pages/homes.rs: one call at the top of `draw`.
- src/gui/ui_snapshots.rs: `snapshot_household`.
- tests/snapshots/household.png.

Tests:
- `the_household_card_in_words`: every row of section 3.9. Red with the no-take-back note removed.
- `no_security_words_on_the_card`: red with "certificate" added.
- `give_asks_first_and_the_confirm_gives`: a headless click on "Give a permit" shows the dated confirm, and "Give" makes one `dm_put`. Red with the confirm skipped.
- `the_lengths_come_from_the_data_file`: a temp data dir with [3, 14].
- The snapshot is rendered with `just snapshot household` and looked at.

### Wave 3
**3A The rig.**
Files:
- scripts/verify-copresence.js: `--household`. Lift `joinBuilder` / `waitFor` / `linesOf` out of `runBuildOnce` (:2972-3699) into a module-level walker pool both modes use; that move is proven by re-judging the last green `--build` manifest with `--dry-verdict`.
- NEW scripts/lib/household-judge.js and scripts/tests/household-judge.test.js.
- Justfile: a `verify-household` recipe beside `verify-shared-build` (:708-709); `rig-tests` (:628) gains sealed-dm.test.js and household-judge.test.js.

Test: a good synthetic run passes, and each doctored record fails exactly its own check.

**Docs (orchestrator, last).**
- docs/design/ship-homes-and-logistics.md: increment 5b "as built", and the stale lines of section 2.
- docs/design/shared-building.md: status.
- docs/PRIORITIES.md (:235, :270-273), docs/STATUS.md, docs/FEATURES.md.
- docs/PAGES.md: Home gains Household.
- docs/reference/retention_and_deletion_semantics.md: permits live only on devices; the relay keeps none.
- The journal.

**Checks per merge:**
- `just verify`;
- `cargo check --features relay --no-default-features`;
- `just validate-data` after Wave 0;
- `just verify-relay` after 1C;
- `just rig-tests` after 1D, 1E and 3A;
- serialized on the one GPU: `just verify-copresence --plots` (all orders) after 1B, and `just verify-shared-build` after 2A, because both touch what those rigs judge.

## 6. The in-game proof: `just verify-household`
`node scripts/verify-copresence.js --household [--dry-verdict <manifest>]`, with evidence in .probe-rig/copresence/runs/<stamp>-household/.

**ONE game, one scripted friend.** It never runs beside another rig or the operator's own game; the machine guard refuses it then.

1. **Setup.** The guard, `runFreshGate`, the Dev sandbox and cleared saved servers, as `--build`. The throwaway relay mirrors the tree's data/.
2. **Ada's plot.** Walker Ada (`TestBotAda`) joins first and gets p1. She builds a wood foundation and a wood wall on it at p1-local (48, 0, 36), the "what is really there". The game boots and joins: the probe shows it welcomed with `my_plot` p2.
3. **Friends.**
   - Ada sends `befriend <me>` (the probe's `household.me`); the game's notice names her request.
   - `{"accept_request":"<Ada>"}`.
   - Ada logs `dm: friend pass from` and `dm: follow from` the game; the probe's friends list her.
   - Judged `friends_made`.
4. **Shut before a permit.** The game walks (door route, `walk_to`) into the Commons and on toward p1's door point. Judged `closed_before_permit`:
   - the camera never passes the Commons' west wall at x 65 (it stands within 1 m of it);
   - the p1 mouths are `others_only: true`;
   - `walkable` and `open_lights` are empty and 0.
   - The rig then walks it back.
5. **The game gives, through its page.**
   - `{"page":"homes"}`; the `ui` find-and-click of "30 days", "Give a permit", then "Give".
   - `give_through_the_page`: the probe shows one given row to Ada (p2, `taken`, expiry within 120 s of now plus 30 days).
   - `give_arrived`: Ada logs `permit received` (plot p2, her did, the relay's did, issuer the game's key) and `permit checks out`.
   - `relay.log` has no line holding a key.
6. **Her permit works, and only for her own pieces.**
   - `{"stock":"1"}`; the game places a foundation on p2.
   - Ada builds on `plot:p2` with the logged permit: the relay built it, and the game's recorder shows it `mine: false` at p2's corner plus local within 1 cm (`give_lets_her_build`).
   - Ada's take-down of the game's foundation is refused `not_allowed/not_your_piece`.
   - The game takes down Ada's piece: Ada logs `saw unbuilt` (`her_pieces_only`).
7. **Renew.** A click on "Renew for 30 days": Ada logs a second permit for p2 with a later expiry, and the probe still has one row for her (`renew_one_press`).
8. **The game holds one.**
   - Ada sends `give-permit <me> p1 30`.
   - `held_and_checked`: a held row from Ada for p1; `relay.log` shows `Game: permit for p1 checked (ok)` within 2 s; the verdict is `works`; `walkable` is [p1]; the p1 mouths have `others_only: false`.
9. **Walks in.** The game walks to p1's door point and on to p1-local (48, 1.7, 44), facing north.
   - `walks_into_p1`: arrived inside p1's box, never corrected; Ada logged the game's moves as the relay passed them on.
   - `lit_inside`: `lit` is p1 and `open_lights` is above 0, where step 4 read 0.
10. **Sees what is there.** Picture `inside-p1.png`. `sees_whats_there`: Ada's wall's screen rect (recorder rows) is inside the view, and wood-tinted pixels are counted in it, the tint measured as `--build`'s step 7 measured it.
11. **Builds there.** `{"place":"wood_foundation@<p1 point>,0"}`. `builds_on_p1`: the relay built it on `plot:p1`; the game's row is `mine: true`; Ada logs `saw built` at the same pose within 1.5 s.
12. **Takes down only its own.** `takes_down_only_its_own`:
    - the game's take-down of Ada's wall is refused at the crosshair with `NotYourPiece`'s sentence (the probe's notices); no take-down line from the game reaches the relay; the wall is still drawn;
    - its own foundation comes down, Ada logs `saw unbuilt`, and the planks come back once.
13. **Over the run:** no correction of the game or Ada, panics 0, `no_builtin_data`.

**Seen red once on a real build before it is trusted:**
- with `send_next` not attaching the permit, step 11 fails `builds_on_p1` alone (refused `not_your_plot`, refunded);
- with door_panels leaving `others_only` set, step 9 fails `walks_into_p1` and the steps that need the game inside (10 to 12).

Every check is also shown failing alone on a doctored copy of a green manifest. Running out is proven by the engine and relay tests, not by the rig (it would hold the GPU for minutes).

## 7. If it must land in one day
**Keep:** Wave 0, 1A, 1B, 1C, 1D, 2A, and 2B without the snapshot. Rig steps 1 to 5, 8, 9, 11 and 13.

**Cut, in this order:**
1. the rig's picture and renew legs (steps 6, 7, 10, 12; their behaviour stays tested);
2. the snapshot;
3. the given list's sync across the giver's own devices (no self-copy; each device lists only what it gave);
4. the 5-minute re-check (check once per welcome);
5. lights only where you stand (light every open plot while open);
6. last, the relay's permit check. Without it, open on the game's own check of the permit and close on the first `permit_*` refusal. This is last because it is the guard against walking into a new holder's home.

The website's swallow is never cut: without it a permit shows as text on the web.

### Critical Files for Implementation
- C:\Humanity\src\engine\household.rs (new: give, renew, ingest, answers, checks, open plots, probe)
- C:\Humanity\src\net\household.rs (new: the note, stored shapes, arrival rules, session state)
- C:\Humanity\src\engine\shared_build.rs (gate and take-down rows, permit attached, status routed, tick hooks)
- C:\Humanity\src\relay\handlers\shared_build.rs (`game_permit_check` over `permit_lets_in`)
- C:\Humanity\src\ship\wall_collision.rs and C:\Humanity\src\ship\door_panels.rs (open plots walkable, doors open for the permit holder)
- C:\Humanity\src\gui\pages\household.rs (new: the Household card on the Home page)
## 8. Amendments by the orchestrator (2026-10-12), which supersede the sections above where they differ

**A1. A permit can be taken back, and Block takes it back (supersedes decision 3 of 2026-10-05, sections 3.6 and 3.9's no-take-back words, and question 1).** Decision 3 rested on "the server stores nothing". Since 2026-10-09 the relay keeps a withdrawal list for friendship passes (`friend_cert_revocations`, keyed by a BLAKE3 fingerprint of the giver's key and a random serial, `src/relay/storage/friend_passes.rs`, `handle_cert_revoke` in `src/relay/handlers/friend_passes.rs`). A permit that a blocked person can still use for up to 90 days, to walk into the giver's home and build there, is the kind of gap the protect-users rule (memory `feedback_protect_users_safe_defaults`) says to close now. So:
- **Permit v2.** The words become `hum/permit/v2\n{server}\n{plot}\n{grantee}\n{serial}\n{expiry}`, where `serial` is 16 random bytes in lowercase hex chosen by the giver (the friendship pass's serial shape). `Permit` gains `serial`. `plot_permit_preimage`, `build_plot_permit`, `verify_plot_permit` and their pinned tests change outright (no v1 kept: nothing was ever given in a game, and nobody uses the platform yet). The scripted walker's `mintPermit` and its preimage test follow.
- **Withdrawal uses the existing list.** The giver sends the existing `cert_revoke {serial}` on its own signed-in socket; the relay records it in `friend_cert_revocations` exactly as for a pass and answers `cert_revoked`. `permit_lets_in` refuses a permit whose serial its issuer has withdrawn, with a new `Why::PermitWithdrawn` ("The plot's holder took this permit back."). The game's `game_permit_check` therefore says "no" at once, so the door shuts on the next check (the holder of a taken-back permit who stands inside is never walled in: the walkable rule of section 3.7 still lets them walk out). The refusal words in `handle_cert_revoke` that name "friendship pass" become words that fit both ("Withdrawal refused: that is not a pass or permit serial."; the cap sentence likewise). The 4,096-a-person cap is shared with passes and stays.
- **The page.** Each given row gets [Take back] beside [Renew]; it asks first ("Take back Bo's household permit for plot p2? Bo can no longer build there or walk in. What Bo built stays; you can take it down."). The no-take-back sentence goes; the intro says "You can take a permit back at any time." A withdrawal not yet confirmed by the server is resent until it is (the pass pattern: `withdrawals_pending`), and the row says "Taking it back..." until then.
- **Renew** gives a new permit with a new serial and, once the server took it (`dm_put_ok`), takes back the old one, so one permit per friend and plot is ever live.
- **Block** takes back every permit given to the blocked person, in the same step that withdraws their passes (`src/engine/dm.rs` block path, `withdraw_passes`).
- **The holder's other devices** learn of a withdrawal from the self-copy pattern already used for passes, so a given row shows "taken back" on every device.
- **Tests to add:** relay `a_withdrawn_permit_no_longer_lets_its_holder_in` (build refused `permit_withdrawn`, and the permit check says so; red with the revocation lookup skipped); engine `take_back_sends_cert_revoke_and_resends_until_confirmed`; engine `block_takes_back_every_permit_given_to_that_person` (red with the block path not calling it); page `take_back_asks_first`.

**A2. Questions 2 and 3 are accepted as recommended:** a permit holder sees the start's design plus what was built through the server (the two become one home when server worlds start everyone fresh, increment 6), and a visited plot is lit only while the permit holder stands in it, charged to nobody's meter.

**A3. Wave 0 carries A1's contract and crypto change** (shared.rs `Permit.serial` and `Why::PermitWithdrawn`, pq_crypto.rs v2 words and pinned tests, scripts/second-player.js `mintPermit` and its test, the relay's `permit_lets_in` revocation check and the `handle_cert_revoke` words), so every later lane builds on v2.
