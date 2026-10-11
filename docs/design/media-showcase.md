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
(`docs/admin/`), which that layer can learn from.

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

**Routes** (all under the existing nginx `/api/upload` location, so no nginx change: a piece is far
under its 1025m body limit):

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
