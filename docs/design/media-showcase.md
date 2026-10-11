# Media showcase (design, 2026-10-10)

**Status: proposed.** The operator asked for "a media page" that "can showcase all the images,
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

## Open questions for the operator

1. Categories: the server's (admins set them; recommended) or anyone may add one?
2. Should Media items be visible to people who are not signed in to the server (the public website
   showcase), or only to its members? **Recommended:** visible to anyone, since the point is to
   show the work; the uploader chooses per item, with "members only" available.
3. Big files: videos follow the uploader's role limit (Server Settings > Roles). Should Media have
   its own, higher limit for trusted roles, and a total size per person?
