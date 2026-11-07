# The privacy notice, the 404 page and the share previews

Status: built 2026-11-07 (operator, 2026-11-07, on the website checklist's four gaps: "Let's get
that done."). This document is the EVIDENCE behind the privacy policy people read: what the server
keeps, read from the code and the live server's configuration, with file and line, so the next
person who changes what is kept can see which sentence of the policy to change with it.

- The policy people read: [`docs/outreach/privacy_policy.md`](../outreach/privacy_policy.md), copied
  into the Library ("Your Privacy" shelf) by `node scripts/build-library.js`, and shown on the
  website at `/privacy` (`web/pages/privacy.html` renders the same file, `data/library/privacy_policy.md`).
- The law behind what it must say: [`docs/reference/findings/2026-11-07-privacy-notice.md`](../reference/findings/2026-11-07-privacy-notice.md).

## 1. Where it lives, in both apps

Native first: the desktop app's Library carries the policy on its "Your Privacy" shelf beside "Who
Can Read Your Messages" and "How Your Privacy Is Built". The website mirrors it: `/privacy` fetches
the same markdown file and draws it with `web/shared/markdown.js`, the renderer the website's
Library uses, so there is one text to keep true. The website links it from every page's footer
(`web/shared/shell.js`, the shared footer) and lists it in `/sitemap.xml`
(`data/site/public_pages.json`). A visitor without JavaScript gets a link to the plain text file.

## 2. The rule for changing what is kept

Any change to what the server or the website keeps, sends, or for how long, changes
`docs/outreach/privacy_policy.md` in the same piece of work, and the inventory below. The policy's
"Last changed" date moves with it, and the devlog says what changed (that is how the policy tells
people about changes, CalOPPA 22575(b)(3)).

## 3. The inventory (read 2026-11-07, at commit 8f3fe914c and on united-humanity.us, read only)

### Logs on the server

| What | Where | Holds | Kept |
|---|---|---|---|
| nginx access log | `/var/log/nginx/access.log*` | The default "combined" format: IP address, time, the address asked for, status, size, referring page, browser name (no `log_format` anywhere in `/etc/nginx/`) | Rotated daily, 2 old files kept (`/etc/logrotate.d/nginx`: `daily`, `rotate 2`): about 3 days |
| nginx error log | `/var/log/nginx/error.log*` | Errors, with the IP address of the request | Same rotation, about 3 days; read by fail2ban |
| fail2ban | `/var/log/fail2ban.log*`, its database | The addresses it banned and why | Log rotated weekly, 4 kept (about 5 weeks); its database purges after 1 day; bans last 1 hour or 1 day (`/etc/fail2ban/jail.d/nginx.local`) |
| The relay's log (journald) | `/var/log/journal` | Keys and names on sign-in and disconnect (`src/relay/relay.rs:3109`, `:6231`; `src/relay/handlers/sign_ups.rs:112`); an IP address only when a limit is hit (`relay.rs:2899-2902`, `sign_ups.rs:62-67`) | No time limit; the disk guard vacuums it to 200 MB (`scripts/humanity-disk-guard.sh:77`); the oldest entry on 2026-11-07 was 2026-10-15 |

### The relay's database (`src/relay/storage/mod.rs`)

No table holds an IP address. Settings that drive expiry: `dm_mailbox_ttl_days` 30
(`src/relay/storage/server_settings.rs:207`), `message_retention_days` 0 = kept
(`server_settings.rs:313`); the expiry pass runs at start, every 6 hours and after a settings
change (`src/relay/expiry.rs:25-28`).

| Data | About the person | Who can see it | Kept | Erase removes it |
|---|---|---|---|---|
| Membership, names, roles, status | Key, chosen name, role, joined, last seen, hide-presence | Members (hidden people shown offline, `src/relay/broadcast.rs:256-266`); the public directory only if listed (`members.rs:30-33`) | Until erased | Yes (`src/relay/storage/account.rs:313-326`) |
| Profile, signed profile | What the person fills in; friends-only fields kept back (`profile.rs:20-27`) | Anyone, and copied to linked servers | Until changed or erased | Yes, and withdrawn from linked servers |
| Room messages, reactions | Key, name, text, time | Rooms open to anyone are public (`chat_access.rs:8-15`); others members only | Kept unless the server sets a limit | Yes, and linked servers are asked to remove them |
| Pinned copies | A copy of the author's key, name and text | As the room | Until unpinned (`expiry.rs:6-7`) | **No** (BUG-279) |
| Direct messages | Recipient key and a sealed blob; no sender (`handlers/dms.rs:12-27`) | Nobody; the server cannot open it | 30 days | Yes |
| Uploads | Key, file name, size; Files page title, description, licence, totals | Chat attachments: anyone with the link (`src/relay/mod.rs:1488`); members-only Files: members; DM and group files sealed by the sender's app | Chat: until deleted or the role's keep count; private sealed files: 30 days; unpublished Files: 1 day (`expiry.rs:12-17`) | Yes (`account.rs:233-260`) |
| Encrypted settings sync (`user_data`) | Notes, to-dos, garden, settings, sealed with a key from the recovery phrase (plain when no key, BUG-283) | The person | **Never deleted** | **No** (BUG-279) |
| Computer specs (`system_profiles`) | OS, CPU, GPU, memory, only when the person presses Save | The person | Until the person deletes it | **No** (BUG-279) |
| Vault backup | Sealed by the person's app | Nobody else | Until erased | Yes |
| Push subscriptions | The browser's push address and keys (`mod.rs:2364`) | The server | Until the push service says it is gone (`push_send.rs:83-86`) | Yes |
| Key cards | The public key others seal messages to | Anyone, and linked servers | Until erased | Yes here; linked servers keep theirs |
| Bans, mutes | Key, name, reason | Admins | Kept, on purpose (`account.rs:181-187`) | No, by design |
| Reports, copyright requests | Who reported whom, why; a claimant's name and contact | Admins | 90 days after the decision (`reports.rs:28`, `takedowns.rs:29`) | The reporter is blanked |
| Bug reports | Key, name, browser name and window size, page | **Anyone** (`GET /api/bugs`, `src/relay/api.rs:3558-3578`; BUG-280) | Kept | **No** (BUG-279) |
| Boards (tasks, comments, listings, trades, guilds, streams, skills, reputation) | Keys, names; listings a place and payment methods | Mostly public | Kept | Only tasks, listings and reviews the person made |
| Signed objects, P2P groups, votes | Author key; for groups the name, members, who posted when (text sealed) | **Anyone** (`GET /api/v2/objects`; BUG-284) and linked servers | Kept | **No** (BUG-279) |
| Game | Plots, built pieces, quests, progress | Other players | Until erased | Yes |
| Erase records | A keyed fingerprint (never the key) and a day | The server | 30 days (`erased_accounts.rs:76-77`) | Written by erase |
| Federation outbox | Copies of frames sent to linked servers | The server, linked servers | 10,000 rows or 7 days | The person's frames are scrubbed |

In memory only, never written: rate-limit buckets per address (`src/relay/ip_limit.rs`), connection
counts per address for 60 s (`relay.rs:2872-2897`), the keys made from one address for 1 hour
(`sign_ups.rs:26-61`), the Files page's view and download marks (a keyed hash under a key drawn
fresh each day, forgotten at midnight; only the totals are written, `files_counts.rs:4-20`), call
forwarding addresses for at most 12 hours (`src/relay/call_forwarder.rs:39-42`).

### Copies

| Copy | How often | Kept | Where | Sealed |
|---|---|---|---|---|
| Database backup | Every 30 minutes | 15 (about 7.5 hours) | The server (`scripts/humanity-backup-db.sh:128-134`) | Yes |
| The relay's own backup | Every 6 hours | 5 (about 30 hours) | The server | Yes |
| "Back up now" | When an admin presses it | 10, no age limit (`src/relay/backups.rs:24`) | The server | Yes |
| Continuous backup | Every 10 seconds | 72 hours by default | The linked server (public.guide) | Yes; that server sees only names, sizes, times |
| The operator's pull | Every 6 hours | 60 (about 15 days) | The operator's own computer | Yes, and he holds the key |

Erasure does not reach the copies; erased data leaves them as they rotate (about 15 days for the
pull, and never for an admin's manual copies until he deletes them). Uploaded files are not in any
copy (the scripts copy only the database).

### What the browser contacts besides this server

| Who | Pages | When |
|---|---|---|
| api.github.com | Every page that loads `web/shared/shell.js` | On load, after 3 s, at most once per 30 minutes (`shell.js` update checker; BUG-281) |
| cdn.jsdelivr.net | Chat | On load: emoji pictures |
| A Solana server, api.coingecko.com | Wallet | On load when the browser holds an identity |
| quote-api.jup.ag; arweave, nftstorage | Wallet | Only on a swap; only when the pictures tab is opened |
| The browser maker's push service | Any page where the person turned notifications on | Each push (sealed; it sees size and time) |

No cookies are set by the website, nginx or the relay. The browser keeps the person's own data in
its storage: the identity (`humanity_key_backup`, in plain form until a passphrase is set,
`web/chat/crypto.js:78`), the private profile (`humanity_profile_full_v1`), DM history in IndexedDB
(sealed), notes and settings.

### The desktop app

By itself it sends: the update check to `api.github.com` (user agent `HumanityOS-Updater`, no
identity, hourly, `src/updater.rs:115-140`) and whatever the person sends to the server(s) they
chose. No telemetry and no crash reports (none in `src/`). Off unless turned on: live weather from
NASA (`src/gui/mod.rs:4709-4712`), Epic game covers, the readable web. On the device only: the
identity vault, DM history, notes, calendar, saves. **No health or body data leaves the device**:
"Body & Measurements" lives only in memory (`src/gui/pages/profile.rs:142-147`); vitals and food in
the game are the game character's.

### Calls

STUN and a room-scoped forwarder run inside the relay (`src/relay/call_forwarder.rs:1-28`);
credentials are scoped to one call and last an hour (`call_credentials.rs:29-46`); the forwarder
relays sealed media and keeps addresses in memory only. The default call mode on both apps relays
through the person's own server; Google or Cloudflare are asked only when the person picks it
(`data/net/call_connection.json`).

## 4. Known gaps the policy names

The policy says these out loud, because it may say nothing the code does not back. Each is a bug
to fix; when one is fixed, the policy's line goes.

- BUG-278: `GET /api/peers` lists every connected person, including those who chose to hide.
- BUG-279: erasing an account leaves pinned copies, task comments, the settings sync blob, computer
  specs, group and vote records, bug reports and reputation behind.
- BUG-280: bug reports, including the reporter's browser name and window size, are public.
- BUG-281: every website page asks GitHub for the newest version.
- BUG-282: a chat attachment posted in a members-only room can be fetched by anyone with its link.
- BUG-283: the website's settings sync falls back to sending plain text when it has no key.
- BUG-284: group names, member lists and who posted when are public through `/api/v2/objects`.

## 5. The 404 page

`web/pages/404.html`: plain words for a newcomer ("This page is not here"), theme tokens only,
words by key, and a way on (the front door, the Library, How-to, Chat). nginx serves it for any
missing address (`error_page 404 /404.html;` in `scripts/nginx/humanity.conf`, the page `internal`)
and no longer names its version (`server_tokens off;` in every server block). nginx's own errors
from the relay's routes (`/api/`, `/uploads/`) pass through untouched: there is no
`proxy_intercept_errors`, so the apps keep reading the relay's JSON errors.

How the change reaches the live server: it does not, by itself. The deploy (`.github/workflows/deploy.yml`)
syncs the web root (`scripts/sync-web-root.sh`, which copies `404.html` with the other pages) but
never touches nginx; `scripts/provision-vps.sh` installs `humanity.conf` only when a server is first
set up. The live config is a hand-edited file (`/etc/nginx/sites-enabled/humanity` is a plain file,
not the usual link), so the operator applies the lines by hand (the commands are in BUG-277).

**The desktop app has no 404 page, and needs none:** it has no addresses to mistype. Its pages are
chosen from the menu, and a page that does not exist cannot be asked for. (Dual-UI parity: this is
the "no, and why" half.)

## 6. Descriptions and share previews

Every page people can reach carries a one-sentence description (what the page is FOR) and a share
preview (`og:title`, `og:description`, `og:image`, `og:url`, `og:type`, `twitter:card`), because
social sites and search engines read the page's HTML without running its scripts, so the tags
must be written into each page. One generator writes them: `node scripts/gen-page-meta.js` reads
`data/site/page_meta.json` (which pages, their address, their description key, a better picture if
a page has one) and `data/i18n/en.json` (the words), and rewrites the marked block in each page's
`<head>`. `scripts/tests/page-meta-web.test.js` fails when a page is missing from the list, when a
listed page lacks a title, a description or a share tag, or when a block differs from what the
generator would write. The desktop app has no equivalent: it is not crawled or shared by address.
