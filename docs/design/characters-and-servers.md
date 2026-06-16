# Characters and servers: self-custodial vs server-authoritative (open vs closed)

**Status: DESIGN (2026-06-16, operator requirement).** When multiplayer lands, a player picks a
character that belongs to the server they are joining. There must be a LOCAL section (characters on
the user's devices, self-custodial) and at least one SERVER section (characters the server holds, to
prevent cheating) -- and some servers allow self-custodial players while others do not. This is
exactly Diablo II's open vs closed Battle.net. This doc grounds that model in the actual code so it
is built right (and so the multiplayer join wire is designed with the character dimension from day
one, not retrofitted). It pairs with [first-playable.md](first-playable.md) (the wiring gap) and
[homes-as-profiles.md](homes-as-profiles.md) (which already seeded "servers store augmented versions").

## The core idea: a character is TWO objects with two owners
The reason the open/closed split is clean here is that a character naturally decomposes into two parts
with different owners and different forge-resistance:

1. **Identity + cosmetics -- ALWAYS self-custodial, in both modes.** The owner is the user's seed-
   derived Dilithium3/ML-DSA-65 key (`derive_pq_identity`, `src/gui/mod.rs:1898`); it is unforgeable
   and never a server row. The cosmetic fields (`character_name`, `appearance`, `outfit`, `design`)
   already exist as plain fields on `WorldSave` (`src/persistence.rs:14-48`), explicitly decoupled
   from the chat name. The design promotes these to a new **`character_v1` signed object** built via
   the existing `ObjectBuilder` (`src/relay/core/object.rs`: canonical CBOR + Dilithium3 sig + BLAKE3
   id -- the same substrate that carries `signed_profile_v1`/`vouch_v1`), so a character cannot be
   claimed/renamed/re-skinned without the seed, and it replicates over `gossip_signed_object`
   (`federation.rs:539`) with no home server -- just like profiles. The on-disk `WorldSave` JSON stays
   as the fast local cache of that object.

2. **Progression -- owned per server policy.** Skills, XP, inventory, currency, reputation, completed
   quests: the cheat-sensitive half. On a CLOSED server the relay is the **sole writer**, stored
   server-side keyed by the verified `public_key`. This is partly built already: `PlayerProgress`
   (`src/relay/storage/game_persistence.rs:67-82`: public_key / current_quest / completed_quests / xp
   / reputation) holds per-pubkey progress, `handle_game_join` (`msg_handlers.rs:3303`) already
   re-seeds a returning player from that row and never from client-sent stats, and `apply_quest_reward`
   (`game_state.rs:757`) already computes XP server-side from the server-owned quest definition.

**The split invariant:** a field is self-custodiable iff forging it grants no competitive advantage.
Cosmetics + identity fail safe (always self-custody). Progression fails dangerous (server-authoritative
on closed). The relay being authoritative over a closed character's in-world STATE never touches
self-custody of IDENTITY -- they are different objects.

## Server policies (the D2 open/closed/hybrid axis)
A server declares `character_policy` in its server-info; the default is `open` (self-custody by default,
backward-compatible).

| Policy | Accepts | D2 analogue | Anti-cheat |
|---|---|---|---|
| **open** | Self-custodial LOCAL characters. The client presents its device-held signed `character_v1`; the relay verifies the Dilithium signature, asserts `author == the identify-bound socket key`, and accepts it as-is. Trusts the save you bring. | Open Bnet: you bring your own `.d2s` from disk; the realm trusts it. | None possible, by design. Forgeable progression, never ranked/ladder. Cheating is sandboxed to that server. This is the existing offline `WorldSave` path made network-reachable. |
| **closed** | Only characters the RELAY mints + holds. The client presents only its pubkey (already proven at `identify`); the relay creates/loads the authoritative row, the client is render-only. Refuses client-supplied stats/inventory/XP/quest-complete; accepts only INTENTS (move here, interact with entity N, craft recipe R) and computes the result itself. Cosmetics may still ride the presented `character_v1` (they cannot grant advantage). | Closed Bnet: Blizzard's servers store the authoritative character; you never hold the file. What makes ladder + anti-cheat + trusted trading possible. | Hard boundary. Relay is sole writer of progression. Ladder-eligible (only relay-computed state can be ranked). Lose the realm, lose that character's earned power (you keep identity + cosmetics). |
| **hybrid** | Both, side by side: a self-custodial LOCAL list (untrusted, casual, unranked) AND the server's own held list (trusted, ladder). The player chooses custody per character at join. The server's ruleset wins over any player-self-imposed lock. | One realm running both lists at once -- the operator's "servers that also allow self-custodial players." | Per-character: a LOCAL pick has open posture (unranked); a server-held pick has closed posture (relay-authoritative, ladder). The two never share an inventory/ladder. |

## The character-select flow (at server-join)
The existing showroom selector (`src/gui/pages/showroom.rs` `draw_character_select:79`, today a single
hardcoded entry with a disabled "+ New Character" and the note "More save slots are coming. Each is its
own home + character." -- that "later" is THIS) splits its left column into SOURCED sections that mirror
D2's which-realm choice:
- **LOCAL section** ("On this device"): the device's signed characters from `saves/` (each `WorldSave`
  is a row; `character_name` the label, `kind`/`design` the sub-label). Self-custodial; usable on open
  + hybrid servers.
- **SERVER section(s)** (heading = server name + policy badge): the characters that relay holds for your
  pubkey, from a new lightweight `GET /api/characters` (keyed by `public_key`, the same key
  `player_progress` already uses; first slice returns the single row).

**Honest greying, never silent hiding:**
- OPEN server -> only LOCAL selectable. Sub-label: "Self-custodial. You own this character; this server
  trusts it as-is."
- CLOSED server -> LOCAL greyed with a reason: "This server holds characters itself to keep play fair.
  Create one below." Only that server's section selectable.
- HYBRID server -> both live; choose custody per character.

**Creation + migration** (deliberately asymmetric + honestly labeled, like D2):
- "+ New local character" mints a self-custodial `WorldSave` + `character_v1` on disk.
- "+ New on this server" (closed/hybrid) sends a signed `server_character_create` (the same Dilithium
  auth envelope as server-join); the relay creates a fresh `player_progress` row with SERVER starter
  state only (the client never sets gear/XP).
- LOCAL -> closed carries **name + appearance only** ("Your name and look come with you. This server
  starts your gear and progress fresh to keep play fair."), never stats.
- closed -> local is forbidden by default (a server MAY offer an explicit read-only snapshot export),
  mirroring D2 where closed characters never became open.

Native `showroom.rs` is canonical (Rust-first); the web mirror reuses `renderServerList`
(`web/chat/chat-ui.js:1368-1488`, which already fetches `/api/federation/servers` and badges
`trust_tier` -- the policy badge slots beside it).

## The Play launcher + offline editing (operator additions, 2026-06-16)
The **Play button becomes the launcher screen** (not a straight drop into FPS): it opens character
select, your homes, and an Enter-World button. Concretely:
- **Section order, top to bottom:** HOMES, then LOCAL / open-net characters, then closed-net
  characters/homes. (Homes are first because you pick where to live, then who you are there.)
- **Default checkbox.** Each character/home has a "default" checkbox. When a default is set, Play
  skips the launcher and drops you straight into the world with it ("so I don't always have to go
  through character select"); a "manage / pick another" affordance stays on the launcher. With no
  default, Play shows the launcher to choose. Stored as a local pref (a `default_save` slot name),
  honored by the Play handler in `lib.rs`.
- **Save / character editor lives in the menu.** You can customize how you look any time in OFFLINE
  mode -- the appearance/wardrobe editor (today's `showroom.rs` `draw_appearance`/`draw_wardrobe`)
  is reachable from the menu, not just at first creation. There is no reason offline customization
  should be gated.
- **Changing station (anti-exploit, server-side, optional).** On a SERVER, changing your look may
  optionally require visiting a "changing station" (a placeable in-world entity/zone), so a player
  cannot swap outfits mid-expedition to dodge a penalty or gain an advantage. This is a per-server
  rule: open/casual servers allow free changes; a competitive/closed server gates cosmetic changes
  to the station. It composes the existing "closed server is sole writer + accepts only intents"
  model -- a `change_appearance` intent is rejected unless the player entity is within a
  changing-station zone the relay knows about. Cosmetics-while-offline are always free.
- **Multi-save is the precondition.** The save infra exists (`persistence::list_saves`,
  `save_world`/`load_world`; `save_load::{load_active_home,save_active_home}`) but the Play flow
  uses a single active home today and `showroom.rs::draw_character_select:79` is a single disabled
  stub. The first build increment is: enumerate `list_saves`, render the three sections, the default
  checkbox, load-the-selected-into-active, and the Play-skips-when-default flow -- all OFFLINE and
  fully verifiable before any networking. The SERVER sections stay placeholders until co-presence
  (first-playable.md) lands.

## Data-model changes (grounded; mostly additive)
- **`character_v1` signed object** via `ObjectBuilder` (`src/relay/core/object.rs`): payload =
  `{schema:'character_v1', character_name, appearance, outfit, design, home_layout_ref?, updated_at}`;
  `author_public_key` IS the owner; replicates over `gossip_signed_object`. The four fields ARE the
  existing `WorldSave` cosmetic fields; add `sign_character()` + re-sign on showroom edit + cache bytes
  next to the JSON save.
- **`character_policy: String`** ("open"|"closed"|"hybrid", default "open") on `ServerInfoResponse`
  (`api.rs:1372`, read from `server_config` like `owner_key`) AND on `FederatedServerEntry`
  (`api.rs:1449`, one more column on `federated_servers` in `misc.rs:229-291`, same idiom as
  `accord_compliant`) so the policy badge renders in the server LIST before click-in.
- **`server_characters` table** (the closed-realm authoritative ledger; generalizes today's implicit
  one-character-per-pubkey `player_progress` to N named characters per realm): `character_id` (relay-
  minted UUID, NEVER client-supplied), `owner_key`, `realm_id`, `name`, `stats_json`, `inventory_json`,
  `xp`, `reputation`, `locked`, `updated_at`, `UNIQUE(owner_key, realm_id, name)`. Mirrors
  `PlayerProgress` and reuses its upsert/load idiom. Moves cheat-sensitive state OUT of the opaque
  `game_world_snapshots` blob into a queryable server-written table. **First slice can stay as the
  existing single `player_progress` row** and only generalize when N-characters/full-inventory lands.
- **Join wire reconciliation** (the protocol drift that blocks all character-select wiring):
  `src/net/protocol.rs:11-14` defines `NetMessage::Join{player_name,public_key}` but the relay
  dispatches on the string tag `"game_join"` (`relay.rs:3428`) and `handle_game_join` reads a raw JSON
  Value (`msg_handlers.rs:3281`). Reconcile to `game_join { player_name, character_mode, character_id?,
  character? }` -- `character` (base64 `character_v1`) on the OPEN path, `character_id` selecting a
  server-held character on the CLOSED path; `handle_game_join` branches on the realm's `character_policy`
  and derives identity from the **identify-bound socket key, not the client `player_name`**.
- **New endpoints:** `GET /api/characters` (caller's server-held characters; first slice = the single
  `player_progress` row) and a signed `server_character_create`. The closed inventory-mutation rule when
  pickup/craft/trade lands: `game_pickup`/`game_craft` carry only `entity_id`/`recipe_id` (an INTENT);
  the relay computes the delta against `inventory_json`, exactly as `apply_quest_reward` computes XP
  today. A closed server must NEVER accept a client-supplied item array, stat, XP number, or
  quest-complete flag.

## How it fits first-playable (decide-before vs defer)
This is the IDENTITY layer of the multiplayer wiring that [first-playable.md](first-playable.md) scopes
as unbuilt. The very first `game_join` the client ever sends is exactly where character-select threads
in, so the join envelope must carry the character dimension from the start.

**Decide BEFORE the first co-presence slice (cannot be cleanly bolted on later):**
1. The `game_join` envelope shape -- it must carry `character_mode` + (`character` bytes | `character_id`)
   so the relay can branch open/closed on day one.
2. Identity at join derives from the identify-bound socket key, NOT the client `player_name`
   (`msg_handlers.rs:3281` currently trusts the name -- the closed path must read the proven key).
3. `character_policy` must exist on `ServerInfoResponse` before the client can render the correct
   selector for a given server.

**Defer until after first co-presence (does not block two avatars standing next to each other):** the
full `server_characters` table with `stats_json`/`inventory_json` (the single `player_progress` row
suffices for the first slice -- it already re-seeds on join), multi-character-per-realm, the intent-based
inventory mutation handlers, `character_v1` cross-server gossip, and closed->local snapshot export. The
closed path needs NOTHING new on the relay for the first slice because `load_player_progress` already
fires in `handle_game_join`; the increment is labeling a server closed and gating which fields the client
may assert.

## Operator decisions (reasonable calls made; flag if you disagree)
- **Closed-server cosmetic override:** default is cosmetics + identity always travel (a closed server
  renders your self-custodied name/look since they cannot grant advantage). Call made: a hardcore CLOSED
  LADDER server MAY additionally opt in to force fresh cosmetics ("everyone looks identical at the start
  line"); the DEFAULT remains cosmetics-travel.
- **Open-server progression trust:** call made: make "accept a self-custodied progression block (full
  cheatable sandbox)" a per-server opt-in flag; the default is "trust cosmetics only, run own
  progression," so the cheatable-everything sandbox is an explicit operator choice.
- **"real" (sensor-owned home) custody:** call made: defer. open/closed/hybrid covers the requirement;
  "real" is a separate future axis (physical sensors own the truth, no PvP/ladder semantics).

## Build order (when multiplayer work starts)
1. **Policy declaration** (no behavior change): add `character_policy` (default "open") to
   `ServerInfoResponse` + `FederatedServerEntry`. Every server self-describes its custody model.
2. **Surface the policy** in both UIs: badge it in web `renderServerList` (beside `trust_tier`) + the
   native post-connect panel. Read-only; proves discovery before any join logic.
3. **Split the selector:** rebuild `draw_character_select` into LOCAL (enumerate `saves/`) + placeholder
   SERVER sections with the greying rule. LOCAL-only flow works against the existing offline save.
4. **Promote the local character to a signed object** (`character_v1`): tamper-evident + replicable;
   nothing depends on the network yet.
5. **Wire the join** (the first-playable seam): reconcile `NetMessage::Join` with `game_join` into one
   envelope carrying `character_mode` + (character | character_id); branch `handle_game_join` on
   `character_policy`. Derive identity from the proven socket key.
6. **Server-held create + list:** `GET /api/characters` + signed `server_character_create` over the
   existing `player_progress` row. The SERVER section populates for real.
7. **(Later, gated by gameplay)** generalize to the `server_characters` table with server-written
   stats/inventory, N characters per realm, and intent-based mutation handlers. The full closed-Bnet
   anti-cheat ledger; lands when pickup/craft/trade gameplay does. The select UI does not change.
