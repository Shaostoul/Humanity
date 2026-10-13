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
12: "Dusk", a video, labelled Flashing lights, members only") with Open the item, which opens that
item's own page on the Media page, asked for by its number (`id=<n>` on the list read) when the
list does not hold it. (Until Media round 5 the desktop's Open filtered the list down to the item,
with Show everything; since 2026-10-12 it opens the item's page, as the website's `/media/<id>`
does.)

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

**As built (relay, round 4; 2026-10-11).** The cancel decision and R4-1 to R4-5, as written above.
Code: relay/resumable.rs (`cancel_core`, `cancel_session`, `write_counted`, `take_back_piece`),
relay/media_ticket.rs (the pool), relay/upload_limits.rs (`unstage`), relay/media.rs (the tests'
`hold::stop_at`), storage/upload_sessions.rs (`remove_session_files`, `count_upload_piece`,
`take_back_upload_piece`, `give_back_unstored_pieces`, `Reconciled::given_back`, the expiry and the
erase's file removal), storage/upload_counts.rs (`add_count_on`, `take_back_on`), storage/members.rs
(`member_row`, `member_key_at_row`), storage/mod.rs (two columns), storage/expiry.rs,
storage/account.rs and relay/mod.rs (the start-up log line). Tests: relay/resumable_round4_tests.rs
(R4-2 to R4-5) and relay/media_round4_tests.rs (R4-1), each seen failing once against a deliberate
break, named at the test.

- **The cancel (R4-2).** `DELETE /api/upload/resumable/<id>`, by what the session is when the
  cancel holds it (it is read again after the one-at-a-time marker is taken, so a finish that ended
  a moment before counts as finished):
  - a piece or the finish being written: 409 `{error, busy: true}`, nothing done;
  - still arriving: its row and its pieces file go (wherever the file is, R4-5): `{ok: true}`;
  - finished, not published: the stored file is removed from the waiting folder, then its
    `user_uploads` row, then the session's row: `{ok: true, removed_stored: true}`. Also when the
    file was already gone (its own Remove, or the day's expiry of unpublished uploads): nothing
    stays either way;
  - finished and published: the item and its file stay; the session's row goes:
    `{ok: true, published: true}`.

  The finished cases run under the room lock, as a publish does, so a publish of the same file
  either came first (the answer is `published`) or finds nothing left to publish. A stored file
  that cannot be deleted is answered 500 with plain words and keeps every row, so nothing is left
  that no row knows of, and the app may ask again. The cancel's work runs off the async executor
  and holds the marker to its end, as a piece does.
- **R4-1, tickets.** At most 16 live per person (`TICKETS_PER_PERSON`); the 17th is still refused
  with 429 and the same words as before, so one person's burst never touches anyone else. The pool
  is 500,000 (`TICKETS_MAX`) and, full, drops its OLDEST ticket to make room: a new person always
  gets one. A ticket keeps no key: the item, a 16-byte BLAKE3 fingerprint of the item's file name,
  the member's row number in `server_members` and a 16-byte fingerprint of their key, and the time
  it runs out, 64 bytes with nothing on the heap (the record is `Copy`, checked when the relay
  compiles), 128 bytes a ticket with its place in the map and in the oldest-first queue
  (`BYTES_PER_TICKET`). A full pool is about 140 MB at most with the tables' spare room (round 3's
  pool of 50,000 was about 200 MB). Each request finds the person by their row number and checks
  the key there against the fingerprint, then that they are a member and not banned, so a row
  number given to someone else after the member left never inherits a ticket.
- **R4-3, a piece counted and never stored.** A piece's count and a note of it on its session
  (`counted_bytes`, `counted_at`) are written in one transaction under the room lock; storing the
  offset clears the note in the same statement; a failed write takes the count back and clears it.
  At start, before anything is cut back or removed, every session still carrying a note gives
  those bytes back to the day window holding `counted_at` (the day it was counted in, not the day
  of the restart) and clears it; the file is cut back to the offset as before. The piece sent
  again is counted once. The start-up log line says how many were given back.
- **R4-4, no copy under the room lock.** `unstage` moves a pieces file back by a rename only. A file
  `put` moved there was renamed, so it is on the same disk; one that still cannot be renamed back
  (a folder gone, another program holding it) stays staged under its session's name, and the next
  finish or the next start brings it back (`restore_staged_part`, which may copy, and runs with no
  lock held).
- **R4-5, staged files go with their sessions.** The expiry pass and the account erase remove a
  session's pieces file from the resume folder and its staged name from the waiting and the public
  folders (`remove_session_files`, which the cancel uses too); the erase's
  `upload_session_files_removed` counts the sessions that had a file removed.

**Honest limits.** The R4-4 test runs on Windows only: a file held open without delete sharing is
how a rename fails there while a copy still works, and no portable way was found to make that
happen. The schema's two new columns are in the `CREATE TABLE` (like the round's three before
them, this version is unreleased): the live server had no `upload_sessions` table on 2026-10-11
(read-only check), so nothing there needs a migration. A finish that stored its file but could not
write its answer on the session (a database error, logged) looks unfinished to a cancel, which then
removes only the session: that file waits until the day's expiry of unpublished uploads, as round
3's limit already says of a relay that stops between the last rename and the row. The tickets'
lookup trusts a member row number while the membership lasts: `server_members` rows are made by
an `INSERT` (never a `REPLACE`, which would give a new number), and nothing runs a plain `VACUUM`
(which may renumber rows),
so it lasts; if it ever changed, the fingerprint check refuses the ticket and the app asks again.

**As built (web, round 4; 2026-10-11).** The web's half of round 4: W4-1 to W4-4 and the cancel
decision at the top of this section, written before the relay's round 4 landed, against the shapes
stated above (`{ok: true, removed_stored: true}`, `{ok: true, published: true}`, 409 `{busy:
true}`) and tested against stand-in answers in them, then checked against the relay's note just
above once it landed (a18ff7d63): the answers match, and its one addition, a 500 in plain words
with every row kept when the stored file cannot be deleted, is asked again and then said in the
server's own words (W4-3 below, test W4-3f). Code: web/pages/media-upload.js (`nap`,
`cancelSession` new, `readAnswer` moved out of the uploader), web/pages/media-app.js (the ticket
renewal, `openViewer`, `sayCancelled`, the unfinished uploads' Cancel; `removeStored` and the
uploader's `onStored` deleted) and web/chat/chat-dms.js (`openPrivatePicture`). Tests:
scripts/tests/media-web-round4.test.js (new, 9 tests), one "W4-4" test at the end of
scripts/tests/private-files-web.test.js, and round 3's 2b in media-web-round3.test.js, whose
expectation round 4 changes (it now proves nothing is removed by hand, and says so). The resumable
upload's stand-ins moved from media-web-round3.test.js into media-web-harness.js
(`resumableRelay`, `uploaderOn`, `aFile`, `sleeper`, `drive`), the stand-in relay answering a
cancel as round 4's relay does. Each test was seen failing once against one break in a copy of
web/ (ten breaks), and every round 4 test fails against round 3's web/ (3077155ed); the break and
the words it tripped are noted above each test.

- **W4-1.** `nap(ms, phase, extra)` is every wait the upload makes, and it returns at once when
  Pause or Cancel has been pressed, BEFORE it shows its phase: a wait that would begin after the
  press never begins, so nothing sleeps and its words never show. Each wait that used to set its
  phase and then nap (a server error's, a dropped connection's, the busy answer's, the allowance's,
  the wait for a token) hands its phase to `nap` instead. That covers the start, which is not
  stopped because its answer names the session, and a piece whose answer was already coming when
  Pause was pressed. A press during a wait still cuts it short (`wake`).
- **W4-2.** A ticket renewal the relay refuses, or that gets no answer, turns the viewer's ticket
  into an error carrying the relay's words, and the viewer is drawn again with "This file could not
  be loaded: " and those words where the video was, instead of a dead video. It is not asked again
  while the item stays open (no loop of requests). Opening the item again asks afresh
  (`openViewer` drops a refused ticket), which is what the relay's words for too many open tickets
  tell the person to do; that holds for a first ticket refused as well.
- **W4-3.** `cancelSession` (media-upload.js) is the one cancel, used by an upload in the list and
  by a row of "Unfinished uploads on the server": `DELETE` with the token in the header, no status
  asked first. 409 `{busy: true}` is asked again after 2 s doubling to 30 s for as long as it lasts,
  never counted as a try. `removed_stored` means the server deleted the file the finish had stored,
  so the page removes nothing by hand and says "Cancelled. The file had already been stored, so it
  was deleted too."; `published` says "Cancelled, but the file was already on the Media page, so it
  stays there. Remove on its tile takes it off."; a 404 is a cancel already done. A dropped
  connection, a 5xx, a 429, a token from before the page signed in again, or no token yet is tried
  again, ten times in all, and then the page says "The cancel could not reach the server, so what
  arrived may still be there. It is listed on this page with Cancel or Remove once the server
  answers again, and the server deletes it by itself within a day." instead of taking it for done.
  When the server did answer every time, with an error (the relay's 500 when the stored file
  cannot be deleted, which keeps every row), the page says "The server could not finish the
  cancel, so what arrived may still be there. It is listed on this page with Cancel or Remove, to
  try again. The server said: " and the server's words. Any other refusal is shown in the server's
  words.
  An unfinished row's Cancel reads "Cancelling..." and is disabled while it goes; a second press
  sends nothing more.
- **W4-4.** `openPrivatePicture` takes an entry out of the Map and puts it back when its picture
  finishes opening, so it is the newest, and the sweep that lets the oldest go reaches the pictures
  that finished before it. Twenty 16 MB pictures finishing in reverse now keep 128 MB (the last
  eight to finish), not 320 MB.

**Honest limits.** The web tests run against stand-ins in the relay's shapes, not against the
relay itself (there is no web rig for the relay); the shapes were checked against the relay's note
and code (resumable.rs `cancel_core`) once it landed. An upload in the list whose cancel could not be confirmed is
forgotten by this browser at once (as before), so it is offered again only through the server's
own lists once the server answers (the unfinished uploads with Cancel, or a stored file among the
uploads waiting with Remove); a row of the unfinished list is forgotten only when its cancel is
confirmed. While the server stays busy, a cancel keeps asking (once every 30 seconds at most) for as
long as the page is open; a page closed meanwhile leaves the session to the server's lists and its
expiry. A renewal that failed only because the connection dropped also shows its words, and waits
for the item to be opened again.

**As built (desktop, round 4; 2026-10-11).** D4-1 to D4-4 and the cancel decision, built against
the cancel answer exactly as stated at the top of this round before the relay's R4-2 landed, and
tested against stand-in answers in those shapes (the server simulated in resumable_tests.rs now
answers a cancel as R4-2 says: `{ok: true}`, `{ok: true, removed_stored: true}` for a finished
upload, removing its stored file, `{ok: true, published: true}`, and 409 `{busy: true}` when a test
scripts it), then run against the relay's own cancel once its round 4 landed (the note just
above): relay_round3_tests.rs `a_finished_upload_cancelled_on_the_real_relay_leaves_nothing`
uploads to the relay's own routes on a loopback port and cancels from the page, and the relay's
`removed_stored` and `published` answers read as built here, nothing changed for them. Its 500
with plain words, when the stored file cannot be deleted, is asked again like any 5xx and, after
the ceiling, shown in the relay's words. Code: gui/pages/media/ `upload_cancel.rs` (new),
`resumable.rs`, `upload_waits.rs`, `uploading.rs`, `publishing.rs`, `waiting.rs`, `upload_run.rs`,
`add.rs`, `open.rs`, `tiles.rs`, media.rs; gui/pages/server_settings/media.rs; gui/connections.rs
(`GuiState::active_server`). Tests: gui/pages/media/`round4_tests.rs` (new, 11 tests), one in
`upload_cancel.rs` and the real relay's one above, each seen failing once against a deliberate
break of the code it guards (the break and what it saw red are written at the test); and the
round 3 tests whose expectations this round changes, each saying how
(page_tests.rs `an_upload_a_restart_cut_off_is_offered_again`, uploads_tests.rs
`the_servers_unfinished_uploads_are_shown_with_cancel`, and resumable_tests.rs
`a_file_that_can_no_longer_be_read_ends_the_upload`, which cut the file at `Started` and was seen
to finish instead once, when the simulated server took every piece before the file was cut; it now
cuts the file while the upload waits for an allowance).

- **D4-1, the empty piece.** The piece of length 0 at the end, which only asks the server to
  finish, never opens the file on this computer (`resumable::work`). A 507's Retry, and Carry on
  after a restart, finish from the server's copy whatever happened to the local file since.
  `resume_with` no longer cancels a record whose file changed: it starts the upload with
  `Plan::file_changed`, which asks the server where it got first. With every piece there it sends
  only the empty piece; with pieces still to come it tells the server to drop what arrived and
  ends with `FILE_CHANGED`, now "The file changed since its upload started, so the rest cannot
  follow what was sent. Add the file again.", followed by "Nothing of it stays on the server." once
  the server confirmed. (`the_empty_piece_never_reads_the_file`,
  `carry_on_finishes_from_the_servers_copy_whatever_the_local_file`.)
- **D4-2, the cancel.** One routine, `upload_cancel::tell`, sends `DELETE` and reads what comes
  back: 2xx is done, `published: true` meaning the item is on Media and stays; 404 is done (there
  is nothing of it there); 409, 429 and 5xx are asked again, waiting 1 s doubling to 30 s, at most
  35 times (about fifteen minutes, what nginx allows a finish); no answer at all is asked three
  times; a token the server no longer takes, or any other refusal, is not asked again. The worker
  uses it for Cancel and for an upload that cannot go on, and its `Event::Cancelled` now carries
  what the server said (`Told`); the page uses it on a worker for everything else (`Job::Discard {
  id }`, answered in `uploading::cancel_answered`). "Cancelled: nothing of it stays on the server."
  is said only for a confirmed cancel. Anything else is said as "Not cancelled on the server yet:",
  the reason, then "What arrived stays there until a cancel reaches it, or for a day after its last
  piece. Press Cancel again to try.", and whatever listed the upload keeps it, with its Cancel: an upload
  in the list goes back to its row, a record stays offered with Carry on and Cancel it, a row of
  "Your unfinished uploads on this server" stays, each saying "Cancelling..." while the cancel is
  on its way. An upload whose file is up but not on Media (the publish refused, no signature to
  publish with, or Cancel just after the last piece) is cancelled with that same `DELETE`, which
  needs the upload token only, so it goes with the identity locked; round 3's signed take-off sent
  nothing then and still said "Cancelled". A Cancel during a lost finish's back-off asks the server
  at once, and the relay's `removed_stored` takes the stored file with the session, so the status
  that would have carried `done` is not needed. A publish cancelled while on its way: its worker's
  take-off answer is what the page hears, and "Cancelled" is said only when it went; when the
  publish's answer was in first, the page's own take-off says it could not go when there is no
  signature (`publishing::NO_SIGNATURE_TO_TAKE_OFF`). A cancel before the server named the upload
  says "Cancelled" unless a start's answer was lost, when the server may hold it: then the person
  is told to look under their unfinished uploads. (`a_cancel_while_the_server_stores_the_finish_is_asked_again`,
  `a_cancel_the_server_did_not_confirm_is_said_and_kept`,
  `cancelling_an_upload_that_is_up_needs_no_signature`,
  `an_unfinished_upload_stays_listed_until_the_server_confirms_its_cancel`,
  `a_cancelled_publish_is_said_cancelled_only_when_taken_off`,
  `a_publish_with_no_signature_to_take_it_off_says_so`, upload_cancel.rs
  `the_cancel_answer_is_read_and_busy_is_asked_again`.)
- **D4-3, the connected server.** `GuiState::active_server` is the server the active connection
  was dialled for (`connected_server_url`), which is the one it signs in to; with no connection
  this session, what the app dials by itself (`dial_address`), which is never a draft either. The
  Media page follows it everywhere: `media::server_here` for forgetting another server's state
  (`sync_server`), the requests' address (`base_of`) and the token (`token_here`, the upload token
  only when it is that server's). Typing into the Chat page's Server field or the main menu's
  leaves uploads, their token and the page as they were. (`typing_a_server_address_never_abandons_uploads`.)
- **D4-4, Resume and Retry.** The worker holds each wait a press ends (`Held::Allowance`,
  `Held::Retry`), set before the page is told of it, so a press that follows what the page shows
  always finds it; Resume sets the retry flag only during an allowance's wait, Retry only during
  either, and the wait starts and ends with the flag clear (`Control::hold_in`, `release`). No
  flag outlives its wait. (`a_stale_resume_never_skips_the_stop_for_retry`,
  `a_stale_resume_never_cuts_an_allowances_wait_short`.)

**Honest limits.**
- The chat's attach and the Files page still read the Server field. They never send a token to an
  address being typed (their token counts only when the field names the server it came from), so a
  draft only stops them with "not signed in yet"; they do not follow the connected server as the
  Media page now does.
- After a Disconnect, with the Server field cleared, the Media page still shows the last server it
  was connected to: that is still the connected server by this rule.
- A cancel the server keeps answering busy is asked for about fifteen minutes on a background
  thread; the row says "Cancelling..." all that time.
- The record of an upload whose publish was cancelled while on its way goes at the press (as in
  round 3). When its worker's take-off is then refused, the page says so and points to Remove, but
  the uploads list no longer offers it.
- Against a relay from before R4-2, a cancel of a finished upload answers a plain `{ok: true}` and
  leaves its stored file in the waiting folder for a day, while the app says "Cancelled" (no
  compatibility before launch: the live relay runs R4-2 once this round is deployed).

## Files on the sender's device and in messages, exact (answers 4, 5, 6 and 9; 2026-10-11)

The operator's "Let's proceed" (2026-10-11) on four of the answers above. Answer 10 (device to
device big files) has its own section after this one. One agent per app does all of this in one
worktree (the same files are touched by each part), and a small relay change carries the profile
field.

**1. Cleaning a file on the sender's device (answer 4).** Every upload path in both apps (chat
attach, paste and drop, public rooms and private conversations, the Files page, Add to Media) runs
the strip on the file before it leaves the device: pictures (JPEG, PNG, WebP; GIF passes as the
relay passes it), videos (the MP4 family), and on the desktop HEIC and AVIF too (the relay's own
Rust strips, `strip_metadata`, `strip_video_metadata_in_buffer` or a temp copy for big files,
`strip_heif_metadata`); the web uses `web/shared/strip-metadata.js` (`HosStrip.stripFile`), which
has no HEIF yet, so a HEIC on the web counts as "could not be checked" (part 2).
- **The attach step shows what is in the file** before sending: `HosStrip.describe` on the web, and
  a Rust `describe` the desktop gains (in src/relay/core/, beside the strips, so the relay and the
  desktop share it), with the same words: "This photo says where it was taken and which camera took
  it.", "This video says where it was filmed.", or nothing when there is nothing. The two describes
  agree on the same test files (the web test compiles the Rust, as the strip test does).
- **One switch:** "Remove location and camera details", on by default, remembered as the last
  choice on this device (desktop config, web localStorage in try/catch). The protected setup always
  removes and shows no switch.
- **Kept by choice:** the upload says `keep_metadata=1` (the relay then skips its own strip on a
  public shelf); removed: the app strips, and the relay strips again on a public shelf (the same
  rules, so a second pass changes nothing).

**2. A file that could not be checked (answer 5).** When the strip reports it stopped early (or
failed open on a picture, or it is a HEIC on the web): on a public shelf the app says so and does
not send it (the relay would refuse it too, unless kept by choice); in a DM or group it asks "This
file could not be read all the way through, so where it was taken may still be in it. Send it
anyway?" On yes, the file's marker carries `"unchecked": true` in its meta (both apps write and
read the same field; `build_file_marker` and its web twin), and the recipient sees one line under
the file: "The sender's app could not check this file for hidden location."

**3. Files in messages (answer 6).** Settings > Privacy, beside "who can reach me", a group
"Files in messages":
- **Accept files from:** Nobody / Friends / Anyone I talk to. Default Friends.
- **Show pictures without asking from:** Friends / Nobody (today's click-to-load rule, made a
  setting; the protected setup is always Nobody).
- **Show the attach button in private conversations:** on or off.
The RECEIVING app enforces it: a file marker from someone the setting does not allow is not
fetched; its card says "Ann sent a file. You accept files only from friends. Accept this one?",
and Accept fetches that one file. The setting is published so a sender's app can say so first:
the person's profile gains `accepts_files` ("nobody", "friends" or "anyone"; relay change below),
and a sender's attach step in a DM says "Ann accepts files only from friends." when it applies.
P2P groups: if the group object already carries settings, its owner gets "Files allowed" and
"Largest file (MB)"; if it does not, this waits for a group-settings increment and the As built
note says so.

**4. Keeping a copy of opened private files (answer 9).** A private file (DM or group) that the
person opened or saved is kept on the device, encrypted at rest with a key derived from their
identity as their DM history is (desktop: a folder in the app's data directory, AES-256-GCM;
web: IndexedDB through the existing encrypted DM store), so it still opens after the server's copy
leaves (30 days). Opening a file looks there first. A size cap per device (Settings > Data,
default 1 GB on the desktop and 200 MB on the web), oldest opened first out, and a "Clear kept
files" button.

**Relay.** `profiles` gains `accepts_files` (ALTER-added, with the pre-migration open test; default
"friends"), accepted on a profile update (only the three words), sent in profile answers and
`ProfileData`, in the person's export, cleared by their erase.

**Proof.** Each part with tests seen failing once, in each app and the relay; the two describes
agree on shared fixtures; a marker with `unchecked` built by one app is read by the other (a
shared fixture string in both test suites).

## Device-to-device big files, exact (answer 10; 2026-10-11)

A file bigger than the person's private file limit (the server's `private_limit_mb` for their role)
never goes to the server's disk: in a DM it goes from one device to the other, while both are
online, once the receiver has said yes. Groups come later (file-transfer-at-scale.md, layer 3).

**What already exists.** Both apps open WebRTC data channels for direct messages (web
`web/chat/chat-p2p.js`, `createDataChannel('dm')`; desktop `src/net/webrtc.rs` on str0m), with the
relay carrying only the signalling (`dc_offer` and friends, which the relay lets through only with a
friendship pass that allows it), and both can force every packet through the server's call
forwarder (relay-only ICE), so neither side learns the other's network address.

**The offer.** The sender's app sends a sealed DM control note (like the choice and block notes,
never shown as a message): `[[hum:offer:v1]]` followed by JSON `{id, name, size, type, blake3,
piece, pieces}`: a random 16-byte `id`, the file's name, size and type, the BLAKE3 of the whole file,
the piece size (256 KiB) and the BLAKE3 of each piece, in order. The note is signed inside the DM
like every DM, so the list of pieces is the sender's word for what the file is.

**The receiver says yes.** The receiving app applies "Files in messages" first (a sender it does not
accept files from gets the one-file Accept card), then shows "Ann wants to send you clip.mp4
(2.3 GB). Accept?" with Accept and Decline. Its answer is another sealed note,
`[[hum:offer-answer:v1]]{id, accept}`. An offer nobody answers expires with the DM mailbox.

**The transfer.** Once accepted and both are online, the two apps open a data channel labelled
`file:<id>` (ordered, reliable) through the existing signalling. Between people who are friends and
both allow direct connections it may go direct; otherwise, by default, it goes through the call
forwarder, so addresses stay private. The receiver asks for pieces by number; the sender sends each
piece with its number; the receiver checks each piece against its BLAKE3 from the offer and writes
it to a part file (desktop: beside the chosen folder, renamed when complete; web: pieces in
IndexedDB, then a Blob saved through the browser, or the File System Access API where the browser
has it so nothing is held in memory). After a drop either side can resume: the receiver asks again
from the first piece it lacks. When every piece has arrived and the whole file's BLAKE3 matches, it
is done; a piece that fails its check is asked for again, and three failures of one piece end the
transfer with words.

**On screen.** Both sides: a row in the conversation with the file's name, size, a progress bar,
Pause, Resume and Cancel, and plain words for each state ("Waiting for Ann to accept", "Waiting for
Ann to come online", "Sending through the server's call relay", "Sent", "Ann declined",
"Cancelled"). The sender's attach step offers this automatically for a file over the limit, with a
line saying why ("Files over 6 MB go straight to Ann's device instead of through the server.").

**What it costs the server.** No disk. Bandwidth only when it goes through the forwarder, held to a
per-person daily amount the admin sets (a new role limit beside the upload ones, "forwarded MB a
day", 0 = no limit, Admin's default 0), counted by the forwarder.

**Needs.** The forwarder's UDP port open on the server (as calls do, `docs/admin/call-forwarder.md`).
Until it is, a transfer between people who may not go direct says so instead of starting.

**Proof.** Each step with tests seen failing once: the offer note's bytes the same in both apps (a
shared fixture), piece checking and resume against a stand-in peer, a decline and an expiry, the
pause and cancel, the forwarder's per-person day amount, and one desktop-to-web transfer between two
real apps through a local forwarder before release.

## Device-to-device big files: the shapes, exact (2026-10-11, refining the section above)

Written before the build so the relay, the desktop app and the website build against the same
bytes. Three things the section above left open are settled here: who may set up the connection
when the two people are not friends, how the server counts what goes through it, and how the list
of piece hashes travels (in the offer note it would be about half a megabyte for a 2 GB file, far
past what a DM carries).

**1. The file pass (the certificate pattern, as friendship passes and plot permits).** The sender's
app mints, for this one transfer, a Dilithium3 signature over

```text
hum/file-pass/v1\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}
```

`server` is the relay's own `did:hum:` (the one the friendship passes use, `Storage::server_did`,
also in `identify_challenge` as `server_did`); `issuer` the sender's key, `grantee` the receiver's
key (Dilithium hex, as everywhere); `id` the transfer's id (32 lowercase hex characters, 16 random
bytes); `expiry` Unix seconds, at most 7 days ahead. Carried as
`{"v":1,"id":"<id>","expiry":<n>,"sig":"<base64>"}`. The relay checks it STATELESSLY, rebuilding
the words from its own DID and the two keys it knows from the frame (never the pass's claims), and
accepts up to one day of clock slack past 7 days, as plot permits do. A pass says "the issuer wants
the grantee to be able to reach them for this file"; it is good for nothing else.

**2. The relay (`src/relay/handlers/reach.rs`, `src/relay/call_credentials.rs`,
`src/relay/call_forwarder.rs`, roles).**
- `webrtc_signal` gains an optional `file_pass` (the object above). A `dc_offer` from S to T passes
  when it did before, or when `file_pass` verifies with issuer T and grantee S
  (`dc_offer_may_pass`). The pass is not forwarded to T. So the receiver (grantee) dials the
  sender (issuer), the way round in which the relay can check it.
- `call_credentials` gains a third request,
  `{"type":"call_credentials","file":"<id>","peer":"<key>","file_pass":{...}}`, answered to the
  asker when the pass verifies with (issuer, grantee) = (peer, asker) or (asker, peer) and its `id`
  is the `file` asked for; otherwise refused as the others are. The scope is a keyed hash of
  `file\n{id}\n{the two keys, sorted}`, so the forwarder passes packets only between the two
  people's allocations for this one transfer.
- **The day amount.** A new role limit `max_forwarded_mb_per_day` beside the upload limits
  (`storage/roles.rs`, with the `LIMIT_NOT_SENT` rule for older apps, editable in Server Settings
  > Roles on both apps), 0 meaning no limit: Admin 0, every other built-in role 2048 by default.
  A file credential carries, inside the part its HMAC covers, a keyed tag of the person it was
  issued to and their limit in MB; the forwarder counts the bytes each tag's allocations send
  through it per UTC day and stops relaying for that tag past its limit until the next day
  (a call's credentials carry no tag and are not counted). Nothing about the count is written to
  disk; a restart starts the day's count again, which errs toward the person.
- A transfer cannot start while the forwarder is not running; `call_credentials` says so with the
  existing `NOT_SET_UP` words, and the apps show them.

**3. The offer and its answer (sealed DM control notes, never shown as messages).**

```text
[[hum:offer:v1]]{"id":"<32 hex>","name":"clip.mp4","size":2468000000,"type":"video/mp4",
                 "blake3":"<64 hex, the whole file>","piece":262144,"pieces":9415,
                 "hashes":"<64 hex, BLAKE3 of the piece hashes, each 32 raw bytes, in order>",
                 "pass":{"v":1,"id":"<same id>","expiry":<n>,"sig":"<base64>"}}
[[hum:offer-answer:v1]]{"id":"<32 hex>","accept":true|false}
[[hum:offer-cancel:v1]]{"id":"<32 hex>"}
```

`pieces` is `ceil(size / piece)` (a zero-byte file is not offered). The file is checked after the
receiver says yes: the receiver's app applies "Files in messages" first (a sender it does not
accept files from never gets as far as the question), then asks "Ann wants to send you clip.mp4
(2.3 GB). Accept?"; Decline sends `accept: false`; an offer nobody answers expires with the DM
mailbox, and its pass by its own expiry. Either side may cancel (`offer-cancel`). A shared fixture,
`scripts/tests/fixtures/file-offer-v1.json`, holds one offer's exact text and pass preimage, and
both apps' tests read it.

**4. The connection.** The receiver, on Accept, dials the sender: a WebRTC connection with one data
channel labelled `file:<id>` (ordered, reliable), its `dc_offer` carrying the pass. **Both apps use
the forwarder only (relay-only ICE)** unless the two are friends (each holds the other's
friendship pass) and the person has turned direct connections on, so neither learns the other's
network address by default; each app guards its own address by its own ICE policy.

**5. On the channel.** Text messages are JSON; data messages are binary, at most 64 KiB each
(browsers cap a data channel message near 256 KiB).
- The receiver sends `{"t":"hashes"}`; the sender answers with the piece hashes as binary
  messages (32 bytes per piece, in order, split at 64 KiB). The receiver checks that BLAKE3 of all
  of them is the offer's `hashes`; otherwise the transfer ends ("The file Ann offered does not
  match its description").
- The receiver asks `{"t":"get","from":<piece>,"count":<n>}` (n at most 16, a window of 4 MB);
  the sender sends each piece as 64 KiB binary messages, each starting with an 8-byte header
  (piece number and the offset within the piece, both u32 big-endian) and holding back while the
  channel's buffer is over 1 MB.
- The receiver checks each whole piece against its hash and writes it (the desktop app to a
  `.part` file beside the chosen folder, renamed when done; the website into IndexedDB, then a
  Blob saved through the browser, or the File System Access API where the browser has it, so
  nothing near the file's size is held in memory). A piece that fails is asked for again; three
  failures of one piece end the transfer with words.
- After a drop either side may resume: the receiver asks again from the first piece it lacks, on a
  new connection with the same pass.
- When every piece is in and the whole file's BLAKE3 matches the offer's, the receiver sends
  `{"t":"done"}` and both show "Sent" / "Saved".
- `{"t":"pause"}`, `{"t":"resume"}` and `{"t":"cancel"}` from either side; cancel also sends the
  `offer-cancel` note so the other side's row says so even if the connection is gone.

**6. On screen (both apps).** The sender's attach step offers this for a file over their private
file limit (`max_private_file_mb` for their role) with a line saying why ("Files over 6 MB go
straight to Ann's device instead of through the server."). Both sides get a row in the
conversation: the name, the size, a progress bar, Pause, Resume and Cancel, and words for each
state: "Waiting for Ann to accept", "Waiting for Ann to come online", "Sending through the
server's call relay", "Sent", "Saved", "Ann declined", "Cancelled", "This server is not set up
for that yet" (the forwarder's words). A transfer survives the app being closed only as an offer:
the sender must be online for it to go, and the row says so.

**Proof (each test seen failing once):** the pass preimage pinned in the pq_crypto tests and read
by the web's tests; the offer note's text from the shared fixture in both apps; the relay letting a
`dc_offer` through with a valid pass and refusing a forged, wrong-way, other-file or expired one;
file credentials only with a matching pass; the forwarder's day count stopping a tag past its
limit and not a call; on each app, piece checking and resume against a stand-in peer, a decline,
an expiry, pause and cancel; and before release one desktop-to-web transfer between two real apps
through a throwaway relay with its forwarder on loopback.

## Device-to-device big files: rules settled by the first review (2026-10-12)

The first build ran website to website through a real browser and our own forwarder (a 20 MB file,
80 pieces, "Sent" and "Saved"), and the desktop to desktop on loopback. A review of the desktop
against the website found the two disagreeing at the seams, and the browser run found one more.
These rules are now part of the shapes above; both apps follow them exactly.

1. **A `get` adds, it never replaces.** `{"t":"get","from":n,"count":k}` adds pieces n to n+k-1 to
   what the sender owes. The sender sends what it owes in ascending order, each piece once, from
   offset 0, and forgets a piece once its last message has gone. A piece asked for while it is
   being sent is sent again, whole, after the current one (the receiver may have found it bad). A
   receiver asks for a piece once, and again only after it failed its hash (`count` 1) or after a
   stall with nothing arriving (30 seconds); it keeps its window full by asking for pieces it has
   not asked for yet, never repeating the rest of a window. A receiver ignores messages for a piece it did not ask for or already
   holds, and holds partial buffers only for pieces it asked for (at most its window, 16).
2. **One connection, several transfers.** Each side keeps one engine per data channel
   (`file:<id>`), on both the dialling and the answering side, and ends a channel's engine without
   touching the others; the connection closes when its last channel ends.
3. **Candidates that arrive early are kept.** A side waiting for its forwarder credentials keeps
   every `dc_ice` (and the offer's own candidates) for that person and applies them once its
   allocation exists; none is dropped or handed to the call code.
4. **Answers reach a sender who takes messages only from friends.** The receiver's answer and
   cancel notes go with the offer's file pass on their `dm_put` (`file_pass`, the same object),
   and the relay lets a `dm_put` through the "who can reach me" gate when the pass verifies with
   the recipient as its issuer and the sender as its grantee (it still counts against the
   stranger's daily budget). Before this, a receiver who was not the sender's friend could accept,
   but the sender never heard it; only the dial then started the transfer.
5. **File credentials need the other person.** The relay gives file credentials only when the
   peer is a key registered on this server, and holds each person to at most 8 live file
   allocations at once (a person tag's allocations, past which Allocate is refused with 486). A
   request it refuses still counts toward the 20-a-minute limit, which is checked before any
   signature is checked. The forwarded day count keeps at most 10,000 people at once (the
   smallest count goes first when it is full).
6. **What the day amount covers.** Bytes sent with file credentials only. Voice rooms and calls
   are not counted (they are held to their rate and their allocation's 12-hour life), so the
   amount limits file transfers, not everything a modified app could push through.

**Settled by the second review (2026-10-12):**

7. **A dial names its transfer.** The relay passes a `dc_offer` let in by a file pass to its target
   with the pass's id as `file`. The answering side (the file's sender) asks the forwarder for that
   one transfer's credentials and makes ONE allocation for the connection; every other transfer
   with that person rides the same connection as its own channel and needs no allocation of its
   own. One person holds at most 16 live file allocations. A peer counts as known to the forwarder
   when they hold a name on the server or are signed in now.
8. **An end is always said.** A side whose transfer ends for any reason other than done (a bad
   piece three times, a file that does not match, storage that refuses a piece, the person's
   cancel) sends `{"t":"cancel"}` on the channel and the cancel note, so the other side's row ends
   too. A channel closed by the other side before `done` ends a sending transfer ("Ann's side
   stopped it"); a receiving transfer that is still live opens its channel again on the
   connection (or dials again) after a short wait, and a pause the other side held is cleared.
9. **A dial is an answer.** A sender whose record still waits for the answer note treats its
   transfer's channel opening (or its first `hashes` or `get`) as Accept, and `done` as Sent, so a
   lost answer note never leaves the row waiting.

## As built (relay, files in messages)

The relay half of part 3, built 2026-10-11. What the two apps build against:

- **The field.** `profiles.accepts_files`, `TEXT NOT NULL DEFAULT 'friends'`, ALTER-added in the
  profiles ALTER loop in `src/relay/storage/mod.rs` and never named in a main schema batch
  (BUG-046). The words are `nobody`, `friends` and `anyone`, exactly, lower case, nothing trimmed
  (`src/relay/storage/profile.rs`: `ACCEPTS_FILES_WORDS`, `ACCEPTS_FILES_DEFAULT`,
  `accepts_files_word`, `accepts_files_of`).
- **In: `profile_update`** gains an optional `accepts_files` string.
  - Not sent (an app from before the field): the stored word is KEPT, and a person's first profile
    row takes `friends`. The same not-sent rule as the role limits (`LIMIT_NOT_SENT`), done here as
    an `Option` and a `COALESCE` in the upsert.
  - One of the three words: stored.
  - Anything else: the WHOLE update is refused and nothing is saved. The person gets a `private`
    notice: `Your profile was not saved: "<word>" is not one of nobody, friends, anyone (who you
    accept files from).` No `profile_data` goes out, and the 30-second profile limit is not spent,
    so a corrected update goes through at once. Refused rather than ignored for the reason an
    unknown audience refuses a whole `reach_set` (handlers/reach.rs): saving the rest would leave
    the person believing their choice was kept, and dropping the word quietly would hide a broken
    app. The storage refuses the word too, as a floor under the handler. A value that is not a
    string fails the frame's parse, as any mistyped field does.
  - A `profile_update` is still a whole-profile save: every other field the app leaves out is
    saved empty, as before. Only `accepts_files` is kept when absent. Send the full profile.
- **Out: every `profile_data` this relay sends carries `accepts_files`**, public and outside the
  privacy map (its point is that a stranger's app can say "Ann accepts files only from friends."
  before sending): both answers to an update (the public broadcast and the person's own copy), the
  answer to a `profile_request` (no friendship pass needed; a name with no profile row answers
  `friends`), and the person's own profile sent on connect. An absent field means a relay from
  before this change.
- **HTTP.** `GET /api/members/{key}` carries `accepts_files` (`friends` when the member has no
  profile row). `GET /api/profile/{key}` carries it only when the key has an account on THIS
  server (found by its current name): the field is this server's setting and not part of the
  signed profile, so a profile gossiped from another server is answered without it, meaning this
  server does not know. The member list (`/api/members`) carries no profile fields and is
  unchanged, and federation gossip is unchanged (the signed profile's words are not touched).
- **Export and erase.** The export's `profile` entry is the whole profile row (`SELECT *`), so it
  lists `accepts_files`; the erase deletes the row, so the word goes with it and the name reads as
  the default afterwards. Nothing in `account.rs` needed changing, and `tests/account_sql_lint.rs`
  checks table names only, so it needed nothing either.
- **Tests**, each seen red once (the doc comment on each says what it saw):
  `relay::storage::profile::tests::an_older_database_gains_accepts_files_with_the_default` (a
  profiles table as v0.1485.1 left it, with a row, gains the column and the row reads `friends`;
  red without the ALTER), `..::only_the_three_words_are_stored_and_not_sending_one_keeps_it`
  (red without the storage's word check), `..::the_export_lists_accepts_files_and_the_erase_clears_it`
  (red with the export narrowed to three columns, and with the erase's profiles delete matching no
  row), and in `src/relay/handlers/msg_handlers_profile_tests.rs`:
  `a_profile_update_sets_it_and_every_profile_answer_carries_it` (red with the word not passed to
  the save), `a_word_that_is_not_one_of_the_three_refuses_the_whole_update` (red without the
  handler's check: the notice was the storage's general "Failed to save profile."),
  `an_older_apps_update_without_the_field_keeps_the_stored_word` (red with the upsert resetting an
  unsent word to the default), and `the_member_and_profile_lookups_carry_it` (red without the
  lookup in `/api/profile/{key}`). The frames in these tests are the JSON an app writes, parsed
  as the relay parses it.

## As built (web, files)

The website's half of parts 1 to 4 above, built 2026-10-11 against the relay's `accepts_files`
(the section before this one). Three new shared scripts, each a classic script in the browser and a
CommonJS module under Node: `web/shared/upload-clean.js` (`HosUploadClean`, the cleaning around
`HosStrip`), `web/shared/files-in-messages.js` (`HosFilesInMessages`, the settings, their rules and
words) and `web/shared/kept-files.js` (`HosKeptFiles`, the kept copies and the Settings controls).
The chat page loads all three and `strip-metadata.js` after `admin-erase.js` and before `crypto.js`
(the list of the chat's script order in CLAUDE.md does not name them yet); the Media page loads
`strip-metadata.js` and `upload-clean.js` before `media-upload.js`; the Settings page loads
`kept-files.js` before `settings-app.js`.

**Part 1, cleaned on this device.**
- Every upload path the website has runs it: the chat's attach button, paste and drop, in a room
  (`chat-messages.js uploadImage`) and in a DM, a group or the scratch pad
  (`sendEncryptedAttachment`, before the file is encrypted), and Add to Media (`media-app.js
  openAddForm`, before the fingerprint, so a resumed upload matches the cleaned file). The web's
  Shared Files page lists files and has no upload of its own; the Dev page's asset drop zone
  uploads without this session's token, which the relay already refuses, and was left alone.
- `cleanFile` reads a picture whole (up to the relay's 64 MB picture cap; larger is "could not be
  checked") and a video in the MP4 family whole up to 64 MB. Above that it reads only the top-level
  box headers and the boxes the relay's walk looks into (`ftyp`, `moov`, a top-level `meta`, `uuid`,
  each up to 256 MB), builds a small stand-in in which every other box is an empty box of the same
  name, strips that, and puts the file back together from the cleaned boxes and slices of the
  original (a `File` of parts: nothing near its size is held in memory). A header whose size makes
  no sense ends the stand-in with its own first bytes, so the walk stops where the whole file's
  does. The result is byte for byte the whole-file strip's, for every layout the strip's tests use
  and every cut of two movies (test below).
- **The attach step is shown only when there is something to say or decide**: hidden details found
  (the describe's sentence, its details in plain words, and the switch), a file that could not be
  checked, or a line about the recipient. A file with nothing hidden in it (a screenshot, a PDF)
  goes at once as before; the switch's remembered choice applies to it. Built with createElement
  and textContent, Send and Cancel. On the Media page the same words and switch are part of the Add
  form, for any picture or video.
- The switch: "Remove location and camera details", on by default, remembered on this device
  (`hos_remove_file_details`, in try/catch) when a file is sent. With the protected setup on
  (read under protected.js's own key and rule; the Media page does not load protected.js) it is
  not shown, a line says the setup removes them, and they are removed whatever was last chosen.
- Kept by choice: the file goes exactly as picked, with `keep_metadata=1` on the single upload and
  `keep_metadata: 1` in the resumable start.

**Part 2, a file that could not be checked.** `unchecked` is the strip's own `unfinished` (it
stopped early, a picture it could not walk, a file whose bytes are not what its name or type says,
no movie index), plus every HEIC or AVIF photo (by its bytes or its name), which the web cannot
clean yet.
- A room or the Media page: not sent; Send (Upload) is off and the words say why, and that turning
  the switch off sends it with everything in it. A HEIC gets the relay's own words ("HEIC and AVIF
  photos cannot be cleaned of where they were taken yet. Save it as a JPEG and send that."). Honest
  note: the relay now cleans a well-formed HEIC itself, so a public HEIC refused here would have been
  accepted and cleaned there; the spec's rule was kept, and letting a public HEIC go to the relay's
  strip is a small later change.
- A DM, a group or the scratch pad: "This file could not be read all the way through, so where it
  was taken may still be in it. Send it anyway?" (a HEIC: "This app cannot clean HEIC and AVIF
  photos of where they were taken yet, ..."), the button "Send anyway". What goes is what the strip
  could clean before it stopped (for a cut video, the XMP box before the cut is cleared). The
  marker's meta then ends `"unchecked":true`, after the desktop app's fields in their order (url,
  k, n, name, mime, size); a checked file's marker has no such field. The shared fixture is
  `scripts/tests/fixtures/file-marker-unchecked.json` (`built`: meta, JSON text and marker text;
  `read`: texts and whether each says unchecked), for the desktop's tests too.
- **Decision, not in the spec:** with the protected setup on, a file that could not be checked is
  not sent anywhere, a DM included (the setup's promise is that location is always removed).
- The recipient: a card whose marker says unchecked carries "The sender's app could not check this
  file for hidden location." under its name, written with textContent (`chat-dms.js`).

**Part 3, Files in messages.** In the chat's Safety page, after "Who can reach me" and "People I
choose": Accept files from (Nobody / Friends / Anyone I talk to), Show pictures without asking from
(Friends / Nobody), Show the attach button in private conversations. Kept on this device
(`hos_files_in_messages`).
- Receiving (`chat-dms.js`): a file "Accept files from" does not allow is never fetched; its card
  says "Ann sent a file. You accept files only from friends. Accept this one?" ("You do not accept
  files in messages." under Nobody, a friend's file included). Accept lets that one file through
  for this page: a picture is fetched and shown at once (Accept is the click), any other file gets
  its Save, which fetches it. "Friends" is the click-to-load rule's friend (a mutual follow I gave a
  pass); "Anyone I talk to" is everyone whose message reached me (who may message me at all is
  "Who can reach me", which the server enforces). My own files always open. "Show pictures without
  asking" replaces the friend exception of the click-to-load rule for private files; the protected
  setup is always Nobody (a friend's picture waits for the click too), and its own rule still hides
  a non-friend's file entirely. Public rooms' picture links are click-to-load for everyone, as
  before.
- The attach button setting hides the button in a DM or a group and stops a paste or a drop there,
  with words saying where it is turned off. The scratch pad keeps it (it stays on this device).
- With the protected setup on, making "Accept files from" more open needs the PIN (the same gate
  as a "Who can reach me" row); closing it never does; the pictures row is a line.
- **Publishing.** The relay saves a `profile_update` whole, so `accepts_files` only ever goes with
  the full profile, and only while a choice made on this device waits
  (`hos_files_accept_unsent`, so a choice made offline still goes); otherwise it is left out and the
  server keeps its word, and a newer choice made on another device is not undone. Nothing goes
  before this connection has heard the server's own copy of my profile, or heard there is none, and
  the profile sent is that copy with only the fields changed in this browser on top
  (`/shared/profile-changes.js`, BUG-195), so it never wipes a profile written elsewhere. The choice
  counts as saved only when my own copy comes back carrying it (an update the server turns down
  leaves it waiting; until the review of 2026-10-11 it counted as saved when sent). Which word wins
  when nothing waits follows BUG-200: a server's word that changed since this browser last saw it
  (`hos_files_accept_seen`, per did:hum) is another device's choice and is taken (never more open
  under the protected setup); otherwise a browser where the person chose
  (`hos_files_accept_chosen`) sends its choice to a server holding another word or no profile,
  and one that never chose takes the server's word.
- The sender's line: opening a DM asks the server once for that person's profile (`profile_request`,
  with my pass when I hold one) and keeps `accepts_files` from the answer. In the attach step: "Ann
  accepts files only from friends. Their app will ask them before it downloads this one." when I am
  not their friend (we do not follow each other, or they gave me no pass), or "Ann does not accept
  files in messages. ..." under Nobody. The file still goes; their app asks.
- **P2P groups:** the group object (`group_v1`) carries only its name and `share_history`, set when
  it is made, and nothing can change it, so the owner's "Files allowed" and "Largest file (MB)" wait
  for a group-settings increment (a signed settings object the owner can replace). Each member's own
  Files in messages settings already apply to every file in a group.

**Part 4, a copy of each private file opened.** `chat-dms.js openPrivateFile` looks for a kept copy
first and opens it without asking the server; a file fetched and opened (a picture shown, a file
saved) is kept, without waiting. The copies live in their own IndexedDB database,
`humanity_kept_files` (a new store inside `humanity_dms` would need a version change that an open
chat tab from before would block), encrypted with the DM store's own key (AES-256-GCM, the record's
id bound in as additional data) through `hosDmStore.keptFileGet` and `keptFilePut`. A record's id is
the SHA-256 of the store's scope, the file's address and its key, so neither appears in the
database; beside each copy only its size and an opening order are kept. A copy that does not open
is removed. Settings > Storage, "Files kept from messages": how much is kept, "Keep at most"
(Nothing, 50 MB, 100 MB, 200 MB by default, 500 MB, 1 GB), the oldest opened going first and a new
cap applied at once, and "Clear kept files", which asks first. A file larger than the cap is not
kept.

**Tests** (each seen failing once against a single break in a copy of web/, the note at each test
file's end names the break and what it saw): `scripts/tests/upload-clean-web.test.js` (the words,
the switch, the strip's bytes, the box-header path against the whole-file path for seven layouts
and every cut of two movies, the could-not-be-checked rules, the protected setup),
`files-in-messages-web.test.js` (the labels, choices and defaults against this spec, the accept and
pictures rules, the words), `kept-files-web.test.js` (the round trip and nothing named in the
database, the cap and its order, the Settings controls), `media-web-files.test.js` (Add to Media:
the cleaned photo goes up, keep_metadata, refusals, the protected setup, a 70 MB video read by its
headers and sent byte for byte as the whole-file strip makes it), and a section at the end of
`private-files-web.test.js` (the chat: the attach step in a room and a DM, keep_metadata, the
question and the unchecked marker read by the recipient's page, the HEIC words, the protected
setup, the shared fixture, Accept, Nobody, the Safety section and the profile frames, the PIN gate,
the sender's line, a kept copy opening after a reload with the server's copy gone). Builders for the
test files are `scripts/tests/file-bytes.js`, copied from strip-metadata.test.js. The older tests
that changed with the behaviour are named at the end of private-files-web.test.js; the older Media
tests run without the cleaning (`clean: null` in media-web-harness.js), because their stand-in
files are zeros named as videos. The four new test files are not in `just rig-tests` yet.

## As built (web, device-to-device)

The website's half of "Device-to-device big files: the shapes, exact" (parts 3 to 6, and the
client side of parts 1 and 2), built 2026-10-11 against the shapes above while the relay half was
built in parallel, and brought to "rules settled by the first review" (rules 1, 2 and 4, and the
Accept click) on 2026-10-12, then to the second review's rules 7, 8 and 9 the same day. The relay's
side (the pass check on `dc_offer` and on a `dm_put`'s `file_pass`, the `file` it puts on a dial,
file credentials, the forwarder) is stood in by the tests. The first build ran website to website
through a real browser and a real forwarder, and so did the second review's changes (2026-10-12,
below); the spec's "one desktop-to-web transfer between two real apps through a throwaway relay" is
still to do.

**Real browser run, second review (2026-10-12).** Two origins in one browser (Ann on
127.0.0.1:8098, Ben on localhost:8098, each its own storage and identity) through
`scripts/preview-server.js --api` to a throwaway loopback relay built from main at edbbdba69 (its
forwarder on a loopback port). What it showed:
- On the fresh server, Ben's browser (whose setting predates BUG-200's fix, so it carried no
  "chosen" mark) took the server's "friends"; choosing "anyone" in Safety reached the server
  (`/api/members` said so) and was remembered under that server and Ben's key.
- Ann's first offer was refused by Ben's "who can reach me" (Messages: friends, the safe default on
  a fresh server), and her row said so in words ("Ben's settings do not let you reach them").
  With Ben's Messages opened to anyone, the offer went through.
- 20 MB through the call relay: Ben's dial reached Ann carrying `file` from the real relay (Ann's
  page answers no file dial without it, rule 7), and the file ended Saved on Ben's side, Sent on
  Ann's.
- 120 MB with Ben's page RELOADED mid-transfer (about 18 s in): Ann's side never ended it as
  "stopped" (sampled every second, it read sending throughout), Ben's reloaded page dialled again on
  its own, and the file finished Saved and Sent (the receiver checks every piece and the whole file
  before saving). So the first of rule 8's two assumptions below holds in Chromium: a page that
  reloads is a drop, not a stop. The second (a channel's queued cancel goes before it closes) was
  not exercised: nothing failed on the receiving side.

**The second review's rules (7 to 9), in short (2026-10-12).**
- Rule 7, `web/chat/chat-file-transfer.js` (`fileTransferSignal`, `ftAnswerDial`, `ftRtcConfig`).
  The sender answers a dial only when it carries `file` (put there by the relay, commit 397ff2750)
  naming a live offer of mine to the dialler, and asks the call relay for THAT transfer's
  credentials alone: one allocation for the connection, one entry in `iceServers`, the same
  transfer the dialler asked for, so the forwarder joins the two. Every other transfer with that
  person rides the connection as its own channel and asks for nothing. Until then it asked for each
  live offer to that person, at most four: the idle allocations used up the person's share, and a
  dial for a fifth offer matched none of the four and never connected. A dc_offer without `file`
  came in some other way (a friend's own-device link) and is no file dial: `fileTransferSignal`
  hands it on to chat-p2p.js, which answers only my own devices. One naming a file of mine that is
  no longer live is taken and not answered. The dial is answered while something can go on it: an
  offer to them whose file this page holds, or a transfer of theirs to me waiting to open its
  channel. The dialler already asked for one transfer's credentials (the one it dials for).
- Rule 8, `web/shared/file-transfer.js` (`Receiver._end`) and `chat-file-transfer.js` (`ftFail`,
  `ftSendChannelGone`, `ftDropChannel`). The Receiver says `{"t":"cancel"}` on the channel for every
  end but one the sender said ('cancelled-there'): a piece damaged three times, a hash list or a
  whole file that does not match, a store that refuses a piece (a new reason, `storage`, "This
  browser could not keep part of the file (its storage may be full), so the transfer stopped."), and
  anything else going wrong (`error`). The page then sends the cancel note, with the offer's pass as
  the answer carries it and a copy to my other devices, and the row says why. Until then these four
  ends said nothing to anyone, and a desktop sender waited for the pass to run out, up to 7 days.
  A transfer's channel closed here now keeps its connection open until the channel has closed (its
  close event): a browser sends what was queued on a channel before closing it, but closing the
  whole connection drops that at once, and the last thing said is the cancel.
  On the sending side, a channel the other side closes before `done` is judged after 15 seconds
  (`FT_STOPPED_WAIT_MS`): still connected, and the transfer not back on a channel, it ends, "Ben's
  side stopped it", with the cancel note to Ben; the connection gone by then (a new dial from them
  replaced it, or the browser stopped saying connected) is a drop, and the transfer waits for the
  next dial, so a receiver whose page reloads still resumes. The wait is there because the two look
  alike when the channel closes: a closing page's channels close one event at a time, while the
  connection still reads connected here until ICE notices nobody answers. A connection already gone
  when the channel closes is a drop at once. A receiving transfer still live opens its channel again
  (or dials again) at the next minute's retry, and a pause the other side held is cleared, as
  before.
- Rule 9, checked and kept (`ftBindChannel`): a sender whose record still waits for the answer
  note takes its transfer's channel opening as Accept (the state moves to sending, so the first
  `hashes` or `get` need no rule of their own) and `done` as Sent, so a lost answer never leaves
  the row waiting.

**The 2026-10-12 round (the review's rules), in short.**
- Rule 1, `web/shared/file-transfer.js`. The `Sender` keeps what it owes as an ascending list: a
  `get` adds to it, the lowest piece goes next from offset 0, and a piece asked for while it is going
  out is sent again, whole, right after (`_owe`, `current.again`). A pause from either side drops
  what is owed. The `Receiver` keeps 16 pieces asked for: the first window, then one more piece
  (`{"from":n,"count":1}`) each time one is kept, which is how the desktop app asks too; a piece that
  fails its hash is asked for again alone, and nothing is asked again except after a 30-second stall.
  It ignores a message for a piece it did not ask for or already holds, keeps a partial buffer only
  for a piece it asked for, and keeps each piece as soon as it checks, in whatever order it arrives
  (a piece asked again after a bad hash comes after the ones asked before it). The store a `Receiver`
  uses now says which pieces it holds (`held()`) instead of how many in a row (`firstMissing()`); the
  page reads them from the IndexedDB keys.
- Rule 2, `web/chat/chat-file-transfer.js`. A link is one connection with one person holding a map
  of channels, one per transfer, each with its own engine (`ftBindChannel`, `chan.engine`). A
  transfer's channel is opened by its receiver whichever side dialled (`ftJoin`, `ftOpenChannel`):
  a receiver with a connection already there opens its channel on it, now or once it is up, and both
  the dialling and the answering side take a channel the other opens (`pc.ondatachannel` ->
  `ftChannelArrived`, which serves it with a `Sender` of its own). A transfer that ends closes its
  own channel (`ftDropChannel`), a channel that closes takes only its transfer off (`ftChanGone`),
  and the connection closes when its last channel has ended (`ftCloseIfIdle`). Checked in headless
  Edge on 2026-10-12: a data channel opened later by either side of a live connection reaches the
  other and opens with no new offer, and closing one leaves the others and the connection up.
- Rule 4. The receiver's answer (yes or no) and its cancel carry the offer's pass on the `dm_put` to
  the sender as `file_pass`, the same object the offer carried; the copy to my own devices carries
  none, and the sender's own cancel carries none (it would not verify that way round).
- **The Accept click (found by the review's real-browser run: a real click on Accept left the
  question on the row, a script's `click()` started the transfer).** The click WAS handled; the
  hint seen ("Tap or click to use this control.") is `shell.js`'s rich tooltip, which has
  `pointer-events: none` and took nothing. Accept called `showSaveFilePicker` and waited for it
  before changing anything, and returned without a word when it was closed. Measured in headless
  Edge on 2026-10-12: after a real click it rejects at once with `AbortError` ("The user aborted a
  request.", there is no dialog to show), and after a script's click with `NotAllowedError` ("User
  activation required."), which the code took as "no dialog" and went on. So the run's real clicks
  hit the silent return and its script click did not. Now one click accepts: the row says
  "Connecting" in the same click, the answer goes and the dial starts, and the save dialog (asked
  for in the same click, since a browser opens it only straight after one) only decides where the
  finished file goes; closed or absent, it is the browser's own download, and still open when every
  piece is in, the row says "Choose where to save it" and the file waits for the place.
- **A second click problem, found while looking:** every redraw emptied the row and made new
  buttons, and a row is redrawn with every piece. A browser sends a click only to the element the
  button went down and came up on, so Pause and Cancel could hardly be pressed while a file moved.
  The row is now built once and changed in place (`ftFill`, `ftRowParts`): each button is the same
  element for the row's life, hidden when it does not apply. Decline and Cancel now redraw the row
  before their slower work (saving the record, dropping the pieces), so every button changes the row
  in the click. Each button carries a `data-tip-desc` saying what it does, so the hint says that.

**Files.** Two new shared scripts, each a classic script in the browser and a CommonJS module under
Node, and one chat script:
- `web/shared/blake3.js` (`HosBlake3`): BLAKE3, the plain 32-byte hash, streamed. The vendored
  noble bundle's BLAKE3 measured about 12 MB/s on this machine, over three minutes to hash a 2.3 GB
  file and a cap of about 12 MB/s on receiving; this one measured about 167 MB/s. It does only the
  default hash mode (no keyed mode, no derive_key, no longer output) and is held to the noble bundle
  at every length around the block, chunk and tree boundaries and for every way of splitting the
  input across updates. (`web/pages/media-upload.js` says the web cannot hash a gigabyte cheaply;
  this would let it, which is the media lane's call.)
- `web/shared/file-transfer.js` (`HosFileTransfer`): the pass's words and JSON, the three notes'
  exact text, the channel's messages, hashing a file a piece at a time, the `Sender` and the
  `Receiver` (the channel protocol, with nothing of the page in them), and the words on screen.
- `web/chat/chat-file-transfer.js`: the attach step's way for a big file, minting the pass, sending
  and taking the notes, the question, the rows, the connection, the pieces kept in IndexedDB and the
  save.
- Changed: `chat-messages.js` (a file over my private limit in a DM goes to `offerFileDirect`; the
  attach step takes `direct` lines and can turn Send off; `acceptsFilesKnown`), `chat-social.js`
  (`ingestDmControl` hands the three notes over, never shown), `chat-p2p.js` (a note arriving on an
  own-device channel is dropped unread), `chat-voice-rooms.js` (the third `call_credentials` request,
  its reply matched on `file`), `chat-voice-calls.js` (`webrtc_signal` asks the file transfers
  first), `chat-dms.js` (the rows, after the conversation's messages), `chat-privacy.js` (the direct
  connections row), `chat-dm-store.js` (`sealBox` / `openBox`, the record's encryption),
  `index.html` (the order: `blake3.js` and `file-transfer.js` after `kept-files.js`,
  `chat-file-transfer.js` after `chat-warnings.js` and before `chat-protected.js`, which stays the
  last script to wrap the frame handler) and `messages.css`.

**Part 1, the pass.** Minted for each transfer by the sender: Dilithium3 over
`hum/file-pass/v1\n{server}\n{me}\n{them}\n{id}\n{expiry}`, the server's `did:hum:` from its
`identify_challenge`, an end 7 days ahead. The receiver checks the signature (and that it has not
ended) before it shows anything; an offer whose pass does not check is dropped.

**Part 2, credentials.** `{"type":"call_credentials","file":"<id>","peer":"<key>","file_pass":{...}}`,
exactly. The relay's reply echoes `file` the way it echoes `room` and `call`
(`src/relay/call_credentials.rs`); a reply without it is never matched and the wait (8 seconds) runs
out. A refusal or no reply gives the spec's "This server is not set up for that yet" on the row,
with Resume to try again. Each side asks once per connection (rule 7): the dialler for the transfer
it dials for, the sender for the one the dial names.

**Part 3, the notes.**
- The offer goes to the recipient alone, with no copy to my other devices (they do not have the
  file). It carries a `ref`, so a `dm_put_refused` shows on the row ("The offer was not delivered:
  Ben's settings do not let you reach them." and the like).
- The answer and the cancel go with a copy to my other devices: a question still open on Ben's other
  device becomes "Answered on your other device", and a cancel reaches them all.
- The receiver's answer and cancel carry the offer's pass on the put to the sender (`file_pass`,
  rule 4), so they reach a sender who takes messages only from friends once the relay lets such a
  put through; a relay from before that ignores the field. They carry no `ref`: a refused one
  changes nothing on the receiver's row, and its dial (whose `dc_offer` carries the pass) still
  starts the transfer.
- A note that cannot go (no connection) is kept with the transfer's record and goes on the next
  member list.
- **Files in messages first**, then "who can reach me", then the protected setup's hide rule. An
  offer from a sender this device does not accept files from is not asked about, and its row says so
  ("Ann offered you clip.mp4 (2.3 GB). You accept files only from friends, so you were not asked.").
  **Decision:** no answer goes back for it. An automatic decline would tell the sender the moment
  the receiver's app was online; the sender's row waits, and says "Ben did not answer in time" once
  the pass has ended. The sender's attach step already says when the other person's published choice
  means their app will not ask them, and then Send is off.
- An offer whose pass has ended is not asked about ("This offer ran out. Ask for it to be sent
  again.").

**Part 4, the connection.**
- **One connection with each person at a time, several transfers on it (rule 2, since
  2026-10-12).** Each transfer has its own data channel and its own engine, opened by its receiver,
  whichever side dialled; a file each way, or several the same way, ride one connection. A channel's
  transfer ends without touching the others, and the connection closes when its last channel ends.
  (Before 2026-10-12 a second file between the same two waited for the first to finish and the
  next minute's retry, and on the answering side every channel went to the first transfer's engine.)
- The receiver dials (on Accept, on Resume, and when the sender appears in the member list; and
  every minute while the page is open), unless a connection with that person is already there, in
  which case its channel opens on that one. The sender answers a dial only when the relay's `file`
  on it names a live offer of mine to them (rule 7, since 2026-10-12), with that transfer's
  credentials alone; every later channel rides that one allocation. (Before, it could not tell
  which file a dial was for, asked for each of its live offers to that person, at most four, and
  put them all in its connection.) A channel whose label is no live offer of mine to them is
  closed.
- Both dial at once (each has a file for the other): the larger key's dial goes on, as the desktop
  app's offerer rule has it, and the other side's receiving transfers open their channels on the
  connection it answers. A dial from someone I already have an open connection with means theirs is
  gone: mine is dropped and the new one answered.
- Relay only (`iceTransportPolicy: 'relay'`) unless the two are friends both ways (a mutual follow,
  and each holds the other's friendship pass) and the person turned on "Let files from friends go
  straight between devices" (Safety > Files in messages; off by default; never with the protected
  setup on). Direct adds this server's own address lookup (`/api/turn-credentials`) and keeps the
  forwarder's credentials. Each side applies its own setting, so each guards its own address.

**Part 5, the channel.**
- **Decision on the 64 KiB:** a message is at most 64 KiB INCLUDING its 8-byte header, so each
  carries 65,528 bytes of a piece and a 256 KiB piece is five messages. The receiver takes any message
  that continues the piece it is filling, so a sender that puts a full 64 KiB after the header still
  works with this receiver. The piece hashes go as messages of exactly 64 KiB (2,048 hashes), no header.
- Rule 1 (since 2026-10-12): a `get` adds to what the sender owes, which goes in ascending order,
  each piece once, from offset 0; a piece asked for while it is going out is sent again, whole,
  after it. The receiver keeps 16 pieces asked for: the first 16 it lacks, then one more each time
  one is kept (one get per run of pieces, so `{"from":n,"count":1}` once the first window is out).
  Nothing for 30 seconds with pieces asked for: it asks again for the first ones it lacks (nothing
  counted against a piece). A piece that fails its hash is asked for again alone; the third failure
  ends the transfer ("Part of the file arrived damaged three times, so the transfer stopped."). A
  message for a piece it did not ask for, or already holds, is ignored. A list of piece hashes that
  does not match the offer, or a whole file that does not, ends it with "The file Ann offered does
  not match its description". Every such end says cancel on the channel and sends the cancel note
  (rule 8, since 2026-10-12), so the sender's row says "Cancelled".
- Pause: each side's pause holds until the same side resumes. A pause from either side drops
  everything the sender owes and every half-received piece; on resume the receiver asks again for
  the first pieces it lacks.
- The sender reads only the piece it is sending and holds back while the channel buffers more than
  1 MB (the `bufferedamountlow` event, with a half-second check as well).
- Pieces are kept in IndexedDB (`humanity_file_transfers`, store `pieces`, one Blob per piece, key
  `<id>:<piece>`, each kept as soon as it checks, in whatever order; which are held is read from the
  keys after a reload). **Decision:** the pieces are not encrypted at rest. The saved file will be in the open in
  any case, and an encrypted piece would have to be opened into memory to make the file's Blob. Each
  transfer's record (who, the offer, how far) is encrypted with the DM store's key, as the DM history
  is. The pieces go when the transfer is cancelled or fails, and two minutes after a save.
- The whole file's BLAKE3 takes the pieces in order as they come (a piece that came early is read
  back from IndexedDB once the gap before it fills) and, after a reload, from the pieces kept. Only
  when it matches is `{"t":"done"}` sent and the file saved: into the place chosen in the save dialog
  where the browser has the File System Access API (the dialog opens in Accept's click and the
  transfer does not wait for it; a file whose pieces are all in waits for the place, "Choose where to
  save it"), and otherwise, or when the dialog was closed, as a Blob made of the kept pieces, saved
  through the browser's own download, with a "Save again" link while the pieces are kept.

**Part 6, the screens.** A row per transfer after the conversation's messages, on both sides: the
name, the size, a progress bar, Pause, Resume and Cancel, and the spec's words for each state,
plus a few of this page's own: "Getting the file ready" (hashing), "Connecting", "Paused", "Paused by
Ann", "Checking the file", "Receiving through the server's call relay", "You declined", "Answered on
your other device", "Choose where to save it", and since 2026-10-12 rule 8's "Ben's side stopped it"
and "This browser could not keep part of the file (its storage may be full), so the transfer
stopped." The sender's row says the file goes from this device
and the page must stay open. **After a reload the sender no longer has the file** (a browser does
not keep a picked file across a reload): the row asks for it to be chosen again ("Choose the file
again"), and only the same file (its size, its BLAKE3 and its piece list) is taken. Every button
changes the row in its own click, the row is changed in place rather than rebuilt (so a redraw
never takes a button from under the pointer), and each button's hint says what it does.

**Not done, or honest notes.**
- The shared fixture `scripts/tests/fixtures/file-offer-v1.json` (minted by the relay's Rust) is
  read by test 2 since 2026-10-12: the pass's words, its signature, the three notes and the file's
  hashes must match it byte for byte.
- The offer has no field saying the file could not be checked for hidden location: the sender is
  asked "Send anyway?" as for any private file, but the receiver is not told.
- After a reload with every piece already kept, the receiver dials once more to check the file and
  say done, so the sender must still be online for the save to happen there.
- Two of the receiver's devices pressing Accept at the same moment may both dial; the signalling
  cannot tell them apart.
- The browser's own storage limit is the only size limit (Firefox and Safari allow less than
  Chromium). Since 2026-10-12 a piece the storage refuses ends the transfer with words and tells the
  sender (rule 8), and a piece counts as kept only once the browser has COMMITTED its write
  (`ftIdb` waits for the transaction's complete event): a full disk is often reported at the
  commit, after the request itself succeeded, and such a transfer used to run to the end and fail
  as "does not match its description" (the review of 2026-10-12; test 21).
- A dial for an offer this page already ended (cancelled here, or failed) is not answered, but its
  cancel note is said again, once in the page's life: a receiver who never got the first one would
  otherwise keep dialling for that transfer, and its other files with this sender would wait
  behind the dial (the review of 2026-10-12; test 20). A note that could not go is saved with the
  record, so it survives the page closing and goes when the socket is back. The same goes for a
  CHANNEL the receiver opens for such a transfer on a live connection (the dial was for another
  file): `{"t":"cancel"}` is said on it before it closes, and the cancel note goes again under the
  same once-a-page rule; before, it was closed without a word and the receiver reopened it every
  minute (the desktop review of 2026-10-12; test 22).
- Rule 8's judging of a closed sending channel rests on two things a real browser does that only
  the stand-ins have shown here: a closing page's connection stops reading connected within the
  15-second wait (its ICE checks go unanswered; how fast a browser says "disconnected" was not
  measured), and a channel's queued cancel goes before the channel closes (the WebRTC spec's closing
  procedure). The first was checked in a real browser on 2026-10-12 (a reload mid-transfer resumed,
above); the second was not. A receiver
  that ends its only transfer without a word AND closes its connection looks like a drop, so the
  sender waits; both apps now send the cancel note on every end, which covers it.
- Groups come later (file-transfer-at-scale.md, layer 3).

**Tests** (each seen failing once against a single break in a copy of web/; the note at each test
file's end names the break and what it saw): `scripts/tests/file-transfer-web.test.js` (BLAKE3; the
pass's words and the notes against the spec's own blocks; fit offers; the hash list; piece checks
and three tries; resume after a drop and after a reload; pause from either side; cancel from either
side; the whole-file check before saving; a 40 MB virtual file read only a piece at a time, messages
of at most 64 KiB, the channel held near 1 MB) and `scripts/tests/file-transfer-chat-web.test.js`
(two chat pages and a stand-in relay and WebRTC: the attach step and the sealed offer with a real
Dilithium3 pass, Files in messages first, decline, the whole transfer through the call relay only,
cancel from either side, pause, a server not set up, resume after a drop and a reload, expiry, a
file that does not match, the other device, direct connections). `private-files-web.test.js` loads
the new scripts and two of its tests changed (a file over the limit in a DM is now offered device
to device); `p2p-direct-offers.test.js` and `calls-through-server.test.js` load the new chat script,
so they hold it to answering no offer that is not a file's.

Added 2026-10-12 for the review's rules (each seen red against a single break, R1 to R7 and C19 to
C26 in the notes at each file's end):
- `file-transfer-web.test.js` 11: the `Sender`, asked by a stand-in that asks exactly as the desktop
  app's receiver does (16, then one piece at a time), sends every piece once, in order, with no
  stall (red with a get replacing what is owed: 25 of 40 pieces, then nothing). 12: a piece asked
  again after a bad hash comes again, and one asked while it is going out comes again straight after
  it, whole (red without the "again", and with a piece never sent twice). 13: the `Receiver` asks the
  first window, then one piece at a time, never a piece twice, keeps at most 16 asked and partial
  buffers only for those, does not keep a piece nobody asked for until it asks for it, ignores a
  damaged copy of a piece it holds, and after a stall asks for the lost piece alone (red with a whole
  window asked on each arrival, with an unasked piece taken, and with the stall asking nothing). Test
  5 now also holds a bad piece to being asked again alone (red with a window asked instead), and no
  longer requires that nothing after a failing piece is kept (pieces are kept in any order now).
- `file-transfer-chat-web.test.js` 13: two files to Ben (the second accepted while the first moves)
  and one each way, all over one connection, a channel each (red with the dial waiting for the
  connection, with the answering side handing a channel to the first transfer's engine, with the
  dialling side taking no channel, and with an ending closing the whole connection). 14: the answer
  (yes and no) and the receiver's cancel carry the pass, and reach Ann where the stand-in relay
  takes only friends' puts or a put whose pass verifies with real Dilithium3 (red with the pass left
  off). 15: one click on Accept with the save dialog closed at once (as headless Edge does), still
  open, or absent; Decline, Pause, Resume and Cancel each change the row in the click; the Pause
  button is the same element across redraws (red with the old Accept, with the row rebuilt on each
  redraw, and with an ended transfer drawn only after its pieces are dropped). The stand-in WebRTC
  now delivers a channel made on a live connection to the other side, as Edge does.

Added 2026-10-12 for the second review's rules 7 to 9 (each seen red against a single break in a
fresh copy of web/, R8, R9 and C27 to C34 in the notes at each file's end; only the test named
failed):
- `file-transfer-web.test.js` 14: a Receiver that ends with a piece damaged three times, a hash list
  or a whole file that does not match, a store that refuses a piece, or a store that cannot read
  back a piece kept before, says cancel on the channel once, and the Sender ends as cancelled there;
  the sender's own cancel is not echoed (red with `_end` saying nothing, and with the store's
  refusal ending as `error`).
- `file-transfer-chat-web.test.js` 17: five offers to Ben live, he accepts the fifth: his dial
  reaches Ann naming it, she asks for that transfer's credentials alone and makes one allocation,
  and a second file he accepts while it moves rides the same connection with no credentials of its
  own; a dc_offer naming no file makes nothing (red with the old four-offer answer, which never
  connected, and with a nameless dial taken as the oldest offer's). 18: Ben's storage refuses a
  piece: cancel on the channel, the cancel note to Ann with the pass and a copy to his devices, his
  row says why, and Ann's row ends from the channel alone with her copy of the note lost; a file
  that does not match with the channel's cancel lost ends her row from the note; a channel Ben's
  side closes without a word ends Ann's transfer after the wait ("Ben's side stopped it", the note
  to Ben, his row "Cancelled"), and a drop does not (red with no cancel said, no note, the
  connection closed before its channel, the close never judged, and the close judged at once). 19:
  Ben's answer to Ann lost: her row goes from "Waiting for Ben to accept" to "Sending through the
  server's call relay" when his channel opens, and to "Sent" on done (red with an offered transfer
  left offered, and with done counted only from sending as well). The stand-ins grew to match: the
  relay puts `file` on a dial its file pass let in, its credentials name the transfer and the
  stand-in WebRTC joins only allocations of the same one (unless both may go direct), a channel a
  page closes sends what was queued first, and a closed connection leaves the other side reading
  "disconnected".

## As built (desktop, files)

The desktop app's half of parts 1 to 4 above, built 2026-10-11 against the relay's `accepts_files`
and the web's half (the two sections before this one), in one worktree.

**The shared describe.** `src/relay/core/describe_metadata.rs` is a port of the web's
`HosStrip.describe`, function for function, beside the three strips (which are unchanged), so the
desktop's attach step says the web's words. It uses the standard library only. JavaScript's text
rules are reproduced where they matter: the browser's UTF-8 decoding (a byte-order mark dropped),
runs of control characters made one space, JavaScript's own list of white space for trimming,
limits counted in UTF-16 units, `parseFloat`, and the regular expressions written out by hand in
their "first match wins" order. The one difference left is a lone UTF-16 surrogate (half an emoji
cut by a length limit, or a broken UTF-16 place name), which Rust cannot hold: dropped at a cut,
replaced inside a name. `describe_file` reads an MP4, MOV, 3GP or HEIF file by its box headers and
loads only the metadata boxes (each up to 64 MB), so a multi-GB video is described without reading
its frames; a picture is read whole up to 64 MB (`PICTURE_READ_MAX`).
- Held together two ways. `tests/fixtures/describe/` holds 58 files built by
  `scripts/lib/describe-corpus.js` with the web's own answer for each in `expected.json`
  (`node scripts/make-describe-fixtures.js` rewrites them, only when the web's describe changes on
  purpose); the Rust's tests and `scripts/tests/describe-agree.test.js` both require those answers.
  The agreement test also compiles the Rust file with rustc alone and requires the same answer as
  the web on 6,058 files (the fixtures and 6,000 generated: text with accents, emoji, control
  characters and byte-order marks, GPS in every ISO 6709 and XMP notation, random cuts and flipped
  bytes), and the same answer from disk as from memory. Its first version passed a deliberately
  broken ISO 6709 reader, because its generated positions almost never came in the
  degrees-and-minutes form where the readings differ; the generator now writes all three forms. It
  is not in `just rig-tests` yet. `.gitattributes` marks the fixtures `-text`.
- HEIC and AVIF: the desktop cleans them (the relay's own HEIF strip), the web cannot yet. Both
  describes report a HEIF photo's top-level XMP the same way; neither reads its Exif item, so the
  attach step says nothing for a typical iPhone HEIC on either app, while the desktop still removes
  it. Reading the HEIF Exif item in both describes is a small later change.

**Part 1, cleaned on this device.** `src/net/file_clean.rs`: `clean_bytes` (a file in memory),
`look_file` (what the attach step shows: the describe plus whether the file can be checked) and
`clean_copy` (a big file on disk, cleaned in a COPY under the app's data folder,
`cleaned-uploads/`, never the person's own file). The verdict on a file that could not be checked is
the relay's own `finish_file_as`: a picture the strip cannot walk, a movie whose boxes stop early,
an MP4-family file with no movie and no HEIF picture, a HEIF whose item tables cannot be read with
certainty, or a file named as one of the cleaned kinds whose bytes are not one.
- Every upload path: the chat's Attach (a room, a DM, a group, the scratch pad; `attach_send::run`
  cleans before it encrypts or uploads, on the upload's worker), a pasted picture (a PNG this app
  encoded, cleaned without a step since nothing can be hidden in it), the Files page's upload to
  Shared Files (`src/gui/pages/files_upload_step.rs`: the file is now read on a worker, not on the
  frame as before), and Add to Media (`media/resumable.rs` `work` cleans a copy before the hash and
  sends `Event::Cleaned`, so the upload's record, kept in the config for carrying on after a
  restart, follows the copy; copies no upload uses are swept after ten idle minutes,
  `uploading.rs sweep_cleaned`). The desktop has no drag and drop into the chat.
- The attach step (`src/gui/pages/chat/attach_step.rs`) follows the web's rule: it shows only when
  there is something to say or decide (hidden details, a file that could not be checked, a line
  about the recipient, or a file that could not be read); otherwise the file goes at once with the
  remembered switch. The look runs on a worker; "Checking the file for hidden details..." shows only
  if it takes over 0.6 s. The same look and switch are drawn by the Files page's step and by the Add
  to Media form (`draw_look`). The switch is `FilesInMessages::remove_details` in the app's config,
  remembered when a file is sent; under the protected setup it is not shown, a line says the setup
  removes them, and they are removed whatever was last chosen.
- Kept by choice: `&keep_metadata=1` on a single upload (`UploadFlags::keep_metadata`), and
  `"keep_metadata": 1` in the resumable start (`start_body`).

**Part 2, a file that could not be checked.** In a DM, a group or the scratch pad the step says
"This file could not be read all the way through, so where it was taken may still be in it. Send it
anyway?" with Send anyway; the file goes cleaned as far as the strips got, and its marker carries
`"unchecked":true` (`dm_pq::build_file_marker_unchecked`, the fields in their order then the flag,
byte for byte the web's `scripts/tests/fixtures/file-marker-unchecked.json`, read with `include_str!`
by `src/net/dm_pq_marker_tests.rs`; `file_marker_unchecked` reads it, only JSON true counting; the
`DmAttachment` struct is unchanged, so no other code changed). A room, Shared Files and Media do not
send it, with words saying that turning the switch off sends it exactly as it is. As on the web,
the protected setup sends such a file nowhere, a DM included. The recipient sees "The sender's app
could not check this file for hidden location." under the file (`attach_view.rs`).

**Part 3, Files in messages.** Settings > Privacy has a "Files in messages" card
(`src/gui/pages/files_settings.rs`): Accept files from (Nobody / Friends / Anyone I talk to), Show
pictures without asking from (Friends / Nobody; under the protected setup a line, always Nobody),
Show the attach button in private conversations, and the switch. One AppConfig field,
`files_in_messages` (`net/files_policy.rs` `FilesInMessages`), so the settings persistence lint now
also scans `files_settings.rs` and `attach_step.rs` (without them it never saw the setting; seen red
with its save leg removed).
- Receiving (`attach_view.rs plan`), in this order: the protected setup's own rule (a non-friend's
  file left out), a click already given, a file on another website, then "Accept files from" (a
  file not accepted is never fetched, picture or not: "Ann sent a file. You accept files only from
  friends. Accept this one?", Accept letting that one file in), then the pictures setting ("Pictures
  from Ann show only when you choose." for a friend's picture under Nobody or the protected setup).
  "Friends" is the click-to-load rule's friend (`picture_friend`); "Anyone I talk to" is anyone
  whose message reached this person. Our own files always open.
- The attach button is hidden in a DM or a group when turned off, and a paste there says where it
  is turned off; the scratch pad keeps it.
- The sender's line: opening a DM asks the server once for that person's `accepts_files`
  (`GET /api/members/{key}`, which also gives their name), and the attach step says "Ann accepts
  files only from friends. They will be asked whether to accept this one." when it applies.
- Publishing, by the web's rules: `accepts_files` goes only while a choice made on this device waits
  (`accept_unsent`, kept in the config, so a choice made offline still goes), once per choice and
  server in a run of the app, and counts as saved only when that server's own copy comes back
  carrying it. It goes in the whole profile every save sends (`src/gui/pages/own_profile.rs`,
  BUG-194 and BUG-196): each server's own copy is kept from its socket (the active one and those
  open in the background), asked for with a `profile_request` for the person's own name when a
  server has sent none, and a save waits for it and for the relay's one-update-per-30-seconds
  limit. While nothing waits, a different word in the server's copy is another device's newer
  choice and is taken (`files_policy::adopt_from_server`), never a more open one under the
  protected setup.
- Differences from the web, each for a reason. Under the protected setup "Accept files from" can
  be closed but not opened here (the more open choices are greyed out); the web asks for the PIN
  instead, which on the desktop needs an arm in `engine::protected::perform`.
- P2P groups: the group object carries only its name and roster (`P2pGroupInfo`), so the owner's
  "Files allowed" and "Largest file (MB)" wait for a group-settings increment; each member's own
  settings apply in groups already.
- The Profile page's Save, the privacy tier chooser and the directory checkbox send the whole
  profile the same way (they sent this device's own fields alone, wiping the rest, until
  2026-10-11: BUG-194).

**Part 4, a copy of each private file opened.** `src/net/kept_files.rs`. What is kept is the
server's ciphertext of the file (sealed with the file's own key, which lives only in the marker
inside the encrypted DM history), encrypted again with AES-256 in counter mode under
`BLAKE3.derive_key("hum/kept-files/v1", seed)`: a random 16-byte counter start, then the bytes. So a
kept copy opens only with both the identity and the marker, a 1 GB file is kept and read a megabyte
at a time, and any changed byte is caught by the file's own AES-GCM tag when it is opened. Names are
a keyed hash of the file's address, key and nonce (`kept-files/<hash>.kept` beside the DM history).
A picture shown in place (`open_kept`) and a Save (`save_file`) look for a kept copy first, keep
what they fetched once it decrypts, and forget a copy that does not open. Settings > Data, "Kept
files": what they take, "Keep up to (MB)" (0 to 64 GB, 1 GB by default), the files opened longest
ago going first (opening one touches its time), and "Clear kept files". The web keeps its copies in
IndexedDB with the DM store's key instead, a different mechanism to the same end.

**Tests**, each seen failing once against a single break (the doc comment on each names the break
and what it saw): `relay::core::describe_metadata` (3: the fixtures from memory and from disk, the
words, nothing left after the strips), `net::file_clean` (5), `net::files_policy` (6),
`net::kept_files` (3), `net::dm_pq::marker_tests` (the shared fixture), `config::...files_in_messages`,
`gui::pages::chat::attach_step` (4), `attach_send::clean_tests` (2), `attach_view::kept_tests` (2),
`attach_view::tests::the_click_to_load...` (rewritten for the accept and pictures settings),
`gui::pages::files_settings` (2), `gui::pages::files_upload_step` (1),
`media::resumable::clean_tests` (2), and `scripts/tests/describe-agree.test.js`. Older tests that
changed with the behaviour: `attach_send_tests.rs file_bytes` is now a real (clean) JPEG, since a
".jpg" that is not one could not be checked, and the Media share test's stand-in file is named
`.dat`, not `.png`, for the same reason.

## As built (relay, device-to-device)

The relay half of "Device-to-device big files: the shapes, exact", parts 1 and 2, built
2026-10-11, plus the Roles editor rows for the new limit. What the two apps build against:

**The file pass** (`src/relay/core/pq_crypto.rs`). The sender's app signs, with its Dilithium3
identity key,

```text
hum/file-pass/v1\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}
```

and carries `{"v":1,"id":"<id>","expiry":<n>,"sig":"<base64>"}`, an object in every frame (never
a string holding JSON; a string is refused). `build_file_pass` writes exactly that text, keys in
that order. `id` is 32 lowercase hex characters (`file_pass_id_ok`, `new_file_pass_id`); `expiry`
is Unix seconds, more than now and at most `FILE_PASS_MAX_DAYS` (7) ahead when minted, and the
check allows `FILE_PASS_CLOCK_SLACK_SECS` (one day) past that. `verify_file_pass(server, issuer,
grantee, pass, now)` refuses, cheapest first: `Malformed` (not an object, `v` not 1, a missing
field, an `expiry` that is not a whole number, a `sig` over 8,192 characters, an empty or
line-broken server or key), `BadId`, `Expired`, `TooLong`, `BadSignature`. The minting refuses
whatever the check would.

**Where the relay reads it**, always through `handlers/reach.rs` `file_pass_of(state, issuer,
grantee, pass)`, which supplies the relay's own server DID and clock:

- **`webrtc_signal` gains `file_pass`** (optional, the object). A `dc_offer` from S to T now also
  passes when the pass checks with issuer T and grantee S (`dc_offer_may_pass`). The offer goes on
  to T with neither `friend_cert` nor `file_pass` (its wire form has no such field). A file pass
  opens no call: a call's `offer`, `answer` or `ice` with one is checked exactly as before.
  Refusals stay silent.
- **`call_credentials` gains a third request**,
  `{"type":"call_credentials","file":"<id>","peer":"<key>","file_pass":{...}}`. Granted when the
  pass checks with (issuer, grantee) = (peer, asker) or (asker, peer), its `id` is `file`, and
  `peer` is not the asker. The reply is the usual one with `"file":"<id>"` instead of `room` or
  `call` (urls, username, credential, ttl 3600); a refusal is `{"type":"call_credentials",
  "file":"<id>","refused":true}` plus a notice: `NO_FILE_PASS` ("No credentials for that
  transfer: its file pass is missing, run out, or not between you and that person."), `NOT_IN_IT`
  for a request naming `file` beside `room` or `call`, `NOT_SET_UP` when the forwarder is not
  listening, `TOO_MANY` past twenty a minute. The scope is the keyed hash of
  `file\n{id}\n{the two keys, sorted}` (`CallKeys::file_scope`), so the forwarder passes packets
  only between the two people's allocations for that transfer. `call_credentials` and
  `webrtc_signal` are both in the Voice feature: an owner who switches voice off switches this
  off too (the forwarder only runs when voice is offered).

**The username of a file credential** is `{expiry}:{scope}-{nonce}-{tag}-{limit}`: the call's
shape, then the asker's person tag (32 hex, `CallKeys::person_tag`, a BLAKE3 keyed hash of the key
under a key derived from this run's secret, `hum/call-person-tag/v1`) and their role's
`max_forwarded_mb_per_day` at the time of issue, decimal. The HMAC covers the whole username, so
neither part can be changed. `parse_username` accepts both shapes; `username_meter` returns the
tag and limit (`Meter`) for a file's and None for a call's or a room's.

**The day count** (`src/relay/call_forwarder.rs`). An allocation made with a file credential keeps
its `Meter`. `deliver` counts the bytes each tag's allocations send through the forwarder per UTC
day (`forwarded_today`) and passes a packet only while the tag's count with it stays within
its limit (0 = no limit); past it nothing more of that tag's passes until the next UTC day.
The limit is the person's, across all their transfers; the receiver's own replies count against
the receiver's tag. A call's and a room's allocations carry no tag and are never counted. The
counts live in memory: a restart starts the day again (and ends every credential anyway).
Yesterday's counts are dropped in the forwarder's once-a-second sweep.

**The role limit** (`src/relay/storage/roles.rs`, `RoleDef::max_forwarded_mb_per_day`). 0 to
`FORWARDED_MB_PER_DAY_MAX` (1,000,000) when saved; Admin 0 and every other built-in 2048 on a
fresh server, and the same by tier (`CASE base_tier WHEN 'admin' THEN 0 ELSE 2048`) for an
existing table, which gains the column through a guarded ALTER after the upload limits' and
before the seed (BUG-046). A role save without the field (an app from v0.1486.0 or older) keeps
the stored value (`LIMIT_NOT_SENT`); a new role saved that way gets 2048. It travels in
`role_list` and `role_upsert` like the other limits.

**Roles editor.** The desktop app's Server Settings > Roles "Upload limits per role" table gains
a seventh column, "Forwarded a day (MB, 0 = no limit)", with a line under the table saying what
it is (`src/gui/pages/server_settings/role_limits.rs`: `COLUMNS`, `FORWARDED_LINE`, `ranges`,
`prefill`). The website still has no Roles editor (see round 2, "Not done here"), so the limit is
edited in the desktop app only.

**The shared fixture**, `scripts/tests/fixtures/file-offer-v1.json`: one pass minted by
`build_file_pass` from the seeds `[7; 32]` (the sender) and `[9; 32]` (the receiver) on the server
of seed `[5; 32]`, at 1,760,000,000 and running out a week later: both keys, the server DID, the
id, the preimage text, the signature, the pass as text and as an object. Beside it, a 600,000-byte
file stated as a rule (byte i is i mod 251), its BLAKE3, its three pieces' BLAKE3 and `hashes`
(the BLAKE3 of the piece hashes, 32 raw bytes each, in order), and the exact texts of the offer
note (`[[hum:offer:v1]]` then compact JSON, keys in the spec's order, the name escaped as
`JSON.stringify` and serde_json both do), the answer for yes and for no, and a cancel.

**Decided here** (the spec left them open):
- A pass is refused as a string holding JSON; only the object counts.
- Credentials are refused with yourself as your own peer, so one person cannot use a pass to
  themselves to pipe data between two of their own devices through the server.
- The custom roles of an existing server get the forwarded day by tier, as the upload limits
  did: a custom role on the Admin tier gets 0 (no limit), any other 2048.
- The count includes what each side sends, each against their own tag (a receiver's requests
  and acknowledgements are small).
- The relay does not tell an app its forwarded day or what is left of it (no field in the
  profile or identify answers); a transfer that stops passing simply stalls. If the apps want to
  say "your daily amount through the server is used up", that is a follow-up.

**Tests**, each seen failing once against a single break (the doc comment on each names the break
and what it saw): `relay::core::pq_crypto::tests::file_pass_roundtrip_and_pinned_preimage` (the
server left out of the words), `a_file_pass_longer_than_a_week_or_malformed_is_refused` (the
week's ceiling removed), `file_pass_fixture_matches_the_builder` (the builder writing `expiry`
before `id`); `relay::handlers::reach::tests::file_passes::a_direct_connection_offer_with_a_file_pass_only_from_its_grantee`
(the pass checked with the sender as issuer); `relay::call_credentials::tests::file_credentials_only_with_a_matching_pass_both_ways_round`
(the id check left out) and `usernames_carry_a_meter_only_for_files` (a sixth part accepted);
`relay::call_forwarder::tests::the_day_count_stops_one_person_past_their_limit_and_never_counts_a_call`
(every allocation's meter left empty); `relay::storage::roles::tests::the_forwarded_day_round_trips_is_held_to_its_range_and_survives_an_older_apps_save`
(the keep step left out) and `an_existing_roles_table_gains_the_forwarded_limit` (the guarded
ALTER removed); and `gui::pages::server_settings::role_limits::tests::the_five_upload_limits_are_edited_and_saved_with_the_role`,
extended to the seventh column (its field drawn over another limit). The four older tests that
call `handle_webrtc_signal` pass `None` for the new argument.

## As built (desktop, device-to-device)

The desktop app's half of "Device-to-device big files: the shapes, exact" (parts 1, 3, 4, 5 and 6),
built 2026-10-11 in one worktree while the relay's half and the web's were built in parallel, then
brought into line with the choices the web half made (the coordinator's note of the same day): one
connection per pair of people, a data message at most 65,536 bytes header included, no automatic
decline. It was then brought to the rules settled by the first review and by the second (both
below). It is tested with stand-ins and the shared fixture; no desktop-to-web transfer has run
yet, and the relay's real `call_credentials` answer for a file has been read only as its shape
(the answer and its refusal echo `file`).

**The words, `src/net/file_offer.rs`.** The pass (`file_pass_preimage`, `mint_file_pass`,
`verify_file_pass`: Dilithium3 from the identity seed, at most 7 days, a day of slack when checked
and none when minted, the plot permits' pattern) and the three notes (`Offer::text`,
`OfferAnswer::text`, `OfferCancel::text`, `parse_note`), written with `serde_json` in the spec's
field order with no spaces. From the shared fixture's seeds this app makes the fixture's keys, its
pass byte for byte, its file hashes and its four note texts (`the_shared_fixture_is_what_this_app_builds`,
the fixture copied unchanged from main). An offer this app does not read: a piece size other than
262,144, a piece count that is not `ceil(size / piece)`, a digest that is not 64 lowercase hex, a
pass for another transfer, an empty file, or one over 256 GiB (`MAX_OFFER_BYTES`, a limit of this
app's own, so the list of piece hashes it holds stays under 32 MB). The receiver checks an offer's
pass on arrival (the server's did:hum, the sender, itself) and does not list one the relay would
refuse. `question` is part 3's order: the protected setup leaves a non-friend's offer out; then
"Files in messages" ("Ann wants to send you a file. You accept files only from friends. Accept this
one?"); only then "Ann wants to send you clip.mp4 (2.3 GB). Accept?". Nothing is sent for an offer
left unanswered (an automatic decline would tell the sender when the receiver came online): when
its pass runs out it ends as `Expired`, the sender's row saying "Ann did not answer in time". Each
transfer is an `OfferRecord` in the DM store (`file_offers`, per identity and server, encrypted with
the rest): the notes are read from the mailbox once, so the record is what survives a restart.
`apply_note` moves a record forward only (`advance`): an ended transfer never changes, an answer
counts only from the receiver while the offer waits, a cancel only from one of the two. Answers and
cancels also go to the person's own mailbox; a device that did not answer shows "Answered on your
other device".

**The channel, `src/net/file_transfer.rs`.** `Receiving` and `Sending` are plain state machines
over a `FileAt` trait (a file, or a `Vec<u8>` in the tests). A data message is at most 65,536 bytes
header included, so a 256 KiB piece is five messages; the receiver places each message by its own
header and takes any message that continues the piece at its next offset, up to 256 KiB (tested with
three splits). The receiver asks for at most 16 pieces at once, one `get` per run of consecutive
missing pieces, and keeps its window full by asking only for pieces it has not asked for yet
(after each piece in, usually `{"from":N,"count":1}`); a piece that fails its hash is asked for
again alone (`count` 1); 30 seconds with nothing it asked for arriving asks the window again; the
third failure of one piece ends the transfer with words and `cancel`. It ignores a message for a
piece it did not ask for or already holds, and holds a partial buffer only for a piece it asked
for, so at most 16. The sender's `get` adds to what it owes (a `BTreeSet`), sent lowest first,
each piece once, from offset 0; a piece asked for while it is being sent goes again, whole, after
it (rule 1 below). A pause from either side stops the flow both ways: the sender drops what it
owes, and a `get` that reaches it while paused; the receiver forgets what it asked when the
sender's pause reaches it, but after its own pause still takes what was on its way, and its resume
asks again from the first missing piece. After a drop the receiver keeps the list and asks
from the first missing piece; after a restart it asks for the list again and checks its part file
16 pieces per turn before asking for anything. `finish_part` reads the whole part file for its
BLAKE3 and names it only on a match, never over an existing file (the downloads' own
`claim_and_move`); on a mismatch nothing is named and the part file is deleted. The sender holds
back while the buffer is over 1 MB, but str0m caps a connection's send buffer at 128 KiB, so its
writes are refused first and retried on the next turn.

**The connection, `src/net/file_link.rs`.** One connection per pair of people, on its own thread,
with one data channel per transfer labelled `file:<id>`, opened by that transfer's receiver
whichever side dialled (str0m opens a channel on a live connection without a new offer); the
sender tells the transfers apart by the label. The receiver dials with the credentials of its
oldest accepted transfer from that person, and its `dc_offer` carries that transfer's pass; the
relay hands it on naming that transfer (`file`), and the dialled side asks for that transfer's
credentials alone (rule 7 below). Each side holds ONE allocation for the connection, on its own UDP
socket, and the forwarder joins the two in that transfer's scope; every other transfer between
them rides the same connection as its own channel. The TURN client is webrtc.rs's `mod turn`, as
it is; the app's WebRTC manager is not used, since it keys connections by person and holds one
call's allocation at a time. Relay only: the one local candidate is the relayed address, a
datagram not from the socket's own forwarder is never read, and a transmit goes out through the
allocation only when the relayed address is its source. The spec's direct connection between
friends who both turned it on is not built: the app has no such setting yet
(`webrtc::DIRECT_CONNECTIONS` is false), so every transfer goes through the forwarder.

**The app, `src/engine/file_transfer.rs`.** `offer` cleans the file on a worker when the attach
step's switch is on (`file_clean::clean_copy` into `offered-files/` beside the DM stores, never the
person's own file; the copy is deleted when the transfer ends), reads it for its hashes, and
`send_offer` mints the pass and sends the note with its self-copy. `screens_dm` takes the three
notes on the DM path after the "who can reach me" rule, so an offer from someone whose messages
this person does not take goes to Requests like any message from them; a parked server's notes go
into that server's records. A conversation with only an offer in it is listed in the DM sidebar.
Accept makes the part file (`<name>.<first 12 of the id>.part` in the downloads folder) at the
file's full size, so too little room says so at once. `pump` dials, per person, when a transfer
from them is accepted here and not paused, they are listed online and no connection stands;
transfers accepted or offered while one stands join it. `on_signal` (from `frame_ws_poll.rs`, before
the app's WebRTC manager) takes a `dc_offer` that names its transfer (`file`), a `dc_answer` or
`dc_ice` from someone a file connection stands with, a `dc_ice` from someone whose dial it is
answering but has no credentials for yet (kept for that person, `Pair::early_ice`, at most 32, and
handed to the connection in `Setup::early_ice`), and a `dc_ice` of a dial set aside when both
dialled (dropped); anything else, a `dc_offer` naming no transfer included, goes on to the manager.
If both dial at once the larger key keeps its dial and the other drops its attempt and answers. A
credentials answer (`read_credentials`, from `engine/call_relay.rs` `on_frame`) that is refused, or
none at all within 8 seconds, is "This server is not set up for that yet", tried again after a
minute or on Resume; a dropped connection is dialled again after 5 seconds. A failure on one side
is told to the other with the cancel note. The receiver's answer and cancel notes carry the offer's
pass on their `dm_put` to the sender as `file_pass` (the self-copies and the sender's own notes
carry none). A transfer with someone blocked ends at once with nothing sent to them.

**Brought into line with the rules settled by the first review (2026-10-12).** A review of this
half against the website's found them disagreeing at the seams; the four rules that touch the
desktop app are now kept as follows.
- Rule 1 (a `get` adds). Before: the sender appended a re-asked piece to its queue but skipped the
  piece it was sending, so after the website asked `get(next, 16)` for a bad piece the website
  waited for a piece that never came again (a stall). The receiver reserved 256 KiB for any piece
  number arriving at offset 0, asked for or not, so a sender naming pieces could make it reserve
  about the whole file and abort the app; and the stall clock was moved by any data message. Now
  as described under the channel above.
- Rule 2 (one engine per channel). Checked: each transfer is its own leg with its own `Receiving`
  or `Sending` on both sides, and one leg ending leaves the others as they are. Two gaps closed.
  The connection closed only once every leg had ended, so on the sending side an offer the other
  person had not accepted (a leg with no channel) kept it up; now it closes when its last channel
  ends (`last_channel_ended`: a receiving leg is a channel, a sending leg once it was opened). And a
  sending leg whose channel closes now forgets what it owed (only a receiving leg did). A finished
  transfer's channel is left open rather than closed, since the website as first built took a
  channel's close as the whole connection's.
- Rule 3 (early candidates kept). Before: a `dc_ice` that came while this side waited for its
  credentials to answer a dial found no connection, fell through to the call code and was lost, and
  the forwarder then dropped the dialler's packets for want of a permission toward its relayed
  address, so a website (which trickles its candidates after its offer) dialling a desktop sender
  might never connect. Now kept and applied once the allocations exist, as the offer's own
  `a=candidate` lines are.
- Rule 4 (answers reach a sender who takes messages only from friends). The receiver's answer and
  cancel notes carry `file_pass` on their `dm_put`. The relay half of this rule (letting such a put
  through the gate) was being built in parallel and is not part of this change.

**Brought into line with the rules settled by the second review (2026-10-12),** against the relay's
half of rule 7 (commit 397ff2750: a `dc_offer` let in by a file pass reaches its target with
`file` = the transfer id).
- Rule 7 (a dial names its transfer). Before: the dialled side asked credentials for its four
  newest live offers to the dialler and held one allocation per credential for the whole
  connection, each on its own socket, so idle offers used up the person's share, and a dial whose
  transfer was not among the four could not connect. Now `on_signal` reads `file`: naming one of
  this device's live offers to the dialler (Offered or Accepted, the file here), it asks for that
  transfer's credentials alone and answers with one allocation (`Pair::asked`, `Setup::relay`,
  the link's single `Alloc`); naming anything else, it is refused quietly (taken, nothing sent or
  shown, never handed to the call code: another of the person's devices may hold the file). A
  `dc_offer` naming no transfer is not a file dial: it goes to the WebRTC manager, which before
  this was handed a friend's direct-connection dial only when this device had no live offer to
  them. A dropped earlier connection with that person is let go (`let_go`) and its rows stop
  saying open.
- Rule 8 (an end is always said). The desktop's own failures were already said both ways (`cancel`
  on the channel from the protocol, the cancel note with the receiver's pass from `on_end`), and
  stay so. Before: a channel the other side closed only reset that leg, which then waited for the
  whole connection to drop, with the row still saying "Sending" or "Receiving" (`live.open` was
  never cleared) and any pause they held still showing. Now the engine hears `TransferClosed` at
  once (not open, their pause cleared), and the connection (`channel_closed`, `closed_channels`):
  a SENDING transfer waits 15 seconds (`STOPPED_WAIT`, the website's `FT_STOPPED_WAIT_MS`, the
  coordinator's ruling of the same day, after the website found that ending at once breaks
  resume: a receiver that restarts closes its channels one at a time while its connection still
  reads connected for a moment). If by then the channel was not opened again and the connection is
  still there, it ends "Ann's side stopped it" (`stop_due`, `Sending::stopped`, `stopped_line`; the
  cancel note follows from `on_end`); otherwise it was a drop, and the transfer waits on that
  connection until it goes, then for the next dial. "Still there" is decided here as anything
  from Ann a second or more after the close (`STOP_PROOF`, `they_stopped`; ICE checks the path
  every 3 seconds at most), not str0m's ICE state, which can read connected for a while after a
  peer has gone. A receiver that really stops says `cancel` on the channel (and sends the note),
  which ends the transfer at once; the wait only covers a stop without one. The connection stays
  up while a close is judged (the leg is still a channel). A RECEIVING transfer opens its channel
  again on the same connection after 2 seconds (`REOPEN_AFTER`), doubling each time it is closed
  again before a piece arrives, at most a minute (`REOPEN_MAX`, so a website sender that lost its
  file and closes each new channel is not hammered). Three more, decided here: a channel opened for
  a transfer that already has one replaces it (only its receiver opens one, and only after it saw
  the last one close, which str0m's own close never tells the other side); one opened for a
  transfer that already ended here, other than done, is answered with `cancel`; and a channel this
  side asks to open that str0m closes before it opens is asked for again at once, three times,
  then after a minute (`Link::opening`, `MAX_QUICK_REOPENS`). That last is a str0m matter: its own
  close never resets an SCTP stream, and after two seconds' cooldown it gives the same stream id
  out again, which the association still holds, so the open is refused; asked again while the
  failed id cools down, it gets another. (A browser's close does reset its stream, so this shows
  only between two copies of str0m, or a close str0m made itself.)
- Rule 9 (a dial is an answer). Before: a sender whose answer note was lost stayed Offered through
  the whole transfer ("Waiting for Ann to accept" with the file going), `done` changed nothing
  (`advance(Offered, Done)` was None and `on_end` returned before `ended()`), and the row later
  ended "Ann did not answer in time". Now the transfer's channel opening is Accept for a sender
  still Offered (`pump_link`, `TransferOpen`; it comes before any `hashes` or `get` on that
  channel), and `done` is Done from Offered too (`advance`), so "Sent".

**Fixed after a review of the merged code (2026-10-12).** Three ways an end could still go unsaid.
- A cancel or failure note that could not go was lost. `cancel` and `on_end` sent it and ignored
  the answer, and with the chat not connected nothing kept it; a receiver who never heard a cancel
  then dialled for that transfer until its pass ran out, up to 7 days, and the sender's other
  files to them waited behind it (a receiver dials with its oldest accepted transfer). Now a note
  that cannot go (the chat not connected, or not yet signed in with its mailbox read, or the note
  not yet sealable) is kept with the transfer's record (`OfferRecord::unsaid`, saved with the DM
  store, at most 8 a transfer, the website's `pendingNotes` and its number) and sent, in order and
  carrying what it would have carried, once the server is connected and signed in again
  (`flush_notes`, every frame; one that still cannot go is tried again every 30 seconds). A note
  to someone since blocked is dropped. Remove leaves a row whose note has not gone on the list,
  saying "clip.mp4 stays on the list until this server is connected again, so Ann hears that it
  ended.", since the record is what keeps the note. The backstop, the same as the website's: a dial
  naming an offer of this device's to the dialler that ended here as cancelled or failed (not
  done, declined or expired: no cancel note went for those) is still not answered, but the cancel
  note goes again, once in this run of the app for that transfer (`say_cancel_again`,
  `Transfers::said_again`).
- A channel the other side opened, on a connection that stands, for a transfer that had already
  ended here before the connection was given it was never answered. The connection holds only live
  transfers, so the channel waited for the engine (`LinkEvent::Wanted`), which ignored an ended
  record; the receiver had asked for `hashes` and its row read "Receiving" at 0 beside the
  transfer that was running. Now the engine answers such a `Wanted` (any end but done) with
  `Cmd::AddEnded`: the connection holds the transfer as a leg already ended (no file opened,
  nothing served), so `bind` writes `cancel` on that channel and on any opened for it again; and
  the cancel note goes again under the same once-a-run rule. A second channel opened for a
  transfer the connection is still asking about now closes the first (it was kept open and
  forgotten), and a channel closed while waiting is forgotten too.
- A receiver's `done` could be lost. When its last channel ended the connection waited at most 2
  seconds for what it wrote to be acknowledged, but SCTP waits a second before sending a lost
  message again and doubles that each time, so a `done` lost twice was never sent again before the
  connection closed, and a website sender then waited until the pass ran out and ended "expired"
  with the file delivered. Now it waits up to 15 seconds (`LINGER`, `may_close`), which ends at
  once in the usual case, when everything is acknowledged.

**The screens.** The attach step (`attach_step.rs` `route_pick`, `open_direct`) offers a DM file over
the person's private file limit instead of refusing it: the look and the switch as for any file, the
line "Files over 6 MB go straight to Ann's device instead of through the server.", that both apps
must be open, and why it cannot go now when it cannot. The rows (`chat/transfer_rows.rs`) sit at the
top of the DM, newest first, the five newest ended ones kept: name and size, the state words of part
6 (plus "Preparing...", "Paused", "Ann paused it", "Waiting for Ann's app to connect", "Ann did not
answer in time", "Answered on your other device"), a progress bar, Pause or Resume, Cancel, Try
again after "not set up", Remove once ended, and the line saying whose app must stay open.

**Tests**, each seen failing once against the break named in its doc comment:
`net::file_offer::tests` (8: the spec's words; the shared fixture; the pass checks only for its two
people, its way round, its file and its server; an offer that does not add up is not read; Files in
messages before the question; notes move a transfer forward only; a file read for its hashes; and
from the second review, `done` ending a transfer still Offered as Done),
`net::file_transfer::tests` (13, against stand-ins written from the spec's words: a whole file in
three splits, the hash-list check, piece checking with three failures, resume after a drop and
after a restart, pause and resume from either side, cancel from either side, the sender's 64 KiB
messages, the rename only after the whole file matches; and from the first review: a stand-in
sender that follows rule 1 exactly, with the receiver's asks reaching it several messages late,
gets the whole file across with no stall, each piece asked once plus once per bad copy, `count` 1;
the sender sends a piece re-asked mid-send again, whole, after it; a stand-in receiver with the
website's pattern, `get(next, 16)` after a bad piece, gets it again without a stall; pieces not
asked for are ignored and reserve nothing; only 30 seconds with nothing asked for arriving asks
again), `net::file_link::tests` (9: three files, two one way and one the other, on one connection
between two apps through the test forwarder, one relayed candidate each side, nothing sent around
the forwarder; the channel label; from the first review, the connection closing when its last
channel ends, an offer nobody opened not keeping it up; and from the second: through the test
forwarder, a channel the receiver closes without a word ending the sending transfer "Rae's side
stopped it" only after the 15 seconds, and a channel the sender closes opened again by the
receiver on the same connection two seconds later with the pause it held cleared and the file then
whole; on a connection with no thread, each branch of the sender's judgement (stopped: the wait
over and Rae heard since; drop: nothing heard since, nothing ends; opened again within the wait:
served; not yet: the wait not over); and the wait, the stop proof and the reopen wait's doubling
as rules. str0m's own close tells the other side nothing, so the network tests close a channel on
one side with `Cmd::CloseChannel` and report it on the other with `Cmd::TheyClosed`, both
test-only; and from the review of the merged code: on a connection with no thread, a channel
opened for a transfer that ended here before the connection had it answered `cancel` once the
engine hands it over, again when opened again, never served, and a second channel opened while
the engine is asked closing the first (seen in a test-only list of the channels this side
closed, since str0m's close tells the other side nothing); and the connection staying, once its
last channel ended, until what it wrote is acknowledged or 15 seconds), and
`engine::file_transfer::tests` (13, two apps with recording sockets: the offer as a sealed note
with its self-copy, listed and never shown; decline, and the other device's "Answered on your other
device"; an unanswered offer runs out with nothing sent; cancel from either side; dialling only
while the sender is online and not paused, with "not set up" and Pause; the larger key winning
when both dial, the set-aside dial's candidates taken and dropped; from the first review: the
receiver's answer, decline and cancel carrying the file pass on the put to the sender and nothing
else carrying one; a `dc_ice` that comes before the credentials kept in order and handed to the
connection; and from the second review: a dial answered with the credentials of the one transfer
it names out of five live offers, a dial naming anything else refused quietly and one naming none
left to the WebRTC manager; through a stand-in connection (`FileLink::stand_in`), a sender whose
answer notes were lost taking the channel opening as Accept and `done` as Sent, the pause cleared
with a closed channel, and "Rae's side stopped it" sending the cancel note; and end to end through
the test forwarder, five live offers, two accepted and one of those answers lost, a dial whose
candidate comes as a `dc_ice` after its offer and before the answering side's credentials, one
credential and one relayed candidate each side, and both files whole on the one connection, the
lost answer's transfer Sent; and from the review of the merged code: cancel and failure notes made
offline, from either side, kept with their records, still there in the DM store read back from
disk, sent once each when the chat is connected and its mailbox read and not before, carrying what
they would have carried, Remove keeping a row whose note waits; a dial naming an offer cancelled
here sending the cancel note again exactly once, a second dial nothing, one naming a sent offer
nothing; and, through a stand-in connection, a channel opened for a cancelled transfer handed to
the connection as ended and the note said again once, nothing for one that ended done).

**Not built yet.** Groups (file-transfer-at-scale.md, layer 3). The direct connection between
friends. Showing the forwarder's day amount (the relay counts it; the app shows only "not set up").
The one desktop-to-web transfer through a throwaway relay with its forwarder on loopback that the
spec asks for before release.
