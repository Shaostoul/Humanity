# Media, round 5: build plan

Date: 2026-10-12. Read against main at 044987fd1 (the newest release is v0.1489.0). Sources: docs/design/media-showcase.md (the whole arc: increment 1 and its fixes F1 to F7, increment 2, the follow-ups A1 to A7 and B1 to B5, rounds 3 and 4, every As built, the operator's answers of 2026-10-10 and 2026-10-11), docs/design/ui-system.md, CLAUDE.md (Rust-first canonical UI, Dual-UI parity, GUI-first configurability, Infinite-of-X, the BUG-046 gotcha, no backwards-compatibility debt before launch, research a legal question), and the code cited below. Every line number was read today. Nothing was edited and nothing was booted.

The operator's asks of 2026-10-12 keep his numbers: (1) edit where it is, (2) several files, (3) descriptions shown, (4) which server, (5) admin on the Media page, (6) a page and link per item, (7) a cover picture, (8) download counts, (9) a licence, (10) featured per category, sorting, load more, (11, phase 2) "Also put this on...".

## 0. In short

- **(1) Edit where it is.** An Edit control on the card and on the item's page, for the uploader and admins. One signed route, `POST /api/media/edit`, takes the title, description, category, labels, licence and the HumanityOS permission, signed over the BLAKE3 of the edit exactly as sent (the categories route's pattern). Who can see it stays `POST /api/media/visibility`. Files have two routes of their own. An admin edits the words, the category and the labels, never the licence or the permission.
- **(2) Several files.** The first file stays the item (`media_items.upload_filename`), so W5's `filename`, tickets, Keep and Remove keep working unchanged. Added files and the cover are rows of a new table, `media_item_files`. Every rule that counts, moves, expires or erases a Media file learns about them:
  - the space and the share count them;
  - members only moves them;
  - removing the item deletes them;
  - a kept item keeps the ones it had when it was kept;
  - an attached file is never taken for an upload waiting to be published. Today the expiry pass would delete it a day after upload.
- **(3) Descriptions.** The whole description on the item's page; its first line on the card. A category's description appears under the category filter when that category is picked. Today the description entered in Server Settings is saved and shown nowhere.
- **(4) Which server.** A strip under the top menu on Media, Watch, Chat and Server Settings (on the website: Media, Watch, Chat and Admin). It names the server, stays put while scrolling, and has Switch. The Media page's title becomes "Media on united-humanity.us".
- **(5) Admin on the Media page.** The section moves whole: the desktop's server_settings/media.rs becomes media/admin.rs, shown as an Admin tab; on the web, the Admin tab replaces the `<details>` at the page's foot. Server Settings and /admin keep one line pointing there.
- **(6) A page per item: `/media/<id>`. The relay serves it.** nginx sends `^/media/[0-9]+$` to the relay; `/media` itself stays static. The relay reads media.html from the web root and writes the item's Open Graph tags into its head. Three cases get the same bare page, byte for byte, so a crawler learns nothing:
  - a members-only item;
  - an item on a members-only Media page;
  - a number that is not an item.

  A labelled item gets no picture. The picture is `GET /api/media/preview/<id>`: a public, unlabelled picture or cover, served inline, and nothing else.
- **(7) A cover picture.** A picture uploaded for Media (cleaned on the device and again by the relay, like any picture) is attached as the item's cover. The card and the item's page show it.
- **(8) Download counts.** `GET /api/media/download/<id>?file=<n>` counts and serves. Views are counted too: they have read 0 since increment 1.
  - A person counts once per UTC day per item.
  - People are told apart by a 16-byte keyed hash of their network address, or of their key for a members-only download.
  - The hash's secret lives in memory and is replaced every day. Nothing about who is written anywhere; only the number is.
- **(9) A licence.** The choices come from `data/media/licences.json`: All rights reserved, CC BY 4.0, CC BY-SA 4.0, CC BY-NC 4.0, CC0 1.0.
  - "All rights reserved" is preselected, and is the licence of every item already published.
  - A separate opt-in, "Let HumanityOS use this in the game and in its own videos, with credit", is off by default.
  - Only the uploader changes either. Each choice is kept with the person's signature over it.
  - **WAITING ON docs/reference/findings/2026-10-12-media-item-licences.md:** each licence's line, the Creative Commons warning, the opt-in's full sentence and the hosting clause in the rules.
  - Until those words are in the data file, both apps and the relay offer "All rights reserved" only. The file decides: a CC licence is offered only when the file carries its warning.
- **(10) Featured per category, sorting, load more.**
  - `featured` holds main, category or none. The shelf shows the main shelf, or the picked category's shelf when a category is picked.
  - The list sorts by newest, most picked or most downloaded, paged by an opaque cursor.
  - The next page loads as the list's end comes into view; More stays for keyboards.
  - Featured items stay in the list too (already so).
- **(11) Phase 2: "Also put this on...", from the desktop app.**
  - The app copies an item (files, details, licence) to the person's other servers through the normal upload.
  - Each server records the BLAKE3 fingerprint of its stored main file. The uploader's app links the copies whose fingerprints match.
  - Each page says "Also on public.guide", and an edit can go to every copy.
- **Room in the files.**
  - src/relay/relay.rs is at 6,370 of 6,400. Only Wave 0 touches it: +4 lines, the day's marks on `RelayState`.
  - No wave touches src/lib.rs (16,485 of 16,700) or src/gui/mod.rs (4,721 of 4,800).
  - New code goes into new files.
  - The ratchet gains src/relay/media.rs and web/pages/media-app.js, so neither can quietly grow.
- **Waves.**
  - Wave 0: the contract, one builder.
  - Wave 1: seven lanes (relay writes; relay counts and pages; desktop list and admin; desktop item; desktop strip; web strip; web list).
  - Wave 2: web item, then the proof.
  - Documents last.
  - Phase 2: four waves of its own.

## 1. Today's code (file:line)

### 1.1 The relay's Media routes (src/relay/media.rs, 1,464 lines, no ratchet entry)
- **Signing:** `check_signed` :166-187 (Dilithium over `purpose\ntimestamp`, at most 5 minutes old and 30 s ahead, replay memory). The purposes are listed in the header :23-37.
- **The labels file:** `parse_labels`, `load_labels`, `labels` :93-154. It reads the disk first, then the built-in copy, and logs `note_builtin_copy`. licences.json follows this pattern.
- **Items:** `item_url` :241-247 (its doc still says the app calls with `?token=`). `item_json` :250-281: `featured` is a bool :260, `filename` :279; there is no licence, files, cover or downloads.
- **The list:**
  - `ListParams` :634-651 (`before`, `featured`, `id`, `of`).
  - `list_core` :670-716: `next_before` is the last id when the page is full :697.
  - `list` / `list_post` :727-735 read no headers.
- **Publishing:** `PublishBody` :775-792. `publish` :804-812, purpose `media_publish:<file name>` :805. `publish_core` :819-875.
- **W5:** `same_item` :886-899.
- **Removing:** `remove_core` :964-991 handles one file, given back or deleted.
- **Featuring:** `FeatureBody` :1019-1030 (`featured: bool`). `feature` :1032-1036, purpose `media_feature:<id>`. `feature_core` :1097-1103.
- **Keeping:** `keep_core` :1074-1094 (a keep is undone only by the same or a higher role).
- **Visibility:** `visibility` :1120-1126; `visibility_core` :1131-1151 moves one file, and puts one back when the change fails.
- **The pattern the edit follows:** `categories_request` :1262-1268, a BLAKE3 over the raw `categories` member as sent (`handlers/reports.rs` `raw_member` :243).
- **Usage and share:** `usage_core` :1301-1321; `mine_core` :1345-1366.
- **The members' file route:** `TokenQuery` :1373-1376; `file` / `file_from` :1390-1412; `file_core` :1417-1433; `serve_item` :1437-1449 (attachment, `private, no-store`, always the item's one file).

### 1.2 Tickets (src/relay/media_ticket.rs, 264)
- `Ticket` :81-87 binds the item and a fingerprint of the item's one file name.
- `admitted` :213-229 compares that fingerprint with `item.upload_filename` :225.
- `TicketBody` :233-237 is `{id, filename}`.
- `ticket_core` :255-264 answers `/api/media/file/<id>?ticket=`.

### 1.3 Storage (src/relay/storage/media.rs 1,217; media_extra.rs 86)
- **Header:** :1-23 says "Four tables" and "one row per upload".
- **`MediaItem`:** :303-340. `featured: bool` :320; `views` carries "Nothing counts views yet" :322-324.
- **Shelf rules:** `was_public` :468-470, `ALREADY_PUBLIC` :473, `published_shelf` :478-484.
- **Kinds:** `kind_of` :498-507.
- **Counting the space:** `media_bytes_on` :517-527 counts shelf 2 or a published item's upload. Its kept exclusion names only `media_items.upload_filename` :522.
- **Reading items:** `ITEM_COLUMNS` / `ITEM_JOINS` :532-541, `item_from_row` :543-567, `items_ordered` :576-580.
- **`publish_media_placing` :628-718:** "already published" checks `media_items` only :658-665; the INSERT names its columns :688-704.
- **`set_media_visibility` :725-764:** calls `place` once, for the item's own file :759.
- **`list_media` :768-793:** `ORDER BY m.id DESC`, `m.id < before` :778, `m.featured = 1` :781.
- **`remove_media` :806-831:** deletes the item and its one upload row.
- **`media_item_for_upload` :834-838:** finds an item by its main file only.
- **"Waiting" means "on the Media shelf and in no `media_items` row" in four places:**
  - `remove_unpublished_media` :843-863 (:848);
  - `expire_unpublished_media_in` :869-888 (:874);
  - `media_usage`'s waiting list :1170-1176;
  - `media_extra.rs` `media_waiting_of` :71-85 (:75).
- **Other:** `set_media_featured` :927-935 (0 or 1). `media_categories` :938-946 reads and sends the description; nothing shows it. `media_usage` :1153-1206: its people :1157-1168 and `kept_bytes` :1198-1203 name only the item's own file.
- **Schema, storage/mod.rs:**
  - The media tables are one batch, :1403-1463 (`media_items` :1420-1437).
  - The guarded-ALTER loop to copy is :1306-1316.
  - Every release since v0.1485.0 creates `media_items`, so both live servers hold it. BUG-046 applies to any column added to it.
- **Account:** storage/account.rs export :126-138, erase :318-336, has-data :408-413.
- **Resumable cancel:** relay/resumable.rs `cancel_session` :436-473 decides "published" by `media_item_for_upload` :450.

### 1.4 Routes, the gate, headers, nginx
- **relay/mod.rs:** the Media routes :1238-1265; `/uploads` :1381. `security_headers` :227-265 replaces every response's CSP with `csp_value()` :167-193, whose `img-src` has no `blob:`. The static pages' nginx CSP has it (scripts/nginx/humanity.conf:53), and the Media page shows members-only pictures through `blob:` addresses.
- **relay/features.rs:** :238 makes `/api/media` part of `Feature::Uploads`; its tests :549-550.
- **relay.rs:** `RelayState.media_tickets` :207, set up at :494. The file is at 6,370 of 6,400 lines.
- **nginx, scripts/nginx/humanity.conf:** `/api/` :110-117; `/uploads/` :176-181; the static pages :256-258 (`try_files $uri $uri.html`). So `/media` is the static media.html, and `/media/12` is a 404 today.
- **nginx, docs/admin/SELF-HOSTING.md:** zones :492-494; `/api/` :553, which passes no `X-Real-IP`; `/uploads/` :592; `/` :601.
- **The relay's web root:** api.rs:2129 reads `WEB_ROOT` (default "."). Both live relays run in /opt/Humanity (SELF-HOSTING.md:410), whose checkout holds web/pages/media.html; the deploy flattens the pages into /var/www/humanity (scripts/sync-web-root.sh).
- **The preview server:** scripts/preview-server.js:110 proxies only `/api/`, `/health` and `/ws`.

### 1.5 The desktop app
- **src/gui/pages/media.rs (776):**
  - `Job` :78-105 (`Feature { id, on: bool }` :91); `MediaAdmin` :120-136; `MediaState` :141-203.
  - `server_here` / `base_of` :210-217.
  - `sync_server` :276-285: a change of server abandons running uploads.
  - `on_done` :369-507.
  - `reload_list` / `featured_page` / `more` :548-591; `refresh_admin` :594-604.
  - `draw_body` :664-760: the heading "Media" :666; the admins' line pointing to Server Settings :687-694; the featured strip :720-731; More :751-759.
- **media/api.rs (898):** `Item` :54-93 (`featured: bool` :69); `ListAnswer.next_before` :208-209; purposes :554-596 (`feature_purpose` :581); `Filters` :628-640; `list_request` :649-704; `publish_request` :720.
- **media/tiles.rs (433):** `draw_filters` :142-181; `draw_tile` :199; opening a picture (the image viewer or the card) :281, :323; the actions :355-403 (Feature / Unfeature :378-381).
- **media/open.rs (412):** the card shows the description :303-304 for models, files, videos and members-only pictures. A public picture opens in the image viewer, with no description. Download :361.
- **Server Settings > ADMIN > Media, src/gui/pages/server_settings/media.rs (327):**
  - `draw_admin` :36-60, called at server_settings.rs:1078.
  - It uses `super::int_input` :80, `super::draw_changed_underneath_note` :90 and `super::send_server_settings_update` :103, which are private in server_settings.rs (:2948, :2345, :2853).
  - Its tests are media_tests.rs (246); its snapshot is ui_snapshots.rs:1801-1810. `render_page_png` :376 is private.
- **Dispatch:** src/gui/dispatch.rs `draw_tool_page` :37-80. The nav bar is drawn before it, in lib.rs:15259-15265.
- **Switching server:** src/gui/pages/chat/left_panel.rs (2,077 of 2,100) carries the same park, unpark and connect block three times: :1094-1121 (the cog), :1126-1169 (the row), and :1239.
- **The server list:** `chat_servers` (gui/mod.rs:641; `ChatServer` state_types.rs:1099); `active_server` (connections.rs:409-412).

### 1.6 The website
- **web/pages/media.html (47):** `<title>Media, HumanityOS</title>` :6.
- **web/pages/media-app.js (2,312):**
  - `listParams` :349-361 (`before`); purposes :374-387.
  - `build` :532-564 (the h1 "Media" :554).
  - `renderFilters` :688-715: the category select shows no description.
  - `renderFeatured` :1018-1023; `renderGrid` and the More button :1025-1046.
  - `openViewer` :1049; `renderViewer` :1110-1180, which already shows the description :1165.
  - `takeAnswer` :1190-1200 (`next_before` :1193); `feature` :1370-1376.
  - `renderAdmin` :2144-2153: a `<details>` at the page's foot.
  - `bootInBrowser` :2238-2298 (`?id=` :2244).
- **web/pages/media-admin.js (553):** `mountMediaAdmin` :134; `startMediaAdmin` :520, called by admin-app.js:128.
- **web/pages/admin.html:** the Media panel :288-297 and its scripts :306-312.
- **web/shared/shell.js:** the fixed `.hub-nav` :358-386; `.hub-nav-spacer` :388; `syncSpacer` :1012-1017.
- **web/chat/chat-ui.js:** names this server by `location.host` :2052. It lists linked servers from `/api/federation/servers` :1961 and opens one in a new tab :2124.
- **web/shared/report.js:101:** links a reported item as `/media?id=<n>`.

### 1.7 Constraints

| File | Lines | Budget (tests/file_size_ratchet.rs:383-424) |
|---|---|---|
| src/relay/relay.rs | 6,370 | 6,400 |
| src/lib.rs | 16,485 | 16,700 |
| src/gui/mod.rs | 4,721 | 4,800 |
| src/gui/state_types.rs | 1,838 | 1,850 (phase 2 only, +2) |
| src/gui/pages/chat/left_panel.rs | 2,077 | 2,100 |
| src/relay/api.rs | 3,761 | 3,930 (untouched) |
| src/relay/media.rs | 1,464 | none: Wave 0 adds 1,650 |
| web/pages/media-app.js | 2,312 | none: Wave 0 adds 2,500 |

- **Lints:** emdash_lint, terminology_lint, theme_token_lint (no literal colours), theme_editor_coverage (a new token needs its editor row), account_sql_lint (every table the export and erase name must be created in storage/mod.rs), page_parity_lint, compiled-in (scripts/lib/compiled-in.js: an `include_str!` outside data/ must be fingerprinted). The relay therefore reads media.html from disk and never embeds it.
- app/web/ is regenerated; never edit it.

## 2. Stale words this round corrects

| Where | Says | Now |
|---|---|---|
| operator's ask (3) | descriptions "are saved and shown nowhere" | the web viewer shows an item's description (media-app.js:1165), and the desktop card does for everything but a public picture (open.rs:303). A category's description is shown nowhere: that part is true |
| storage/media.rs:1-16 | "Four tables", "one row per upload" | six tables; an item has one main file and up to 19 more plus a cover |
| storage/media.rs:322-323 | "Nothing counts views yet" | views and downloads are counted, once a person a day |
| storage/media.rs:766, media-showcase.md:183-184, :269-270 | newest first, paged by `before` | three sorts, paged by `cursor`; loads more on scroll |
| relay/media.rs:23-37, api.rs:8-12, media-app.js:53-57, media-showcase.md:196, :246-249 | `media_publish:<file name>`, `media_feature:<item id>` | `media_publish:<file>:<licence>:<0\|1>`, `media_feature:<id>:<main\|category\|none>`, plus the new purposes in 3.4 |
| relay/media.rs:239-240 | the app calls the members' route "with `?token=`" | the header or a ticket (W3b, R3-8) |
| media-showcase.md:267 | "A strip of featured items at the top" | the main shelf, or the picked category's |
| media-showcase.md:124, :293; pages/media.rs:33-34, :687-694; media-admin.js:1-6; admin.html:288-297; docs/PAGES.md:59, :158 | the admins' Media section is in Server Settings, or on /admin | the Media page's Admin tab; Server Settings and /admin point there |
| docs/PAGES.md:162, report.js:101, media-app.js:45, :467, :2195, chat-reports.js:47 | `/media?id=<n>` | `/media/<n>` |
| docs/FEATURES.md:283-284 | four tables; `server_settings/media.rs` | the new tables and files |
| item_json, api.rs:69, media-app.js:954 | `featured` is a bool | `"main"`, `"category"` or `"none"` |

## 3. The design

### 3.1 Storage, and the BUG-046 rule

`media_items` has shipped since v0.1485.0, so the live databases already hold it. Its new columns are ALTER-added, and an index that names one goes after the ALTERs.

1. **Three guarded ALTERs** in a loop shaped like storage/mod.rs:1306-1316, placed right after the media tables' batch (:1463). They are never named in a CREATE TABLE, so a fresh database gains them by the same path as an old one:
   - `licence TEXT NOT NULL DEFAULT 'all_rights_reserved'`
   - `project_use INTEGER NOT NULL DEFAULT 0`
   - `downloads INTEGER NOT NULL DEFAULT 0`
2. **After those, in a batch of its own:**
   - `CREATE INDEX IF NOT EXISTS idx_media_items_downloads ON media_items(downloads, id);` names an ALTER-added column, so it can only go here.
   - `CREATE INDEX IF NOT EXISTS idx_media_items_featured ON media_items(featured, category_id, id);`
3. **`featured`** keeps its column and gains a value: 0 none, 1 main shelf, 2 its category only. Rows written so far are 0 or 1 and mean the same as before. No ALTER.
4. **New tables, in a batch of their own, every index over its own columns:**
   ```sql
   CREATE TABLE IF NOT EXISTS media_item_files (
       id              INTEGER PRIMARY KEY AUTOINCREMENT, -- the file's number; 0 is the item's own file
       item_id         INTEGER NOT NULL,
       upload_filename TEXT    NOT NULL UNIQUE,
       role            TEXT    NOT NULL,                  -- 'file' or 'cover'
       position        INTEGER NOT NULL,
       added_at        INTEGER NOT NULL
   );
   CREATE INDEX IF NOT EXISTS idx_media_item_files_item ON media_item_files(item_id, role, position);
   CREATE TABLE IF NOT EXISTS media_licence_log (
       id             INTEGER PRIMARY KEY AUTOINCREMENT,
       item_id        INTEGER NOT NULL,
       licence        TEXT    NOT NULL,
       project_use    INTEGER NOT NULL,
       set_by         TEXT    NOT NULL,   -- the signer's key
       signed_purpose TEXT    NOT NULL,   -- exactly what was signed
       signed_body    TEXT    NOT NULL DEFAULT '',  -- an edit's `edit` text, so the hash can be checked again
       signed_ts      INTEGER NOT NULL,
       signature      TEXT    NOT NULL,
       set_at         INTEGER NOT NULL
   );
   CREATE INDEX IF NOT EXISTS idx_media_licence_log_item ON media_licence_log(item_id, id);
   ```
   A file's size comes from its `user_uploads` row; no size is copied.
5. **One meaning of "waiting".** The const `NOT_ON_AN_ITEM` (`filename NOT IN (SELECT upload_filename FROM media_items) AND filename NOT IN (SELECT upload_filename FROM media_item_files)`) replaces the four "waiting" tests in 1.3. `Storage::media_item_holding(name) -> Option<(item_id, file_no)>` finds the item holding a file, whether as its main file or as an added one. It is used by:
   - publishing, which refuses an upload already on an item ("That upload is already part of a Media item.");
   - the resumable cancel (resumable.rs:450), which answers `published: true` for an attached file.
6. **Kept.** The const `KEPT_FILES` is each kept item's main file, plus its added files and cover with `added_at <= kept_at`.
   - `media_bytes_on` :522, the usage's people :1163 and `kept_bytes` :1198-1203 use it.
   - A file added after the keep counts against the uploader's share until someone keeps the item again (Keep again stamps a new `kept_at`). Question 2 asks the operator.
7. **Totals.** `ITEM_COLUMNS` gains `licence`, `project_use`, `downloads`, `total_bytes` (the sum over the main file, the added files and the cover) and `file_count` (the main file and the added files, not the cover). `items_ordered("largest")` sorts by `total_bytes`. One more query, `files_of(&[ids])`, fills each item's files and cover in the order of `position`.
8. **Sorts and the cursor.** `MediaQuery` gains `sort: Sort { New, Picks, Downloads }` and `cursor: Option<(i64, i64)>`. The ORDER BY is `value DESC, m.id DESC`, where value is `m.id`, `(SELECT COUNT(*) FROM media_picks p WHERE p.item_id = m.id)` or `m.downloads`. Paging uses the row value `(value, m.id) < (?, ?)`, so ties never skip or repeat an item.
9. **The featured filter.** With no category: `m.featured = 1`. With category c: `m.featured IN (1, 2) AND m.category_id = c`.
10. **Writes (storage only; the routes are lanes 1A and 1B):**
    - `attach_media_file(signer, id, upload, role, max_file_mb, place)`;
    - `detach_media_file(requester, is_admin, id, file_no, file_name)`;
    - `edit_media(requester, is_admin, id, &Edit, &Signed)`, which appends to the log when the licence or the permission changes;
    - `set_media_featured(id, Featured)`;
    - `add_media_view(id)` and `add_media_download(id)`;
    - `publish_media_placing` stores the licence and the permission, and logs them with the publish's signature.
11. **Account.**
    - The export adds `media_item_files` of the person's items and `media_licence_log WHERE set_by`, and the three new columns to `media_items`.
    - The erase deletes the person's items' `media_item_files` and `media_licence_log` rows before the items. The files themselves are the person's uploads, which the erase already removes.
12. **The pre-migration open test** (pattern: storage/uploads.rs:249-276 `opens_a_pre_v0675_database_and_migrates_it`): `opens_a_v0_1489_media_database_and_migrates_it`.
    - It builds a raw `media_items` exactly as storage/mod.rs:1420-1437 has it, with one item, plus its `user_uploads` row.
    - `Storage::open` must survive. The item must read licence `all_rights_reserved`, permission off, downloads 0, featured main, no files. A downloads-sorted list must answer, and a second open must change nothing.

### 3.2 Wire: one item (every route that gives items, `item_json`)

```json
{
  "id": 12,
  "url": "/uploads/1760_x.png",
  "filename": "1760_x.png",
  "original_name": "dusk.png",
  "title": "Dusk",
  "description": "First line\nThe rest",
  "category": {"id": 3, "name": "Fire flow arts"},
  "kind": "picture",
  "labels": ["flashing"],
  "visibility": "public",
  "featured": "category",
  "size_bytes": 1234,
  "total_bytes": 9876,
  "views": 5, "downloads": 3, "picks": 1,
  "uploaded_at": 1760000000000, "published_at": 1760000001000,
  "uploader_key": "<hex>", "uploader_name": "Ann",
  "keeps_file": false, "kept": false,
  "licence": "cc_by_4",
  "project_use": true,
  "file_count": 2,
  "files": [
    {"file": 0, "filename": "1760_x.png", "original_name": "dusk.png", "kind": "picture", "size_bytes": 1234, "url": "/uploads/1760_x.png"},
    {"file": 7, "filename": "1761_y.zip", "original_name": "textures.zip", "kind": "file", "size_bytes": 4000, "url": "/uploads/1761_y.zip"}
  ],
  "cover": {"file": 9, "filename": "1762_c.jpg", "kind": "picture", "size_bytes": 4642, "url": "/uploads/1762_c.jpg"},
  "link": "/media/12"
}
```

- **Field notes:**
  - `featured` is `"main" | "category" | "none"`; it was a bool, and both apps change with it in Wave 0, with no shim.
  - `cover` is null when there is none.
  - `picked_by_me` is in the members' view only, as before.
- **Members-only items:** a members-only item's every `url` is `/api/media/file/12?file=<n>`; `file` defaults to 0.

### 3.3 Wire: the list (`GET /api/media`, `POST /api/media/list`, both picks routes)

- **New parameters (query or body):**
  - `sort` = `new` (the default), `picks` or `downloads`;
  - `cursor`, the last answer's `next_cursor`, opaque to the apps;
  - `featured=1` together with `category=c` asks for that category's shelf.
- **Removed outright:** `before` and `next_before`.
- **The cursor** is `"<value>.<id>"` (value is the id, the picks count or the downloads count). Anything else is refused with 400: "That cursor is not one this server gave."
- **The answer** gains `next_cursor` (or null) and `licences`, shaped like `labels`. On a members-only page the public gets none of it.
  ```json
  "licences": {"default": "all_rights_reserved",
               "offered": [{"id": "all_rights_reserved", "name": "All rights reserved", "url": "", "cc": false, "line": ""}],
               "project_use": {"label": "Let HumanityOS use this in the game and in its own videos, with credit", "line": "", "offered": false},
               "cc_warning": ""}
  ```
  `GET /api/media/categories` carries `licences` too.

### 3.4 Wire: writes (each signed with `check_signed`; each item write names its file, so `same_item` applies)

- **`POST /api/media`** (publish) gains `licence` (default `all_rights_reserved`) and `project_use` (default false). Purpose: `media_publish:<file name>:<licence id>:<0|1>`. The log keeps the purpose, the timestamp and the signature.
- **`POST /api/media/edit`:**
  `{"key","timestamp","sig","id":12,"filename":"1760_x.png","edit":{"title":"Dusk","description":"...","category_id":3,"labels":["flashing"],"licence":"cc_by_4","project_use":true}}`
  - Purpose: `media_edit:12:<BLAKE3 hex of the edit member's JSON text exactly as sent>` (`raw_member(text, "edit")`).
  - Every field of `edit` is required: the form sends what it shows.
  - Answer: `{ok: true, item}`.
  - Refusals:
    - 400: the publish's own words for lengths, labels and the category;
    - 400: "That licence is not offered on this server yet." for a licence the file does not offer, or the permission while its `line` is empty;
    - 403: "Only the person who put it there, or an admin, can edit a Media item.";
    - 403 to an admin whose edit changes the licence or the permission: "Only the person who put it on the Media page can change its licence or what HumanityOS may do with it.";
    - 400, 404 and 409 per W5.
- **`POST /api/media/files/add`:** `{"key","timestamp","sig","id":12,"filename":"1760_x.png","add":"1761_y.zip","as":"file"|"cover"}`. Purpose: `media_file_add:12:1761_y.zip:file`. Uploader only.
  - The upload must be the signer's, on the Media shelf and on no item (`NOT_ON_AN_ITEM`), and not `.enc`. It is held to the role's largest Media file (R3-3) and to at most 20 files on an item.
  - A cover must be `kind_of` picture. A new cover replaces the old one (the old row and file go).
  - Under the room lock, the file moves from the waiting folder to the item's folder (public or members).
  - Answer: `{ok, item}`.
  - Refusals:
    - 404 "There is no upload of yours waiting with that name.";
    - 403 "Only the person who put it on the Media page can add files to it.";
    - 400 "A cover must be a picture: PNG, JPEG, GIF or WebP.";
    - 400 "An item can hold at most 20 files.";
    - 413, the role's words;
    - 409 "That upload is already part of a Media item.".
- **`POST /api/media/files/remove`:** `{"key","timestamp","sig","id":12,"filename":"1760_x.png","file":7,"file_name":"1761_y.zip"}`. Purpose: `media_file_remove:12:7`. Uploader or admin.
  - File 0 is refused with 400: "The first file is the item itself: remove the item instead."
  - A `file_name` that is not that file's name is refused with 409, in the W5 words.
  - The file was uploaded for Media, so its row and its file are deleted. Answer: `{ok, item, file_removed: true}`.
- **`POST /api/media/feature`:** `{..., "id", "filename", "featured": "main"|"category"|"none"}`. Purpose: `media_feature:12:category`. Admins only.

### 3.5 Wire: files, tickets, downloads, views

- **`GET /api/media/file/{id}?file=<n>`** takes the token in the header or a ticket, as now; `n` defaults to 0.
- **`POST /api/media/ticket`:** `{id, filename, file: 0, download: false}`.
  - It answers `/api/media/file/<id>?file=<n>&ticket=…`, or, when `download` is true, `/api/media/download/<id>?file=<n>&ticket=…`.
  - A ticket binds the fingerprint of the named file's stored name, so it opens that one file.
- **`GET /api/media/download/{id}?file=<n>[&ticket=]`** serves the file as an attachment named by its original name: `Content-Disposition: attachment; filename="<ASCII fallback>"; filename*=UTF-8''<percent-encoded>` (quotes, line breaks and control characters can never reach the header).
  - Who may download: anyone, for a public item on a public page; a ticket, or the token header, for a members-only item.
  - Anything else gets a plain 404 with the same words for every case: "There is no such file on the Media page, or it is not yours to open."
  - It counts one download for the item when the request has no Range, or a Range starting at byte 0, and this person has not been counted for that item today.
- **`POST /api/media/viewed`:** `{id}`, unsigned; the `X-Upload-Token` header is optional and is needed only to count a members-only item. It counts one view the same way. It always answers `{ok: true}`, so it never says whether an item exists.
- **Who a person is.**
  - For a ticket, or a token: their key.
  - Otherwise: the `X-Real-IP` header, which nginx sets from the connection. It is never taken from `X-Forwarded-For`, which a client can write. Missing, it counts as one shared "unknown" person: an undercount, never an overcount.
- **The day's marks** (`relay/media_counts.rs` `Marks`, on `RelayState.media_marks`):
  - a mark is the first 16 bytes of BLAKE3, keyed with a secret drawn at random for each UTC day and held only in memory, over `hum/media-count/v1\n<view|download>\n<item>\n<m:key | a:address>`;
  - the set is cleared, and the secret replaced, at UTC midnight;
  - at most 500,000 marks a day (`MARKS_PER_DAY`, about 32 MB); past that nothing more is counted that day, with one log line;
  - only the counters (`views`, `downloads`) are written to disk;
  - a restart forgets the marks, so a person can be counted twice on the day of a restart (question 4).

### 3.6 The item's page for social sites (`/media/<id>`): the relay serves it, nginx routes it

- **nginx** gains, in scripts/nginx/humanity.conf and docs/admin/SELF-HOSTING.md (with `zone=general` there), and on the live servers as an operator step with `nginx -t` and a reload, as R3-9 was:
  ```nginx
  # One Media item's page (2026-10-12, docs/design/media-round-5-plan.md): the relay answers it with
  # the item's title, description and picture in Open Graph tags for social sites, and with nothing
  # about it when it is members only or not there. /media itself stays a static page.
  location ~ ^/media/[0-9]+$ {
      limit_req zone=api_read_limit burst=10 nodelay;
      limit_req_status 429;
      proxy_pass http://127.0.0.1:3210;
      proxy_http_version 1.1;
      proxy_set_header Host $host;
      proxy_set_header X-Real-IP $remote_addr;
      proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
      proxy_set_header X-Forwarded-Proto $scheme;
  }
  ```
  - A regular-expression location wins over the `location /` prefix, so `/media/12` reaches the relay and `/media` stays static.
  - The guide's `/api/` location also gains `proxy_set_header X-Real-IP $remote_addr;`.
- **The relay's route** (`relay/media_page.rs`, `GET /media/{id}`):
  - It reads `$WEB_ROOT/media.html` when `WEB_ROOT` is set, else `web/pages/media.html` under its working folder (/opt/Humanity on both servers). It reads the file on every request (2 KB), so a web-only deploy shows at once.
  - With neither, it answers 503: "This server cannot find its Media page (media.html in its web root). Its operator can set WEB_ROOT."
  - It writes the tags at the marker line `<!-- media:og -->`, which Wave 0 adds to media.html's head, and replaces `<title>`.
  - It sets `Cache-Control: no-cache` and its own CSP, the static pages' one (`img-src 'self' data: blob:`, and so on). Wave 0 makes `security_headers` keep a CSP that a route set itself.
- **A public item on a public page** gets:
  ```html
  <title>{title}, Media on {host}</title>
  <link rel="canonical" href="https://{host}/media/{id}">
  <meta property="og:type" content="website">
  <meta property="og:site_name" content="{server name}">
  <meta property="og:title" content="{title, else the original name, else 'A {kind} on {host}'}">
  <meta property="og:description" content="{first 200 characters of the description, else 'Shared by {name} on {host}.'}">
  <meta property="og:url" content="https://{host}/media/{id}">
  <meta property="og:image" content="https://{host}/api/media/preview/{id}">   (only when there is a preview)
  <meta name="twitter:card" content="summary_large_image">                     ("summary" without one)
  ```
  - Every value is HTML-escaped (`& < > " '`).
  - `{host}` is the request's `Host` only when `https://{host}` is in the relay's `allowed_origins()`; otherwise the first https origin there. A forged Host header can never point a preview elsewhere.
  - A labelled item gets no picture, and its description starts with the label names, for example "Labelled: Flashing lights." (question 3).
- **The hidden page.** A members-only item, any item on a members-only page, and a number that is no item all get the same bare page, byte for byte, still 200 so a member's browser loads the page and signs in:
  - `<title>Media on {host}, HumanityOS</title>`;
  - `og:title` "Media on {host}";
  - `<meta name="robots" content="noindex">`;
  - nothing else.
- **`GET /api/media/preview/{id}`** serves the cover, else the picture itself when the item is a picture. It requires a public item on a public page, no label, and at most 5 MB (question 10).
  - It is served inline with its own `image/*` type, `Content-Security-Policy: default-src 'none'` and `Cache-Control: public, max-age=300`.
  - Everything else is 404 "No preview.", the same words for every case.
- **The gate:** relay/features.rs puts `/media` under `Feature::Uploads`.

### 3.7 Licences (data, rules, and the words that wait)

- **`data/media/licences.json`:**
  ```json
  {
    "_comment": "The licences an item on the Media page can carry (docs/design/media-round-5-plan.md). `line`, `cc_warning` and `project_use.line` hold the findings' exact words; a CC licence is offered only once `cc_warning` is filled, and the HumanityOS permission only once its `line` is.",
    "words_from": "docs/reference/findings/2026-10-12-media-item-licences.md",
    "default": "all_rights_reserved",
    "licences": [
      {"id": "all_rights_reserved", "name": "All rights reserved", "url": "", "cc": false, "line": ""},
      {"id": "cc_by_4",    "name": "CC BY 4.0",    "url": "https://creativecommons.org/licenses/by/4.0/",    "cc": true, "line": ""},
      {"id": "cc_by_sa_4", "name": "CC BY-SA 4.0", "url": "https://creativecommons.org/licenses/by-sa/4.0/", "cc": true, "line": ""},
      {"id": "cc_by_nc_4", "name": "CC BY-NC 4.0", "url": "https://creativecommons.org/licenses/by-nc/4.0/", "cc": true, "line": ""},
      {"id": "cc0_1",      "name": "CC0 1.0",      "url": "https://creativecommons.org/publicdomain/zero/1.0/", "cc": true, "line": ""}
    ],
    "project_use": {"label": "Let HumanityOS use this in the game and in its own videos, with credit", "line": ""},
    "cc_warning": ""
  }
  ```
- **How it is read.** The relay reads it the way it reads labels.json: disk first, then the built-in copy, with `note_builtin_copy`. `relay/media_licences.rs` holds `parse_licences`, `load_licences`, `licences()` and `offered()`.
  - **The parse refuses:** a missing default; an id that is not a to z, digits and underscore; an id listed twice; a `cc` licence whose url is not `https://creativecommons.org/`.
  - **The apps** read licences from the server's answer (3.3), as they read labels: the server's words decide.
- **WAITING ON docs/reference/findings/2026-10-12-media-item-licences.md:**
  - (a) each licence's `line`;
  - (b) `cc_warning`, the edit screen's warning that a Creative Commons licence cannot be taken back for copies already taken;
  - (c) `project_use.line`, the permission's full sentence (the operator's own words are its checkbox label);
  - (d) the hosting clause in web/pages/rules.html, a new rule 7 on what putting something on the Media page lets this server do;
  - (e) whether choosing a CC licence asks the person to confirm they made the work or hold the right to licence it, and its words;
  - (f) the exact names and deed URLs above, checked against the findings;
  - (g) how long the licence log is kept after an item is removed.
  
  Until (b) and (c) are filled, `offered()` holds back the CC licences and the permission, so main stays shippable.
- **Lint.** `the_licence_words_name_their_source`: when any of those words is filled in, `words_from` must name a file that exists under docs/reference/findings/.
- **The rules clause (d)** is added by the orchestrator once the findings land, never by a lane.

### 3.8 Permissions per action

| Action | Route | Uploader | Admin | Moderator | Member | Anyone |
|---|---|---|---|---|---|---|
| Edit title, description, category, labels | `/api/media/edit` | yes | yes | no | no | no |
| Change the licence or the HumanityOS permission | `/api/media/edit` | yes | no (403) | no | no | no |
| Who can see it | `/api/media/visibility` (exists) | yes | yes | no | no | no |
| Add a file, set the cover | `/api/media/files/add` | yes | no | no | no | no |
| Remove an added file or the cover | `/api/media/files/remove` | yes | yes | no | no | no |
| Feature: main, category, none | `/api/media/feature` | no | yes | no | no | no |
| Keep on the server | `/api/media/keep` (exists) | no | yes | yes | no | no |
| Remove the item | `/api/media/remove` (exists) | yes | yes | no | no | no |
| Download a file, count a view | `/api/media/download`, `/viewed` | when they may see it | yes | yes | yes | public items on a public page |
| Copy link | (the apps) | yes | yes | yes | yes | yes |
| Phase 2: link copies | `/api/media/copies` | yes | no | no | no | no |

### 3.9 The desktop app, in plain words

- **The heading:** "Media on united-humanity.us" (the host of `active_server`).
- **Tabs, for admins:** "Media" and "Admin". The Admin tab holds the old Server Settings section, unchanged. Server Settings > ADMIN keeps one line, "The Media page's settings, categories and space are on the Media page, under Admin.", with "Open them", which goes to the Media page with Admin selected.
- **Filters:**
  - the existing ones, plus **Sort**: Newest, Most picked, Most downloaded;
  - with a category picked, its description shows under the filter in muted text;
  - the shelf reads "Featured", or "Featured in Fire flow arts" when a category is picked, and asks for that shelf.
- **The grid:** when its end comes into view, the next page is asked for once. More stays below it.
- **A card:**
  - the cover, else the picture;
  - the title;
  - the description's first line, at most 120 characters, cut at a word with "...";
  - "by Ann";
  - "3 files" when there are several;
  - the picks and downloads counts;
  - "Featured" or "Featured in its category";
  - an Edit button for the uploader and admins.
  
  A click opens the item's page.
- **The item's page** (a window, media/item_view.rs; the image viewer stays behind "View full size"):
  - the picture, the video's Play here, or the cover; the label cover rules are unchanged;
  - the title; "by Ann in Fire flow arts"; the labels; "Kept by the server";
  - "4 picks, 12 views, 3 downloads";
  - the whole description, as plain text with its line breaks;
  - the licence by name, linked to its deed, with its line; and "HumanityOS may use this in the game and in its own videos, with credit." when it is on;
  - the files, each with its name, kind, size and Download (counted through the download route), and Remove for the uploader and admins (never on the first);
  - Copy link (`https://<host>/media/<id>`), Edit, Pick, Feature (three choices, for admins), Keep, Report and Remove item.
- **Edit** (media/edit.rs, a window):
  - title, description, category (its description under it), the label ticks, who can see it (the existing rules and words), the licence and the HumanityOS tick;
  - picking a CC licence shows `cc_warning` before Save can be pressed;
  - an admin editing someone else's item sees the licence and the tick read-only, with "Only Ann can change these.";
  - Files: Remove on each added file, "Add a file" and "Cover picture" (a picker; pictures only for the cover). Each is a row under "Uploads in progress" and is attached when it is up: the same upload, attached instead of published;
  - Save sends the edit, and the visibility separately when that changed. Cancel discards.
- **The strip** is described in 3.11.

### 3.10 The website, in plain words

- **The same page as the desktop's:**
  - the h1 and document title "Media on united-humanity.us";
  - Media and Admin tabs for admins, with `/media#admin` opening Admin;
  - Sort, the category's description, and the shelf per category;
  - the next page loads when a sentinel after the grid comes into view (an IntersectionObserver with a 600 px margin); More stays;
  - the cards and the three-way Feature as on the desktop.
- **The item's page** is the viewer grown (2A):
  - opening a tile pushes `/media/<id>` onto the browser's history, and closing goes back to `/media`;
  - loading `/media/<id>` opens the item;
  - it holds the files with Download (counted), the counts, the licence, Copy link and Edit.
  
  `?id=` and `/media?id=` are removed outright. The Reports view links to `/media/<id>`.
- **Edit** is a form inside the item's page, with the same fields, rules and words as the desktop's. Add a file and the cover use the existing resumable upload, attaching when the upload is up.
- **/admin's Media panel** becomes one line: "This server's Media settings, categories and space are on the Media page, under Admin." It links to `/media#admin`.

### 3.11 The strip (both apps)

- **Where:** fixed under the top menu on Media, Watch, Chat and Server Settings (on the website: Media, Watch, Chat and Admin).
- **What it says:** "{server name}  {host}", with a dot, green when connected, and **Switch**.
- **The desktop app:**
  - `src/gui/pages/server_strip.rs` draws a `TopBottomPanel::top("server_strip")`, called from `dispatch::draw_tool_page` for those four pages. The nav bar's panel is added first in lib.rs, so the strip sits under it and does not scroll.
  - Switch lists the saved servers (`chat_servers`) with their dots; picking one does what clicking the server's row in Chat does, through one new `GuiState::switch_to_server(url, name)` in connections.rs. left_panel.rs's three copies call it and the file shrinks.
  - With uploads running, it asks first: "2 uploads are on their way to united-humanity.us. Switching stops them where they are; they carry on when you come back."
  - "Manage servers" opens Chat.
- **The website:**
  - `web/shared/server-strip.js` adds a `position: sticky` bar at `top: var(--hos-nav-h)`. shell.js's `syncSpacer` sets that variable on `<html>`. The name comes from `/api/server-info` (`server_name`) and the host from `location.host`.
  - Switch lists the servers this one is linked with (`/api/federation/servers`) and opens the same page on that server (`https://<their host>/media` and so on), with "You sign in on each server's website separately." (question 6).
  - The bar is built with createElement and textContent, and only https addresses are linked.
- **Tokens:** the existing theme tokens; no new token.

### 3.12 The shared fixture

`scripts/tests/fixtures/media-round5-v1.json`, written by an `#[ignore]` relay test from fixed rows, holds:
- an item in the members' view and the same item members only;
- a list answer sorted by downloads, with its `next_cursor`;
- the `licences` answer;
- an edit body's exact text and its purpose;
- the publish, feature, file add and file remove purposes.

The relay's `the_shared_fixture_is_what_the_relay_makes` holds the builders to it byte for byte. The desktop's and the website's tests read it (precedent: scripts/tests/fixtures/file-offer-v1.json).

### 3.13 Phase 2: "Also put this on..." (after phase 1 is live)

- **Storage:**
  - `media_items.blake3 TEXT NOT NULL DEFAULT ''`: a guarded ALTER, no index.
  - `media_item_copies (item_id INTEGER NOT NULL, server TEXT NOT NULL, remote_id INTEGER NOT NULL, PRIMARY KEY (item_id, server)) WITHOUT ROWID`, a new table.
- **The fingerprint:** the BLAKE3 of the stored main file, worked out at publish (streamed, off the executor, before the room lock). An older item's fingerprint is worked out when its uploader first asks to link copies.
- **The wire:** items gain `blake3` (only where the item itself is shown) and `also_on: [{server, id, url: "https://{server}/media/{id}"}]`.
- **`POST /api/media/copies`:** `{key, timestamp, sig, id, filename, copies: [{server: "public.guide", id: 42}]}`, purpose `media_copies:<id>:<BLAKE3 hex of the copies member as sent>`. Uploader only.
  - Each server is a bare lower-case host (letters, digits, dots, hyphens, an optional port), never this server's own; at most 20.
  - It replaces the list. The relay contacts no other server.
- **The desktop app:**
  - Parked connections keep their own upload token: `ServerConnection.upload_token`, state_types.rs +2 lines, set from each background socket's peer list in engine/bg_connections.rs.
  - On your own item's page, "Also put this on..." lists your saved servers where you are signed in now. For each one ticked, a worker:
    - fetches every file from this server (public addresses, or tickets) into part files;
    - uploads each through the resumable upload to that server;
    - publishes the main file with the title, description, category (the same name there, else General), the labels that server knows, the licence, the permission, and members only when the item is or when that page is;
    - attaches the other files and the cover;
    - reads back that server's `blake3`.
  - Equal fingerprints: the app sends `copies` to every server holding a copy. Unequal: "The copy on public.guide differs from this one (its server changed the file), so they are not linked."
  - Edit gains "Also change the copies on: public.guide [x]", which sends the same edit, and visibility, to each copy, by that copy's own number and file name.
- **The website** shows "Also on public.guide" links. On your own item it says "Copy it to your other servers from the HumanityOS desktop app." (question 11).

## 4. Questions only the operator can answer (each with a recommendation)

1. **Who edits someone else's item.**
   - Recommend: admins may change the words, category, labels and visibility, and remove an added file. They never change the licence or the HumanityOS permission: a licence is the rights holder's grant.
   - Moderators edit nothing (they still Keep).
2. **Files added to a kept item.**
   - Recommend: they count against the uploader's share until someone keeps the item again. Otherwise an uploader could grow a kept item without limit.
3. **Labelled items on social sites.**
   - Recommend: title and description, prefixed "Labelled: Flashing lights.", but no picture. A social site would otherwise show unasked what the label promises to hide until clicked.
4. **What "a person" is for the counts.**
   - Recommend: a network address, or a key for members-only downloads, held only in memory for the UTC day.
   - Accept that a restart can count someone twice that day; it writes nothing about who looked.
5. **Views.**
   - Recommend counting them now, when an item's page opens, by the same rule. They have read 0 since increment 1, and "least viewed" sorts by them.
6. **Switch on the website.**
   - Recommend: the servers this one is linked with, each opening the same page there, with a line that each server's website signs in separately. A browser keeps nothing across sites.
   - The desktop app lists your saved servers.
7. **How the strip names the server.**
   - Recommend: "{server name}  {host}" in the strip, and the host alone in the title, as written: "Media on united-humanity.us".
8. **The licence log.**
   - Recommend: keep each licence choice with the person's signature over it, in the person's export, removed with the item and with the account.
   - WAITING ON the findings for whether it must outlive a removed item.
9. **Limits.**
   - Recommend: 20 files an item (a constant now; a server setting later if asked).
   - A cover up to the 64 MB picture cap, cleaned like any picture.
   - Added files always uploaded new (an existing chat or Shared Files upload cannot be attached).
10. **The social picture.**
    - Recommend: the original picture or cover only when it is at most 5 MB, nothing bigger.
    - Resizing joins the long-planned previews (increment 4). The 5 MB is to be checked against each site's own documented limit in the proof.
11. **Phase 2 choices.**
    - Recommend: the category with the same name, else General.
    - Members only on a members-only page.
    - Links made only by the uploader's app, after the fingerprints match. Servers never look for copies by themselves: that would need server-to-server requests on a member's say-so.
    - Copying from the desktop app only.
12. **Until the findings land.**
    - Recommend: the apps and the relay offer "All rights reserved" only (the data file decides).
    - The HumanityOS permission is not offered until its full sentence is written.
13. **Load more.**
    - Recommend: keep a More button under the grid as well, for keyboards and screen readers.

## 5. Waves

**Rules that hold for every wave:**
- **Ownership:** each file belongs to exactly one lane per wave. A file two waves edit is edited in sequence.
- **Every test** is red on the code before its lane with the named break, and its comment records the failure text (CLAUDE.md, "seen failing once").
- **Shared files:**
  - **src/relay/relay.rs:** Wave 0 only, +4, to about 6,374 of 6,400.
  - **src/lib.rs and src/gui/mod.rs:** no wave.
  - **Wave 0 only:** storage/mod.rs, relay/mod.rs, relay/features.rs, storage/account.rs and tests/file_size_ratchet.rs.
  - **The orchestrator:** `just rig-tests` and the documents.
  - **Phase 2 wave P0:** src/gui/state_types.rs, +2.

### Wave 0: the contract (one builder; two commits merged together)

Player-visible: nothing new, and nothing broken. Both apps keep working against the changed shapes, with no shim.

**Files:**
- **Relay storage:**
  - storage/mod.rs: 3.1 items 1 to 4;
  - storage/media.rs: `MediaItem` fields; `ITEM_COLUMNS`; `NOT_ON_AN_ITEM` and `KEPT_FILES` in their places; `list_media`'s sort, cursor and featured filter; publish storing and logging the licence; the header words;
  - storage/media_extra.rs: `media_waiting_of`;
  - NEW storage/media_files.rs and its tests: `ItemFile`, `Featured`, `Sort`, `files_of`, `media_item_holding`, `attach_media_file`, `detach_media_file`, `edit_media`, `add_media_view`, `add_media_download`;
  - storage/account.rs: 3.1 item 11.
- **Relay wire:**
  - relay/media.rs: `item_json`; `ListParams`; `list_core`'s `next_cursor` and `licences`; publish's licence and purpose; `FeatureBody` and its purpose; `TokenQuery.file`; `serve_item_file(folders, item, file_no, req, disposition)` replacing `serve_item`; the members' route serving `file=<n>`;
  - relay/media_ticket.rs: `TicketBody` `{id, filename, file, download}`; the binding to the named file; `admitted(state, ticket, id, file, now)`;
  - relay/resumable.rs:450: `media_item_holding`.
- **New relay files:**
  - relay/media_licences.rs: the loader and `offered()`;
  - relay/media_counts.rs: `Marks`, real; the `viewed` and `download` handlers as stubs answering 501 "Not built yet.";
  - relay/media_edit.rs: `edit`, `files_add` and `files_remove` as 501 stubs, with their final signatures and body structs;
  - relay/media_page.rs: `page` and `preview` as 501 stubs.
- **Shared relay files:**
  - relay/mod.rs: the eight new routes (`/api/media/edit`, `/files/add`, `/files/remove`, `/viewed`, `/download/{id}`, `/preview/{id}`, `/media/{id}`, plus `/api/media/file/{id}`'s `file`); `security_headers` keeps a CSP a route set itself;
  - relay.rs: `media_marks` and its init;
  - relay/features.rs: `("/media", Feature::Uploads)`.
- **Data and fixture:** data/media/licences.json; scripts/tests/fixtures/media-round5-v1.json and its `#[ignore]` writer.
- **The website, kept working:**
  - web/pages/media.html: the `<!-- media:og -->` line;
  - web/pages/media-app.js: the purposes for publish (licence default) and feature; `featured` read as a string (:954); `cursor` and `next_cursor` (:349-361, :1193, :1239);
  - the harness and the media-web tests that pin `next_before` or the old purposes, each changed test saying how.
- **The desktop app:**
  - media/api.rs: `Item`'s new fields; `Featured`; `ListAnswer.next_cursor` and licences; `Filters.sort`; `list_request`'s cursor and sort; `publish_request`'s licence, permission and purpose; `feature_request` and `feature_purpose`;
  - NEW media/api_items.rs: `edit_request` (the text sent is the text hashed), `file_add_request`, `file_remove_request`, `viewed_request`, `download_url`, `ticket_request(file, download)`;
  - media.rs: `Job::{Item, Edit, FileAdd, FileRemove, Viewed}` and `Feature { id, to: Featured }`; `MediaState.{tab, item}`; on_done arms delegating to NEW media/item_view.rs `on_answer` (real: it stores the item or the words);
  - tiles.rs: only what the `Featured` type needs.
- **The desktop app, room-making moves with no change of behaviour:**
  - `git mv` src/gui/pages/server_settings/media.rs to src/gui/pages/media/admin.rs, and media_tests.rs to media/admin_tests.rs;
  - server_settings.rs: the `mod media` line goes, the call at :1078 points to `media::admin::draw_admin`, and `int_input`, `draw_changed_underneath_note` and `send_server_settings_update` become `pub(crate)`;
  - `tiles::draw_filters` (:142-181) moves to NEW media/filters.rs;
  - ui_snapshots.rs: the moved path at :1802, and `render_page_png` becomes `pub(crate)`.
- **tests/file_size_ratchet.rs:** `("src/relay/media.rs", 1_650)` and `("web/pages/media-app.js", 2_500)`, with a dated note.

**Tests:**

| Test | Proves | Red with |
|---|---|---|
| `opens_a_v0_1489_media_database_and_migrates_it` | a database from v0.1489.0 opens and migrates (3.1 item 12) | the downloads index moved into the media tables' batch ("no such column: downloads" at open) |
| `an_attached_file_is_never_taken_as_waiting` | after two days of expiry the added file and its row stay; it is not listed waiting (mine, usage) and not removable by name; a cancel of its session answers `published` | `NOT_ON_AN_ITEM` left out of the expiry ("the added textures.zip was deleted after a day") |
| `a_published_upload_or_an_attached_one_is_never_published_again` | publish refuses an upload already on an item | the holding check left out of publish |
| `every_file_counts_against_the_space_and_the_share` | the main file, two added files and the cover all count; the share refusal names the sum | the added files left off shelf 2 |
| `a_kept_item_keeps_the_files_it_had_when_kept` | a later file counts until the item is kept again | `KEPT_FILES` without the `added_at` rule |
| `the_list_sorts_and_pages_by_cursor_without_skipping_ties` | seven items with equal counts, pages of two, each item once and in order | the cursor comparing the value alone ("item 4 listed twice, item 5 never") |
| `featured_in_a_category_shows_on_its_shelf_only` | the shelf rules of 3.1 item 9 | the category clause dropped |
| `edit_media_keeps_the_licence_to_the_uploader_and_logs_it` | storage refuses an admin's licence change and logs the uploader's with its signature | the requester check removed |
| `the_licences_file_parses_and_offers_cc_only_with_its_warning` | `offered()` follows the data file | `offered()` returning every licence |
| `the_licence_words_name_their_source` | 3.7's lint | `words_from` pointed at a missing file while a line is filled |
| `a_ticket_opens_only_the_file_it_was_given_for` | the ticket's file binding | `admitted` comparing the main file's name |
| `the_day_marks_count_a_person_once_and_start_again_at_midnight` (media_counts.rs) | the day, and the cap | the day left out of the reset |
| `a_route_that_sets_its_own_csp_keeps_it` (relay/mod.rs) | the carve-out | the middleware overwriting it |
| `the_shared_fixture_is_what_the_relay_makes` | the fixture, byte for byte | `featured` sent as a bool |
| `the_media_page_is_exported_and_erased_with_its_files` (account.rs, extended) | the new tables in the export and the erase | the erase leaving `media_item_files` rows |
| desktop `the_desktop_reads_and_signs_the_shared_fixture` (media/api_items.rs) | `Item` parses files, cover, licence, `featured` and `next_cursor`; the purposes match | the edit purpose hashed over re-serialised JSON instead of the text sent |
| web, in media-web-round2.test.js | the changed purposes and `next_cursor` | the old purpose kept |

**Checks:**
- `just verify`;
- `cargo check --features relay --no-default-features`;
- `just verify-relay`;
- `node scripts/lib/compiled-in.js` (licences.json is data-disk-first; nothing outside data/ is embedded);
- `just rig-tests`;
- `cargo test --test account_sql_lint --test file_size_ratchet --test emdash_lint` (on Windows, the standalone lints as in CLAUDE.md).

Size: the largest lane, about 1,300 lines with tests. If it runs long, the same builder lands it as 0a (relay, fixture, web) then 0b (desktop), merged together.

### Wave 1 (seven lanes in parallel)

**1A, relay item writes.** Player-visible: an item can be edited, given more files and a cover, and every file moves, counts and goes with it.
- **Files:**
  - relay/media_edit.rs and NEW media_edit_tests.rs;
  - relay/media.rs: `visibility_core` moves every file and puts every moved file back on failure; `remove_core` deletes the added files and the cover;
  - storage/media.rs: `set_media_visibility` calls `place` per file; `remove_media` returns the added files.
- **Tests:**

| Test | Red with |
|---|---|
| `the_uploader_edits_an_item_and_the_signature_covers_the_edit_as_sent` | the hash taken over the parsed body: a reworded body kept the old signature |
| `an_admin_edits_the_words_but_not_the_licence` | the licence rule removed from the route |
| `a_cc_licence_is_refused_while_its_warning_is_missing` | `offered()` skipped |
| `add_a_file_moves_it_into_the_items_folder` (members-only: the added file is in data/media-members and its `/uploads/` path answers 404) | the file left in the waiting folder |
| `members_only_moves_every_file_and_public_brings_them_back` (an added file's `/uploads/` address must answer 404 while members only) | the loop removed: "textures.zip stayed public while the item was members only" |
| `a_cover_must_be_a_picture_and_a_new_one_replaces_the_old` | the kind check removed |
| `an_item_holds_at_most_twenty_files` | the limit removed |
| `remove_a_file_but_never_the_first` | file 0 allowed |
| `removing_an_item_deletes_its_added_files_and_cover` | the added files not returned |
| `someone_elses_upload_cannot_be_attached` | the owner check removed |
| `the_members_route_serves_file_n_to_a_member` | the `file` parameter ignored |

- **Checks:** `just verify-relay`, `cargo check --features relay --no-default-features`. About 600 lines.

**1B, relay counts and pages.** Player-visible: downloads and views count; `/media/<id>` previews on social sites.
- **Files:**
  - relay/media_counts.rs and NEW media_counts_tests.rs;
  - relay/media_page.rs and NEW media_page_tests.rs;
  - scripts/nginx/humanity.conf;
  - docs/admin/SELF-HOSTING.md (the location, and `X-Real-IP` on `/api/`);
  - scripts/preview-server.js (`/uploads/` and `^/media/\d+$` proxied when `--api` is given).
- **Tests:**

| Test | Red with |
|---|---|
| `a_download_counts_once_per_person_per_day` | the mark not consulted (2) |
| `a_range_after_the_first_byte_counts_nothing` | ranges not looked at |
| `the_download_is_named_by_its_original_name_and_nothing_else` (a name holding a quote and a line break) | the raw name put in the header |
| `a_members_only_download_counts_by_key_through_its_ticket` | the ticket's key not used |
| `an_address_is_x_real_ip_never_x_forwarded_for` | XFF read: a spoofed header counted twice |
| `a_view_counts_once_and_the_answer_says_nothing` | the answer differing for a missing item |
| `a_public_items_page_carries_its_open_graph_tags_escaped` (a title `"><script>`) | escaping removed |
| `a_members_only_item_a_members_only_page_and_a_missing_item_answer_the_same_bytes` | the members-only case writing its title |
| `a_labelled_item_has_no_picture` | the label check removed |
| `a_forged_host_never_reaches_the_tags` | the Host used raw |
| `the_preview_serves_only_public_unlabelled_pictures_inline_under_default_src_none` | the visibility check removed |
| `the_page_keeps_blob_pictures_in_its_csp` | the route's CSP left to the middleware |
| `no_media_html_says_so_in_words` | 500 instead of the words |
| `nginx_sends_item_pages_to_the_relay` (both config files, as `nginx_answers_a_refused_rate_with_429_in_the_upload_locations` does) | the location missing from the guide |

- **Checks:** as 1A. About 550 lines.

**1C, desktop list and admin.** Player-visible: "Media on united-humanity.us", the Admin tab, sorting, loading on scroll, the shelf per category, the category's description.
- **Files:**
  - media.rs: the heading, tabs, the scroll sentinel, `featured_page` with the category;
  - media/filters.rs: Sort, the description, the shelf's heading;
  - media/admin.rs;
  - server_settings.rs: the one line and "Open them";
  - NEW media/round5_list_tests.rs.
- **Tests (headless, the `headless_run` click pattern of ui-system.md):**

| Test | Red with |
|---|---|
| `the_heading_names_the_server` | "Media" |
| `the_admin_tab_is_for_admins_and_holds_the_settings` | the tab drawn for a member |
| `server_settings_points_to_the_admin_tab` (a click lands on Media with Admin selected) | the tab not set |
| `the_end_of_the_list_coming_into_view_asks_for_one_more_page` | no in-flight guard: "asked 37 times" |
| `a_new_sort_asks_afresh_with_its_sort` | the sort not sent |
| `a_picked_category_shows_its_description_and_its_shelf` | the shelf asked without the category |

- **Checks:** `just verify`; `cargo test --test theme_token_lint`. About 350 lines.

**1D, desktop item.** Player-visible: the item's page, Edit, licence, files, cover, the card's first line, Download, Copy link.
- **Files:**
  - media/item_view.rs and media/edit.rs (NEW behaviour);
  - media/api_items.rs; tiles.rs (card, Edit, open the page); open.rs (Download through the counting route, per file);
  - add.rs, publishing.rs and uploading.rs (`Of::Attach { item, role }`: an upload attached when it is up, instead of published);
  - NEW media/item_tests.rs; NEW media/relay_round5_tests.rs (the relay's own routes on a loopback port, as relay_round3_tests.rs).
- **Tests:**

| Test | Red with |
|---|---|
| `a_tile_opens_the_item_with_description_files_licence_and_counts` | the description left out |
| `the_card_shows_the_first_line_and_the_file_count` | the whole description on the card |
| `a_creative_commons_licence_shows_its_warning_before_save` | the warning skipped |
| `an_admin_sees_the_licence_but_cannot_change_it` | the field editable |
| `add_a_file_uploads_then_attaches` | publish sent instead of attach |
| `copy_link_is_the_items_page_address` | `/media?id=` |
| `download_goes_through_the_counting_route` | `/uploads/` fetched |
| `an_item_edited_and_given_files_on_the_real_relay` (loopback, after 1A merges; until then against the stand-in) | `edit_request` sending re-serialised JSON |

- **Checks:** `just verify`. About 900 lines.

**1E, desktop strip.** Player-visible: the server named under the top menu on the four pages, and Switch.
- **Files:**
  - NEW src/gui/pages/server_strip.rs;
  - src/gui/pages/mod.rs (+1);
  - src/gui/dispatch.rs (+6);
  - src/gui/connections.rs (`switch_to_server`);
  - src/gui/pages/chat/left_panel.rs (its three switch blocks call it; the file shrinks, staying under 2,100).
- **Tests:**

| Test | Red with |
|---|---|
| `the_strip_shows_on_the_four_pages_and_no_other` (through `draw_tool_page`) | the strip on every page |
| `switching_from_the_strip_parks_and_unparks` | the switch without the park |
| `switching_with_uploads_running_asks_first` | no question |
| `the_chat_rows_and_the_strip_switch_the_same_way` | a block left inline |

  `dispatch_table_covers_every_page_except_the_engine_bound_ones` must stay green with the strip on an empty state.
- About 300 lines.

**1F, web strip.** Player-visible: the same strip on the website, and /admin's one line.
- **Files:**
  - NEW web/shared/server-strip.js;
  - web/shared/shell.js (+2: `--hos-nav-h`);
  - web/pages/watch.html, web/chat/index.html (a script tag each);
  - web/pages/admin.html (the Media panel becomes one line; its media scripts go; the strip's tag);
  - web/pages/admin-app.js (the `startMediaAdmin` call goes);
  - NEW scripts/tests/server-strip-web.test.js.
- **Tests:**

| Test | Red with |
|---|---|
| names the server, by `/api/server-info` and the host | |
| sticks under the nav | the variable not set |
| Switch links the same path on each linked server, https only | a `javascript:` address passed |
| a server name of `<img src=x onerror=…>` is text | innerHTML |
| /admin's Media panel is one line linking `/media#admin` | |

- **Checks:** `just rig-tests`. About 250 lines.

**1G, web list.** Player-visible: the web's half of 1C, plus `/media/<id>` addresses.
- **Files:**
  - web/pages/media-app.js: the title, tabs, Sort, cursor paging and the observer, the shelf per category and the three-way Feature, the category's description, `/media/<id>` read from the path and pushed onto the browser's history, `?id=` removed;
  - web/pages/media-admin.js (`startMediaAdmin` goes);
  - web/pages/media.html (the strip's tag, versions);
  - web/shared/media.css;
  - web/shared/report.js:101 (`/media/<id>`); web/chat/chat-reports.js:47 (the comment);
  - NEW scripts/tests/media-web-round5.test.js; the older media-web tests that change, each saying how.
- **Tests**, each red against one break in a copy of web/:
  - the h1 and title name the host;
  - the Admin tab only for an admin;
  - a Sort change asks afresh with `sort` and reads `next_cursor`;
  - the sentinel coming into view asks once;
  - the shelf asks `featured=1&category=` when a category is picked;
  - the description under the filter;
  - `/media/12` opens item 12;
  - Report links `/media/12`.
- **Checks:** `just rig-tests`. About 450 lines.

### Wave 2

**2A, web item** (after 1D; copies the desktop's page as built). Player-visible: the website's item page, Edit, licence, files, cover and counted downloads.
- **Files:**
  - NEW web/pages/media-item.js;
  - NEW web/pages/media-edit.js;
  - web/pages/media-app.js (the viewer hands the item's page to media-item.js; the cards);
  - web/pages/media-upload.js (attach when up);
  - web/pages/media.html, web/shared/media.css;
  - NEW scripts/tests/media-web-item.test.js (reads the shared fixture).
- **Tests** mirror 1D's, each red against one break.
- About 800 lines.

**2B, the proof.**
- **Files:**
  - NEW scripts/verify-media-round5.js (section 6);
  - src/gui/ui_snapshots.rs: `snapshot_media_item`, `snapshot_media_edit`, `snapshot_media_admin_tab`, `snapshot_server_strip_media`, with `snapshot_media_settings` renamed and moved to the Admin tab.

**Documents (orchestrator, last):**
- media-showcase.md: a "Round 5" section with As built; every row of section 2;
- docs/PAGES.md :59, :158, :162; docs/FEATURES.md :283-284;
- docs/design/ui-system.md: the strip in the component registry;
- docs/design/in-app-ops.md: the nginx location is an operator step;
- CLAUDE.md: the file map and the schema's new columns;
- docs/STATUS.md, the journal;
- each new test file added to `just rig-tests`;
- the licence words and rule 7, once the findings land.

**Checks per merge:**
- `just verify`;
- `cargo check --features relay --no-default-features`;
- `just verify-relay` after 0, 1A and 1B;
- `just rig-tests` after 0, 1F, 1G and 2A;
- `just snapshots` after 2B (one GPU, single-threaded; read the PNGs);
- `node scripts/lib/compiled-in.js` after 0 and 1B.

### Phase 2 (after phase 1 is live)
- **P0, contract:** the ALTER and table of 3.13; `blake3` and `also_on`; the `/api/media/copies` stub; a phase-2 block in the fixture; the desktop's `copies_request` and `Item` fields; `ServerConnection.upload_token` (state_types.rs, +2 lines, to 1,840 of 1,850) and engine/bg_connections.rs keeping it.
- **P1, relay:**
  - the copies route;
  - the fingerprint at publish and on first ask.
  
  Tests:
  - the fingerprint is the BLAKE3 of the stored bytes;
  - only the uploader may link;
  - hosts are checked and this server's own host is refused;
  - the list is replaced whole;
  - the export and erase cover it.
  
  Each red against a named break.
- **P2, desktop:** NEW media/copy_out.rs (the dialog and the worker); edit.rs (every copy); item_view.rs ("Also on"). Tested against two loopback relays.
- **P3, web:** the "Also on" links, and the line pointing to the desktop app.
- **P4, the proof:** two throwaway relays. A desktop copy shows the same fingerprint, licence, three files and cover on both; each lists the other; one edit changes both.

## 6. The in-browser proof, and the desktop's

**The website (2B; the preview server and a throwaway relay):**
- **Setup.** `node scripts/verify-media-round5.js`:
  - starts a throwaway relay from the release exe (scripts/lib/throwaway-relay.js, with `WEB_ROOT=<repo>/web/pages`);
  - starts the preview server on 8099 with `--api`;
  - prints both addresses and the relay's claim code.
- **In the browser pane, at http://localhost:8099:**
  1. Chat: make a test identity for this app on this local host, sign in, and claim admin with the claim code.
  2. The Media page: the strip names the server and stays while scrolling; the title is "Media on localhost:8099".
  3. Add a picture with a two-line description, then a `.glb` item. On the `.glb`, Edit: Add a file twice (a `.zip`, a `.stl`) and a cover. The CC licences are not offered while the words wait. With a scratch licences.json that carries a warning, picking CC BY 4.0 shows it.
  4. The `.glb` card shows the cover, the first line and "3 files".
  5. Open it: the address bar reads `/media/<id>`, with the description, the licence and each file's Download. Download the same file twice: downloads reads 1.
  6. Ask `/media/<id>` as a crawler (`curl -A facebookexternalhit`): it carries og:title, og:description and og:image, and the image answers as a picture. Make the item members only: the answer equals `/media/999999`'s byte for byte, and each added file's `/uploads/` address answers 404.
  7. Feature it in its category: the main shelf does not show it; picking its category does.
  8. The script publishes 60 small pictures through the API (a seed in its own temp folder). Scrolling to the end brings the second page with no press. Sort by Most downloaded puts the downloaded item first.
  9. The Admin tab holds the settings, the categories (a description entered there shows under the filter when its category is picked) and the meter. /admin shows the one line.
- **Seen red once:** the same run with 1B's hidden-page branch reverted, where step 6's crawler sees the members-only title.

**The desktop app:**
- **Snapshots:** `just snapshot media_item`, `media_edit`, `media_admin_tab`, `server_strip_media`, each read as a PNG.
- **Headless interaction tests:** 1C, 1D and 1E.
- **Against the real relay:** 1D's `an_item_edited_and_given_files_on_the_real_relay` on the relay's own routes on a loopback port.
- **Rule:** no other HumanityOS instance running while snapshots render.

## 7. If it must land in one day

**Keep:** Wave 0, 1A, 1B, 1C, 1D, 1G, and the strip's names (1E, 1F).

**Cut, in this order:**
1. Phase 2 (it was asked as a later phase).
2. The strip's Switch in both apps; the names stay.
3. The website's item page (2A): the web keeps its viewer, which already shows the description, and edits wait.
4. Views counting.
5. Sorting by picks and downloads; Newest and loading on scroll stay.
6. The cover picture.

**Never cut:**
- the BUG-046 order and the open test;
- members only moving every file;
- an attached file never taken as waiting;
- the crawler learning nothing of a members-only item;
- "All rights reserved" as the default, and the licence the uploader's alone;
- no legal words before the findings.

## 8. Adjacent, noted, not in scope

- **Public visitors get previews only** is still not enforced. `/uploads/` serves originals to anyone, as before; the download route follows it the day previews exist.
- **The website still has no Roles editor** (rounds 2 and 3).
- **Links in chat.** The desktop app does not open a `/media/<id>` link inside the Media page.
- **The web chat's sidebar** names the server by `location.host || 'united-humanity.us'` (chat-ui.js:2052).
- **The ticket pool.** Tickets per file share the person's 16 (R4-1). An item with several videos open at once uses more of them.

### Critical Files for Implementation
- C:\Humanity\src\relay\storage\media.rs (with the new C:\Humanity\src\relay\storage\media_files.rs): items, files, waiting, kept, sort and cursor
- C:\Humanity\src\relay\storage\mod.rs: the guarded ALTERs, the index after them, the new tables (BUG-046)
- C:\Humanity\src\relay\media.rs: item_json, the list, publish, feature, visibility, remove, the members' file route
- C:\Humanity\src\gui\pages\media.rs: the desktop page, its jobs, the Admin tab, the item page's hooks
- C:\Humanity\web\pages\media-app.js: the website's page, which the item page, the edit form and the strip join
## 9. Decisions by the orchestrator (2026-10-12), each the plan's own recommendation; each can be overruled

All thirteen questions of section 4 are taken as recommended, so the waves can run:

1. Admins may change another person's item's words, category, labels and visibility and remove an added file, never its licence or the HumanityOS permission (a licence is the rights holder's grant); moderators edit nothing and still Keep.
2. Files added to a kept item count against the uploader's share until someone keeps the item again.
3. A labelled item on social sites: title and description prefixed with its label, no picture.
4. "A person" for the counts: a network address (a key for members-only downloads), held in memory for the UTC day only.
5. Views are counted from now on, when an item's page opens, by the same rule.
6. Switch on the website lists the servers this one is linked with; the desktop app lists your saved servers.
7. The strip reads "{server name}  {host}"; the Media title "Media on {host}".
8. Each licence choice is kept with the person's signature, in their export, removed with the item and the account (whether it must outlive a removed item waits on the findings).
9. 20 files an item; a cover up to the 64 MB picture cap, cleaned like any picture; added files are always new uploads.
10. The social picture: the original picture or cover only when at most 5 MB.
11. Phase 2 (copies on other servers): the same-named category, else General; members only stays members only; copies linked only by the uploader's app after the fingerprints match; copying from the desktop app only.
12. Until the operator approves the licence wording (docs/reference/findings/2026-10-12-media-item-licences.md): only "All rights reserved" is offered, and the HumanityOS permission is not offered.
13. Load more by scrolling, and a More button under the grid as well.

The operator's own answers of 2026-10-12 (chat): featured items may show twice; featured per category, sorting and load more wanted; a licence per item with "All rights reserved" preselected; one item on several servers wanted as phase 2.

## 10. The operator's answers of 2026-10-12 (later the same day)

- **The editor states the facts plainly:** a Creative Commons licence cannot be taken back for copies already taken, and on a members-only item any CC licence lets every member who receives it republish it publicly.
- **The hosting paragraph** (approved to draft; "whatever you believe is best longest term"), shown in the rules (web/pages/rules.html and the desktop's rules) and beside Add to Media on both apps:
  > **Your uploads stay yours.** When you put a file on this server, you keep every right you have in it. You give the people who run this server, and anyone who runs it after them, a free, non-exclusive permission to store your file, keep backup copies, make previews and smaller copies of it, and show it and offer it for download to the people you chose (everyone, or this server's members), on this server only, for as long as you leave it here. When you remove the file or your account, the permission ends, apart from backup copies, which age out on the server's own schedule. This permission lets no one else use your file: what others may do with it is set by the licence you chose. Only upload what you have the right to share.
- **The game takes only CC0 and CC BY items** (recommended to the operator; replaces the named-recipient opt-in of item 9 and decision 12): public licences that cannot be withdrawn and need no recipient, so nothing depends on a person or an organisation outliving anyone. An "Offer this for the game" tick appears only when the item's licence is CC0 or CC BY 4.0. "All rights reserved" and NonCommercial items stay out of the game.
- **Metadata:** stripping stays the default for privacy; every item page shows its creator and licence; the existing switch keeps the details for anyone who wants their copyright fields kept.
- **DMCA agent:** not registered yet (the service provider's street address is published, and the operator has only a home address). Until then, Media uploads stay with trusted roles, and a takedown contact and process are added now. A legal entity's address, or whether a commercial or virtual office address counts, is being researched (findings to come).
- **A legal entity for what needs an owner** (domains, servers, donations, a DMCA address) and what survives the operator: being researched into a dated findings document, including the effect on the operator's income cap.
