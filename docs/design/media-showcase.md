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

**As built (web, round 2, 2026-10-11).** The web Media page against the relay above, and the fixes
the review of the round 1 page asked for. Code: web/pages/media-app.js (the page),
web/pages/media-upload.js (new: the resumable upload and the uploads remembered in the browser),
web/pages/media-admin.js (the admins' section), web/shared/relay-session.js (now keeps the role
list), web/shared/pq-relay-auth.js, web/shared/report.js, web/chat/chat-reports.js,
web/chat/chat-messages.js and app.js (the private limit). Tests: scripts/tests/media-web.test.js
(round 1, moved onto a shared harness, scripts/tests/media-web-harness.js) and
scripts/tests/media-web-round2.test.js, one test in private-files-web.test.js, the contexts check
in report-web.test.js, and a `Media (kinds)` pair in tests/page_parity_lint.rs; each seen failing
once against a single break in a copy of web/.

- **No two signed requests share a timestamp.** getPqSignedAuth takes the next millisecond after the
  last one it signed (`_nextTimestamp`, like the desktop's `next_timestamp`), so Accept on a
  suggestion, which reads the list twice at once, is never refused as a replay.
- **W5.** Pick, Unpick, Remove by number, Feature and Keep send `filename`: the item's `filename`
  when the answer carries one, else the name in its `/uploads/` address. A 409 "not the Media item
  you were looking at" loads the list and the strip again and says so ("the page was out of date,
  so it has been loaded again. Nothing was changed."). **Gap found, relay side:** `item_json` gives
  a members-only item's address as `/api/media/file/<id>` and carries no stored file name, so
  neither app can name such an item's file; the web page then sends nothing and says why. The fix
  is one field, `"filename": i.upload_filename`, in `item_json` (the name is not a way in: the
  members' folder is served by nothing, and only members are sent members-only items); the page
  already reads it.
- **A4.** "You have X of your Y left" is `share_bytes - used_bytes - in_progress_bytes` from
  `POST /api/media/mine` (an upload of the same file already part way up is not counted twice),
  with the role's largest Media file (`file_limit_bytes`) and what the whole space has left
  (`budget_left_bytes`) as stops too; asked again after every upload, publish and removal. The
  person's uploads waiting to be published are listed with Publish (the form, with the visibility
  choice) and Remove (by file name).
- **W3b.** Members-only files are fetched with the token in `X-Upload-Token` and shown through
  object URLs, at most four fetched at once; no token is in any address on the page. An object URL
  lives while its item is shown (the grid, the strip or the viewer) and is revoked when it is not,
  and all of them when the page goes. Download saves the fetched file under `original_name`; a
  model or file is fetched only when Download is pressed.
- **The viewer** is drawn again only when what it shows changes (its item, or a members-only file
  arriving), so a playing video keeps playing through a reconnect, a new token or a list reload.
- **A2 and A6.** The strip asks for `featured=1` (up to 100); the kind filter and each kind's name
  and mark come from data/media/kinds.json, which the desktop reads too.
- **Settings Save** ends at the relay's answer to it: a `server_settings_state` with no `target`
  (an admin's save) carrying what was sent, as the relay stores it; any other notice than another
  admin's save, the game clock or a suggestion's answer (so "Only admins can update server
  settings.", "Update failed: ..." and "Server settings row missing ..." all end it); a dropped
  connection; or 15 seconds with no answer. The answer to the page's own settings request never
  counts.
- **Add to Media** only when the person's role may upload: no role, a role with `can_upload` off,
  or the role list not here yet (it comes with the settings) means no button; a role known not to
  upload gets one line saying why.
- **A7.** Keep on the server and Stop keeping on tiles for admins and moderators (signed
  `media_keep:<id>:<on|off>`), a "Kept by the server" badge for everyone, Keep in the admins' prune
  lists, and `kept_bytes` on its own line under the meter.
- **A3.** My picks is `POST /api/media/picks` (the list's signed body plus `of`), members-only
  picks included; Pick and Unpick follow each item's `picked_by_me`.
- **A1.** Remove's confirmation from `keeps_file`: "Remove from Media. The file stays where it came
  from." or "Remove from Media and delete the file." (the either-way words only when it is absent).
- **A5.** Report on a tile: the reasons from data/safety/report_reasons.json with their help, the
  help outside this server for the two danger reasons (the chat dialog's rules, report.js), a note,
  and `report_v2` with context `media` and evidence `[{kind: "media", id}]`, signed by the reporter
  over the relay's words and sent on the page's own socket; `report_received` closes it with the
  chat's line, a refusal shows in the dialog. report.js gained the context, `mediaEvidenceItem` and
  the Media fields in `reportItemView`; the chat's Reports view shows such evidence by title, kind,
  labels and visibility (names from the two data files) with an Open to `/media?id=<n>`, which the
  page loads with `id=<n>` on the list route and opens in the viewer. Not on the web report: "Also
  block them" (the Media page has no block list) and the recovery-phrase check on the note (it
  lives in the chat's warnings script).
- **B2.** The chat's private-file limit is the person's own peer-list `private_limit_mb`
  (`window.myPrivateLimitBytes`), 6 MB only until the server has said it; the room limit no longer
  caps it.
- **Resumable uploads.** Every Add to Media file goes this way (no `blake3`): pieces of the server's
  `chunk_max`, a progress bar, Pause (after the piece on its way) and Resume, Cancel (DELETE), a
  dropped connection waited out (2 s doubling to 30 s) then `GET` for where the server got, a token
  that changed (the page signed in again) waited for, a piece at the wrong place taken from the
  409's `offset`, a busy 409 answered by asking again, the 429 allowance in the relay's words plus
  when it resets on this device's clock with Resume, a 507 at the finish in its words with Retry
  sending the empty piece, and `done` taken from `GET` when an answer was lost. The upload is
  remembered in localStorage (`humanity_media_uploads`: whose, the session id, the file's name, size
  and last change; every read and write wrapped) and picking the same file after a reload carries
  on. The publish is signed after the upload.
- **Phones.** `.media-select` is held to the width it has, so a long category or person name no
  longer widens the filters past 375 px.
- **Not done here.** The Roles editor fields (follow-up B and answers 2 and 3): the web has no
  Roles editor (Settings shows a fixed description of the roles), so the five limits are edited in
  the desktop app only. The sender-side metadata strip and its switch (answer 4), the
  can-not-be-read-through question in DMs (answer 5), Files in messages (answer 6) and the local
  copy of opened private files (answer 9) are their own increments.

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

## As built (desktop, round 2; 2026-10-11)

The desktop app's half of the follow-ups and the operator's answers 1 to 3, the resumable upload,
and the fixes from the review of the desktop Media page. Native first; the web mirrors it in its
own round. Every test below was seen failing once against a deliberate break, noted at the test.

**Every action names the item's file (W5).** Remove by number, Pick, Unpick, Feature and Keep send
`filename`, the item's stored name: the item's own `filename` field when the server sends it (the
relay adds it to every item for members-only items, whose `url` holds no name), else the name in
its `/uploads/` address (`Item::stored`, `Item::reference`). An item whose name is not known is not
acted on, and the page says why. A 409 to any of these reloads the list, the strip and the share
and says "That item changed since this page was loaded, so nothing was done. The page has been
loaded again." (`STALE_LINE`). Requests return a `Refusal` (status and words) so the page can tell a
409 from any other refusal.

**No upload token in any words.** The members' file route is asked with the token in the
`X-Upload-Token` header (pictures through `image_cache::download_bytes_with`, the card's Download
and Play here through `open::fetch_to`), never in an address. Every error said or logged (the
upload's `upload_error_words`, the image cache's fetch, the Media requests, the resumable routes,
which read the token from the address) goes through `image_cache::hide_token`, which turns
`token=...` into `token=(hidden)`.

**Cancel stops the upload and the publish; Remove's words are the item's own.** Before the file is
up, Cancel stops the worker, which tells the server (`DELETE`); once it is up and its publish is on
its way, the publish's answer is followed at once by the upload's removal by name. The window's
own close does nothing while something is being sent: only Cancel stops it. Remove's confirmation
reads `keeps_file`. A publish that fails after the upload turns the form into a publish of the
waiting upload, so pressing again does not upload again.

**The share is the server's count.** `POST /api/media/mine` (signed `media_mine`) answers "You have
X of your Y left" (share less used less what is still arriving) and the stop before a byte is
sent, which also stops a file over the role's largest Media file or over what the whole space has
left. It is asked again when the form opens and after every upload, publish and remove. Its
`waiting` uploads are listed on the page with Put it on Media (the form, visibility choice and
all) and Remove (by file name, after a confirmation).

**Picks, featured, kept.** My picks is `POST /api/media/picks` with the signed list body and `of`,
paged by More like the list; Pick or Unpick reads each item's `picked_by_me` and a pick updates
every copy of the item on the page. The strip is every featured item (`featured=1`, up to ten pages
of 48). A kept item shows "Kept by the server"; admins and moderators get Keep on the server and
Stop keeping on tiles, admins Keep on the prune rows; the meter has a line for `kept_bytes`.

**The category sent is the category shown.** With none chosen the form shows the general category
(id 1, by the name the server gives it) and sends its number; a category the server no longer
lists falls back to it, shown and sent.

**Unsaved category edits survive a refresh** (an answer updates what the page knows, not the
working copy) until Save or Discard changes. **The Media settings say when the server's settings
changed under unsaved edits**, as Server policy does.

**The page belongs to one server and one identity.** A restore from a recovery phrase (or any
change of key) forgets it as a server change does (`sync_server` compares both). An upload under
way is stopped where it is (`Control::abandon`: the server keeps it a day), and the record of an
unfinished upload says whose it is, so it is offered only to that person on that server. The Files
page's shared list is forgotten on a server switch too (`files::forget_other_server`).

**Pictures and files.** A members-only picture refused under one upload token is asked for again
when the token changes (a reconnect), and any failed picture says so and tries again on a click.
A picture over the image cache's 16 MB says so and opens its card, whose Download takes any size;
a members-only picture opens its card too (with the picture and View full size), since the image
viewer offers no Download for a token-fetched file. Download is named by `original_name`, so a
members-only file keeps its extension. Download and Play here have no limit on a whole transfer's
time, only on a connection that will not open or goes quiet for a minute. Play here's temporary
copy is deleted when its card closes, at exit (engine/quit.rs) and, for what a crash left, at the
next start; a failed or cut-off fetch leaves no partial file.

**Report.** A tile's Report opens the existing dialog with the context `media` and the evidence
`{kind: "media", id}`. The Reports page shows such evidence as what the server kept ("Media item
12: "Dusk", a video, labelled Flashing lights, members only") with Open the item, which opens the
Media page on that item alone (`id=<n>` on the list) with Show everything.

**Roles.** Server Settings > Roles has an "Upload limits per role" table under the roles table:
Per file (the existing `max_upload_mb`), Media file (0 = up to the share), Private file, Private
held, A day and An hour (0 = no limit), with the line about private files under it. It edits the
same working copies as the roles table, and every role save sends the whole role, the five
included. Their ranges are read through the relay's own `RoleDef::with_limits_in_range`. The add-role
form's Prefill sets them by tier as the relay's defaults do.

**The chat's private-file limit is the server's.** Each person's own peer-list entry's
`private_limit_mb` is the limit on a file in a DM, a group or the scratchpad (6 MB until the server
says; never past 1 GB). The receiving side now takes a file up to the relay's 1 GB ceiling plus its
4 KiB: a picture shown in place is still fetched into memory, up to the image cache's 16 MB; Save
streams the ciphertext to a hidden file in the downloads folder and decrypts it from there a
megabyte at a time (`net/dm_file_stream.rs`: GHASH over the whole file first, so nothing is written
unless the tag holds, then AES in counter mode), so a gigabyte is never held in memory. It uses
`ctr` and `ghash`, the versions aes-gcm itself already pulled in (Cargo.lock gained two dependency
lines and no crate); the tests hold it to `decrypt_attachment` on the same bytes.

**Add to Media is the resumable upload** (`media/resumable.rs` the work, `media/uploading.rs` the
page's side). On a worker: the file's BLAKE3 a megabyte at a time, the start (`media: 1`,
`blake3`), then pieces of the server's `chunk_max`; a 409 with an `offset` is followed, a 409
`busy` waits and asks; a dropped connection or a 5xx waits a little longer each time (1 s doubling
to 30 s), asks the status and carries on; a 429 shows the server's words and when it resets, and
carries on by itself then (or at Resume); a 507 at the finish shows its words and Retry sends the
empty piece at the end; a finish whose answer was lost is found in the status answer's `done`; a
refused token (a reconnect) waits for the page's new one. A progress bar with Pause and Resume; the
publish happens after the upload, signed then; the upload carries on whatever page is open
(`poll_upload`, every frame). When the server gives the upload its id, the app's config keeps
`media_unfinished_upload` (server, identity, id, path, size, modification time, and what the form
said); after a restart the page offers Carry on, which reopens the form and sends only the rest,
and only from the same file unchanged (otherwise the server is told to drop it and the person
why). One upload at a time: starting a new one cancels an older unfinished one.

**Not in this round** (the operator's answers 4, 5, 6, 9 and 10): the sender's own strip of
hidden information with its switch, the "could not be checked" prompt for DMs, the Files in
messages settings, the local copies of opened private files, and device-to-device transfer. They
are their own increments.

## HEIC and AVIF photos cleaned in place (relay, 2026-10-11)

**As built.** The R7 refusal above is replaced for every HEIC or AVIF photo the relay can read
through: its location is removed and the photo is stored. Code: `src/relay/core/strip_heif_metadata.rs`,
called from upload_rules.rs `finish_file_as`, so both upload routes use it. Tests: the module's
own twelve, and four in relay/upload_rules_tests.rs; each seen failing once against a deliberate
break, the doc comments say how.

- **What is removed.** A HEIF still keeps its picture as numbered items in the top-level `meta`
  box, found through two tables there: `iinf` (each item's number and type) and `iloc` (where each
  item's bytes are). The strip reads both and overwrites with zeros, where they stand, every byte
  of every `Exif` item (on a phone: the GPS position, the time, make, model, lens, Apple's maker
  notes) and of every `mime` item of type `application/rdf+xml` (XMP). The file keeps its exact
  size and nothing moves, so the picture, its thumbnail, its tiles, the item properties (`iprp`),
  the references (`iref`) and the tables are byte-identical. The video strip runs first and clears
  an XMP box at the top level, if there is one.
- **What it reads.** An `ftyp` naming `heic`, `heix`, `heim`, `heis`, `mif1`, `msf1`, `avif` or
  `avis` (as the major brand or a compatible one) and no `moov`; one top-level `meta` (version 0);
  `iinf` versions 0 and 1 with `infe` entries of versions 2 and 3; `iloc` versions 0, 1 and 2 with
  offset, length, base offset and index sizes of 0, 4 or 8 bytes; construction method 0 (file
  offsets) and 1 (offsets inside `meta/idat`).
- **API.** `strip_heif_metadata_in_place(path) -> io::Result<HeifStrip>`, the same on bytes in
  memory (`strip_heif_metadata_in_buffer`), and `is_heif_still(first_bytes)`. `HeifStrip` reports
  the kind (`Still`, `WithMovie`, `NotHeif`), each item zeroed (item number, item type, bytes),
  the byte ranges written, and a plain sentence for anything not done (`stopped_early`);
  `complete()` is true only for a still whose tables were read through and whose every Exif and
  XMP item was zeroed.
- **Safety.** An extent is zeroed only when it lies wholly inside an `mdat` box's body (method 0)
  or the `idat` box's body (method 1) and touches no byte of any other item; otherwise that whole
  item is left as it is, and said. When the tables themselves cannot be read with certainty (a
  size that makes no sense, a version not listed above, two `iinf`, `iloc`, `idat` or `meta`
  boxes, an item number listed twice, a table cut short), nothing is written at all. The file is
  read through a read-only handle and opened for writing only when there is something to zero, so
  a video, a JPEG or a HEIC with nothing in it is never opened for writing; the zeros are synced
  before the upload is stored. All arithmetic is checked, and 4,000 fixed-seed random inputs (and
  every cut of a whole picture) run without a panic, a size change, or a byte changed outside a
  reported range.
- **In the upload rules.** `.heic`, `.heif` and `.avif`, and the declared types `image/heic`,
  `image/heif` and `image/avif`, are allowed. Such a file never takes the picture path (no PNG or
  JPEG magic check, no 64 MB picture cap; the role's own file limit holds): it is a plain file,
  stored under `.heic`, `.heif` or `.avif`, the upload's answer says `"type": "document"`, and on
  the Media page it is a "file" (`kind_of`), because neither app shows HEIC or AVIF inline. A
  `.heic` DECLARED as `image/jpeg` is judged as a JPEG and refused when it is not one. A still that
  is not `complete()` is refused with the R7 words as they were ("HEIC and AVIF photos cannot be
  cleaned of where they were taken yet. Save it as a JPEG and send that."), unless the person
  chose `keep_metadata=1` (B4), which takes it as sent. A file in the MP4 family with no movie and
  no HEIF brand, a video cut off before its movie box, is now told so (`VIDEO_UNREADABLE`) instead
  of being called a photo: that was R7's honest limit.

**What it does not cover (honest limits).**
- Only `Exif` items and XMP `mime` items are cleaned. Left as they are: item names, `uri ` items,
  `mime` items of any other type, the colour profile and other item properties, the pixels
  themselves (a street sign or a landmark in the picture still says where it is), and auxiliary
  pictures (depth map, Portrait matte, HDR gain map) as pictures. A preview kept as a JPEG item
  (item type `jpeg`) could carry an EXIF block inside its own bytes, which is not looked for
  (iPhones keep HEVC thumbnails, which cannot). XMP attached to an auxiliary picture is zeroed
  with the rest, so an iPhone HDR photo may lose its HDR look on Apple devices (not checked
  against a real file).
- The zeroed items stay listed in the tables: a viewer finds an Exif item full of zeros. Readers
  are expected to skip an Exif block they cannot parse, but no real viewer was tried (the tests
  build files byte by byte and the relay has no HEIF decoder); the picture's own items are
  untouched either way.
- Refused rather than cleaned: an Exif or XMP item built from other items (construction method
  2), kept through a data reference, given an extent of length 0 ("to the end"), lying outside
  `mdat` and `idat`, or overlapping another item; the table shapes listed under Safety; a file
  cut short. `infe` versions 0 and 1 (allowed by the base format, not by HEIF) are refused too.
- A HEIF file with a movie in it (an image sequence or burst, `msf1`, `heis` or `avis` with a
  `moov`) is left to the video strip, which neutralises a top-level `meta` in a file with a movie,
  cover picture and Exif together; it is not cleaned item by item.
- Not scrubbed, as with the video strip: stale bytes inside `free` boxes, data after the last box,
  and a `udta` box at the top level of a still (not a HEIF structure, but nothing stops a tool
  writing one).
- A file NAMED `.heic` whose bytes are not a HEIF file (a JPEG renamed) is stored as a plain
  `.heic` with its EXIF: only what the bytes are is cleaned. The same was already true of a
  picture renamed to any other allowed extension (`.pdf`, `.zip`); only pictures are checked to be
  what they say. A HEIC named `.mp4` is cleaned and stored as `.mp4`.
- The apps' attach pickers still list only PNG, JPEG, GIF and WebP among pictures; a HEIC reaches
  the relay from the web as whatever the browser declares, and from a file sent as
  `application/octet-stream` by its extension. Comments in api.rs `upload_file` and resumable.rs
  `finish` still say a HEIC is refused; they call `finish_file_as` and are out of date until those
  files are next edited. resumable_follow_ups_tests.rs keeps its HEIC refusal test: its synthetic
  photo has an item list that cannot be read, so it is still refused, for the right reason now.

## Round 3: fixes from the round 2 review (2026-10-11)

Three critics read everything merged since the first review (relay fc091c413..22bc6a6b5, desktop
6c39a4403, web aeca812d3) and found 24 defects. The fixes, by owner. Relay first; the apps build
the parts that need the relay against the shapes below.

**Relay.**
- R3-1 (review 1): the day and hour allowances are counted INSIDE the store, under the room lock,
  dated by the store time, on every route: concurrent single requests can no longer all pass, a
  connection closed during the store cannot leave an uncounted file, and an upload crossing
  midnight UTC counts in the day it was stored.
- R3-2 (review 2): the identify peer list sends the socket's OWN token, never the registration
  snapshot's (two sockets of one person reconnecting together could both get one token, and
  closing one then broke the other).
- R3-3 (review 3): the role's largest Media file is checked when publishing too.
- R3-4 (review 4): the HEIF strip protects every item whose bytes are in this file, a
  self-contained data reference included.
- R3-5 (review 5): a resumable upload's only copy is never lost moving into place: a failed or
  interrupted move puts the pieces file back with its session, and start-up restores a staged
  file to its session rather than deleting it.
- R3-6 (for the apps): a 429 allowance answer carries `resets_in_s` (seconds, by the server's own
  clock) and `left_bytes` (what is left of the window), and a piece smaller than `chunk_max` is
  accepted, so an app can send what still fits and then wait the server's own number of seconds.
- R3-7 (for the apps): `/api/media/mine` lists `uploads_in_progress: [{id, filename, size,
  offset, touched_at}]`, the person's own resumable sessions, so an app can show and cancel one
  it lost track of.
- R3-8 (for the apps): watching or downloading a members-only file without a token in any
  address: `POST /api/media/ticket` (token in `X-Upload-Token`, body `{id, filename}`) answers
  `{url: "/api/media/file/<id>?ticket=<random>", expires_in_s: 600}`. A ticket opens that one file
  for ten minutes (range requests included, so a video streams), reveals nothing about the
  session, and a copied link stops working. Kept in memory only.
- R3-9: the resumable routes take the token from `X-Upload-Token` too (it is in their query
  today), and nginx answers a refused rate with 429, not 503 (`limit_req_status 429`), in the
  repo config and the guide.

**Both apps.** A 5xx or a 429 without the allowance's JSON is "wait and try again" with back-off,
never the end of an upload; the same server error ten times running shows the server's own words
with Retry. Pause stops every request in every phase. An allowance wait is the server's
`resets_in_s` (at least 5 seconds), and a piece is cut to `left_bytes` when that is smaller. A
file that can no longer be read ends the upload with words saying so. Uploads in progress are a
list on the page (each with its own bar, Pause and Cancel); opening another form never orphans
one, and an upload is published by its own form. Unfinished server sessions from
`uploads_in_progress` are shown with Cancel. Members-only video and big downloads use tickets
(R3-8); pictures stay fetched with the header and are cancelled when no longer shown. The
resumable routes send the token in the header (R3-9).

**Desktop only** (review 1 to 11): Put on Media from Files never replaces an upload's form;
private pictures over 16 MB get Save; downloads and Save write to a hidden part file and rename
when complete, and leftovers are swept at start and exit; Play here's temporary name is unique per
fetch; sending a private file reads it off the UI thread and never holds three copies;
`chunk_max` is held to at most 64 MiB; `private_limit_bytes` resets with the server and identity;
a publish answered after a quit or switch is recognised (the relay's "already on the Media page"
for one's own item counts as done).

**Web only** (review 1 to 8): the record of an upload to resume is dated by its last progress and
forgotten when its session is cancelled or fails; Close on a failed upload cancels it on the
server; resuming checks a fingerprint of the file's first and last megabyte, not only its name,
size and date; `/media?id=` keeps the label cover and the click-to-load rule; members-only
pictures are fetched with an AbortController and are not re-fetched by a reload; private files a
browser cannot hold are a Save card, not an inline picture (pictures inline up to 16 MB, Save up to
512 MB, above that "open it in the desktop app"), and an opened private file is not downloaded
again on every redraw.

**As built (relay, round 3; 2026-10-11).** R3-1 to R3-9 as written above; the apps build against
the shapes below. Code: relay/media.rs (`store_upload`, `Allowance`, the file route, `mine_core`,
`publish_core`), relay/media_ticket.rs (new), relay/resumable.rs (`write_counted`,
`session_token`, the finish), relay/upload_rules.rs, relay/upload_limits.rs (`move_file`,
`unstage`), relay/api.rs (`upload_file_in`), relay/handlers/live_conns.rs (`peer_list_for`),
storage/upload_sessions.rs (`staged_part_name`, `restore_staged_part`, `settle_uploads_at_start`),
storage/upload_counts.rs, storage/media.rs, core/strip_heif_metadata.rs, relay/mod.rs (the route
and the start-up order), scripts/nginx/humanity.conf and docs/admin/SELF-HOSTING.md. Tests:
relay/upload_round3_tests.rs (R3-1, the single request), relay/resumable_round3_tests.rs (R3-1
for pieces, R3-5, R3-6, R3-9), relay/media_round3_tests.rs (R3-3, R3-7, R3-8), and one each in
handlers/live_conns.rs (R3-2) and core/strip_heif_metadata.rs (R3-4); each seen failing once
against a deliberate break, named at the test.

- **R3-1.** The day's and the hour's allowances are checked and counted together under the room
  lock, at that moment's time, inside the blocking work, on every route. A single request: the
  check at its start stays, only as an early refusal (a file that cannot fit is never read); the
  store looks again before the file is put in place, then checks and counts with the room check
  that counts (`allowance_at_store`: the hour's uploads, and this file's bytes against what is
  left of the day; 429, the same plain words as before). A resumable start checks and counts its
  upload under the lock. A piece's bytes are checked and counted under the lock when it arrives,
  then the piece is written and the offset stored, all in the blocking work; a write that fails,
  or a session cancelled meanwhile, takes the bytes back off the count
  (`Storage::take_back_upload_count`). The relay reads the time for this from
  `RelayState::upload_now_ms` (the wall clock; a test sets its own).
- **R3-2.** The identify peer list is built by `live_conns::peer_list_for`, which puts THIS
  socket's own token on the person's own entry.
- **R3-3.** `publish_core` reads the publisher's role and `publish_media_placing` refuses a file
  bigger than its `max_media_file_mb` (0: none): 413 with the upload's own words, "Media files are
  limited to N MB for your role on this server." (`Refusal::TooLarge`, new).
- **R3-4.** Every item that is not Exif or XMP is protected, whatever its data reference. The
  `dref` table is not read, so an item really kept in another file guards bytes it does not have:
  that can only refuse a photo, never damage one.
- **R3-5.** A finish stages the pieces file in the Media waiting folder as
  `.incoming-resume-<session id>` (`staged_part_name`). A refusal after that moves it back
  (`move_file`: a rename, else a copy that takes the name only when whole); if it cannot go back
  it is KEPT there under that name, never deleted, and the next finish brings it back first (an
  error 500 "could not be put back in place on the server yet" while it cannot). At start,
  `settle_uploads_at_start` runs in this order: a staged pieces file of a session still in
  progress goes back to the resume folder; one whose session is gone, finished, or already has
  its pieces file is deleted; then the plain half copies are cleared (`clear_staged_files` never
  touches the session names, so a database that cannot be read deletes nothing); then the
  reconciliation. A cancel removes a staged pieces file too.
- **R3-6.** A 429 allowance answer from the resumable routes is `{error, allowance: "day"|"hour",
  resets_at, resets_in_s, left_bytes}` (a piece's adds `offset` and `size`). `resets_in_s` is whole
  seconds to `resets_at` by the relay's clock, rounded up. `left_bytes` is what the day still
  allows (the day's allowance less what is counted); 0 when the HOUR's allowance refused (no
  upload can start until it resets) and for a day used up. Any piece up to `chunk_max` was and is
  taken, a piece of exactly `left_bytes` included. The single request's 429s stay plain words.
- **R3-7.** `GET` and `POST /api/media/mine` add `uploads_in_progress: [{id, filename, size,
  offset, touched_at}]`: the member's own resumable sessions still arriving, oldest first
  (`touched_at` in Unix ms). A finished one is not in it (its file is in `waiting`).
- **R3-8.** `POST /api/media/ticket`, the upload token in `X-Upload-Token` ONLY (no `?token=`
  here), body `{id, filename}`, answers `{url: "/api/media/file/<id>?ticket=<64 hex characters>",
  expires_in_s: 600}`. Refusals: 401 no header, 403 a token that is not a session, someone who is
  not a member or is banned, 400 or 404 or 409 as every write that names an item (W5), 429 when
  the person already holds 256 live tickets (or the relay 50,000). `GET
  /api/media/file/<id>?ticket=` serves the file as the token does (ranges, 206, attachment,
  `private, no-store`) while the ticket is live (ten minutes), was given for that item, the item's
  file is still the one it was given for, and the person it was given to is still a member and
  not banned (checked at every request, so a ban ends their tickets). Anything else is 403 with
  one sentence for every case: "That link to a members-only file has run out (links last ten
  minutes) or is not for this file. Open the file again from the Media page." A refused ticket
  falls back to a token when the request also carries one. Tickets are 256 random bits from the
  operating system, kept in memory only (`RelayState::media_tickets`; a restart ends them).
- **R3-9.** All four resumable routes read the token through `session_token`: the
  `X-Upload-Token` header, else `?token=`; the header wins when both are sent. Both upload
  locations, in scripts/nginx/humanity.conf and in the guide, carry `limit_req_status 429;`.
  **The live server's nginx still needs the same line** in its two upload locations (and
  `nginx -t` then a reload); this round changed only the repo's copy.

**Honest limits.** A relay that stops in the moment between a finish's last rename and its
`user_uploads` row (one rename and one insert, under the lock) leaves the file stored in the
waiting folder with no row, and the next start drops its session as one with no pieces file: the
file is not lost, but nothing lists it, the expiry pass does not know it, and the person sends it
again. The staged name keeps the pieces file, which is the risk this round was about.
`resets_in_s` is the relay's own count and does not know the app's clock; an app that waits it out
and finds the allowance still used (a second upload of the same person counted meanwhile) is
refused again with a new number.

**As built (web, round 3, 2026-10-11).** The web's half of round 3: the "Both apps" and "Web only"
items, written against the relay shapes R3-6 to R3-9 above before they landed and tested against
stand-in answers in those shapes, then checked against the relay's note just above once it landed
(the 429 allowance JSON, `uploads_in_progress`, the ticket's request, answer and refusals, the
header on the four resumable routes): they match, and nothing on the web changed for it. Code: web/pages/media-upload.js (the resumable upload),
web/pages/media-app.js (the page), web/chat/chat-dms.js and chat-messages.js (private files),
web/shared/media.css and web/chat/messages.css. Tests: scripts/tests/media-web-round3.test.js (new,
17 tests), three "review 7" tests at the end of scripts/tests/private-files-web.test.js, and the
round 2 tests whose expectations round 3 changes (D, E, N3, N4 and N8 in media-web-round2.test.js,
the start's header in media-web.test.js test 6), each changed test saying how. Every one was seen
failing once against a single break in a copy of web/ (34 breaks in all, the break and the words it
tripped noted above each test).

- **Review 1, server errors.** A 5xx (any but 507) or a 429 without the allowance's JSON (nginx's
  own page) is "wait and try again", at the start and on a piece: 2 s doubling to 30 s, then the
  upload asks where the server got and carries on. The same status ten times running stops in
  `server_error` with the server's words and Retry (which starts a fresh count) and Cancel. A
  proxy's HTML is never shown: such an answer reads "The server answered 503.". A 409 busy is asked
  again after the same growing wait (it used to fire about once a second).
- **Both apps, Pause.** Pause stops the request on its way (an AbortController on every request but
  the start, whose answer names the session that a Cancel then deletes) and cuts short every wait:
  a dropped connection's, a server error's, the busy wait and an allowance's. No request is made
  after it, and Resume first asks the server where it got, since a stopped piece may have arrived.
- **Both apps, the allowance.** A 429 with `allowance` (or `resets_in_s`): when `left_bytes` is
  smaller than the piece, the piece is cut to it and sent at once; otherwise the upload waits the
  server's `resets_in_s`, at least 5 seconds, says when that is on this device's clock ("It carries
  on by itself then."), and carries on by itself, with Pause and Cancel while it waits. Before
  R3-6, `resets_at` against this device's clock is the fallback.
- **Both apps, R3-9.** Every resumable request (start, status, piece, cancel) carries the token in
  `X-Upload-Token`; none has it in its address.
- **Review 4, a file that can no longer be read.** Each piece is read into memory before it is sent
  (8 MiB at most), so a read that fails (the file moved, changed or its drive unplugged) ends the
  upload: `"clip.mp4" can no longer be read on this device (it was moved, changed, or the drive it
  is on was unplugged), so the upload stopped.` It is never taken for a dropped connection.
- **Review 2, uploads are a list.** Pressing "Upload and add" moves the form, fields and all, into
  #media-uploads, where it shows its own bar, words, Pause, Resume or Retry, and Cancel (its parts
  named by `data-upload-*`, its ids given the upload's number); the form area is free at once, so
  Add to Media, Put on Media or Publish opened meanwhile never touches it. Each upload publishes
  itself with what its own form says when its file is up; a publish that fails leaves it in the
  list with its words, "Put on Media" again and Close. The same file cannot be started twice while
  it goes. Cancel while the server is storing the last piece is asked again while the server says
  busy, and when the store finished first (the piece's answer, or `done` on the status) the stored
  file is removed too (`POST /api/media/remove` by its name, signed `media_remove:<name>`).
- **Review 3, abandoned sessions.** The record in this browser is dated by its last accepted piece
  (the relay's day runs from the last piece) and forgotten when the upload finishes, fails or is
  cancelled. Close on a failed upload cancels it on the server. The server's
  `uploads_in_progress` (R3-7) that no upload on this page carries are listed under "Unfinished
  uploads on the server" (name, how much of it was sent) with Cancel: `DELETE` with the token in
  the header, then this browser forgets it and the share is asked again.
- **Review 8, the fingerprint.** A record carries `fp`, SHA-256 (WebCrypto) of the file's first and
  last megabyte, and a resume needs it to match as well as the name, size and date; a record
  without one is not resumed. The page works it out once when the file is picked and hands it to
  the upload.
- **Review 5, members-only files.** Pictures stay fetched with the header, at most four at once,
  each with an AbortController: a picture no longer shown has its fetch stopped (its slot frees)
  and its object URL revoked; a list loaded afresh keeps the pictures it showed until it answers,
  so they are neither revoked nor fetched again. A members-only video plays from a ticket
  (`POST /api/media/ticket`, the token in the header, body `{id, filename}`), so the browser streams
  it; when the video errors on a ticket over a minute old, a new ticket is put under the same
  element where it stopped, playing on. Every members-only Download (but a picture already here)
  asks for a ticket of its own and lets the browser save from it, drawn without redrawing the
  viewer, so Download beside a playing video never restarts it. A ticket's address must be this
  server's `/api/media/file/<that id>?ticket=`.
- **Review 6, `/media?id=`.** The viewer keeps the label cover: a labelled item opened by its number
  (or any way) shows its labels and "Click to show", with no Download and nothing fetched until the
  click.
- **Review 7, private files in the chat.** By the marker's size: a picture is shown in place only up
  to 16 MB (a friend's larger "picture" is a Save card saying it is larger than is shown here),
  Save opens any file up to 512 MB, and a larger file is a card saying to open it in the HumanityOS
  desktop app, with nothing to press and nothing fetched. The server's own length is held to the
  same limits (its Content-Length when it gives one, else the bytes received), so a marker that
  says less than its file cannot make a browser open more. An opened picture is kept for the page
  (its object URL, by address and key, at most 128 MB of them, oldest let go first), so a
  conversation drawn again does not download it again. The sender lets the plain bytes go as soon
  as they are encrypted and the ciphertext once the upload's Blob holds its own copy; while
  WebCrypto's AES-GCM runs, both copies exist, since it takes and gives the whole file.
- **Honest limits.** The video's new ticket depends on the browser firing `error` when a range
  request is refused (all current ones do); a ticket younger than a minute is not renewed, so a
  file that fails at once fails once. The allowance and back-off waits run only while the page is
  open: a closed tab's upload is listed under the unfinished ones on the next visit. The web Roles
  editor still does not exist (see round 2). Members-only pictures are not ticketed, as the spec
  says.

**As built (desktop, round 3; 2026-10-11).** The desktop's half of round 3: the "Both apps" and
"Desktop only" items, built against the shapes R3-6 to R3-9 first and tested against stand-in
answers in them, then run against the relay's round 3 once it landed (relay_round3_tests.rs below
serves the relay's own route functions on a loopback port and talks to them with the app's own
clients). Code: gui/pages/media/ `uploading.rs` (the uploads list and the records),
`publishing.rs` (new), `waiting.rs` (new), `upload_waits.rs` (new), `upload_run.rs` (new, the page's
handle on a running upload, moved out of resumable.rs), `ticket.rs` (new), `resumable.rs`,
`open.rs`, `add.rs`, `api.rs`, media.rs; gui/widgets/`part_file.rs` (new); gui/pages/chat/
`attach_body.rs` (new), `attach_send.rs`, `attach_view.rs`; net/dm_pq.rs
(`encrypt_attachment_in_place`); gui/mod.rs (`upload_limits`); config.rs
(`media_unfinished_uploads`, a list now); engine/quit.rs. Every test named below was seen failing
once against a deliberate break, recorded at the test.

- **Review 1, uploads are a list of their own.** Sending the Add to Media form closes it: the
  upload is a row under "Uploads in progress", carrying what its form said (title, description,
  category, labels, visibility), with its bar, its words, Pause or Resume (Retry when the server
  has no room or keeps refusing), and Cancel. Once up, it is published from that, by itself; a
  publish that does not go stays in the list with its words, Put it on Media and Cancel. Several
  run at once (the relay allows three per person; a fourth waits with the relay's words). Every
  publish answers to the number of the upload or form that asked (`Job::Publish { who }`), so Put
  on Media from the Files page, or a late answer, never lands on another form.
  (uploads_tests.rs `put_on_media_during_an_upload_never_touches_it`,
  `a_late_publish_answer_never_lands_on_another_form`.)
- **Review 2, Pause.** Every request (start, status, piece) waits behind `upload_waits::hold`,
  and hashing too: while paused nothing is sent, in any phase. (resumable_tests.rs
  `pause_stops_every_request_in_every_phase`, which counts the starts the server saw in half a
  second: 41,315 with round 2's loop.)
- **Review 3, the allowance's wait.** The server's own `resets_in_s` (whole seconds, R3-6), else
  `resets_at` less this PC's clock, never less than 5 seconds, never more than 25 hours.
  (`an_allowance_already_past_by_this_clock_still_waits`: 5,561 pieces in half a second with
  round 2's; upload_waits.rs `an_allowance_waits_the_servers_own_number`.)
- **Review 5.** A refused piece larger than `left_bytes` is cut to it and sent at once; the rest
  waits for the reset (`an_allowance_smaller_than_a_piece_still_moves`, and against the real relay
  `the_whole_upload_against_the_real_relay_with_its_allowance`, whose relay clock reads 2001, so
  its `resets_at` is long past by this PC's). A 5xx waits a little longer each time, said as a
  server problem, never as a dropped connection; the tenth of the same status and words in a row
  stops it with the server's own words and Retry, sending nothing meanwhile
  (`a_server_error_that_never_clears_waits_for_retry`). A 429 without the allowance's JSON (nginx's
  rate limit, the relay's "You have 3 uploads in progress") is waited out the same way, with the
  relay's words or, for nginx's page, a line of our own (`a_429_without_an_allowance_is_waited_out`).
  A 507 at the start is shown with Retry, as at the finish. A file that can no longer be read ends
  the upload with words saying so, and the server is told.
- **Review 4, private pictures over 16 MB.** Routed by the marker's size: a picture type over the
  image cache's 16 MB is a card with Save, never fetched into memory
  (attach_view_tests.rs `a_private_picture_over_16_mb_is_a_card_with_save`).
- **Reviews 6 and 7, part files.** Download, Play here and a private file's Save write to a hidden
  `.humanityos-<process>-<n>-<nanos>.part` in the folder the file goes to and take their name only
  when the last byte is in. The name is claimed, not checked and then made: a hard link that fails
  when the name exists, or on a file system without them an empty file made with `create_new` and
  the part moved over it; "x (2)" and on when taken. A part not finished is deleted when dropped;
  the exit deletes every part this run is still writing (engine/quit.rs); the first frame of the
  next run deletes this app's parts in the downloads folder that nobody has written to for ten
  minutes. Save decrypts into a second part, so the decryption never writes under the final name.
  Play here's copy is named per fetch (the item's hash, the process, a number). (part_file_tests.rs
  three tests; open_tests.rs `a_download_has_its_name_only_when_complete`,
  `play_heres_copy_has_a_name_of_its_own_for_each_fetch`; attach_view_tests.rs
  `save_writes_through_part_files_the_exit_deletes`.)
- **Review 8, sending a private file.** The picker hands the worker a path and size
  (`attach_body::Body::File`); the worker reads it (at most its limit), encrypts it in its own
  buffer (`dm_pq::encrypt_attachment_in_place`, the same bytes `encrypt_attachment` makes, which now
  calls it), and the upload streams its multipart framing around that buffer with its
  Content-Length set (`attach_body::Multipart`). One copy of the file in memory, never three.
  (attach_send_tests.rs `a_picked_file_is_read_by_the_worker_and_kept_to_one_copy`: red for each
  of the three.)
- **Review 9.** `chunk_max` is held to 64 MiB (`upload_waits::piece_size`).
- **Review 10.** The two limits became `GuiState::upload_limits`, which carries the server and
  identity they were told to; anywhere else they read as not said (6 MB), and a peer list from
  another server or person starts them again (`the_limits_belong_to_one_server_and_one_identity`).
  The Files page reads its room limit the same way (`chat::room_limit_here`).
- **Review 11.** A publish answered 409 ("That upload is already on the Media page", the relay's
  owner check comes first, so it is one's own) counts as done, on the page and in the publish's
  worker (`a_publish_already_on_media_counts_as_done`). The worker that sends a publish holds the
  server's address and the signer, and a fate it shares with the page: when Cancel came first it
  takes the item off itself once the publish is answered, so a switch in between does not leave
  it on Media; when the answer came first the page takes it off at the press
  (`cancel_during_a_publish_then_a_switch_still_takes_it_off`, against a loopback server). The
  record of an upload goes at the Cancel.
- **R3-7.** `parse_mine` reads `uploads_in_progress`; those this app neither runs nor has a record
  of are listed under "Your unfinished uploads on this server" with how much arrived and Cancel,
  which tells the relay (`DELETE`, the token in the header). One with a record is offered with
  Carry on instead, not listed twice. (uploads_tests.rs
  `the_servers_unfinished_uploads_are_shown_with_cancel`; against the real relay
  `the_servers_unfinished_upload_is_listed_and_cancelled_on_the_real_relay`.)
- **R3-8.** A members-only item's Play here and Download ask `POST /api/media/ticket` (the token in
  `X-Upload-Token`, body `{id, filename}`) on their worker and fetch the answer's address with no
  token at all; the address is used only when it is that item's `/api/media/file/<id>?ticket=` on
  this server. `hide_token` hides `ticket=` as it hides `token=`. (ticket.rs two tests; open_tests.rs
  `a_members_only_file_is_fetched_with_a_ticket`; against the real relay
  `a_members_only_file_comes_through_a_ticket_from_the_real_relay`.)
- **R3-9.** The four resumable requests send the token in `X-Upload-Token` only
  (`the_token_rides_in_a_header_on_the_resumable_routes`); the real relay's routes take it there
  (the real-relay upload test sends nothing else).

**Not done, and honest limits.**
- Members-only pictures are still fetched whole with the token in the header (at most 16 MB, 20
  seconds) and are NOT stopped when no longer shown: the image cache's fetch is a blocking ureq
  request on its own thread, which nothing can interrupt. The web stops them (AbortController).
- Play here fetches the whole video, through its ticket, to its temporary file before it plays: the
  app's player (src/media) opens a file, not a stream. A ticket outliving a slow fetch is not a
  problem (it is checked when the transfer starts).
- A Cancel during a publish followed by the app quitting before the publish is answered: the
  worker ends with the app, the item may stay on Media, and its record is already gone, so nothing
  offers it; Remove on its tile takes it off.
- The image viewer's own Download (image_cache.rs `download`, a picture already shown, at most 16
  MB, one write) still writes straight to its name.
- The private file sent is still read whole into memory (one copy, plus the 16-byte tag): the
  relay's `/api/upload` takes one request, so the file is not streamed from disk.
- The upload token is still kept by server only, not by server and identity as the limits now
  are (not part of this round's findings).
- Only the header carries the token on the resumable routes: a relay from before round 3 refuses
  them (no compatibility before launch).
- No snapshot was taken this round (the page's new rows are checked by the headless tests above).

## Round 4: fixes from the round 3 review (2026-10-11)

Two critics read round 3 (relay fbfa814cc, web 888a220fd, desktop 51b24aca2 and 95d59f16f) and
found 12 defects, smaller than the rounds before. One decision for all three: **a cancel means
nothing stays.** `DELETE /api/upload/resumable/<id>` on a session that finished but whose file is
not yet published removes that stored file from the waiting folder too, and answers
`{ok: true, removed_stored: true}`; on a session whose file was already published it leaves the
item (publishing was the person's own choice) and answers `{ok: true, published: true}`; while a
piece or the finish is being written it still answers 409 `{busy: true}`, and the apps ask again
until it is not busy.

**Relay.**
- R4-1 (review 2): tickets: at most 16 live per person; each ticket stores a short fingerprint of
  the key, not the key's 3,904 characters; the relay-wide pool is large (500,000) and, when full,
  drops its OLDEST tickets rather than refusing anyone, so no number of accounts can lock others
  out.
- R4-2 (review 4): the cancel above.
- R4-3 (review 6): a piece counted and then cut back at start-up (the relay stopped between the
  write and the offset) gives its bytes back to the window it was counted in.
- R4-4 (review 7): moving a staged pieces file back never copies under the room lock: a rename
  only, and a file that cannot be renamed back stays staged (round 3 already keeps it).
- R4-5 (review 8): the expiry pass and the account erase remove a staged pieces file with its
  session.

**Web.**
- W4-1 (review 1): every wait (`nap`) returns at once when the upload is stopping, so Cancel or
  Pause during the start, a 429 or a 5xx takes effect immediately.
- W4-2 (review 3): a refused ticket renewal shows the relay's words on the video.
- W4-3 (review 4): Cancel uses the new cancel answer (asks again while busy; nothing to remove by
  hand).
- W4-4 (review 5): the opened-picture cache re-inserts an entry when it finishes, so eviction
  holds the 128 MB bound.

**Desktop.**
- D4-1 (review 1): the empty piece that only asks the server to finish never reads the local file,
  and a missing local file never cancels a session whose pieces are all on the server (offset ==
  size): Retry and Carry on finish it from the server's copy.
- D4-2 (review 2): Cancel asks again while the server says busy, uses the new cancel answer, and
  says "Cancelled" only when the server confirmed; a cancel that could not reach the server says so
  (and the unfinished-uploads list keeps the row until it is confirmed).
- D4-3 (review 3): the Media page follows the CONNECTED server (the one the socket is signed in
  to), never the text in a server address field being typed, so a draft address never abandons
  uploads.
- D4-4 (review 4): Resume clears its retry flag when it is used, so a stale one never skips the
  ten-in-a-row stop or a wait.
