# Media showcase (design, 2026-10-10)

**Status: proposed; the operator answered the open questions 2026-10-10 (below).** The operator asked for "a media page" that "can showcase all the images,
screenshots, videos, 3D models we upload with HumanityOS", with categories ("I have pictures of me
spinning a fire flow art contact staff AND we have the pictures/videos of the game we're
developing. I also have personal pictures/videos I'd like to share on my relay"), and "a section
that's like sharing/pinning other people's photos/videos". This document turns that into a design.
The choices marked **Recommended** are the defaults built unless the operator says otherwise.

## What exists to build on

- `POST /api/upload` with `?share=1` puts a file in the server's public **Shared Files** library
  (`user_uploads.shared = 1`, exempt from the per-person keep-N FIFO), listed by
  `GET /api/uploads` (`src/relay/api.rs` `list_shared_uploads`): url, original name, size, time,
  uploader key and name. Today only 3D-model formats are shared automatically, from a server's
  room. The web page is `web/pages/shared-files.html`; the desktop has its Files page manager.
- Uploads honour each person's role limit since 2026-10-10 (`role_upload_limit`), streamed to disk.
- Files in DMs and P2P groups are encrypted and can never be shown publicly; they are not eligible.

## The page

**Media**, on both apps (native first, web mirrors it; one page, not a second thing beside Shared
Files: Shared Files becomes its "Files" filter). A grid of tiles: pictures and screenshots as
thumbnails, videos with a play mark and their length, 3D models with a model mark (a preview
render is a later increment). Click opens the existing image viewer, the media player, or the model
viewer. Filters: by **category**, by **kind** (pictures, videos, models, files), by **person**, and
a search over titles.

## Putting something on it

Nothing is ever on the Media page unless the person who uploaded it put it there. A chat picture
is not shown just because it was posted in a room, and nothing from a DM or a group can be.

- **From the Media page:** "Add to Media" opens the file picker, then asks for a title (optional),
  a category, and a short description (optional). The file uploads with `share=1` and its details.
- **From chat:** a picture or video posted in a server's room gets "Add to Media" in its row menu
  (only on your own posts), which publishes that same upload with a category; nothing is uploaded
  again.
- **From the game and the Studio:** the screenshot command and a Studio recording get a "Share to
  Media" button (a later increment).
- **Taking it down:** the uploader can remove their item (the file too, if it is not used
  elsewhere); an admin can remove anyone's (with the existing moderation log).

## Categories

**Recommended:** categories are the server's, set by its admins in Server Settings (a list of
names, each with an optional description), so each server has its own: this one might have "Fire
flow arts", "Game development", "Personal", "Builds". Every item has exactly one category; a
server starts with "General". Data-driven (CLAUDE.md "Infinite-of-X"): stored in the database, not
in code. People cannot make new categories themselves (on a public server that becomes clutter);
they ask an admin.

Alternative the operator may prefer: anyone may type a new category name when adding an item.

## Pinning other people's

Two separate things, both recommended:

- **Picks (yours):** anyone can pin someone else's Media item to their own **Picks**, a list on
  their profile ("Shaostoul's picks"). It is a reference, not a copy: if the uploader removes the
  item, it leaves every Picks list. The uploader is always credited and linked.
- **Featured (the server's):** admins can feature items; featured items lead the page and can be
  shown on the server's front page. This is the "showcase" for visitors and the donation story.

## Privacy and safety

- Only items the uploader published; only from a server's room or the Media page itself.
- Pictures keep the existing metadata strip (no GPS or camera serial survives upload).
- A Media item can be reported like a post (design blocking-and-safe-mode.md section 10e); the
  report never shows an admin a file from a private conversation (none can be here).
- Pictures from people who are not your friends load as they do in chat (click to load on the
  desktop when they come from another website; never from another website on the web).
- The protected setup (10h) hides the Media page's pictures from people who are not friends, as
  it does in chat.

## Storage (relay)

- `user_uploads` gains `media` (0/1), `title`, `description`, `category_id`, `kind`, `featured`
  (ALTER-added columns, so their indexes go after the ALTER block: CLAUDE.md BUG-046 rule, with a
  pre-migration-shape open test).
- `media_categories (id, name, description, position)` and `media_picks (picker_key, upload_id,
  picked_at)`.
- Routes: `GET /api/media?category=&kind=&by=&q=&before=` (paged), `POST /api/media` (publish an
  existing upload or a new one), `POST /api/media/remove`, `POST /api/media/pick` and `/unpick`,
  `POST /api/media/feature` (admin), category admin through the existing server settings frame.
  Every write is signed like the other identity-keyed API calls (Dilithium, `content\ntimestamp`).
- Account erase and export include a person's Media items and Picks.

## Increments

1. Relay: the tables, routes and limits; Server Settings categories (both apps' admin UI).
2. Media page, both apps: grid, filters, viewer, Add to Media, remove.
3. "Add to Media" on your own room posts; Picks on profiles; Featured.
4. Game screenshot and Studio "Share to Media"; 3D preview renders; video length and poster.

## The operator's answers (2026-10-10)

Verbatim, then what it means for the build.

- **Categories:** "Seeing as the server is my relay I'd prefer only I choose the categories. I
  don't mind people suggesting them... when the server is large enough then I guess there may be a
  point where we want members suggesting categories? Maybe we could use the governance voting
  system for it?" So: admins set the categories; members can **suggest** one (a short form; admins
  approve or decline, and the suggester is told); a later server setting, "Who decides categories:
  admins / admins with suggestions / a member vote", hands it to the existing governance proposals
  on a big server. Built in that order; the vote is a later increment.
- **Public or members only:** "it should be configurable between public/private." A server setting
  (the whole Media page public or members only) and a per-item choice (an item can be members only
  on a public page).
- **Bandwidth:** "maybe we should gate limits between open public and members only for bandwidth
  purposes?" **Recommended:** public visitors get thumbnails and lighter previews (pictures resized,
  videos at a lower rate or as a poster frame with a short preview); members get the originals and
  downloads. Both are server settings, with a per-visitor rate limit for the public.
- **Labels:** "We should also consider a NSFW or similar tag. Maybe others?" Content labels hide an
  item until clicked: **Spoiler**, **Flashing lights** (photosensitivity), **Graphic**. The protected
  setup always hides labelled items, and they are never shown to the public without the click.
  **Adult (NSFW) content is held back**: several US states and the UK require age verification for
  services carrying adult material, so it stays OFF until it is researched into a dated findings
  document (CLAUDE.md, "Research a legal question") and the operator decides; until then the
  server's rules do not allow it on the Media page.
- **Size and a meter:** "maybe media should have its own size limit but, maybe it should also have a
  progress bar showcasing how full it is? That way I can prune stuff." Media gets its own storage
  budget (a server setting), a per-person share of it, and a meter, "12.4 of 50 GB used", in Server
  Settings and on the Media page for admins, with a prune view sorted largest first, oldest first or
  least viewed. The general upload cap (`max_total_upload_mb`, 500 MB on united-humanity.us today)
  stays for chat files.
- **Big files:** "What if I want to upload a multi-GB file?" One file in one request is held to the
  1 GB ceiling: a dropped connection loses the whole upload. Media uploads use **resumable
  uploads** instead (the file sent in pieces, an interrupted one picking up where it stopped; the
  tus protocol's approach), and with them the per-file ceiling becomes the Media budget and the
  role's limit. Specified below ("Resumable uploads, exact"); the relay half is built beside
  increment 2 and the apps switch to it right after.

## Questions still open

1. Adult content: research and a decision later (above).
2. The public preview sizes and rates: defaults to be measured on the VPS once the page exists.

## Sharing big files peer to peer (the operator, 2026-10-10)

"Is this chunked system essentially like torrenting from my PC to the server and then the server
acts like a seed? My PC could act like a seed too, right?" Resumable upload is one-to-one (a PC to
the server, in checked pieces, resuming after a drop); torrenting is many-to-many (everyone holding
pieces passes them on). Both the server and the uploader's PC can seed, which takes load off the
server's bandwidth. The cost is privacy: in a swarm every downloader sees every other one's network
address, the very thing the call forwarder keeps private. So peer-to-peer distribution is a later,
opt-in layer for big public files (the uploader and each downloader choose it), never the default;
the server always serves the file itself too. The project already distributes releases by torrent
(`docs/admin/`), which that layer can learn from. The whole direction, pieces, group
swarms, public seeding and fair shares at a million people, is
[file-transfer-at-scale.md](file-transfer-at-scale.md) (2026-10-11).

## Increment 1, exact (relay; 2026-10-11)

New tables (a fresh table each, so no column is ALTER-added to an existing one; still, any index
over a later-added column goes after the ALTER block, CLAUDE.md BUG-046):

- `media_items (id INTEGER PRIMARY KEY, upload_filename TEXT NOT NULL UNIQUE, uploader_key TEXT NOT
  NULL, title TEXT NOT NULL DEFAULT '', description TEXT NOT NULL DEFAULT '', category_id INTEGER
  NOT NULL, kind TEXT NOT NULL, labels TEXT NOT NULL DEFAULT '', visibility TEXT NOT NULL DEFAULT
  'public', featured INTEGER NOT NULL DEFAULT 0, size_bytes INTEGER NOT NULL, views INTEGER NOT NULL
  DEFAULT 0, created_at INTEGER NOT NULL)`. `kind` is `picture`, `video`, `model` or `file`, from
  the file's type; `labels` the comma-joined subset of `spoiler`, `flashing`, `graphic` (the list
  lives in one place: data, not code); `visibility` `public` or `members`.
- `media_categories (id INTEGER PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT
  '', position INTEGER NOT NULL DEFAULT 0)`, seeded with "General" when empty.
- `media_category_suggestions (id INTEGER PRIMARY KEY, suggester_key TEXT NOT NULL, name TEXT NOT
  NULL, note TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL, state TEXT NOT NULL DEFAULT
  'open')`, `state` `open`, `accepted` or `declined`.
- `media_picks (picker_key TEXT NOT NULL, item_id INTEGER NOT NULL, picked_at INTEGER NOT NULL,
  PRIMARY KEY (picker_key, item_id))`.

Server settings (the existing settings row and its update frame, both apps' Server Settings
editors): `media_visibility` (`public` or `members`, default `public`), `media_budget_mb` (default
2048), `media_per_person_mb` (default 512), `media_public_previews_only` (default true: the public
gets thumbnails and previews, members the originals; increment 2 makes the previews).

Upload: `POST /api/upload?media=1` stores like a shared file (exempt from the keep-N FIFO, never
from the size limits), but does not enter the Shared Files list until it is published as Media.

Routes. Every write is signed like `delete_shared_upload` (Dilithium over `purpose\ntimestamp`,
five minutes, a fresh nonce), with the purpose named per route:

- `GET /api/media?category=&kind=&by=&q=&before=&limit=` (newest first, paged by `before` id,
  `limit` at most 100): public items to anyone when `media_visibility` is `public`; members-only
  items, and everything on a members-only server, only to a signed request from a member
  (`media_list`). Each item: id, url, title, description, category, kind, labels, visibility,
  featured, size, views, uploaded time, uploader key and name, and how many picks it has.
- `GET /api/media/categories`: the list, in position order.
- `POST /api/media` (`media_publish`): publish one of the signer's own uploads (never an encrypted
  one) with title, description, category, labels and visibility. Refused over the Media budget or
  the person's share of it, with both numbers in the words.
- `POST /api/media/remove` (`media_remove`): the uploader, or an admin; removes the item, its
  picks, and the file.
- `POST /api/media/pick` and `/unpick` (`media_pick`); `GET /api/media/picks/{key}`.
- `POST /api/media/feature` (`media_feature`, admins).
- `POST /api/media/suggest` (`media_suggest`, members): a category suggestion; `GET
  /api/media/suggestions` and `POST /api/media/suggestions/decide` (`media_suggestions`, admins;
  accept adds the category).
- `POST /api/media/categories` (`media_categories`, admins): the whole list at once (add, rename,
  remove, reorder; removing a category moves its items to General).
- `GET /api/media/usage` (`media_usage`, admins): used of budget, each person's use, and the
  largest, oldest and least-viewed items for pruning.

Account export includes a person's Media items, picks and suggestions; erasing an account removes
their items (and files), their picks and suggestions.

## Increment 1 fixes, from its review (2026-10-11)

A read-only review of the merged relay half found these; each is fixed before increment 1 ships.

- **F1. An unpublished Media upload expires.** A `media=1` upload not published within 24 hours is
  deleted, file and row (the expiry sweep, `storage/expiry.rs`, and at start). Until then its owner
  or an admin can remove it (`POST /api/media/remove` with its file name instead of an item id), and
  the admin's usage lists it under "waiting to be published" with its size. It never makes room
  for a chat file: a `media=1` file is a Media file.
- **F2. Removing an item never takes a file something else uses.** An item remembers where its
  upload was before it was published (`media_items.from_shelf`: 0 a chat file, 1 a Shared Files
  file, 2 uploaded for Media). Removing an item from Media gives the upload back to that place
  and keeps the file; only an upload made for Media (2) loses its file. An admin's removal of
  someone's chat photo from Media therefore never breaks the chat message.
- **F3. Members only means members only, and says what a member is.**
  - A members-only item's file is moved out of the public folder into `data/media-members/` and
    served only by `GET /api/media/file/{id}?token=<the session's upload token>`, to a member who is
    not banned; public items stay at their `/uploads/` address. Making an item public again moves
    it back.
  - **A file that is already public stays public (decided 2026-10-11, after the fix review).** A
    chat upload is at its `/uploads/` address inside the messages that carry it, and a Shared Files
    upload is in that list, so either goes on Media as public only. Moving one into the members'
    folder would break the chat message and empty the list entry while it was published, and it
    could not take back the copies anyone had already saved, so "members only" would promise
    something it cannot keep. Asking for it is refused at publishing and at the switch, with the
    words "That file is already public where it was shared ... To share something with members
    only, upload it on the Media page itself." (`storage::media::was_public`, `ALREADY_PUBLIC`;
    test `a_file_already_public_goes_on_media_as_public_only`). Only a file uploaded for Media
    (`from_shelf` 2) can be members only.
  - On a members-only server the categories list and the labels are members only too.
  - "Member" is anyone signed in to the server, as everywhere else on the relay. On an open server
    anyone can become one by signing in, so the Settings line under the switch says so plainly:
    "Members are everyone signed in to this server. To keep it to people you let in, turn on
    lockdown."
- **F4. Budgets hold when uploads finish together:** the room check, the file's move into place
  and its row are done under one lock.
- **F5. A signature is good for one thing, once.** Timestamps more than 30 seconds ahead are
  refused (the replay memory covers the whole window), and each signed purpose names its target:
  `media_publish:<file name>`, `media_remove:<item id or file name>`, `media_pick:<item id>`,
  `media_feature:<item id>`, `media_suggestions:<suggestion id>:<accept|decline>`,
  `media_categories:<BLAKE3 hex of the request's category list as sent>`. Reads keep `media_list`,
  `media_usage` and `media_suggestions`.
- **F6. Sizes come from the file.** Publishing an upload whose recorded size is 0 records its size
  on disk, so the budget counts it.
- **F7. Fail closed.** A settings read that fails, or a visibility word the relay does not know,
  counts as members only. `views` waits for increment 2's viewer. The stale comment about a nonce
  on `delete_shared_upload` is corrected.

## Increment 2, exact (both apps; drafted 2026-10-11)

Native first, web mirrors it (CLAUDE.md "Dual-UI parity"); both against the relay as it stands after
the increment 1 fixes (signed purposes with targets, F5; members-only files through
`GET /api/media/file/{id}?token=`, F3).

**Where.** A **Media** page beside Watch and Studio (native `GuiPage::Media`, `src/gui/pages/media.rs`
and its children; web `web/pages/media.html` + `media-app.js`, with its row in docs/PAGES.md and
the nav, the page-parity lint satisfied). Shared Files stays as it is; the Media page links to it.

**The page.**
- A strip of featured items at the top when there are any.
- Filters: category (from `GET /api/media/categories`), kind (Pictures, Videos, Models, Files), a
  person (by name, from the list's uploader names), "My picks", and a search box over titles.
- A grid of tiles, newest first, paged by "More" (`before`): pictures and video posters as
  thumbnails (until increment 4's previews, the picture itself, loaded under the chat's
  click-to-load rule for people who are not friends on the desktop, and only from this server
  on the web); a label badge for Spoiler, Flashing lights and Graphic, and a labelled tile shows
  its label instead of its picture until clicked; a pick count; the uploader's name.
- Opening a tile: pictures in the existing image viewer; videos in the existing media player (the
  web: a `<video>` element); models and files: name, size and Download.
- Actions on a tile: Pick and Unpick (not on your own); Remove (yours, or any as an admin, with a
  confirmation naming what happens: "Remove from Media. The file stays where it came from." or
  "Remove from Media and delete the file."); Feature and Unfeature (admins); Report (the existing
  report dialog, a Media item as its context).
- **Add to Media**: the file picker, then a short form: title (optional), category, description
  (optional), labels (the three, as ticks), visibility (Public or Members only; when the server
  is members only, Members only and the choice hidden). The file uploads with `?media=1`, then
  publishes with `POST /api/media`. The person's own share is shown above the form ("You have
  412 MB of your 512 MB left"), and an upload over it is stopped before it starts.
- **Put on Media** from a chat picture or a Shared Files row (the same form without the picker):
  Public only, the visibility choice not offered and a line saying why ("This file is already
  public where it was shared."), because the relay refuses members only for it (`was_public`).
- **Suggest a category** for members: a name and a note; the answer comes as a notice.
- Members-only items are fetched through the token route; a person who is not signed in to the
  server sees only public items and a line "Sign in to this server to see its members' media."

**Admins (Server Settings, a Media section, both apps).** Visibility (Public / Members only) with
the F3 sentence under it; Media budget and each person's share (MB); "Public visitors get
previews only"; the category list (add, rename, reorder, remove; General stays); suggestions
waiting (Accept / Decline, the suggester named); and the **meter**: "12.4 GB of 50 GB used", a
bar, and the prune lists (largest, oldest, least viewed, waiting to be published) each with
Remove.

**Proof.** Native: unit tests for the request and signing helpers (each signed purpose with its
target), headless egui tests for the page's states (empty, items, a labelled tile hidden until
clicked, members-only line, over-share stop), snapshots of the page and the settings section
rendered and read (no other HumanityOS instance running). Web: node tests of the same states and
requests, with a fake DOM that answers "not there". Each seen failing once.

## Resumable uploads, exact (relay; 2026-10-11)

The operator's "What if I want to upload a multi-GB file?" One request carries at most 1 GB, and a
connection that drops at 900 MB loses all of it. A resumable upload sends the file in pieces; each
piece is written and counted before the next is sent, so after a drop the app asks the server how
far it got and carries on from there. The approach is the tus protocol's (an open, published
protocol for exactly this); the wire format here is our own small version of it, so neither app
needs a library for it.

**Scope.** Media uploads only (`media: 1`): chat and Shared Files keep the single request and its
limits. Large files in private conversations wait for the operator's answer on that question.

**Routes** (all under `/api/upload`, so the relay's Uploads switch covers them. **Corrected
2026-10-11, while building: they DO need an nginx change.** The `/api/upload` location limits
each address to 2 requests a minute (`limit_req_zone ... zone=upload_limit rate=2r/m`, burst 2 on
the live server, burst 5 in docs/admin/SELF-HOSTING.md), which would hold an upload to two 8 MiB
pieces a minute, about 16 MiB a minute, and answer 503 to the rest; and the general `/api/`
location keeps nginx's default 1 MB body limit, so pieces cannot simply go there instead. The
relay half is built as written; nginx needs its own location for the pieces, more specific than
`/api/upload`, so it wins:

```nginx
# in conf.d/humanity-limits.conf:
limit_req_zone $binary_remote_addr zone=resumable_limit:10m rate=120r/m;
# in the server block, beside location /api/upload:
location /api/upload/resumable {
    limit_req zone=resumable_limit burst=20 nodelay;
    client_max_body_size 9m;        # one piece (8 MiB) plus a little
    proxy_request_buffering off;
    client_body_timeout 300s;
    proxy_send_timeout 300s;
    proxy_read_timeout 900s;        # the last piece hashes and stores the whole file
    proxy_pass http://127.0.0.1:3210;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Two pieces a second is 16 MiB a second, above most home upload speeds; the relay's own limits
(three uploads at once, the Media budget and each person's share) bound what one address can do.
The same block belongs in docs/admin/SELF-HOSTING.md.)

- `POST /api/upload/resumable?token=<session upload token>`, JSON
  `{filename, size, content_type, media: 1, blake3?}`. Answers 201
  `{id, offset: 0, size, chunk_max}`. Before anything is written: the token names a key; the role
  may upload (`upload_role_check`); the same type rules as `POST /api/upload` (blocked and allowed
  extensions and types, the image and file sharing switches), shared with it in one function, not
  copied; a picture over `PICTURE_MAX_BYTES` is refused here; the Media room check
  (`upload_room` on the Media shelf) with the person's other open sessions counted as already used,
  so two uploads that fit alone but not together cannot both start; at most 3 open sessions per
  person. There is no 1 GB ceiling: the limit is the room in the Media budget and the person's
  share, with a sanity ceiling of 64 GiB.
- `GET /api/upload/resumable/{id}?token=` answers `{offset, size}`: how far the server got.
- `PATCH /api/upload/resumable/{id}?token=&offset=<n>`, body the next piece
  (`application/octet-stream`, at most `chunk_max`, 8 MiB, which the server states so neither app
  hardcodes it). `offset` must be the server's offset, or 409 with `{offset}`; nothing past `size`
  is taken. The piece is appended and synced to disk, then the offset is stored. One piece at a
  time per session: a second PATCH while one is being written gets 409. Answers `{offset, size}`,
  or, for the last piece, the finished upload (below).
- `DELETE /api/upload/resumable/{id}?token=` cancels: the row and the file go.
- A session belongs to the person (the key), not the connection: upload tokens change on every
  reconnect, and any current token of the same key continues it. Another person's token gets 404,
  the same answer as a session that does not exist.

**Finishing** (when the offset reaches `size`), off the async executor: if a `blake3` was given at
start, the file's hash must match (refused 422 and the session removed when it does not: a resume
went wrong and the file is not the one the app has); a picture is read back for its magic bytes and
the metadata strip, as in the single request; then `store_upload` (the room check again, under its
lock, which is the one that counts) moves the file into `data/uploads/` under the same naming rule
as the single request. The answer is the same JSON as `POST /api/upload`, so the apps then publish it
with `POST /api/media` exactly as before.

**Storage.** Table `upload_sessions (id TEXT PRIMARY KEY, public_key, filename, content_type, size,
offset, shelf, blake3, created_at, touched_at)`: `id` is 32 random hex characters. Pieces go to
`data/uploads-resume/<id>.part`, a folder `clear_part_files` does not touch. At start the relay
reconciles the two: a file with no row is deleted, a row with no file is deleted, and a file longer
than its stored offset (a crash between the write and the offset update) is cut back to the offset.
Sessions untouched for 24 hours are removed in the expiry pass, row and file. An account erase
removes the person's sessions and their files; the export lists open sessions (file name, size,
offset), since they are the person's.

**Proof.** Relay tests, each seen failing once: a file sent in three pieces finishes on the Media
shelf with its size and the single request's answer shape; a wrong offset gets 409 with the right
one; a different token of the same key continues the session and another person's token gets 404;
a file over the share is refused at start with the share words; two sessions that fit alone but not
together, the second refused; a piece past `size` refused; a blocked extension refused at start; a
`blake3` mismatch refused and cleaned up; a picture's location data stripped at finish; DELETE
cleans up; an idle session expires (row and file); the start-up reconciliation cuts a long file back
to its offset and removes orphans; an account erase removes sessions. The `upload_sessions` table
added with a pre-existing-database open test (CLAUDE.md BUG-046 rule).

**As built (relay, 2026-10-11).** `src/relay/resumable.rs` (routes), `src/relay/upload_rules.rs`
(the rules both upload routes share: token, role, type lists, sharing switches, the picture
finish, the stored name, the answer), `src/relay/storage/upload_sessions.rs` (the table, the
expiry share, the start-up reconciliation); tests in `src/relay/resumable_tests.rs`. Where it
differs from or adds to the text above:

- **Reservations count everyone's uploads in progress against the Media budget** (each person's
  own against their share), not only the person's own: otherwise two people could each start an
  upload that fits alone, send gigabytes, and one be refused at the end. Only the start counts
  them; a single request's room check, and the check when a finished file is stored, count what
  is stored (that last check is the one that counts). The refusal words add "That counts N MB of
  uploads still in progress." when part of "used" is still arriving.
- **No room at the finish keeps the upload.** If the room has gone by the end (an admin lowered
  the budget, or a single request took the space), the finish is refused with the room words
  (507) and the session is KEPT, file and all, until the day's idle expiry; an empty piece at
  `offset = size` tries the finish again, so nobody sends gigabytes twice. A hash mismatch, a
  picture that is not what it says, or a role or sharing switch that no longer allows it removes
  the session (those files can never be stored). The role and the switches are checked again at
  the finish, since a long upload can outlive a change.
- **What each answer is.** Start: 201 `{id, offset: 0, size, chunk_max}`. GET: `{offset, size,
  chunk_max}` (chunk_max added, for an app resuming after a restart). PATCH: `{offset, size}`, or
  the finished upload `{url, filename, size, type}` (made by one function, `upload_answer`, for
  both routes). DELETE: `{ok: true}`. A piece at the wrong offset: 409 JSON `{error, offset,
  size}`; a piece while another is being written (and a cancel then): 409 JSON `{error, busy:
  true}`. Every other refusal is plain words, as `POST /api/upload`'s are: 400 (not Media, an
  empty file, a malformed hash, no `offset`), 403 (no role, uploads off, a sharing switch off, a
  token that is not a session), 404 (no such upload, or someone else's), 410 (its pieces' file
  was deleted by hand), 413 (over 64 GiB, a picture over 64 MB, a piece over `chunk_max` or past
  the end), 422 (hash mismatch), 429 (a fourth upload at once), 507 (no room).
- **The limit is the Media room, not the role's `max_upload_mb`**, as written above: a Verified
  person (25 MB per file in one request) can send a file up to their Media share this way. The
  role must still allow uploads at all, and the sharing switches still apply.
- **`touched_at`** moves with every piece (at its start, so the expiry pass never takes a session
  mid-piece); a status read does not keep an upload alive.
- The start-up reconciliation also lowers an offset to the file's length when the file is
  SHORTER (a disk that lost a write), so the app sends the rest again.
- Account export lists `filename, size, offset, created_at, touched_at` (not the id, which with
  any of the person's tokens carries the upload on); the erase receipt adds
  `upload_session_files_removed`.

**The apps** (a follow-up to increment 2, once this is live): Add to Media sends every Media file
this way, with a progress bar, a Pause and Resume, and a resume after a dropped connection; the
desktop app also offers to resume an unfinished upload after a restart (it keeps the session id and
the file's path).

**Found while writing this: a phone video keeps its location.** The metadata strip covers pictures
only. Phone cameras write the recording's location into an MP4's metadata (Apple's
`com.apple.quicktime.location.ISO6709` key, the `©xyz` atom on Android and others), so a video
uploaded to chat or Media today publishes where it was filmed. A video metadata strip (the same
whole-box removal approach, no re-encode) is its own increment, ahead of the Media page going
public.

**As built (2026-10-11): the video strip exists, not yet wired into the uploads.** It lives in
`src/relay/core/strip_video_metadata.rs`. One change from the plan above: boxes are not removed,
they are neutralised in place. Removing a box shifts every byte after it, and when the movie's
index (`moov`) comes before the frames (`mdat`), the index's chunk offsets (`stco`/`co64`) would
then point at the wrong bytes. So each metadata box keeps its size, its name is changed to `free`
(the standard "empty space, skip me" box every player already skips) and everything inside it is
overwritten with zeros. The file keeps its exact size and nothing moves.

- **API.** `strip_video_metadata_in_place(path) -> io::Result<VideoStrip>` works on a file on disk
  (the resumable uploads); `strip_video_metadata_in_buffer(&mut [u8])` does the same on bytes in
  memory (the single-request upload holds the file in memory before writing it). `is_mp4_family`
  answers from the first 8 bytes. `VideoStrip` reports the kind of file, whether a `moov` was
  found, each neutralised box (its path, offset and size), the bytes zeroed, and a plain sentence
  for each place the walk had to stop early; `complete()` is true only for a movie whose whole
  structure walked cleanly.
- **Neutralised:** every `udta` and `meta` directly inside `moov` and directly inside each `trak`
  (Apple's location key, make, model and software; Android's `©xyz`; 3GPP's `loci`; Adobe's
  `udta/XMP_`; iTunes-style titles and cover art); every XMP `uuid` box at the top level, in `moov`
  and in each `trak`; a top-level `meta`, but only in a file that also has a `moov` (a HEIF or AVIF
  still picture keeps the picture itself in its top-level `meta`, so it is never touched there).
- **Never touched:** `mdat`, `mvhd`, a track's `tkhd`, `mdia` and `edts`, other `uuid` boxes (360
  degree video settings), and everything else. Only box headers are read; `mdat` is stepped over
  by its size, so the work grows with the metadata, not the file.
- **Broken files:** a size under 8, or one running past its parent or the file, stops that level
  only; what was already neutralised stays neutralised, and the report says where it stopped. It
  never writes outside the file and never panics. Not an MP4 (WebM, Matroska, anything without
  `ftyp` first): never even opened for writing.
- **Proof:** 15 tests, files built byte by byte, each seen failing once against a deliberate break:
  Apple `moov/meta`, Android `moov/udta/©xyz`, `trak/udta` with `loci`, XMP `uuid` zeroed while a
  360 video `uuid` stays, `ftyp`/`mvhd`/`tkhd`/`edts`/`mdia`/`stco`/`mdat` byte-identical with
  `moov` before `mdat` and after it, a 64-bit `mdat`, size-0 last boxes, every truncation of two
  movies, a box running past its parent, WebM (a read-only file, proving no write handle), a HEIF
  still, a second run changing nothing, and 4,000 fixed-seed random files (no panic, no size
  change, no byte changed outside a reported box).

**What it does not cover (honest limits).** The capture time stays: it is in `mvhd`, `tkhd` and
`mdhd`, which playback reads. Location written as a timed metadata track lives inside `mdat`
(GoPro's GPMF `gpmd` track, DJI drones, some dashcams and action cameras), and so does anything a
camera puts inside the video stream itself (H.264/HEVC SEI messages): removing those means removing
the track or re-encoding. Not walked: `moof`/`traf` in fragmented files, `mdia/udta`, `meco`,
compressed `cmov` movies (old QuickTime), and data after the last box (Samsung's trailer); a
QuickTime file old enough to have no `ftyp` first is reported as not an MP4 and left alone.
Stale metadata an earlier editing tool left inside an existing `free` box is not scrubbed. After a
broken box size, the boxes behind it at that level cannot be found, so a report with
`stopped_early` entries means "better, not proven clean"; the wiring should decide what that means
for the upload (keeping such a video private is one option).

**HEIC and AVIF photos (R7, 2026-10-11).** An iPhone photo saved as HEIC, or an AVIF, is in the
same box family as an MP4 but has no movie in it: its location sits in the top-level `meta` box,
which also holds the picture, so this strip leaves it alone, and nothing else here can clean one
yet. Such a file is refused on every shelf that is not encrypted ("HEIC and AVIF photos cannot be
cleaned of where they were taken yet. Save it as a JPEG and send that."), unless the person chose
to keep its hidden information (`keep_metadata=1`). A video cut off before its movie box looks the
same and gets the same words. A file sent as `application/octet-stream` is now judged by its
extension alone, so a `.heic` cannot slip in that way either.

## Increment 2 follow-ups, exact (relay first, then both apps; 2026-10-11)

What both apps' builders found missing from the server, and the operator's answer on file sizes.

**A. What the apps need from the Media routes.**
1. **Every item says what Remove does to its file and what it is called.** Items carry
   `keeps_file` (true when the upload came from chat or Shared Files, `from_shelf` 0 or 1, so
   removing the item gives it back; false for an upload made for Media, whose file is deleted)
   and `original_name` (the uploader's own file name, from `user_uploads`, so Download names a
   members-only file properly). The apps then use the spec's exact Remove words for every item.
2. **A featured filter:** `featured=1` on the list (both the GET and the signed POST form), so
   the strip shows every featured item, not just those on the pages loaded.
3. **Picks by a signed POST:** `POST /api/media/picks` with the signed list body plus `of: <key>`
   (purpose `media_list`, as the list), answering like the list, members-only items included for
   a member. The GET form stays for public picks. In the members' view every item also carries
   `picked_by_me`.
4. **A member's own use:** `GET` and `POST /api/media/mine`, signed (purpose `media_mine`),
   answering `{used_bytes, share_bytes, budget_left_bytes, waiting: [{filename, original_name,
   size_bytes, uploaded_at}]}`: the same counting as the room check (`media_bytes_on`), so
   "You have X of your Y left" is the relay's own number, waiting uploads included.
5. **Reporting a Media item:** the reports `CONTEXTS` gain `media`, with an evidence item
   `{kind: "media", id}`. The relay looks the item up as it does a public post: `checked: true`
   when the item exists and its uploader is the reported person, and the evidence it keeps names
   the item's title, kind, labels and visibility, never the file itself (a report hands the admin
   a pointer, not a copy). A members-only item is reportable by members. Both apps' report dialogs
   offer it on a tile; the admins' Reports page shows the item with a link that opens it.
6. **The kinds are one data file for both apps:** `data/media/kinds.json` (added by the desktop
   build) is read by the web page too, and a pair in `tests/page_parity_lint.rs` holds both to it.

**B. File sizes per role, set in Server Settings > Roles (operator, 2026-10-11).** "I figure make
it configurable in the server menu? Like the file size limits?" Two new limits on each role,
editable beside the per-file upload limit in both apps' Roles editors:
1. `max_media_file_mb`: the largest single Media file this role may send (resumable or not). 0
   means "up to the person's Media share", which is today's behaviour and the default for every
   role, so nothing changes until an admin sets it. Checked at the resumable start and at the
   single request with `media=1`, with words naming the limit.
2. `max_private_file_mb`: the largest file this role may send in a DM or a P2P group (an
   encrypted upload). Default 6 for every role, which is what both apps enforce today with a
   built-in number; the server now enforces it on `encrypted=1` uploads (still under the 1 GB
   ceiling of one request), and each person's own peer-list entry carries `private_limit_mb`
   beside `upload_limit_mb`, so the apps use the server's number instead of their own.
   Both are ALTER-added columns on the roles table, with the pre-migration-shape open test
   (CLAUDE.md BUG-046 rule). The type of a private file cannot be limited by the server: it is
   encrypted on the sender's device and the server receives a sealed blob, so the Roles editor
   says so in one line under the field rather than offering a switch that could not work.

**C. Upload tokens end with their connection.** A person with two connections (the chat and a
standalone page such as Media, Trade or Tasks) had the first connection's token replaced in the
peer entry while it stayed valid in `upload_tokens` until the relay restarted (the teardown
returned early while another connection of the same key was open). Each connection's token is
removed when that connection closes, whatever else is open, and a test holds it.

**Proof.** Relay tests for each of A1 to A5, B1, B2 and C, each seen failing once; then the apps'
own tests for what they draw from them.

**As built (relay, 2026-10-11).** The relay half of A1 to A5 and A7, B1 to B5, C, and the
reviews' W3b, W5, W9 and R1 to R7. The apps are written against this. Code: relay/media.rs (routes),
storage/media.rs and its child storage/media_extra.rs, relay/upload_rules.rs (the size limits and
allowances both upload routes share), storage/upload_counts.rs, storage/private_files.rs,
relay/resumable.rs, handlers/reports.rs, storage/roles.rs, relay.rs. Tests:
relay/media_follow_ups_tests.rs, relay/resumable_follow_ups_tests.rs, relay/upload_rules_tests.rs,
the A5 test in handlers/reports_tests.rs, the C and W9 tests in relay/features.rs, the roles
upgrade test in storage/roles.rs; each seen failing once.

- **A1.** Every item carries `original_name` (the uploader's own name for the file, '' when
  unknown), `keeps_file` (true: Remove gives the file back to chat or Shared Files; false: Remove
  deletes it) and `kept` (A7).
- **A2.** `featured=1` on `GET /api/media`, `POST /api/media/list` and both picks routes. Added
  beside it: `id=<n>`, one item by its number when the viewer may see it, which is how the
  Reports page opens a reported item (A5).
- **A3.** `POST /api/media/picks`: the list's signed body (`key, timestamp, sig`, purpose
  `media_list`) plus `of` (whose picks; 400 without it), answered like the list. In the members'
  view every item of every list answer carries `picked_by_me`; the public view has no such field.
- **A4.** `GET /api/media/mine?key=&timestamp=&sig=` and `POST /api/media/mine` with
  `{key, timestamp, sig}`, purpose `media_mine`, members only (403 "Only members of this server
  have a share of its Media space."). Answer: `{used_bytes, share_bytes, budget_left_bytes,
  in_progress_bytes, file_limit_bytes, waiting: [{filename, original_name, size_bytes,
  uploaded_at}]}`. `used_bytes` is the room check's own count (items and uploads waiting to be
  published; kept items left out). Two fields added to the spec: `in_progress_bytes`, the person's
  resumable uploads still arriving (a new resumable start counts them as used, so "left" for a
  resumable upload is `share_bytes - used_bytes - in_progress_bytes`), and `file_limit_bytes`,
  the role's largest Media file (B1; 0 means none, up to the share).
- **A5.** Report context `media`, evidence `{kind: "media", id}`. Kept as a pointer, never the file
  or an address: `{kind: "media", id, uploader, title, media_kind, labels, visibility, checked:
  true}` when the item exists and its uploader is the reported person (the title's links and file
  addresses replaced by a note, as a post's text is); `{kind: "media", id, checked: false}`
  otherwise. Nothing in it names the reporter, so moderators are sent it whole, like admins.
- **A7.** `POST /api/media/keep` `{key, timestamp, sig, id, filename, kept}` (`kept` defaults to
  true), purpose `media_keep:<id>:<on|off>`, for admins, the owner and moderators (403 "Only an
  admin or a moderator can keep Media items on the server."); answer `{ok, id, kept}`.
  `media_items.kept_by` and `kept_at` are in the table's CREATE TABLE. A kept item counts against
  the Media budget and not its uploader's share (`media_bytes_on`), is in no prune list and in no
  person's line of the usage, and the usage answer gains `kept_bytes`. Un-keeping counts it against
  the share again, past it if so: nothing is deleted for that. The uploader can still remove it.
  An admin's or moderator's account erase blanks `kept_by` to '' (the item stays kept); the
  export lists `media_items_kept`.
- **W5.** `media_items.id` is AUTOINCREMENT, so a removed item's number is never reused. Remove
  by number, Feature, Keep, Visibility, Pick and Unpick all require `filename` (the item's stored
  file name, or its `/uploads/` address): none is 400 "Name the item's file too (`filename`), so a
  page that is out of date never acts on another item."; another item's file is 409 "That is not
  the Media item you were looking at: the page is out of date. Load it again."; no such item 404.
  The signed purposes are unchanged.
- **W3b.** `GET /api/media/file/{id}` takes the session's upload token in the `X-Upload-Token`
  header as well as `?token=`. The apps move to the header and show the file through an address of
  their own; `?token=` keeps working for now.
- **W9.** The "no admin yet" notice goes to the socket that signed in, only.
- **B1.** Role field `max_media_file_mb` (0, every role's default: up to the share). Checked at the
  resumable start and on `media=1` single requests: 413 "Media files are limited to N MB for your
  role on this server." A single request keeps `max_upload_mb` too: the tighter of the two holds.
- **B2.** Role field `max_private_file_mb` (default 6, kept 1 to 1024 when saved). An `encrypted=1`
  upload is held to it instead of `max_upload_mb`, with 4 KiB allowed on top for the encryption's
  own bytes, under the 1 GB ceiling: 413 "Files in private conversations are limited to N MB for
  your role on this server." The person's own peer-list entry carries `private_limit_mb`.
- **B3.** Role fields `max_upload_mb_per_day` and `max_uploads_per_hour`, 0 meaning no limit.
  Defaults by tier (new roles and the upgrade alike): Admin 0 and 0; Moderator 4096 MB and 200;
  Verified, Donor and any role on the verified tier 1024 MB and 60; Unverified and anything else
  100 MB and 20. Why: a day's 1024 MB lets a Verified person send a whole default Media share (512
  MB) and about twenty phone videos in a day; 60 uploads an hour lets a burst of phone photos
  through and stops a script; a moderator gets four times that; Unverified cannot upload until an
  admin allows it, and then gets a tenth. The windows are FIXED, the UTC day and the clock hour, so
  the words can say exactly when they reset. Counted by the relay (table `upload_counts`; export
  and erase include it) over every route: a single request (chat, Shared Files, Media, private)
  counts its bytes and one upload when stored, and is refused up front when the hour's uploads
  are used or the file would pass what is left of the day (429, plain words: "Your role may
  upload N MB a day on this server, and you have uploaded X today, and this file would pass that.
  The allowance resets at midnight UTC, in 5 hours 12 minutes." / "Your role may start N uploads
  an hour on this server, and you have started N this hour. More can start at the top of the hour,
  in 12 minutes."). A resumable start counts one upload and is refused when the hour's are used or
  the day's bytes are all used; each piece's bytes count when they arrive, and a piece that would
  pass the day is refused with nothing of it written and the session kept (touched, so it outlives
  the reset). Resumable refusals are JSON: 429 `{error, allowance: "hour"|"day", resets_at}` (Unix
  ms), a piece's adding `offset` and `size`.
- **B4.** `keep_metadata=1` on the single request's query and in the resumable start's JSON
  (stored on the session). The picture strip and the video strip are skipped; a picture is still
  checked to be what it says; an encrypted upload is never opened either way. **Decision:** with
  `keep_metadata` a video the strip could not read through is ACCEPTED, and so is a HEIC or AVIF
  photo (R7): the person chose to keep everything, so there is nothing to check, and the refusal
  existed only because the relay could not clean such a file.
- **B5.** A private file (stored as `.enc`, only an encrypted upload is) leaves after
  `dm_mailbox_ttl_days` counted from its upload, row and file, in the expiry pass. Role field
  `max_private_held_mb`, 0 meaning no limit: Admin 0, Moderator 120, Verified and its tier 60,
  the rest 30, about ten files at the default 6 MB private limit for Verified (a month of a few
  photos a week) and half that for Unverified. Checked with the room under the same lock: 507 "Your
  private files on this server are full (X of N MB). The oldest leaves on 10 November 2026, or
  remove some from Settings > Data." Private files are OUT of the keep-N FIFO: in it, a person's
  DM files pushed out their own room posts, and their room posts deleted the files their DMs
  point at. The peer-list entry carries `private_held_limit_mb` (0 = no limit).
- **The Roles frames.** `role_list` and `role_upsert` carry the five new `RoleDef` fields; a save
  holds each to its range. A role sent WITHOUT them gets the careful defaults (Media 0, private 6,
  100 MB a day, 20 an hour, 30 MB held), so both apps' Roles editors must send all five, or a save
  from an editor that does not know them resets them.
- **C.** Each socket's own upload token is removed when that socket closes, first thing in the
  teardown, whatever else stays open; the registration shows the newest socket's token.
- **R1.** The hash is checked once, before the strip changes the file, and recorded on the session
  (`hash_checked`); a retried finish never hashes again.
- **R2.** A finished session's row stays until the idle expiry with its answer (`done_answer`).
  GET answers `{offset, size, chunk_max, done: <the single request's answer>}`; a PATCH on it
  answers that same JSON and does nothing; a cancel of it removes only the row. The one-piece
  marker is shared with the blocking work (written piece and finish alike), so a dropped request
  keeps it until the work is done and a second request gets 409 busy.
- **R3.** Only a 422 from the strip (a file that cannot be read through, or a HEIC photo) removes
  the session; a read or write failure keeps it.
- **R5.** `store_upload` takes a quick room look, then copies (or renames) the file into the
  destination folder under a hidden `.incoming-` name OUTSIDE the room lock, then under the lock
  does the room check that counts, a rename within the folder and the row. A failure removes the
  partial copy (a resumable upload's pieces' file is renamed back, or kept when it was copied);
  hidden names are never counted, never served at `/uploads` (a dot or `%2e` after a slash is 404)
  and cleared at start.
- **R6.** A Media upload (single request or resumable) is stored in `data/media-waiting/`, which
  nothing serves; publishing moves it to `data/uploads` or `data/media-members` under the room
  lock; the room checks, the usage, the unpublished expiry and a removal by name all reach it
  there. The upload's answer keeps `url: /uploads/<name>` (where it will be once published as
  public), which serves nothing until then.
- **R7.** `application/octet-stream` must have an allowed extension. A file in the MP4 family
  with no movie in it (a HEIC or AVIF photo) is refused on every shelf that is not encrypted (422
  "HEIC and AVIF photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send
  that."), unless `keep_metadata=1`. Honest limit: a video cut off before its movie box reads the
  same way and gets the same words.

## The operator's answers (2026-10-11) and what they mean for the build

Verbatim where it matters, then the decision. Parts marked (relay) are added to the follow-ups
above; parts marked (apps) are the next app increment.

1. **Kept by the server.** "What if the files on media are ones added by admin/mod and remove
   itself from the user max limit. So, if a particular piece of art is great, the admin could be
   like, I love that, continue sharing it forever. Then the user gets a chance to upload a new
   picture." An admin or moderator action, **Keep on the server**, separate from Featured
   (Featured is where an item is shown; Kept is who carries its storage). A kept item stops
   counting against its uploader's share, still counts against the Media budget (the meter shows
   "kept by the server" as its own line), and is left out of the prune lists. The uploader can
   still take it down, since it is theirs; an admin can stop keeping it, and it then counts against
   the uploader's share again (allowed past the share, shown as over it, never deleted for it).
   (relay) `media_items.kept_by` and `kept_at` (the table is new in this release, so they go in
   its CREATE TABLE), `POST /api/media/keep` signed `media_keep:<id>:<on|off>`, `kept` on items,
   the counting in `media_bytes_on` and the usage answer. (apps) the action on a tile and in the
   prune lists, a "kept" badge.
2. **Limits per role, in Server Settings > Roles.** "Make it configurable in the server menu?
   Like the file size limits?" Follow-ups B1 and B2: the largest Media file and the largest
   private file, per role.
3. **How much and how often.** "Should we introduce like limit of number of uploads per user and
   a max upload bandwidth per tier of user with admin using as much as possible?" Two more per-role
   limits beside them: **uploads a day (MB)** and **uploads an hour (count)**, counted by the
   relay over every upload route (chat, Shared Files, Media, private files, resumable pieces as
   they arrive), 0 meaning no limit, which is the Admin role's default. A daily allowance is the
   control that matters for a server's disk and bill; throttling the speed of one connection
   would only make honest uploads slow. Downloads: public visitors get previews and members the
   originals (increment 4); nginx's per-address limits stay the backstop. (relay) B3.
4. **Choosing what hidden information to keep.** "What do you think about adding a step that
   allows for choosing what metadata to exclude/include? ... save last chosen config to the
   attach popup." Yes, and it moves the strip to where it can cover everything: the **sender's
   own app** strips before the file leaves the device, so it covers DMs and groups too, which
   the server can never clean because it only ever receives a sealed blob (until now a photo
   sent in a DM kept where it was taken). The attach step shows what it found ("This photo says
   where it was taken and which camera took it") with one switch, **Remove location and camera
   details**, on by default and remembered as the last choice. The protected setup always strips
   and shows no switch. The server keeps stripping public uploads as a backstop, skipping it only
   when the upload says `keep_metadata=1`, which the apps send only from that switch. (relay) B4:
   the flag. (apps) both apps strip pictures and videos before upload with the same rules as the
   relay's strip (the desktop app uses the same Rust code; the web gets a JavaScript port held to
   the same test files), plus the switch.
   **As built (the web's strip, 2026-10-11), not yet wired into any page.** `web/shared/strip-metadata.js`
   puts `HosStrip` on `window` (and is a CommonJS module under Node). `stripFile(bytes, nameOrType)`
   is a port of `strip_metadata.rs` and `strip_video_metadata.rs` with their rules: JPEG, PNG and
   WebP lose whole segments or chunks, GIF passes through untouched as on the relay, and MP4, MOV
   and 3GP boxes are zeroed and renamed `free` in a copy of the same length (the caller's bytes
   are never written). A picture that cannot be walked comes back unchanged and a video that stops
   early says where; both are marked `unfinished`, which is answer 5's "Send anyway?" case. HEIC
   and AVIF are not cleaned (no HEIF strip on main yet) and the report says so. The bytes decide
   the kind; the name or type is reported beside it. `describe(bytes, nameOrType)` gives the attach
   step its sentence ("This photo says where it was taken and which camera took it.") and plain-word
   details, only for what the strip removes, with coordinates only when asked for.
   `scripts/tests/strip-metadata.test.js` mirrors the Rust tests, and where rustc is installed it
   compiles a small program around the relay's two real source files and requires the same bytes
   and the same reports on about 9,500 inputs; it is not yet in `just rig-tests`.
5. **Files that cannot be fully cleaned.** "Refusal makes sense from a safety standpoint. But
   maybe there might be a reason to keep it? Maybe someone is trying to help someone else fix the
   file?" Public shelves keep refusing them: the server cannot ask every future viewer, and public
   is public. In a DM or a group, the sender's app asks instead: "This video could not be read all
   the way through, so where it was filmed may still be in it. Send it anyway?" The sender decides,
   and the file carries a note the recipient sees ("The sender's app could not check this file for
   hidden location"). That covers helping someone with a broken file without a special pass or
   command, because a DM already is the private channel between the two people. (apps)
6. **Settings for files in DMs and groups.** "Maybe someone wants no files sent to them. Maybe
   they want to be able to send but, not receive. Maybe someone only wants to receive." A
   **Files in messages** group in Settings > Privacy, beside "who can reach me":
   - **Accept files from:** Nobody / Friends / Anyone I talk to (default: Friends, the same line
     the click-to-load rule already draws). "Send but not receive" is Nobody here.
   - **Show pictures without asking from:** Friends / Nobody.
   - **Show the attach button in private conversations:** on or off ("receive only" is off).
   The RECEIVING app enforces it: it never downloads a file from someone the setting does not
   allow, and shows "Ann sent a file. You accept files only from friends. Accept this one?" The
   relay cannot enforce it and must not try: with sealed sender it cannot tell which DM carries a
   file, and making it able to would tell it who sends files to whom. The setting rides in the
   person's signed profile (`accepts_files`), so a sender's app can say so before anything is
   sent. For a P2P group, its owner sets "Files allowed" and "Largest file" in the signed group
   object; each member's own setting still applies on top. (apps)
7. **The nginx change.** "Yes, do whatever you believe is best for this system long term. No
   bandaids, the real solution." Applied on the server with this release.
8. **The attach tooltip over the chat bar.** "We could just force it to be above the text box
   instead of overlaying the text input area that way people still see it?" Both apps: the hints
   of the composer's buttons open above the whole composer, never over the text being typed
   (done with this release).
9. **How long the server keeps files sent in DMs and groups.** "Yes. Seems to make sense to link
   the file sharing to the DM messages life." A private file (an encrypted upload) is removed
   from the server after the DM mailbox's own lifetime (`dm_mailbox_ttl_days`, 30 by default, the
   same server setting, so the two can never disagree), by the expiry pass, row and file. The apps
   keep their own copy of any private file the person has opened (encrypted at rest with the same
   seed-derived key as their DM history), so nothing already looked at disappears. (relay) B5.
   (apps) the local copy.
10. **One person flooding the server.** "If someone wants to flood our server with files to a DMd
   user, then couldn't they mess things up? Say we have 1TB or storage for files and someone sends
   a 1TB file to a user that's not online. Does that prevent everyone else from sending files in
   DMs? Should we require the users to ... have had an active message within some limit ... and
   then send them P2P instead of through our server?" As built until now, yes in part: chat and
   private files share one pool (`max_total_upload_mb`), and whoever fills it blocks everyone's
   files until something expires. What stops it, layer by layer:
   - only a role with upload rights can upload at all (someone who just arrived cannot until an
     admin verifies them), and each role has a largest private file (B2), a daily allowance and an
     hourly count (B3);
   - **new:** each person may hold at most `max_private_held_mb` of private files on the server at
     once (per role, like the Media share), so one person can never take the whole pool, and the
     words when it is full say it is THEIR room that is full, not the server's;
   - private files leave after the mailbox lifetime (answer 9), so the pool drains by itself;
   - **files bigger than the server's private limit never touch its disk: they go device to
     device**, only while both people are online and only once the receiver accepts that transfer
     ("Ann wants to send you clip.mp4, 2.3 GB. Accept?"). The receiver's yes is the consent the
     operator asked for, stronger than "they messaged recently". Device to device shows each side
     the other's network address, so between people who are not friends it goes through the call
     forwarder (the server's bandwidth, never its disk, and capped per transfer like a call).
   (relay) B5 (the per-person cap). (apps) the device-to-device transfer is its own increment.
