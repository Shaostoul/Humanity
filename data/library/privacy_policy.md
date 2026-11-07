# Privacy: what this server keeps about you

*Last changed 7 November 2026. This page covers the server at united-humanity.us and its website.*

HumanityOS is free, open-source software. Anyone can run a server of their own, and each server's
operator decides for that server; this page is about the one at **united-humanity.us**, run by the
project's maintainer, who goes by **Shaostoul**: one private person in Washington State, USA, not a
company. There are no ads, nothing is sold, nobody is paid for your data, and there are no
analytics or tracking scripts.

What you read here was checked against the server's code and its settings on 7 November 2026, not
written from memory. Where the server does something we would rather it did not, this page says so
(see "What is not right yet").

## The short version

- **You do not sign up.** Your identity is a key made on your own device from your recovery phrase.
  We never ask for your name, email, phone number, birth date or a password.
- **The server keeps what you publish** (your chosen name, your profile, messages in rooms, files
  you share) so that others can see it, **until you erase it.**
- **Private messages are sealed** on your device before they leave it. The server holds them for at
  most 30 days and cannot open them.
- **Like every website, the server sees your internet address** (IP address) when you visit. The
  web server writes it in its logs, which are deleted after about 3 days.
- **No cookies.** The website sets none.
- **You can download what the server holds about you, and erase your account yourself**, at any
  time, without asking anyone.

## What is kept, why, and for how long

| What | Why | How long |
|---|---|---|
| **The web server's log** of each visit: your IP address, the time, the address you asked for, the page that sent you, and your browser's name | To keep the site working and to block attacks and abuse | Deleted after about 3 days |
| **The block list's log**: the addresses blocked for too many requests | To stop attacks | About 5 weeks; a block itself lasts 1 hour or 1 day |
| **The chat server's log**: your key and chosen name when you connect; an IP address only when it hits a limit | To find and fix faults and abuse | Kept until the log reaches its size limit (on 7 November 2026 it reached back about 3 weeks) |
| **Your key and chosen name, your role, when you joined** | So people know who wrote what, and so the server knows your role | Until you erase your account |
| **Your profile** (what you choose to fill in) | So others can see it; fields you mark for friends only are kept back from everyone else | Until you change it or erase your account |
| **Messages in rooms**, reactions | So the room can read them. Rooms open to anyone are public | Kept until you delete them or erase your account |
| **Private messages** | To deliver them when the other person comes online | Sealed; deleted after 30 days |
| **Files you upload** | To share them. Location and camera details are removed from pictures and videos, on your device and again on the server, unless you choose to keep them | Until you delete them or erase your account; files sent in private messages, 30 days |
| **Your settings sync** (notes, to-dos, garden, settings, sealed with a key from your recovery phrase) | So your settings follow you | Until you erase your account (but see "What is not right yet") |
| **Notifications** you turned on: your browser's push address | To send you a notification | Until you turn them off or your browser drops them |
| **Reports** you make or that name you | So moderators can act on them | 90 days after a decision |
| **Bans** | So a ban holds | Kept, even after an erase |
| **Bug reports** you send | To fix the problem | Kept (see "What is not right yet") |
| **Your progress in the shared game world** | So the world remembers what you built | Until you erase your account |
| **Backups** of the server's database (sealed) | To recover from a failure | Each copy for between about 7 hours and about 15 days; a copy an admin makes by hand, until it is deleted |

The server does **not** keep your IP address in its database, a list of who follows whom, a record
of which pages you read, or anything about you from other websites.

### Things we never ask for, and do not want

Health, body measurements, diet and fitness details stay on your own device: the desktop app keeps
them in memory, and the website keeps its private profile in your browser. None of it is sent to
the server. Please do not post health details in public rooms.

## Who can see it

- **Everyone:** what you publish in public: your chosen name, your profile (except friends-only
  fields), messages in rooms open to anyone, files you share publicly, and your public key.
- **Members of this server:** what is posted in members-only rooms.
- **The operator and the admins:** what the server holds, except what is sealed (private messages,
  private files, your settings sync, your vault backup). They use it only to run the server and
  keep it safe.
- **Linked servers:** HumanityOS servers can link up and share rooms. A server linked to this one
  (on 7 November 2026, public.guide, run by the same maintainer) gets copies of the public parts:
  messages in shared rooms, public profiles, key cards, bans, and our sealed backups. When you erase
  your account, linked servers are asked to remove your messages and profile.
- **Services your browser contacts for some pages:** every page asks GitHub
  (api.github.com) for the newest version number; the chat loads emoji pictures from
  cdn.jsdelivr.net; the wallet asks a Solana server and CoinGecko for balances and prices. Each of
  them sees your IP address when your browser asks. If you turn on notifications, your browser's
  maker (Google, Mozilla or Apple) carries them sealed: it sees when one arrives, not what it says.
- **Nobody else.** We do not sell, rent or trade anything about anyone, and we do not share it for
  advertising.

If the law requires it, the operator may have to hand over what the server holds. He cannot hand
over what he cannot read: sealed private messages and files.

## Your choices

- **See what is kept:** "Export my data" downloads everything the server holds about you as a file
  (website: Chat, then your account controls; desktop app: Settings, Account).
- **Change it:** edit your profile and delete your messages and files at any time.
- **Erase it:** "Erase account" removes your account and its data from this server, and asks
  linked servers to remove your messages and profile. It is permanent. The server remembers for 30
  days that an account was erased (a scrambled fingerprint, never your key), so a device of yours that was
  offline at the time cannot sign you up again by itself. Bans stay. Backups age out as described above.
- **Object, or ask a question:** message an admin in the chat (a private message is sealed end to
  end), or open an issue on [GitHub](https://github.com/Shaostoul/Humanity/issues) (issues there are
  public). We aim to answer within 30 days.
- **Complain to a regulator:** if you live in the European Union or the United Kingdom you may
  complain to your data protection authority (in the UK, the Information Commissioner's Office).

Nothing is required of you: you can read the site without giving us anything but what every
website sees, and you choose what to post.

Your choices in the app, once more in brief: rooms you post in, a profile you fill in or leave
empty, friends-only fields, notifications on or off, "Show others when I am online", and blocking anyone.

## Decisions made by machine

The server and the web server automatically slow down or block an address that sends too many
requests (for 1 hour or 1 day). Nothing else is decided about you by a machine, and nothing about
you is used to build a profile of your interests.

## Do Not Track

We do not follow you across other websites, so there is nothing for a browser's "Do Not Track"
signal to turn off: every visit is treated the same with or without it. No other party collects
information about you across websites through this site, apart from the services named under "Who
can see it", which see your IP address when your browser contacts them.

## Children

HumanityOS is for a general audience and is not meant for children under 13. If you are a parent
and believe your child has used it, message an admin and we will erase what the server holds about
them.

## Where it is

The server is in the United States. If you use it from another country, what you send it goes
there.

## What is not right yet

These are faults we found on 7 November 2026 while writing this page. Each one is being fixed, and
this list will shrink:

- The list of who is online (`/api/peers`) also shows people who turned off "Show others when I am online".
- Erasing an account leaves some things behind on the server: copies of messages that were pinned,
  comments on tasks, your settings sync and computer details, group and vote records, bug reports
  and reputation.
- Bug reports, including the browser name and window size sent with them, are public.
- Every page of the website asks GitHub for the newest version, so GitHub sees each visitor's IP
  address.
- A file attached in a members-only room can be opened by anyone who has its link.
- If your browser has no key to seal it with, the website's settings sync is sent unsealed.
- The names, member lists and posting times of groups are public (their messages stay sealed).

## Changes to this page

When what the server keeps changes, this page changes with it, its date at the top moves, and the
[devlog](https://united-humanity.us/devlog) says what changed. Every past version is kept in the
project's public history on GitHub.

---

*Laws behind this page, and what they ask: the project's dated research in
`docs/reference/findings/2026-11-07-privacy-notice.md`. The full list of what the server keeps, with
where in the code each part lives: `docs/design/privacy-notice.md`.*
