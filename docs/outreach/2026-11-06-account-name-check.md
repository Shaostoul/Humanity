# One name on every platform, and the profile text: checked 6 November 2026

**What this is.** A dated record for choosing the name of HumanityOS's own social accounts, and the
text for their profiles. The operator, 6 November 2026: "We'll have to figure out a name we can
consistently use across all platforms." And on the profile line "Drafted with AI help (Claude),
checked by Shaostoul": "I think that's fair. How do we mention HumanityOS and the website?" The
design these accounts serve is [`docs/design/outbox-and-inbox.md`](../design/outbox-and-inbox.md)
(sections 14 and 17). Names get taken and rules change, so every line here is as of the day it was
checked; check again on the day of signing up.

Nothing here created an account, signed in anywhere, or filled in any form. None of the project's
accounts is to post anything yet: the operator, the same day, "Let's get more of the security stuff finished before you
start posting." (the gate in the design's section 14). Reserving a name is not posting.

## The recommendation

**Use `JoinHumanityOS` as the handle everywhere, with the display name "HumanityOS".**

- It is free on every platform we could check: Mastodon (both servers checked), Bluesky, X,
  YouTube, Telegram, Rumble, Patreon and TikTok, 8 of the 14. Instagram, Threads, Facebook, Reddit
  and LinkedIn would not let an automated reader tell a taken name from a free one, and Discord
  usernames have no public address.
- It is 14 letters and nothing else, so it passes every platform's rules: X allows at most 15
  characters, and only plain letters and digits are accepted everywhere (hyphens, underscores and
  dots are each refused somewhere; see the rules table below).
- It is an invitation, which suits a cooperative; Mastodon's own site, joinmastodon.org, is named
  the same way. One honest catch: "join" beside "No sign-up" in the profile could read as a
  contradiction to someone new. It means joining the people, not making an account, and the profile
  says "no sign-up" plainly.
- The display name carries the plain name "HumanityOS", which is what people search for. (Display
  names are not unique, so it costs nothing that others use it.)

**On Bluesky, use the website itself as the handle: `@united-humanity.us`.** Nobody but whoever runs
the domain can hold it, so the handle proves who it is, and it is not set up by anyone yet
(checked below). It breaks the "same everywhere" rule on one platform for a stronger thing: the name
IS the address. If the operator wants strict sameness instead, `joinhumanityos.bsky.social` is free
too. Setting up the domain handle is a DNS record (`_atproto.united-humanity.us`, a TXT record whose
value is `did=` and the account's identifier) or a file at
`https://united-humanity.us/.well-known/atproto-did`, both from the AT Protocol's handle
specification (atproto.com/specs/handle, read 6 November 2026); the operator's step, at sign-up.

**Runner-up: `HumanityOSApp`**, free on 7 of the 8 platforms we could check and closer to the plain
name. On TikTok, `@humanityosapp` exists with the display name "HumanityOS", no posts and no bio.
**If that account is the operator's**, HumanityOSApp is clean everywhere we could check, and the
choice between the two is taste.

**Why not plain `HumanityOS`:** other accounts already hold it on X, YouTube, Telegram and TikTok.
**Why not `UnitedHumanityOS`:** 16 characters, one more than X allows. **Why not `UnitedHumanity`:**
taken on six platforms. **Why not `HumanityOSorg`:** free wherever checked, but humanityos.org is a
parked domain someone else holds (and humanityos.com is offered for sale), so the handle would point
people at a site that is not ours.

**Next, when he chooses:** reserve the name on each platform soon, because names do get taken (the
plain "HumanityOS" already is, on four). The platforms we could not read (Instagram, Threads,
Facebook, Reddit and LinkedIn, and Discord's usernames) say at sign-up whether a name is free.

## The names, platform by platform (checked 6 November 2026, about 20:10 to 20:45 Pacific)

"Free" means the platform's own public address for that name showed nobody holding it; "taken"
means it showed someone. "Could not check" means the platform did not let an automated reader tell the two
apart; nothing is guessed. Lengths in characters.

| Platform (address read) | HumanityOS (10) | HumanityOSApp (13) | JoinHumanityOS (14) | HumanityOSHQ (12) | UnitedHumanityOS (16) | UnitedHumanity (14) |
|---|---|---|---|---|---|---|
| Mastodon, mastodon.social | free | free | free | free | free | free |
| Mastodon, mastodon.online | free | free | free | free | free | free |
| Bluesky, name.bsky.social | free | free | free | free | free | taken |
| X | taken | free | free | free | too long for X (15 at most) | taken |
| Instagram | could not check | could not check | could not check | could not check | could not check | could not check |
| Threads | could not check | could not check | could not check | could not check | could not check | could not check |
| Facebook | could not check | could not check | could not check | could not check | could not check | could not check |
| YouTube | taken | free | free | free | free | taken |
| Reddit (user and community) | could not check | could not check | could not check | could not check | could not check | could not check |
| LinkedIn (company page) | could not check | could not check | could not check | could not check | could not check | could not check |
| Discord | see note 1 | see note 1 | see note 1 | see note 1 | see note 1 | see note 1 |
| Telegram | taken | free | free | free | free | taken |
| Rumble (channel and user) | free | free | free | free | free | free |
| Patreon | free | free | free | free | free | taken |
| TikTok | taken | taken (note 2) | free | free | free | taken |

**The Bluesky domain handle `@united-humanity.us`:** not set up by anyone (Bluesky's public
`resolveHandle` answered "Unable to resolve handle"); only the domain's holder can claim it.

1. **Discord** usernames have no public address, so they could not be checked. Its server addresses
   (`discord.gg/<name>`, a "vanity URL") are in use for none of the six names, but a vanity URL needs
   a server at Discord's highest boost level (Discord's help page refused automated reading; this is
   a search engine's summary of it), so it is not free to claim. An ordinary invite link works for
   any server.
2. **TikTok `@humanityosapp`**: an account with the display name "HumanityOS", no posts and no bio.
   Whose it is, we cannot tell; if it is the operator's, HumanityOSApp is free everywhere we could
   check.

Who holds the plain name, briefly: on X, `@HumanityOS` is an account named "Rethinking Humanity";
on YouTube, `@humanityos` is a channel named "Humanos"; on TikTok, `@humanityos` is another account
also named "HumanityOS". None of them looks like this project's.

### How each was checked

- **Only public addresses were read**, one plain request each, from the operator's PC, with no
  sign-in and no cookies. Mastodon through each server's public account lookup
  (`/api/v1/accounts/lookup?acct=<name>`: an account, or "Record not found"); Bluesky through its
  public `com.atproto.identity.resolveHandle`; every other platform through its profile address
  (`x.com/<name>`, `youtube.com/@<name>`, `t.me/<name>`, `rumble.com/c/<name>` and
  `rumble.com/user/<name>`, `patreon.com/<name>`, `tiktok.com/@<name>`, `discord.com/invite/<name>`,
  `instagram.com/<name>/`, `threads.com/@<name>`, `facebook.com/<name>`,
  `linkedin.com/company/<name>`, `reddit.com/user/<name>/` and `reddit.com/r/<name>/`).
- **Every platform had two controls**: a name known to be taken (NASA, Gargron on both Mastodon
  servers, jay.bsky.team, durov, russellbrand on Rumble, kurzgesagt on Patreon, minecraft on
  Discord) and a made-up name (`zzqxjho9481q`). A platform's answers count only when the two controls
  came back different; where they came back the same, the platform is "could not check".
- **What told the two apart:** Mastodon, a 200 answer or a 404; Bluesky, an identifier or "Unable
  to resolve handle"; X, YouTube, Rumble and Patreon, the profile page or a "not found" page (404);
  Telegram, the page's name line (its `og:title`), which shows the channel's or person's own name
  for a name someone holds and the generic "Telegram: Contact @name" for one nobody does; TikTok, the account's
  details in the page or TikTok's "user not found" code; Discord, the server's name or Discord's
  generic invite page.
- **Where the platform would not say:** Instagram, Threads and Facebook gave the same page for the
  taken control, the made-up name and every candidate. Reddit answered every request with a "Prove
  your humanity" check and LinkedIn with a Cloudflare check; we did not try to get around either.
- **The identification sent:** a first pass sent a desktop browser's identification; every result
  above comes from a second pass that sent the request tool's own identification (curl), and the two
  passes agreed.
- **The limits of "free":** a free answer means nobody shows at that address today. A platform
  may still hold back a name (a reserved word, a recently deleted account), which only its sign-up
  form will say.

## Each platform's own rules for a name

Read from each platform's own pages on 6 November 2026. Where the page refused automated reading,
it says so and gives what we could find instead, marked; nothing is from memory.

| Platform | Length | Characters | Source, and whether it could be read |
|---|---|---|---|
| Mastodon (mastodon.social) | up to 30 | letters, digits, underscore | Mastodon's source, `app/models/account.rb` (`USERNAME_LENGTH_LIMIT = 30`, local names `[a-z0-9_]`), read; docs.joinmastodon.org/methods/accounts/ gives the characters, read |
| Bluesky, free handle | 3 to 18 before `.bsky.social` | letters, digits, hyphen (not first or last); no dot, no underscore | Bluesky's server code (`packages/pds/src/handle/index.ts`) and the handle specification (atproto.com/specs/handle), read; Bluesky's help article refused automated reading |
| Bluesky, domain handle | up to 253 overall, 1 to 63 a part | as above; stored in lowercase | atproto.com/specs/handle, read. A search summary of Bluesky's help desk mentions a 30-character cap on domain handles that neither the code nor the specification has; unconfirmed, and `united-humanity.us` (18) fits either way |
| X | at most 15 | letters, digits, underscore | X's developer docs (docs.x.com, data dictionary: "Typically a maximum of 15 characters"), read; help.x.com's username rules refused automated reading (a search summary of it: 15 at most, the same characters) |
| Instagram | not stated on Meta's pages | periods, numbers and underscores are suggested additions to a taken name | facebook.com/help/instagram/182492381886913 and related, read; no length found. (Not the platform's page: 30 characters at most, letters, numbers, periods, underscores) |
| Threads | the Instagram name | as Instagram | facebook.com/help/instagram/1207185479989246, read: a Threads profile made with an Instagram account shares its username |
| Facebook (a Page's username) | at least 5 | letters, digits, periods; capitals and periods do not make a name different | facebook.com/help/105399436216001, read |
| YouTube (handle) | 3 to 30 | letters, digits, underscore, hyphen, period, middle dot (those four not first or last); not case-sensitive | support.google.com/youtube/answer/11585688, read |
| Reddit | user 3 to 20; community 3 to 21 | user: letters, digits, underscore, hyphen; community: letters, digits, underscore | Reddit's help page refused automated reading; from Reddit's own archived source code (about 2017, may be out of date), read |
| LinkedIn (company page address) | not stated | lowercase letters, digits, hyphens; at least one non-digit | linkedin.com/help/linkedin/answer/100826, read |
| Discord | username 2 to 32 | lowercase letters, digits, underscore, period | Discord's help page refused automated reading; a search summary of it; docs.discord.com (read) still describes the older system |
| Telegram | 5 to 32 | a to z, 0 to 9, underscore; not case-sensitive, shown as written | core.telegram.org/method/account.updateUsername and telegram.org/faq, read |
| Rumble | at least 3 (6 or more recommended, under 20 suggested) | letters and digits only; cannot be changed later | rumble.support/help/usernames-requirements, read |
| Patreon (page address) | not found | not found | Patreon's help pages refused automated reading; no rule found anywhere |
| TikTok | not read | not read | TikTok's help page needs a script to show anything. (Not the platform's page: 24 at most, letters, digits, underscore, period) |

**What this means for a name:** at most 15 characters (X), at least 5 (Facebook, Telegram), and only
plain letters and digits, which every platform accepts. Capitals help reading (`JoinHumanityOS`),
but LinkedIn and Discord show only lowercase, and Bluesky stores handles in lowercase.

## The profile text

Each one names HumanityOS and says what it is first (for a stranger who has never heard of it), gives
the website, and carries the operator's line, "Drafted with AI help (Claude), checked by
Shaostoul", whole. Counted with Node's `Intl.Segmenter` (graphemes, which is how Bluesky and
Mastodon count) and as plain characters; every character in them is a single grapheme.

**X (160 at most): 155 characters** (160 if X were to count the address as 23, as it does in posts;
it fits either way):

> HumanityOS: free, open-source tools for food, water, power and helping each other. united-humanity.us · Drafted with AI help (Claude), checked by Shaostoul

**Bluesky (256 graphemes at most): 234:**

> HumanityOS: free, open-source tools and guides for food, water, power and helping each other, plus a space game that teaches the same skills. No ads, no sign-up. united-humanity.us · Drafted with AI help (Claude), checked by Shaostoul

**Mastodon (500 at most, an address counted as 23): 385 graphemes as written, 382 as Mastodon counts
it:**

> HumanityOS is a free app and website with guides and tools for growing food, keeping water safe, making power and helping each other, plus a space game that teaches the same skills.
>
> No sign-up, no ads, nothing to buy. Open source, built in the open since 2019.
>
> Website: https://united-humanity.us
>
> Posts by the HumanityOS project. Drafted with AI help (Claude), checked by Shaostoul.

Mastodon also has up to four labelled fields under a profile: put "Website" there too, with
`https://united-humanity.us`. It shows a green check when the website links back to the profile
with `rel="me"`, a later one-line change to the website once the account exists.

**Everywhere else (a short bio): 146 characters**, for the platforms with short profile lines; the
Mastodon text fits the long "About" sections (YouTube, Facebook, LinkedIn, Patreon, Rumble):

> HumanityOS: free, open-source guides and tools for food, water and power. united-humanity.us · Drafted with AI help (Claude), checked by Shaostoul

The other platforms' own limits for a profile line were not read for this record; check each one's
box at sign-up (it counts for you), and cut from the front half, never the AI line.

### What every word rests on

- **"free", "no ads", "no sign-up", "nothing to buy"**: the website's own front door
  (`front_door.facts` in `data/i18n/en.json`: "No sign-up, no ads, nothing to buy.").
- **"guides and tools for growing food, keeping water safe, making power and helping each other,
  plus a space game that teaches the same skills"**: `front_door.what` in the same file.
- **"app and website"**: the front door sends people to the desktop app (`front_door.dest_game_hint`)
  and every tool is on both (`front_door.dest_tools_hint`).
- **"open source", "built in the open since 2019"**: `front_door.facts`, and the repository's
  [`LICENSE`](../../LICENSE): the code and the documents (the guides included) are CC0 1.0, given to
  the public domain, which is more open than most open-source licences. Not everything the app
  carries is ours to give away: a few real-world datasets keep their own terms (OpenStreetMap's map
  regions under ODbL 1.0, two star catalogues under CC BY-SA, some planet textures under CC BY 4.0,
  and the Minor Planet Center's asteroid orbits under that centre's own conditions;
  [`LICENSES.md`](../../LICENSES.md)). So the text says "open-source tools and guides", never
  "everything is public domain".
- **The limits**: X's 160 is the figure given for this work (X's help pages refused automated
  reading); Bluesky's 256 graphemes is its profile record's `description` field, `maxGraphemes:
  256` (`lexicons/app/bsky/actor/profile.json` in Bluesky's `bsky` repository, read 6 November
  2026); Mastodon's 500 is `NOTE_LENGTH_LIMIT = 500` in `app/models/account.rb`, counted in
  graphemes with each address counted as a fixed length (`app/validators/note_length_validator.rb`,
  read the same day).

## Open for the operator

1. **The name.** Recommendation: `JoinHumanityOS` everywhere, display name "HumanityOS", and
   `@united-humanity.us` on Bluesky.
2. **Is the TikTok account `@humanityosapp` (display name "HumanityOS", no posts) yours?** If it is,
   `HumanityOSApp` is equally clean and the choice is taste.
3. **New project accounts, or your own @Shaostoul** (the design's question 3, not yet answered in
   words): the name question reads as new accounts, so the recommendation stands: new accounts, your
   own staying your voice.
