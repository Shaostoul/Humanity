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
  role's limit. Built with increment 2.

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
