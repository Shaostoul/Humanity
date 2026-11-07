# Who can read your private messages? HumanityOS and 13 other platforms, compared

*As of 6 November 2026.*

When you send a private message, two questions matter more than any promise in a privacy policy.
Who, besides the person you wrote to, can read it? And when a court or the police ask for it, what
can the company actually hand over? A company can only hand over what it can read, and only keep
what it chose to collect.

We read each platform's own published pages on 6 November 2026 and put the answers side by side,
with HumanityOS's in the same table, its limits included. Platforms change these things (Instagram
ended its end-to-end encrypted messages on 8 May 2026), so every line says "as of" that date, and
our research notes, linked at the end, give each source with its own date.

## Private messages: who can read them (as of 6 November 2026)

"End-to-end encrypted" means only the people in the conversation hold the key: the company in the
middle carries the message but cannot open it.

| Platform | End-to-end encrypted? | On by default, or switched on? | Who else can read the rest |
|---|---|---|---|
| HumanityOS | Yes: every direct message and every group chat | Always on: there is no unencrypted way to send one | Nobody else can read what they say. Channels on a server, even members-only ones, are not encrypted this way: whoever runs that server can read them |
| Mastodon | No | | The people who run your server and the other person's server |
| Bluesky | No (in 2024 it said encryption would come later) | | Bluesky, which can access them for trust and safety work |
| X | Only in X Chat | You choose X Chat; older direct messages are not encrypted | X, for older direct messages (X says it cannot read X Chat) |
| Facebook Messenger | Personal chats: yes | On by default, still being rolled out; not for chats with businesses or Pages | Meta, for chats with businesses and Pages. Group chats: Meta's own pages disagree |
| Instagram | No: it ended on 8 May 2026 | | Meta |
| Threads | Unclear: no Meta page we found says | | Unclear |
| YouTube | No | | YouTube, which may review messages against its rules |
| Reddit | It makes no claim of it | | Reddit, which collects private messages and chats |
| LinkedIn | It makes no claim of it | | LinkedIn, which scans messages for its own tools |
| Discord | Text: no. Voice and video calls: yes | Calls: always on, since 2 March 2026 | Discord, for text messages |
| Telegram | Only "Secret Chats" | You start one yourself: one to one, on one device | Ordinary chats are not end-to-end encrypted and are kept on Telegram's servers |
| Rumble | It has no private messages that we found | | |
| Patreon | No | | Patreon, which collects and moderates messages |

## When the law asks (as of 6 November 2026)

Each company's own published rules for requests from the police and courts, as we read them on 6
November 2026. Most describe United States legal process, and the kinds of legal paper they name
differ in how hard they are to get.

| Platform | To get what your messages say | To get your account details and network addresses |
|---|---|---|
| HumanityOS (united-humanity.us) | No published rules yet. Direct messages and group chats: the server holds only sealed envelopes it cannot open, and none older than 30 days unless its operator chose a longer time. Channel messages: readable | Each member's public key, display name and joining date, and whatever they chose to publish (a profile, posts, channel messages). No email address, no phone number, no friends list. Network addresses: only in the web server's logs, kept at most three days |
| X | A search warrant, for what X can read (it says it cannot read X Chat) | A subpoena, a court order or other valid legal process |
| Meta (Facebook, Messenger, Instagram, Threads) | A search warrant. For an end-to-end encrypted chat it says it cannot give what was said, but still gives logs and addresses | A subpoena for basic records, including a recent login address; a court order for message records, such as who wrote to whom, and addresses |
| Google (YouTube) | A search warrant | A subpoena for subscriber details and some network addresses; a court order for other records |
| LinkedIn | A search warrant; it says a civil subpoena or order is not enough | A subpoena for basic details and login addresses; a court order for who messaged whom, and when |
| Bluesky | A court order, for direct messages | Subpoenas, warrants and court orders (no breakdown published) |
| Discord | The legal process United States law requires (no breakdown published) | The same |
| Telegram | It says it has disclosed no user messages to anyone, governments included | A network address and phone number, on a valid judicial order about a suspect; it keeps such records up to 12 months |
| Mastodon | Whatever the law where each server is run requires (mastodon.social: German law) | The same; mastodon.social keeps your latest login address up to 12 months |
| Reddit | Not read: its page for law enforcement would not let our research tools read it | Not read |
| Rumble | A subpoena, court order, government request or other legal process (no breakdown published) | The same |
| Patreon | Subpoenas, court orders and warrants (no breakdown published) | The same |

## What the tables say, in plain words

Messages that only the people in them can read are still rare. Of these thirteen platforms, three
offer it for private messages: X in X Chat, Messenger in personal chats (by default, still rolling
out), and Telegram in Secret Chats, which you switch on yourself. Discord encrypts every voice and
video call that way, and that cannot be turned off. Each of those is real work worth credit.

Several platforms are also straightforward about their limits, which matters as much. Mastodon's own
documentation says plainly that it is not an encrypted messenger, and names apps that are.
LinkedIn says a civil subpoena is not enough to get what your messages say. Bluesky says it gives
direct messages to others only when a court orders it. Telegram says it has disclosed no user
messages to anyone, governments included. X, Meta and Discord say they tell you when someone asks
for your data, unless the law forbids it.

Everywhere else, the platform keeps a copy of your private messages that it can read, and the
platforms that publish rules for the police (X, Meta, Google, LinkedIn and Bluesky) say they hand
message content over on a search warrant or a court order. That is not a scandal; it is what
keeping a readable copy means. The only way to make sure a copy cannot be handed over is never to
hold one that can be read.

## What HumanityOS does differently

HumanityOS is free, open-source software. There is no company in the middle: anyone can run a
server, and each server answers to the law where it runs. What a server can hand over is limited by
what the software lets it hold:

- **Every direct message and group chat is end-to-end encrypted, always.** The server refuses to
  store a direct message that is not in the sealed format, so an old or faulty app cannot quietly
  send one in the clear.
- **The server does not write down who sent a direct message.** It keeps each sealed envelope
  addressed only to its recipient, with the day it arrived and no sender, and deletes it after 30
  days by default (you can clear yours sooner). Your history is kept on your own devices, encrypted.
- **No friends list on the server.** A friendship is a signed pass that your own device keeps and
  shows when you send a message; the server checks it and keeps no list of who is friends with whom.
- **No sign-up.** No email address, no phone number, no password stored on any server. Your account
  is a key made on your own device, backed up by a 24-word recovery phrase that you keep.
- **Hidden until you choose.** New members are not shown as online and are not listed in the member
  directory until they choose to be.
- **Your photos keep your secrets.** Photos and videos lose their hidden location and camera details
  on your own device before they leave, unless you turn that off, and files sent in private chats are
  encrypted on your device first.
- **Your message lengths stay private.** Messages are padded to a few fixed sizes before they are
  sealed, so a one-word reply and a long paragraph look alike.
- **Your data, your buttons.** You can download everything a server holds about you, or erase your
  account yourself, and no server operator can switch either button off. Server backups are
  encrypted.

## What HumanityOS cannot protect you from

A privacy page that hides its own limits is just marketing, so here are ours.

- **Your network address while you are connected.** Any server can see the address of a device
  connected to it; that is how the internet works. HumanityOS does not keep addresses in its
  database, and the web server's logs keep them at most three days. Server operators can add a Tor
  "onion" address, which hides yours from the server when you connect through Tor, but
  united-humanity.us does not have one yet (checked 6 November 2026), and that door does not yet
  serve the website.
- **The moment you send.** The server you are connected to knows, at that moment, that it is you
  sending and whom the sealed envelope is for. It does not write that down, but a server changed to
  record it could, from then on.
- **Group chats show who is talking.** What is said in a group is sealed, but the server that carries
  a group can see its name, who is in it, and who posted when.
- **The other person.** Whoever you write to can screenshot, forward or report your messages, and
  because every message carries your signature, they can prove you wrote it.
- **Your own device.** Anyone holding your unlocked device, or with harmful software on it, can read
  what you can read. Protect your device and put a passphrase on your key.
- **The first time you meet someone.** The app trusts a person's key the first time it sees it and
  warns you if it ever changes, but a way to compare keys in person is not built yet.
- **Notifications in a web browser.** If you turn them on, your browser maker's notification service
  sees when a notice arrives and how big it is, never the words.
- **Public is public.** A post in a public channel is public, like anywhere else, and once it has been
  copied to other servers or devices, nobody can delete every copy.

## What is still coming

- **A plain badge on every message** saying who else can read it (its words are being tested on
  people new to the app first), once the Inbox brings messages from other platforms beside your own,
  with your copies kept on your device and a one-tap choice to keep them, keep them for a while, or
  let them go.
- **Every link showing the site it really goes to**, with look-alike names flagged and a "Where does
  this go?" check that follows a shortened link without opening it.
- **An app lock**, a gentle check that your computer's disk is encrypted, and **disappearing
  messages** (permanent by default, a timer always one tap away).
- **Fixed on 6 November 2026:** the server could be made to send requests into its own network
  through a notification address someone gave it. Now every address someone else hands the server is
  checked at the moment it connects, and only the public internet is allowed.
- **Fixed on 7 November 2026:** the order in which the server stored sealed envelopes could pair a
  message with the sender's own copy of it. The server on united-humanity.us now keeps them by
  recipient, not in the order they arrive, so a copy of its database no longer pairs them, and the
  website sends your own copy a random 20 seconds to 2 minutes after the one for the person you
  wrote to. The desktop app does the same from its next update; until then, the server's log of
  recent changes, and any backup made from it, still holds the desktop app's two copies one after
  the other. Even after, on a server so quiet that nothing else is written in those minutes, that
  log could hint at a pair. Other servers get the fix when they update. Envelopes stored in the old
  order are deleted when a server updates, fetched or not, instead of waiting out the 30 days.

## Check it yourself

Everything above is open source, so none of it has to be taken on trust: the code, the database
layout and our research notes are public.

- The project: united-humanity.us
- The code: github.com/Shaostoul/Humanity
- Our research notes on the 13 platforms, with every source and its date:
  github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-11-06-social-platform-apis.md

*Published 7 November 2026. Every comparison is as of 6 November 2026.*
