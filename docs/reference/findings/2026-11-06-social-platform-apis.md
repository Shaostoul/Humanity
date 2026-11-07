# Posting to other platforms from the app, reading replies and messages there, and labelling AI-written posts

**Research date: 6 November 2026.** Every source below was read on that date by the author or by
one of eight research agents working for this finding. Each source carries its own published or
last-updated date beside it, which is often older, sometimes much older. If you are reading this
later, compare those dates with whatever has changed since; three of these platforms changed
something important in the last six weeks alone (Reddit, YouTube, Instagram).

**I am not a lawyer. This is a reading of public sources, not legal advice.** Nobody qualified has
reviewed it. Terms of service are contracts, the laws below are the texts as written, and how a
court or a platform's enforcement team reads either is not something this document can promise.

It extends [`2026-09-25-permission-routes-and-launchers.md`](2026-09-25-permission-routes-and-launchers.md),
which read Discord's Terms on 25 September 2026: their dates have not changed since ("Effective:
September 29, 2025", "Last Updated: August 29, 2025"). The design built on this finding is
[`docs/design/outbox-and-inbox.md`](../../design/outbox-and-inbox.md).

## The question

Can a free, open-source desktop app, holding each person's own sign-in on their own device and
never routing anything through the project's servers, post for that person to their accounts on
Mastodon, Bluesky, X, Threads, Facebook, Instagram, YouTube, Reddit, LinkedIn, Discord, Telegram,
Rumble and Patreon, read their replies, mentions and direct messages there and answer them, and let
an AI helper draft what they send; what do those platforms' rules and the laws on AI-written posts
require of it; and for each platform, can the platform itself read a person's messages and be made
to hand them over?

Eight sub-questions were asked of every platform: (a) posting for a person, at what cost and after
what approval; (b) reading replies, mentions and DMs and sending DMs; (c) rules on automated and
AI-run accounts; (d) rules on keeping and showing fetched content; (e) how an app gets access, and
whether an open-source desktop app can hold its own client secret; (f) the practical numbers (post
length, media, rate limits, whether there is a stream the app can open itself); (g) whether direct
messages are end-to-end encrypted; (h) what the platform's own law-enforcement guidelines say it
hands over, and under what legal process. Rumble, Patreon, (g) and (h) were added at the
operator's request during the research (6 November 2026).

## The short answer

**Two platforms fit the design completely: Mastodon and Bluesky.** Both let any app post, read
mentions and replies, and read and send DMs, free, with no review, through a sign-in a public
open-source desktop app can do by itself (Mastodon registers the app on each server at run time;
Bluesky has no client secret at all). Mastodon also offers a WebSocket the app opens itself for
new notifications and DMs. Neither encrypts DMs end to end.

**Four can be reached only partly, or only with the person bringing something of their own:**

- **X** allows everything (posting, mentions, DMs, even its encrypted X Chat) through a sign-in a
  desktop app can do, but every call is paid by the developer app's owner: $0.015 a post, **$0.20
  a post with a link**, $0.005 for each post read. Self-serve apps may reply only when summoned,
  and AI reply bots need X's prior written approval.
- **LinkedIn** lets an app post to a person's own profile (an open permission) but forbids using
  the API "to automate posting" (so no scheduling), closes reading comments and DMs, and needs
  LinkedIn to switch on its secret-free sign-in for each app.
- **Discord** forbids any app acting as a person's account; posting into a channel works with a
  webhook or a bot the person or group creates, and Discord's terms forbid shipping even an
  Application ID in open-source code.
- **Telegram** lets a full client act as the person (reading and sending their DMs) or a bot post
  to channels, but the client's credentials must not be public, and its API terms forbid using
  Telegram data for the "deployment" of AI, which reaches an AI helper that drafts replies.

**Three are closed to this design by their holders' rules or prices:** **Threads** (a Business-type
app after App Review and Business Verification, "consumer app" use "strictly prohibited", and a
token exchange that needs the app secret, which Meta forbids in a client), **Facebook** (no app may
post to a personal profile since 2018; Pages need App Review, Business Verification and the app
secret) and **Instagram** (professional accounts only, the same review and secret, media fetched
from a public address).

**Four offer nothing usable, or will not by spring:** **Reddit** stopped accepting new API access on
31 October 2026 and ends public API access in March 2027; **YouTube** has no API for community posts
or messages at all (video uploads and comments only, from one shared daily quota, and its policies
forbid API credentials in open-source projects); **Rumble** has no posting API and forbids automated
access without written permission (its only API is a read-only feed of a creator's own live chat);
**Patreon** cannot create posts through its API and has no API for comments or messages.

**Messages that only the two people can read are rare.** Of these thirteen, DMs are end-to-end
encrypted only in X Chat (optional; X's older DMs are not), in Telegram's Secret Chats (optional,
one to one, tied to one device) and in Messenger's personal chats (by default, still rolling out;
not chats with a business or a Page). Everywhere else the platform keeps a copy it can read, and
every platform that publishes law-enforcement guidelines says it hands message content over on a
search warrant (X, Meta, Google, LinkedIn) or a court order (Bluesky). HumanityOS's own sealed DMs
are the one kind in this design that no server can read ([`cryptography.md`](../cryptography.md),
"DM E2EE").

**The laws on AI-written posts do not require the label the design puts on AI-drafted posts, but
the EU's has teeth the design must meet.** The EU AI Act's Article 50 has applied since **2 August
2026**. Its duty to disclose AI-generated text published to inform the public on matters of public
interest is waived when a person reviews the text and someone holds editorial responsibility, but
the Commission's guidelines (20 July 2026) say a cursory approval does not count, fact-checking is
the minimum, and any AI change after approval voids the waiver. Whether the project is itself a
"provider" of an AI system (whose output must then be machine-marked, with no review exception) is
the biggest open question. California's bot law (Business and Professions Code 17940 to 17943)
reaches only accounts where substantially all posts are not a person's, used with intent to mislead
for a sale or a vote, and disclosure is a complete defence. The US Federal Trade Commission has no
rule requiring AI-written posts to be labelled; its 2024 rule bans fake reviews, AI-written ones
included. Utah's law reaches consumer transactions when someone asks; nothing in Washington State's
law reaches text posts.

### The platforms in one table

"Desktop sign-in" means: can a public, open-source desktop app get a person's token by itself,
with nothing secret in its code and no server of ours in the middle.

| Platform | Post as the person | Replies and mentions | DMs | Cost and approval | Desktop sign-in | DMs end-to-end encrypted |
|---|---|---|---|---|---|---|
| Mastodon | yes | yes, with a stream | read and send | free, no review | yes (registers itself per server) | no |
| Bluesky | yes (a post-only permission exists) | yes, polled | read and send (separate permission) | free, no review | yes (public client) | no |
| X | yes | yes | read and send; X Chat too | paid per call by the app's owner | yes (public client) | X Chat only, optional |
| Threads | Business apps only, after review | yes, after review | no API | free, review and business verification | no (needs the app secret) | not reachable |
| Facebook | Pages only, after review; never a personal profile | Page comments and tags | Page inbox, 24-hour window | free, review and business verification | no (needs the app secret) | personal Messenger chats yes; Page chats no |
| Instagram | professional accounts only, after review | yes | professional inbox, 24-hour window | free, review and business verification | no (needs the app secret) | no, since 8 May 2026 |
| YouTube | video uploads only | comments only | no API | free, one shared daily quota | Google says yes; YouTube's policy forbids credentials in open source | no |
| Reddit | no new access since 31 Oct 2026 | (as left) | (as left) | closed; public API ends March 2027 | n/a | no claim of it |
| LinkedIn | yes, to the person's own profile; no automated posting | closed | partners only | no price stated; needs a LinkedIn Page | only if LinkedIn switches it on | no claim of it |
| Discord | never as the person; a webhook or bot into a channel | a bot's own channels | not the person's | no fee stated | no shared credentials in open source | text no; calls yes |
| Telegram | yes (full client) or a bot in channels | yes (full client) | yes (full client) | free | credentials must not be public | Secret Chats only |
| Rumble | no API | a creator's own live chat, read only | no | written permission for anything else | a personal secret address | no DM feature found |
| Patreon | no (posts read only) | no | no | free | the creator's own token | no |

### Legal process in one table

| Platform | Message content handed over with | Records and addresses handed over with |
|---|---|---|
| X | a search warrant, for what X can read (it says it cannot read X Chat) | subpoena, court order or other valid process |
| Meta (Facebook, Messenger, Instagram, Threads) | a search warrant; not for end-to-end encrypted chats (logs and addresses still given) | subpoena (basic records, recent login address); 2703(d) court order (message headers, addresses) |
| Google (YouTube) | a search warrant ("Private message content" listed for YouTube) | subpoena (subscriber details, some addresses); court order (other records) |
| LinkedIn | a search warrant; civil process never | subpoena (basic details, login addresses); court order (who messaged whom, when) |
| Bluesky | a court order | subpoenas, warrants, court orders, emergency requests (no breakdown) |
| Discord | "the appropriate type of legal process" under the US statutes (not broken down) | the same |
| Telegram | says it has disclosed "0 bytes" of messages | address and phone number on a valid judicial order about a suspect |
| Mastodon | whatever the server operator's law requires (mastodon.social: German law) | the same; mastodon.social keeps the latest login address up to 12 months |
| Reddit | not read (its guidelines refuse automated reading) | not read |
| Rumble | "subpoena, court order, government request, or other legal process" (no breakdown) | the same |
| Patreon | subpoenas, court orders and warrants served on its agent (no breakdown) | the same |

## How this was researched, and how exact the quotations are

Primary sources only: each platform's own developer documentation, developer terms and policies,
terms of service, help and privacy pages, law-enforcement pages and pricing pages; for the laws,
the official texts (EUR-Lex, the Official Journal, eCFR, the Federal Register, govinfo, the
California Legislature's own data, Utah's and Washington's legislatures). No blog, news article or
third-party summary was used for any claim. Where a platform's own newsroom or blog is cited (Meta,
Discord, Bluesky, Telegram's founder's channel), it is marked as such.

**Two ways of reading, two kinds of exactness.** Most pages were downloaded whole and their text
compared character by character with the quotation (Mastodon's documentation and its server's own
API, Bluesky's documentation and its lexicons on GitHub, X's documentation in its raw Markdown,
Meta's Threads and Instagram documentation in its `.md` copies, Google's pages, Reddit's help
articles through its own help-centre API, Patreon's and Rumble's pages, Telegram's privacy policy
and FAQ, EUR-Lex, eCFR, the Federal Register and California's bulk legislative data). Some pages
could only be read through a summarising fetch tool that returns quotations in pieces of at most
125 characters (linkedin.com, Discord's developer policies, several Meta help pages, some Facebook
developer pages). Those quotations are pieces of the page joined in order; they are marked
**(pieces)** below and should be re-read in a browser before anyone relies on their exact
punctuation. LinkedIn's law-enforcement PDF has no text layer and was transcribed from images of
its pages.

**A check of the agents' work.** Fifty of the quotations that carry the most weight were checked
by the author against the raw downloads on the same day: 41 were found verbatim; the other nine
came from pages with no raw copy (LinkedIn's API Terms and its native sign-in page, Discord's
Developer Terms and Policy, Instagram's encryption page, Bluesky's 2025 transparency report,
Telegram's API Terms, Discord's safety page, LinkedIn's Community Management overview). Eight of
those nine were re-read by the author through the fetch tool that day and matched; Discord's safety
page was not (its point is confirmed by Discord's own blog and help article, DC-9). For Telegram's
section 1.5 the fetch tool returned only the sentence's first half (TG-4).

**Quotations are short on purpose.** The platforms' pages are copyrighted, so each quotation is the
few words that carry the meaning, with the rest described in plain words and the page linked. The
laws are quoted more fully: statutes, US federal rules and EU legislation are free to reuse.

Dates: "(no date shown)" means the page carries no date of its own; where a documentation page is
built from a public repository, the date its source file last changed is given instead, marked
"(source changed ...)". For docs.x.com, which shows no dates, the page's own metadata
`dateModified` is given, marked "(metadata ...)".

---

## Findings: Mastodon, and the fediverse through ActivityPub

Mastodon is software anyone can run; each server sets its own rules. mastodon.social, run by
Mastodon GmbH under German law, is quoted where a server's rules matter.

**(a) Posting**

- **MA-1. Posting is one call with a person's token holding the `write:statuses` scope.**
  https://docs.joinmastodon.org/methods/statuses/ (Last updated July 29, 2026): "Publish a status
  with the given parameters." The same page offers an `Idempotency-Key` header so a post sent twice
  is kept once.
- **MA-2. An app must let a person sign in on any server, registering itself there.**
  https://docs.joinmastodon.org/api/guidelines/ (Last updated November 23, 2025): "you must ask for
  the server’s domain and use the app registrations API". Registration
  (https://docs.joinmastodon.org/methods/apps/, Last updated May 1, 2026) is marked "OAuth: Public":
  no review, no approval.
- **MA-3. It is free.** mastodon.social's terms, read through its own API at
  https://mastodon.social/api/v1/instance/terms_of_service (effective 2026-08-31): "You can use the
  Service without paying us anything." No developer pricing page exists.

**(b) Replies, mentions and DMs**

- **MA-4. Mentions arrive as notifications** (scope `read:notifications`).
  https://docs.joinmastodon.org/methods/notifications/ (Last updated June 11, 2026): "Someone
  mentioned you in their status". A reply that mentions the person arrives the same way (the
  researcher's inference, not stated on the page).
- **MA-5. A DM is a post with "direct" visibility; DMs are read as conversations.**
  https://docs.joinmastodon.org/entities/Status/ (Last updated May 22, 2026): "Visible only to
  mentioned users." https://docs.joinmastodon.org/methods/conversations/ (Last updated May 1, 2026):
  "Direct conversations with other participants."

**(c) Automation and AI**

- **MA-6. The bot flag is optional and only a mark.** https://docs.joinmastodon.org/user/profile/
  (Last updated April 15, 2026): "treats the bot flag as a visual indication only."
- **MA-7. mastodon.social requires AI use to be disclosed.** https://mastodon.social/api/v1/instance/rules
  (no date shown), one rule's text: "use of generative AI must be disclosed"; its hint: "Accounts
  may not solely post AI-generated content." Other servers set their own rules.
- **MA-8. Every post names the app that posted it.** https://docs.joinmastodon.org/client/token/
  (Last updated October 10, 2024): the app's name and website "will be shown on statuses if
  applicable."
- No ban on automation in the API documentation was found; the rules are each server's.

**(d) Keeping and showing fetched content**

- **MA-9. No retention, caching or deletion rule for clients was found** in the API documentation or
  mastodon.social's terms (a negative finding). Deletion does not reliably reach other servers
  (terms): "does not guarantee that it will be deleted from other servers". The stream announces
  deletions (https://docs.joinmastodon.org/methods/streaming/, Last updated May 1, 2026): "A status
  has been deleted."
- **MA-10. Show where a person is from.** API guidelines (November 23, 2025): "It should be possible
  to see that a given user is from another server".

**(e) Access**

- **MA-11. Every registration gets a client secret, so each installation registers itself and keeps
  its own; nothing secret is in the source code.** https://docs.joinmastodon.org/methods/apps/
  (May 1, 2026): "Mastodon only supports provisioning confidential clients" and "Treat the client_id
  and client_secret properties as if they are passwords." The live sign-in metadata of mastodon.social
  (https://mastodon.social/.well-known/oauth-authorization-server, read 2026-11-06) lists only
  secret-based token authentication.
- **MA-12. PKCE from Mastodon 4.3, S256 only; an out-of-band redirect suits a desktop app.**
  https://docs.joinmastodon.org/spec/oauth/ (Last updated October 10, 2024): "supported by Mastodon
  4.3.0 and above"; https://docs.joinmastodon.org/methods/oauth/ (Last updated October 19, 2026):
  "Must be S256"; apps page: "use urn:ietf:wg:oauth:2.0:oob in this parameter".
- **MA-13. Ask for the fewest scopes.** https://docs.joinmastodon.org/api/oauth-scopes/ (Last updated
  June 11, 2026): "request the most limited scopes as possible".

**(f) Numbers**

- **MA-14.** Default post length (https://docs.joinmastodon.org/user/posting/, Last updated July 8,
  2026): "The default character limit is 500 characters." Each server reports its own in
  `/api/v2/instance` (`configuration.statuses.max_characters`). mastodon.social's live values
  (2026-11-06): 500 characters, 4 attachments, URLs counted as 23, images to 16 MiB, video to about
  99 MiB.
- **MA-15. Default rate limits** (https://docs.joinmastodon.org/api/rate-limits/, Last updated
  November 20, 2022): "All endpoints and methods can be called 300 times within 5 minutes." Media
  uploads 30 in 30 minutes; each server can change them.
- **MA-16. A stream the app opens itself.** Streaming page (May 1, 2026): "Open a multiplexed
  WebSocket connection to receive events." The `user` stream carries notifications; the `direct`
  stream carries DMs. (Web Push, by contrast, needs a server address.)

**(g) Encryption**

- **MA-17. DMs are not end-to-end encrypted; both servers' administrators can read them.** Posting
  page (July 8, 2026): "Mastodon is not an encrypted messaging app like Signal or Matrix". mastodon.social's
  privacy policy (https://mastodon.social/api/v1/instance/privacy_policy, updated 2022-10-07): "the
  operators of the server and any receiving server may view such messages".

**(h) Legal process**

- **MA-18. Each server's operator answers under its own law.** mastodon.social's help centre
  (https://help.joinmastodon.org/article/20-moderation-decisions-and-appeals, last updated November
  5, 2026): it responds to authorities "disclosing only what the law requires" (pieces). Its privacy
  policy: "The latest IP address used is stored for up to 12 months." No US-process breakdown, no
  emergency or preservation policy, no government-request numbers published.

**ActivityPub.** https://www.w3.org/TR/activitypub/ (W3C Recommendation, 23 January 2018) defines
"A server to server federation protocol" and "A client to server protocol", and says an
implementation can do either or both. How widely the client-to-server half is implemented: unclear,
no primary page says; Mastodon's clients use its own REST API.

## Findings: Bluesky (the AT Protocol)

docs.bsky.app now redirects to https://bsky.network/docs/ (guides) and https://endpoints.bsky.app/
(the reference, read through its OpenAPI files); lexicons live in github.com/bluesky-social/bsky.

**(a) Posting**

- **BS-1. A post is a record in the person's repository.**
  https://bsky.network/docs/bluesky-api/creating-a-post (no date shown; source changed 2026-08-12):
  "Bluesky posts are repository records".
- **BS-2. Apps with a sign-in should use OAuth; app passwords are for bots and command-line tools.**
  https://bsky.network/docs/oauth-client (source changed 2026-07-30): "Applications with their own
  end-user login flow should implement OAuth."
- **BS-3. A post-only permission set exists.** https://atproto.com/guides/permission-requests (source
  changed 2026-06-09): "the much more narrowly scoped app.bsky.authCreatePosts permission set"; its
  lexicon (changed 2026-08-12): "Can not update or delete posts."
- **BS-4. Free, no review: registration is automatic.** https://atproto.com/specs/oauth (source changed
  2026-10-08): "server discovery and client registration are automated". https://bsky.network/: "Free
  and hostable."
- **BS-5. Shared accounts: unclear.** https://bsky.social/about/support/tos (Last Updated: 14 August,
  2025): "Do not share your Account with others", while a person may use it on behalf of an
  organisation they represent.

**(b) Replies, mentions and DMs**

- **BS-6. Replies, mentions and quotes arrive as notifications, polled.** Lexicon
  `app.bsky.notification.listNotifications` (changed 2026-09-21): "Enumerate notifications for the
  requesting account."
- **BS-7. DMs live on a central chat service and need their own permission.**
  https://bsky.network/docs/api-directory (source changed 2026-07-14): "the chat.bsky.* centralized
  chat/DM APIs". The OAuth spec: the general scope gives "no access to DMs"; `transition:chat.bsky`
  adds them, and a granular set `chat.bsky.authFullChatClient` covers listing, reading and sending.
  Reading is a cursor log (`getLog`), polled; no stream for DMs was found.

**(c) Automation and AI**

- **BS-8. Automated interactions that notify someone count as abuse.**
  https://bsky.network/docs/developer-guidelines (source changed 2026-08-12), in its list of what is
  not allowed: "Generating automated or bulk interactions, including any that would cause a
  notification". This rules out automatic replies and DMs.
- **BS-9. A bot label is a best practice, not a rule.** https://atproto.com/guides/bot-tutorial
  (source changed 2026-04-13): "bot accounts should identify themselves by adding a self-label".
- **BS-10. No rule on labelling AI-written content was found** in the Community Guidelines
  (https://bsky.social/about/support/community-guidelines, Last Updated: September 19, 2025) or the
  terms (a negative finding).

**(d) Keeping and showing**

- **BS-11. Apps must delete what a person asks, and apps with social features must offer reporting
  and blocking.** Developer Guidelines: services "must have a method for deleting content a user has
  requested to be deleted"; also "A method to report illegal content" and "A method for blocking
  abusive users from the app". No retention or caching limit was found.

**(e) Access**

- **BS-12. No client secret exists; the app publishes a small file describing itself on the public
  web.** OAuth spec: "There is no client_secret shared between servers and clients." and "Clients
  must publish a "client metadata" JSON file on the public web." https://atproto.com/guides/about-oauth
  (source changed 2026-04-16): a client on the user's own device "cannot protect a client secret."
- **BS-13. PKCE, PAR and DPoP are required of every client.** OAuth spec: "PKCE and PAR are required
  for all client types"; DPoP "is mandatory for all clients".
- **BS-14. A desktop app returns from the browser by a custom URI scheme or an HTTPS address on the
  same site as its description file.** OAuth spec: "the redirect_uri may use a custom URI scheme".
  A loopback address appears only under the exception for development; whether a released app may
  use it: unclear.
- **BS-15. Sessions of a public client last at most two weeks, and tokens stay on one device.** OAuth
  spec: lifetimes "should both be limited to 2 weeks"; "Tokens must not be shared or reused across
  client devices."

**(f) Numbers**

- **BS-16.** Post text 300 graphemes (lexicon `app.bsky.feed.post`, `maxGraphemes: 300`); DM text
  1,000 graphemes; up to 4 images of 2 MB each; video to 300 MB. Writes
  (https://bsky.network/docs/rate-limits): "5,000 points per hour and 35,000 points per day", about
  1,666 new records an hour; the same page allows 3,000 API requests per 5 minutes per network
  address. Jetstream (https://bsky.network/docs/jetstream) is a public WebSocket of
  the whole network needing no sign-in, but it filters by author, not by who is mentioned, so new
  replies are polled.

**(g) Encryption**

- **BS-17. DMs are not end-to-end encrypted.** https://bsky.social/about/support/privacy-policy (Last
  Updated: August 14, 2025): "These are unencrypted and can be accessed for Trust & Safety
  purposes." The DM launch post (https://bsky.social/about/blog/05-22-2024-direct-messages, 22 May
  2024) promised end-to-end encryption "down the line".

**(h) Legal process**

- **BS-18. DMs go to third parties only on a court order.** 2025 transparency report
  (https://bsky.social/about/blog/01-29-2026-transparency-report-2025, 29 January 2026): "do not
  provide DMs to third parties unless it's ordered by a court." Requests come through a portal; no
  breakdown by kind of process; the privacy policy allows disclosure for "legal process, like a court
  order or subpoena".

## Findings: X

docs.x.com was read in its raw Markdown; developer.x.com is a script-drawn shell; help.x.com
refused the fetch tool (403) and two of its pages were read by a plain download.

**(a) Posting**

- **X-1. Every call is paid, in advance, by the developer app's owner; there is no free tier.**
  https://docs.x.com/x-api/getting-started/pricing (metadata 2026-10-16): "pay-per-usage pricing. No
  subscriptions". The table: "Post: Create | $0.015 per request", "Post: Create (with URL) | $0.200
  per request", a summoned reply $0.010.
- **X-2. The free tier ended on 6 February 2026.** https://docs.x.com/changelog (entry "Feb 6,
  2026"): "Public Utility Apps continue to receive free scaled access." A "Public Utility" credit
  programme exists for public-good apps listed on X's Exhibit (https://docs.x.com/exhibit, no date
  shown): "you cannot mark your own listing as a utility."
- **X-3. Pay-per-use is for small apps; a widely used one must move to Enterprise.**
  https://docs.x.com/developer-terms/policy (metadata 2026-10-29): designed for "supporting
  applications with limited end-users". Buying credits needs "A verified phone number"
  (https://docs.x.com/developer-terms/ppu-agreement).
- **X-4. Self-serve apps may reply only when summoned.**
  https://docs.x.com/x-api/posts/manage-tweets/integrate (metadata 2026-09-02): replies are "only
  permitted if the replying account has been explicitly summoned". Changelog, 16 April 2026:
  quote-posts, likes and follows "removed from all self-serve tiers".
- **X-5. Show exactly what will be posted first.** Policy: "Show exactly what will be published".
  Each post carries the app's name as its source label.

**(b) Replies, mentions and DMs**

- **X-6. Mentions: the 800 most recent, at $0.005 each read.** Timelines page: "Up to 800 most recent
  mentions". The $0.001 "Owned Reads" price applies only when the person signed in is the owner of
  the developer app (pricing page).
- **X-7. The older DMs: read and send, 30 days back, paid.** "DM Event: Read | $0.010 per resource",
  "DM Interaction: Create | $0.015 per request"; https://docs.x.com/x-api/direct-messages/lookup/introduction:
  "Events from up to 30 days ago are available".
- **X-8. Storing DMs needs the person's express consent** (policy, in its list of what needs it).

**(c) Automation and AI**

- **X-9. Automation means actions without a person performing them.**
  https://help.x.com/en/rules-and-policies/x-automation ("Updated April 2026"): "take repeated actions
  without a person actively performing them". Signing in is not consent to automation: it "does not
  by itself constitute sufficient consent to take automated actions".
- **X-10. AI reply bots need X's prior written approval.** Same page: "operation of any AI reply bot
  requires prior written and explicit approval". https://docs.x.com/developer-guidelines: "Requires
  prior approval from X". Whether a draft a person approves counts: unclear.
- **X-11. No automated bulk DMs; no near-duplicate posts across accounts.** Automation page: no
  "duplicative or substantially similar posts on one account or over multiple accounts".
- **X-12. A bot account must say what it is and who runs it.** Policy: "clearly indicate what the
  account is and who is responsible for it". An "Automated" profile label exists.
- **X-13. No API field labels a post as AI-made** (the docs were searched; the only disclosure field
  marks a paid partnership). An AI label can only be in the text.
- **X-14. No training on X's content.** https://docs.x.com/developer-terms/agreement ("Last Updated:
  April 27, 2026"): no use to "fine-tune or train a foundation or frontier model".

**(d) Keeping and showing**

- **X-15. Stored content follows deletions and edits on X, within 24 hours of a request.** Policy:
  "delete or modify any content you have if it is deleted or modified on X".
- **X-16. Content fetched with one person's sign-in is not shown to someone not allowed to see it.**
  Policy: "may not serve DM content to people who are not authorized to view".
- **X-17. Display rules.** https://docs.x.com/developer-terms/display-requirements (metadata
  2026-10-23): the author's picture, @username and display name shown and linked; the text "may not
  be altered or modified."
- **X-18. A risk for a unified inbox.** Agreement: no use "to create a substitute or similar service"
  to X's own apps. Whether an inbox that shows X replies beside others' is one: unclear (the
  researcher's reading).

**(e) Access**

- **X-19. A desktop app is a public "Native App" using PKCE, with no secret.**
  https://docs.x.com/fundamentals/developer-apps (metadata 2026-07-25): "Mobile or desktop apps that
  can't secure secrets" and "Public clients use PKCE only." Tokens last two hours; `offline.access`
  adds a refresh token. Redirect addresses must match exactly; `http://127.0.0.1` is named for local
  development; whether a released desktop app may use it: unclear.
- **X-20. One app per use case; never ask for passwords or others' keys.** Policy: "You may not
  register multiple applications for a single use case". Whether each person registering their own
  developer app is allowed: unclear. A mode called "API Connect", where "the request bills the user's
  own account", is mentioned once (https://docs.x.com/x-api/bots/create-a-bot) and documented nowhere
  else.

**(f) Numbers**

- **X-21.** 280 weighted characters, a link counting 23 (https://docs.x.com/fundamentals/counting-characters);
  up to 4 photos, or one GIF, or one video; images 5 MB. Rate limits
  (https://docs.x.com/x-api/fundamentals/rate-limits): posting 100 per 15 minutes per person; mentions
  300 per 15 minutes per person; DM reads 15 per 15 minutes. The real-time activity stream needs the
  app's own secret bearer token and allows two connections per app, so a public desktop app polls.

**(g) Encryption**

- **X-22. X Chat is end-to-end encrypted; the older DMs are not.** https://docs.x.com/xchat/introduction
  (metadata 2026-07-31): "X routes ciphertext and cannot read plaintext content." The cryptography
  primer: "Encryption hides the what, not the who or when." Activity events: "Legacy (unencrypted) DM
  events." X Chat has its own API. Whether a message falls back to unencrypted when one side has not
  joined X Chat: search snippet only (the help page refused).

**(h) Legal process**

- **X-23.** https://help.x.com/en/rules-and-policies/x-law-enforcement-support (metadata
  2026-09-28): non-public information only with "a subpoena, court order, other valid legal process"
  or a valid emergency request; contents of communications "require a valid search warrant or
  equivalent"; a 90-day preservation; users notified unless prohibited. The page says nothing about
  X Chat.

## Findings: Threads (Meta)

Read through Meta's Markdown copies at `developers.facebook.com/documentation/threads/*.md`; Threads'
own terms could not be read (a script shell).

- **TH-1 (a). Posting for a person is allowed, shown back only to that person.**
  https://developers.facebook.com/documentation/threads/overview (no date shown): display posts
  "solely to the person who created it".
- **TH-2 (a). Only testers until App Review approves the app; Advanced Access needs Business
  Verification.** Create-replies page: until approval, "you can only post on behalf of Threads
  testers"; access levels page: Business Verification "is required to get Advanced Access."
- **TH-3 (a). Consumer apps are forbidden.** https://developers.facebook.com/documentation/development/permissions
  (no date shown): "using threads_content_publish  permission in a consumer app is strictly
  prohibited." The Business app type is for apps that help "businesses, creators, and
  organizations manage their presence". Whether a person's own Outbox qualifies: unclear.
- **TH-4 (a). Pictures and video are fetched by Meta from a public address.** Get started: "the media
  must be hosted on a publicly accessible server". No upload endpoint exists.
- **TH-5 (b). Replies to the person's own posts and public mentions can be read, after review; there
  is no DM API.** Mentions page: after approval "the API returns other users' public posts". The
  reference index lists no messaging endpoint.
- **TH-6 (c). An AI label can be set when posting.** https://developers.facebook.com/documentation/threads/posts:
  "set `is_made_with_ai=true` when creating the media container"; leaving it out "does not prevent
  automatic AI labeling."
- **TH-7 (c, d). Meta's Developer Policies and Platform Terms.** https://developers.facebook.com/devpolicy/
  (Last updated February 3, 2026): "Obtain consent from people before publishing content". Platform
  Terms (https://developers.facebook.com/terms/, Last updated February 3, 2026): "Update or delete
  Platform Data promptly after receiving a request from us or the User".
- **TH-8 (e). The token exchange needs the app secret, which must never be in a client.** Access
  tokens page: `client_secret` "Required."; long-lived tokens page: requests "should only be made in
  server-side code, never in client-side code". No PKCE is documented. A person may make their own
  Meta app, where Standard Access covers people with a role on it, but would then hold a secret Meta
  says not to store on a device.
- **TH-9 (f).** Overview: "Text posts are limited to 500 characters."; 250 posts and 1,000 replies a
  day per profile; images 8 MB; video 5 minutes and 1 GB. Webhooks only, needing App Review and a
  verified business; otherwise polling.
- **TH-10 (g).** Threads has had DMs since 1 July 2025 (Meta newsroom,
  https://about.fb.com/news/2025/07/introducing-messaging-highlighted-perspectives-threads/); no Meta
  page found says whether they are end-to-end encrypted. Unclear; and not reachable by API.

## Findings: Facebook (personal profiles and Pages)

- **FB-1 (a). No app can post to a person's own profile.**
  https://developers.facebook.com/docs/graph-api/changelog/non-versioned-changes/apr-24-2018 (24 April
  2018): "The `publish_actions` permission has been removed." Meta's 2018 announcement
  (https://developers.facebook.com/blog/post/2018/07/31/platform-update-publish-permission/, updated
  1 August 2019) points developers to "Facebook's Share dialogs". The Share dialog's documented
  parameters include a link but no post text (from the fetch tool's description of
  https://developers.facebook.com/docs/sharing/reference/share-dialog; no verbatim quotation was
  returned).
- **FB-2 (a). Posting to a Page works as the Page, after App Review for most uses.**
  https://developers.facebook.com/docs/graph-api/reference/page/feed/ (no date shown): "Posts will
  appear in the voice of the Page." https://developers.facebook.com/docs/pages-api/: "a Facebook App
  Review is required" for most Page management.
- **FB-3 (a). Without Advanced Access an app serves only people with a role on it; its posts in
  Development mode are seen only by them.** Access levels: Standard Access "can only be requested
  from app users who have a role on the requesting app"; app modes: data made in Development mode
  "can only be seen by role users." Business Verification is needed for Advanced Access; a yearly
  Data Use Checkup follows.
- **FB-4 (b). Page comments, tags and the Page inbox can be read; DMs can be sent within a window.**
  Comments (https://developers.facebook.com/docs/pages-api/comments-mentions); tags
  (`/{page-id}/tagged`); the Conversations API, with details for "the 20 most recent messages"; the
  Send API (send-messages.md): the window "lasts 24 hours", a Human Agent tag allows 7 days and
  needs App Review and Business Verification.
- **FB-5 (c). Prefilling messages is limited; automated chats disclose where law requires.**
  Developer Policies: "Don't prefill any content in messages unless (a) it was created by the person"
  (pieces), or by a business's own staff. Messenger policy
  (https://developers.facebook.com/docs/messenger-platform/policy/policy-overview): "When required by
  applicable law, automated chat experiences must disclose" it, and Meta recommends it always.
  Whether an AI draft a person approves counts as "created by the person": unclear.
- **FB-6 (c). Facebook's terms bar automated collection without permission.**
  https://www.facebook.com/terms.php (effective 1 January 2025): "using automated means (without our
  prior permission)" (pieces).
- **FB-7 (c). Meta's AI labels are required for photorealistic video and realistic audio, not
  images.** https://www.meta.com/help/artificial-intelligence/1783222608822690/ ("Updated: 2 weeks
  ago" on 2026-11-06): "This requirement does not apply to images." (pieces)
- **FB-8 (e). The app secret never goes in client code.**
  https://developers.facebook.com/docs/facebook-login/security: "Never include your App Secret in
  client-side or decompilable code." A long-lived token needs the secret; a desktop sign-in that
  returns a token in the address gives a short-lived one; a secret-free PKCE flow exists but "this
  functionality is still in testing" (oidc-token.md).
- **FB-9 (f).** Page call budget "4800 * Number of Engaged Users" a day; the Conversations API 2 calls
  a second per Page. Webhooks need a server with a valid certificate. Scheduling: one page says 10
  minutes to 30 days ahead, another 10 minutes to 75 days (they disagree).
- **FB-10 (g). Personal Messenger chats are being made end-to-end encrypted by default; chats with
  businesses are not.** https://www.facebook.com/help/messenger-app/786613221989782 (no date shown):
  Meta is "in the process of securing personal messages with end-to-end encryption by default";
  not supported for "chats with businesses or accounts using business messaging tools" (pieces).
  Messages shared with Meta AI are readable by Meta. A Page's inbox, which is what an app can reach,
  is a business chat. Group chats: Meta's own pages disagree (unclear).
- **FB-11 (h).** https://www.meta.com/safety/communities/law/guidelines/ (no date shown): a subpoena
  for "basic subscriber records" (name, length of service, email, recent login address); a 2703(d)
  court order for records "not including contents of communications", such as message headers and
  addresses; a search warrant for "the stored contents of any account". For an end-to-end encrypted
  chat, Meta "will no longer be able to provide unencrypted message content" but still gives logs
  and addresses. 90-day preservation; notice to users unless barred.

## Findings: Instagram

- **IG-1 (a). Only professional (business or creator) accounts can use the API.**
  https://developers.facebook.com/docs/instagram-platform/overview/: "your app users must have an
  Instagram professional account." The personal-account API was switched off on 4 December 2024
  (changelog: "The Instagram Basic Display API has been deprecated.").
- **IG-2 (a). Serving accounts the developer does not own needs App Review and Business
  Verification; media must be at a public address.** Overview.md: Standard Access suffices only for
  "your Instagram professional account or an account you manage"; content-publishing.md: "the media
  must be hosted on a publicly accessible server".
- **IG-3 (b). Comments, mentions and the professional inbox are reachable.** The Conversations API
  gives details of "the 20 most recent messages"; messaging-api.md: a 24-hour window, a 7-day Human
  Agent window, "Group messaging is not supported."
- **IG-4 (c). An AI label can be set when posting.** Changelog (entry 22 June 2026): the API "now
  supports self-disclosure of AI-generated content at publish time" (`is_ai_generated`).
- **IG-5 (d). Not just a display.** Developer Policies (February 3, 2026): "Don't use the Instagram
  Platform to simply display User Content". An inbox that only mirrors content may be at odds with
  this (the researcher's reading).
- **IG-6 (e). The Instagram sign-in needs the app secret, on a server.** Business login page:
  `client_secret` is the app's secret; long-lived tokens "must be made in server-side code". A
  long-lived token can be refreshed without the secret.
- **IG-7 (f).** Captions: "Maximum 2200 characters, 30 hashtags, and 20 @ tags."; JPEG only, 8 MB;
  DM text "1000 bytes or less". Posts a day: one page says 100, another and the quota endpoint say
  50 (read the live quota). Webhooks only; otherwise polling.
- **IG-8 (g). End-to-end encryption ended.** https://www.facebook.com/help/instagram/491565145294150:
  "End-to-end encrypted messaging on Instagram is no longer supported as of May 8, 2026." (read by
  the author too.)
- **IG-9 (h).** Meta's guidelines (FB-11) apply.

## Findings: YouTube

developers.google.com pages carry a "Last updated" line; Google help pages show no date.

- **YT-1 (a). Uploads to the person's own channel are allowed, from one daily quota per project
  that every user shares.** https://developers.google.com/youtube/v3/determine_quota_cost (Last
  updated 2026-11-03): the upload method "100 quota per day. Each call costs 1 quota."; 10,000 units
  a day for everything else. More needs a compliance audit
  (https://developers.google.com/youtube/v3/guides/quota_and_compliance_audits).
- **YT-2 (a). Uploads from unaudited projects are no longer forced private (since 2 November 2026).**
  https://developers.google.com/youtube/v3/revision_history (entry "November 2, 2026"): uploads from
  unverified projects "are no longer restricted to private viewing mode".
- **YT-3 (a). No API for community posts.** The quota table lists no posts or messages resource;
  revision history, 30 April 2024: "The API no longer supports the ability to insert or retrieve
  channel discussions."
- **YT-4 (a, c). The person keeps final control, and every write is shown first.**
  https://developers.google.com/youtube/terms/developer-policies (last change logged 2026-06-01):
  users "must have final control over the data that will be published"; actions must be identified
  and "the user must expressly consent to those actions prior to their actual execution" (pieces).
  The API Services Terms require an upload notice beside the Upload button naming YouTube's Terms of
  Service and Community Guidelines.
- **YT-5 (b). Comments can be read (1 unit) and answered (50 units); there is no mentions or DM
  API.** commentThreads.list "has a quota cost of 1 unit"; comments.insert "a quota cost of 50
  units". YouTube has in-app messaging (https://support.google.com/youtube/answer/16650958, no date
  shown; expanded to the US in June 2026), with no API.
- **YT-6 (c). Realistic AI content must be disclosed; AI help with a script or title need not be.**
  https://support.google.com/youtube/answer/14328491: "disclose AI-generated or altered content that
  seems realistic"; "Production assistance" such as a script or title is listed as not needing it.
  The API's `status.containsSyntheticMedia` carries the disclosure.
- **YT-7 (d). Stored data is deleted or refreshed every 30 days and shown only to the person or
  agents they approve.** Developer Policies: after 30 calendar days "the API Client must either
  delete or refresh the stored data"; nobody else sees it except "agents expressly approved by that
  user". Deletion within 7 days of a revocation in the app; a link to YouTube's Terms and Google's
  Privacy Policy is required; YouTube shown as the source.
- **YT-8 (e). Google says a desktop app cannot keep a secret; YouTube's policy forbids credentials
  in open source.** https://developers.google.com/identity/protocols/oauth2/native-app (Last updated
  2026-10-30): "Desktop apps are public clients and cannot keep secrets confidential." Developer
  Policies: you must not "embed your API Credentials in open source projects", and must create
  "exactly one (1) API Project" per client. Whether credentials shipped only in the built program,
  not the repository, satisfy this: unclear.
- **YT-9 (e). Sensitive scopes need Google's verification; until then a lifetime cap of 100 users;
  in testing, sign-ins expire after 7 days.** https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification:
  verification "can take up to 10 days"; https://support.google.com/cloud/answer/15549945: the cap
  "cannot be reset or changed".
- **YT-10 (f).** Upload to 256 GB; title 100 characters; description 5,000 bytes. Push is a
  server-to-server hub for new uploads only; the one stream an app opens itself is a live stream's
  chat (`liveChatMessages.streamList`). Comments are polled.
- **YT-11 (g). YouTube messages are not end-to-end encrypted.** Help page 16650958: "messages may be
  reviewed against Community Guidelines" (pieces) and are "encrypted as they are sent and stored".
- **YT-12 (h).** https://policies.google.com/terms/information-requests (no date shown): a subpoena
  for "basic subscriber registration information and certain IP addresses"; a court order for
  "non-content records"; a search warrant for "the content of communications". Google's FAQ
  (https://support.google.com/transparencyreport/answer/7381738) lists "Private message content"
  for YouTube.

## Findings: Reddit

reddit.com and redditinc.com refuse automated reading for much of their content; Reddit's help
articles were read through its own help-centre API (dates are the articles' `edited_at`).

- **RD-1 (a). New API access closed on 31 October 2026; public access ends in March 2027.**
  https://support.reddithelp.com/hc/en-us/articles/54353370049684 (Changelog, 8 October 2026):
  "October 31, 2026: New requests for public API access will no longer be accepted." and "March
  2027: Remaining public API access ends." Apps move to Reddit's Developer Platform, which runs on
  Reddit's servers; its FAQ (https://developers.reddit.com/docs/guides/faq): "A Devvit login doesn't
  issue a client ID or client secret for an external application."
- **RD-2 (a). Even before, approval was required, and use for a business is commercial.**
  Responsible Builder Policy (https://support.reddithelp.com/hc/en-us/articles/42728983564564, edited
  2026-10-05): "You must request access and get explicit approval". Help article 14945211791892
  (edited 2026-05-28): commercial includes use "on behalf of a business". The often-quoted $0.24 per
  1,000 calls was not found on any primary page.
- **RD-3 (b). Private messages became chat; a chat API is unclear.** Changelog of 7 August 2025:
  replying to existing private messages disabled; the API reference documents no chat endpoint,
  though the Data API wiki gives chat rate limits.
- **RD-4 (c). The rules cover bots and AI agents; no platform-wide AI label rule.** Responsible
  Builder Policy: it applies to apps "including bots, AI agents, or non-human operated accounts";
  "Apps must not engage in spamming activity through automated posts". Developer Terms
  (https://redditinc.com/policies/developer-terms, last revised March 24, 2026): no training of AI
  models without permission.
- **RD-5 (d). Deleted content must be removed; Reddit recommends deleting stored data within 48
  hours.** Data API wiki (article 16160319875092, edited 2026-05-11): apps must "remove any user
  content in your possession that has been deleted from Reddit".
- **RD-6 (g). No claim of end-to-end encryption anywhere** (a search of the help centre for
  "encryption" returned nothing); article 360043048412: Reddit collects "private messages, chats".
- **RD-7 (h). Not read.** Reddit's law-enforcement guidelines (https://redditinc.com/policies/guideline-for-law-enforcement)
  refused automated reading. A help article (50446339398804, edited 2026-10-06) says Reddit notifies
  users and pushes back on overly broad requests.

## Findings: LinkedIn

learn.microsoft.com pages were read whole; linkedin.com pages through the fetch tool (pieces).

- **LI-1 (a). Posting to the person's own profile is an open permission.**
  https://learn.microsoft.com/en-us/linkedin/shared/authentication/getting-access (updated
  2026-06-03): open permissions are "available to all developers without special approval", and
  `w_member_social` posts "on behalf of an authenticated member."
- **LI-2 (a). Posting as an organisation's Page needs a vetted programme for registered legal
  organisations, commercial use only.** https://learn.microsoft.com/en-us/linkedin/marketing/community-management-app-review
  (updated 2026-02-11): "only available to registered legal organizations for commercial use cases
  only."
- **LI-3 (b). Reading the person's own posts and comments is closed; DMs are for partners.**
  https://learn.microsoft.com/en-us/linkedin/marketing/community-management/community-management-overview
  (updated 2026-05-15): "`r_member_social` is a **closed** permission." (read by the author too). The
  Messages page (https://learn.microsoft.com/en-us/linkedin/shared/integrations/communications/messages,
  updated 2023-12-20): "restricted to approved partners"; a message must follow a member action, which
  does "not include an automated or scheduled event." Members in the EEA and Switzerland can export
  their inbox under the EU Digital Markets Act (a snapshot, not an inbox).
- **LI-4 (c). The API may not be used to automate posting.** API Terms of Use
  (https://www.linkedin.com/legal/l/api-terms-of-use, "Last revised on December 13th, 2022"), section
  3.1: "Use the Content or the APIs to automate posting on the LinkedIn Services." (read by the
  author too). The User Agreement (effective November 3, 2025, section 8.2) bars bots that "send or
  redirect messages, create, comment on, like, share, or re-share posts" (pieces).
- **LI-5 (c). AI-generated content: review it, it is yours.** User Agreement 3.6: "Please review and
  edit such content before sharing with others." (pieces). No labelling rule found.
- **LI-6 (d). Store only as long as needed, nowhere a third party can reach.** API Terms 4.1: "store
  Content only for the duration necessary" (read by the author too); content "must not be stored in a
  data repository that would enable any third party access" (pieces); delete on request (4.4).
- **LI-7 (e). The ordinary sign-in needs the client secret and a server address; a secret-free
  sign-in for desktop apps exists but LinkedIn switches it on per app.**
  https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow-native
  (updated 2023-04-03, read by the author): "reach out to your point of contact at LinkedIn, and we
  will enable PKCE"; it returns to a loopback address and the token request carries no secret. Every
  app needs a LinkedIn Page (help answer a548360). Tokens last 60 days.
- **LI-8 (f).** 3,000 characters a post (help answer 86780); 150 requests a day per member, 100,000
  per app; webhooks only for approved uses; no stream.
- **LI-9 (g). No claim of end-to-end encryption; LinkedIn scans messages for its own tools.** Privacy
  policy (https://www.linkedin.com/legal/privacy-policy, effective November 3, 2025): "we scan messages
  to provide “bots” or similar tools" (pieces).
- **LI-10 (h).** Data Request Guidelines (https://content.linkedin.com/content/dam/help/tns/en/Data-Request-Guidelines-Full.pdf,
  "Updated March 2024", transcribed from page images): subpoenas for basic details including login
  addresses; court orders for messages' "from, to, date & time"; search warrants for "Messages". A
  civil subpoena or order "is not sufficient" for contents.

## Findings: Discord

Discord's developer docs are Markdown in github.com/discord/discord-api-docs; its policies and help
articles refuse the fetch tool but were read through its help centre's own API.

- **DC-1 (a). No app may act as a person's account.** OAuth2 docs (oauth2.mdx, last change
  2026-10-16): developers must refrain "from automating standard user accounts (generally called
  "self-bots")".
- **DC-2 (a). Posting into a channel works by webhook, under the webhook's name, or by a bot.**
  webhook.mdx (last change 2026-02-24): "a low-effort way to post messages to channels in Discord";
  they "do not require a bot user or authentication".
- **DC-3 (b). A person's DMs cannot be read.** oauth2.mdx: `dm_channels.read` is "only available to
  approved partners". A bot reads its own channels with an approved intent, and DMs sent to it.
- **DC-4 (b, c). Bots message people only with permission.** Developer Policy
  (https://support-dev.discord.com/hc/articles/8563934450327, effective July 8, 2024): "Do not
  contact users on Discord without their explicit permission." (pieces)
- **DC-5 (c). No training on message content.** Developer Policy, rule 21: not "to train machine
  learning or AI models (including large language models)" without Discord's permission (pieces;
  read by the author too). No rule requiring an AI or bot disclosure was found. Terms
  (https://discord.com/terms, dates unchanged since 2026-09-25): no "auto-messaging" (pieces).
- **DC-6 (d). Delete on a person's request.** Developer Terms
  (https://support-dev.discord.com/hc/articles/8562894815383, effective July 8, 2024, edited
  2025-03-17), 5(b): delete API data when "the applicable user requests you delete it" (pieces).
- **DC-7 (e). Credentials, the Application ID included, may not be in open-source projects.**
  Developer Terms 2(d): "developer credentials may not be embedded in open source projects." (read by
  the author too); the credentials include "your Application ID". Each person or group would create
  their own webhook or bot and paste it in.
- **DC-8 (f).** Messages up to 2,000 characters; 50 requests a second; a bot's Gateway is a WebSocket
  the bot opens itself.
- **DC-9 (g). Text is not end-to-end encrypted; voice and video are, compulsorily, since 2 March
  2026.** https://discord.com/safety/more-private-and-secure (17 September 2024): "messages on
  Discord are unencrypted" (pieces); help article 25968222946071 (edited 2026-02-20): encrypted
  audio and video "cannot be turned off." Discord's blog (18 May 2026): "We have no current plans to
  extend E2EE to text messages."
- **DC-10 (h).** https://discord.com/safety/360044157931-working-with-law-enforcement (updated May 2,
  2025): Discord discloses "consistent with" the Stored Communications Act, the Pen Register Act and
  the Wiretap Act, and asks for "the appropriate type of legal process"; emergency disclosure; 90
  days' preservation (plus 90); users notified unless prohibited.

## Findings: Telegram

- **TG-1 (a). A bot can post to channels; a full client can act as the person.** Bot API
  (https://core.telegram.org/bots/api, version 10.3, changelog to 24 August 2026); API Terms
  (https://core.telegram.org/api/terms, no date shown): third-party apps are welcome "free of charge",
  and "You must obtain your own api_id for your application." (pieces)
- **TG-2 (a, c). Conditions on full clients.** API Terms 1.4: no actions "without the user's knowledge
  and consent" (read by the author too); 2.3: the name "must not include the word “Telegram”"; 3.3:
  "you must include support for official sponsored messages in Telegram channels" (read by the author
  too). https://core.telegram.org/api/obtaining_api_id: such accounts are "automatically put under
  observation."
- **TG-3 (b). Bots cannot write first; a full client reads and sends the person's DMs.**
  https://core.telegram.org/bots: "Bots can't start conversations with users." The user API pushes
  updates over the client's own connection (https://core.telegram.org/api/updates).
- **TG-4 (c). Telegram's data may not be used to deploy AI.** API Terms 1.5: "prohibited from using,
  accessing or aggregating data obtained from the Telegram platform to train, fine-tune" (read by the
  author too) and, per the research agent's reading of the same section, the "development,
  enhancement or deployment of artificial intelligence" (pieces). The content-licensing page
  (https://telegram.org/tos/content-licensing) allows exceptions only where "all relevant users"
  consent (pieces), and a limited exception for operating "a legitimate third-party Telegram Client".
  Whether an AI helper drafting a reply from a fetched message counts as deployment: unclear.
- **TG-5 (d). Delete when no longer needed; encrypt at rest.** Bot Developer Terms
  (https://telegram.org/tos/bot-developers): delete user data "when retention thereof becomes
  unnecessary" and keep it "encrypted at rest" (pieces).
- **TG-6 (e). Credentials must not be public.** Bot Developer Terms 4.5: an API ID and hash are
  confidential and "you are explicitly prohibited from making them available to the public" (pieces).
  Telegram's own open-source desktop client (https://github.com/telegramdesktop/tdesktop/blob/dev/docs/api_credentials.md)
  leaves them out of its source: builders "provide your own **api_id** and **api_hash**".
- **TG-7 (f).** Text "1-4096 characters"; captions 1,024; a bot sends files to 50 MB; about one
  message a second per chat; `getUpdates` long polling is a client-side stream needing no server.
- **TG-8 (g). Only Secret Chats are end-to-end encrypted, one to one and tied to one device.**
  https://telegram.org/privacy (changelog to 21 August 2026), 3.3.2: "Secret chats use end-to-end
  encryption." 3.3.1: cloud chats are stored on Telegram's servers so they reach every device.
  https://core.telegram.org/api/end-to-end: secret chats "are associated with specific devices".
- **TG-9 (h).** Privacy policy 8.3 (changed 23 September 2024): on "a valid order from the relevant
  judicial authorities" about a suspect, Telegram "may disclose your IP address and phone number".
  FAQ (https://telegram.org/faq): "we have disclosed 0 bytes of user messages to third parties",
  governments included. Metadata kept "for 12 months maximum". No US-process guide.

## Findings: Rumble

- **RU-1 (a, c). No posting API; automated access needs written permission.**
  https://rumble.com/s/terms ("Last modified September 1, 2026"): no "automated software to interact
  with or access the Rumble Site" without "the prior written permission of Rumble". Uploads are by
  its web form and Rumble Studio; auto-syndication pushes outward only, to YouTube, Facebook and
  Vimeo (https://rumble.support/help/auto-syndication, 15 November 2022). A read-only "Platform API"
  with keys issued by Rumble: search snippet only (its pages refused).
- **RU-2 (b, e). A creator's own Live Stream API is a read-only address with no sign-in.**
  https://rumble.support/en/help/how-to-use-rumble-s-live-stream-api (20 November 2025): live chat
  messages and rants, followers and subscribers, at most 50 recent items, only while live;
  "Authentication is not required for this version of the API."; the address can be reset "to revoke
  access any time." No comments, mentions, DMs or sending.
- **RU-3 (c). Comments and chat may not ask for donations.** Terms: no messages "requesting donations".
  No bot-label or AI-label rule was found; Rumble takes a licence to train AI on uploads.
- **RU-4 (d).** Terms: no "collection, compilation, library, database or directory" of its content
  without written permission.
- **RU-5 (f).** A Short is "180 seconds or less" (https://rumble.support/help/upload-shorts, 6 May 2026).
- **RU-6 (g). No DM feature found on Rumble; nothing is claimed encrypted.** The word "encrypt"
  appears nowhere in its privacy policy or terms. Locals, a Rumble subsidiary, has member messages
  with no encryption statement (https://locals.com/site/terms, 19 July 2024).
- **RU-7 (h).** https://rumble.com/s/privacy ("Date Last Updated: Jun 19, 2026"): disclosure "in
  response to a subpoena, court order, government request, or other legal process"; it "may attempt
  to notify you". No law-enforcement guide or transparency report was found.

## Findings: Patreon

- **PA-1 (a). Posts cannot be created through the API.** https://docs.patreon.com/ (newest changelog
  entry 2026-10-07): the endpoint list has only reads for posts, "Provides read access to the posts
  on a campaign."; the only writes are early-access Lives and webhooks. API v1 retires on 20 January
  2027 at the latest.
- **PA-2 (b). No API for comments, mentions or DMs.** DMs exist in Patreon's apps between a creator
  and members only (https://support.patreon.com/hc/en-us/articles/203913429, updated July 6, 2026).
- **PA-3 (c, d).** Community Guidelines (https://www.patreon.com/policy/guidelines, updated 2026-10-09):
  no "repetitive, unsolicited messages"; deceptive manipulated media and training AI on other
  creators' works are banned; no AI-label rule. No API-specific terms yet
  (https://www.patreon.com/portal/help/candid-questions): "We are preparing more detailed API-specific
  terms of use."
- **PA-4 (e). The token exchange needs the client secret on a server; a creator's own token works for
  their own tool.** Docs: "Please never reveal your Client Secret(s)."; a "Creator's Access Token"
  serves "a single creator application or tool". A shared app serving many creators needs Patreon's
  approval.
- **PA-5 (f).** 100 requests a minute per token; webhooks only; no sandbox.
- **PA-6 (g). Not encrypted end to end; Patreon reads and moderates them.** Privacy policy (May 27,
  2026, read from its privacy centre's own data file): "we collect the contents of those messages and
  comments". Transparency report 2026: 2025 was "the first full year of moderating member chats".
- **PA-7 (h).** https://support.patreon.com/hc/en-us/articles/360032829791 (updated May 30, 2025):
  "subpoenas, court orders, warrants" served on its registered agent; disclosure "only when legally
  required or when necessary to prevent imminent harm"; preservation up to 90 days; users may be
  notified. Which process unlocks which data is not said.

---

## Findings: the laws on disclosing AI-written and bot-written posts and messages

### The EU AI Act, Regulation (EU) 2024/1689

Read on EUR-Lex: the original (https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:L_202401689,
OJ L, 12.7.2024) and the consolidated text of 27 July 2026
(https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:02024R1689-20260727).

- **EU-1. Article 50(1), AI that talks with people.** "Providers shall ensure that AI systems intended
  to interact directly with natural persons are designed and developed in such a way that the
  natural persons concerned are informed that they are interacting with an AI system, unless this is
  obvious from the point of view of a natural person who is reasonably well-informed, observant and
  circumspect, taking into account the circumstances and the context of use."
- **EU-2. Article 50(2), marking AI output.** "Providers of AI systems, including general-purpose AI
  systems, generating synthetic audio, image, video or text content, shall ensure that the outputs of
  the AI system are marked in a machine-readable format and detectable as artificially generated or
  manipulated." It does not apply "to the extent the AI systems perform an assistive function for
  standard editing or do not substantially alter the input data provided by the deployer or the
  semantics thereof". There is no exception for human review.
- **EU-3. Article 50(4), second subparagraph, published text.** "Deployers of an AI system that
  generates or manipulates text which is published with the purpose of informing the public on
  matters of public interest shall disclose that the text has been artificially generated or
  manipulated. This obligation shall not apply where the use is authorised by law to detect, prevent,
  investigate or prosecute criminal offences or where the AI-generated content has undergone a process
  of human review or editorial control and where a natural or legal person holds editorial
  responsibility for the publication of the content."
- **EU-4. Article 50(5), how.** "The information referred to in paragraphs 1 to 4 shall be provided to
  the natural persons concerned in a clear and distinguishable manner at the latest at the time of the
  first interaction or exposure. The information shall conform to the applicable accessibility
  requirements."
- **EU-5. Who is who (Article 3).** (3) "‘provider’ means a natural or legal person, public authority,
  agency or other body that develops an AI system or a general-purpose AI model or that has an AI
  system or a general-purpose AI model developed and places it on the market or puts the AI system
  into service under its own name or trademark, whether for payment or free of charge;" (4)
  "‘deployer’ means a natural or legal person, public authority, agency or other body using an AI
  system under its authority except where the AI system is used in the course of a personal
  non-professional activity;" (10) "‘making available on the market’ means the supply of an AI system
  or a general-purpose AI model for distribution or use on the Union market in the course of a
  commercial activity, whether in return for payment or free of charge;"
- **EU-6. Reach (Article 2).** 2(1)(c): it applies to providers and deployers in a third country
  "where the output produced by the AI system is used in the Union;" 2(10): "This Regulation does not
  apply to obligations of deployers who are natural persons using AI systems in the course of a purely
  personal non-professional activity." 2(12): open-source systems are exempt "unless they are placed
  on the market or put into service as high-risk AI systems or as an AI system that falls under
  Article 5 or 50." So open source does not exempt anyone from Article 50.
- **EU-7. Article 50 has applied since 2 August 2026.** Article 113: "It shall apply from 2 August
  2026." with exceptions that do not name Chapter IV, which holds Article 50.
- **EU-8. The Digital Omnibus on AI changed only the marking deadline for older systems.** Regulation
  (EU) 2026/1744 of 8 July 2026 (https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32026R1744,
  OJ L, 24.7.2026, in force 27 July 2026) adds Article 111(4): systems generating synthetic content
  "that have been placed on the market before 2 August 2026 shall take the necessary steps in order
  to comply with Article 50(2) by 2 December 2026." It rewrites 50(7) (codes of practice) and leaves
  Article 113's date for Chapter IV unchanged. A system first offered after 2 August 2026 has no grace
  period.
- **EU-9. The Commission's guidelines on Article 50 (non-binding).** C(2026) 5054 final, Annex, 20
  July 2026
  (https://ai-act-service-desk.ec.europa.eu/sites/default/files/2026-07/guidelines_on_the_implementation_of_the_transparency_obligations_for_certain_ai_systems_under_article_50_of_the_ai_act_bzptwqhk0ikg1dtlddap41psfy_131215.pdf;
  press page https://digital-strategy.ec.europa.eu/en/news/commission-publishes-guidelines-transparency-obligations-providers-and-deployers-certain-ai-systems,
  last update 27 July 2026). The passages that bear on this design:
  - Para 5: "These Guidelines are non-binding."
  - Para 13, deployers outside the EU are covered "where the deployer itself foresees dissemination
    and use of the AI outputs in the Union (i.e. by directing or authorising distribution within the
    Union, including by posting deep fakes on the globally accessible internet)."
  - Para 19, professional use: "Any activity through which natural persons gain an economic benefit
    on a regular basis or are otherwise involved in a professional, business, trade, occupational or
    freelance activity should be considered as a ‘professional’ activity."
  - Para 23: "providers and deployers of open-source AI systems within the scope of Article 50 AI Act
    still need to ensure compliance with their respective transparency obligations."
  - Para 30(iii), direct interaction excludes cases where "customer service representatives use AI
    assistance tools to help them communicate with natural persons", but "the mere possibility for
    humans to intervene or review the AI system's outputs should not be used to circumvent" it; AI
    outputs are exempt only if "properly reviewed and sent by humans as the main interlocutors".
  - Para 31: "AI agents must be designed and developed in such a way that they disclose both their
    artificial nature and the person on whose behalf they are acting"; its examples include "managing
    correspondence" and "AI bots on social networks and media".
  - Section 4.3 lists as needing marking "AI-generated summaries of text; paraphrasing or rewriting
    text that changes style, structure and meaning beyond mere grammatical and minor stylistic
    correction;". Para 74: a provider "may rely on the marking solution implemented by an upstream
    model provider".
  - Para 131(i), "published": text accessible by "an indeterminate, fairly large number of unrelated,
    potential readers"; not "private, interpersonal correspondence". Para 131(iii), public interest
    includes "politics", "public health", "environmental protection", "consumer safety" and "any
    economic, financial, political, scientific, or cultural development that may be relevant subject
    of public debate"; a company's advertisement or product descriptions are outside it.
  - Para 134: "Fact-checking the accuracy of the content is a minimum requirement that should be part
    of that review."
  - Para 135: "Superficial, solely formal or procedural checks (e.g. spell-checking or grammatical
    correction), the mere existence of an editorial policy, automated review processes or cursory
    editorial approval without substantive engagement by the human reviewer or the editorial entity,
    cannot fulfil the conditions for human review or editorial control for the purposes of this
    exception."
  - Para 136: "Any substantive AI intervention occurring after the human review or editorial control
    process has taken place will therefore cause the exception to become void."
  - Para 138: "the identity and contact details of the legal person, the natural person or the
    function with editorial responsibility should be made publicly available on an easily findable
    location (if not yet otherwise available)."
- **EU-10. The Commission's questions and answers** (https://digital-strategy.ec.europa.eu/en/faqs/transparency-obligations-under-article-50-ai-act,
  last update 24 July 2026): "Content generated prior to 2 August 2026 does not need to be labelled
  retroactively." It attributes the interaction duty to "Article 50(2)" in one sentence, which reads
  as a slip for 50(1).
- **EU-11. A voluntary Code of Practice on Transparency of AI-generated Content** was found adequate by
  the Commission on 8 July 2026
  (https://digital-strategy.ec.europa.eu/en/library/commission-opinion-assessment-code-practice-transparency-ai-generated-content);
  "Adherence to the code does not constitute conclusive evidence of compliance". The Code's own text
  was not read.

### California

leginfo.legislature.ca.gov refused automated reading (a challenge page). The texts were read in the
Legislature's own bulk database (https://downloads.leginfo.legislature.ca.gov/pubinfo_2025.zip,
file dated 2026-11-01), the official data of the Office of Legislative Counsel.

- **CA-1. Business and Professions Code 17940 to 17943 (SB 1001, Stats. 2018, Ch. 892), operative 1
  July 2019, unamended.** 17940(a): "“Bot” means an automated online account where all or
  substantially all of the actions or posts of that account are not the result of a person." 17941(a):
  "It shall be unlawful for any person to use a bot to communicate or interact with another person in
  California online, with the intent to mislead the other person about its artificial identity for the
  purpose of knowingly deceiving the person about the content of the communication in order to
  incentivize a purchase or sale of goods or services in a commercial transaction or to influence a
  vote in an election. A person using a bot shall not be liable under this section if the person
  discloses that it is a bot." 17941(b): the disclosure "shall be clear, conspicuous, and reasonably
  designed to inform persons with whom the bot communicates or interacts that it is a bot." 17940(c)
  defines an "online platform" by "10,000,000 or more unique monthly United States visitors or users",
  and 17942(c) spares platforms any duty: the prohibition itself applies to "any person".
- **CA-2. The California AI Transparency Act (BPC 22757 and on; SB 942 of 2024, as amended by AB 853
  of 2025 and SB 1000 of 2026) covers images, video and audio, not text.** 22757.6: "This chapter
  shall become operative on August 2, 2026." SB 1000 (effective September 30, 2026) removed the user
  threshold from "covered provider", now "a person that creates, codes, or otherwise produces a
  generative artificial intelligence system that is publicly accessible within the geographic
  boundaries of the state." 22757.3(a) requires a latent disclosure "in any image, video, or audio
  content".
- **CA-3. Newer laws that do not reach this design.** SB 243 (companion chatbots, BPC 22601 and on,
  from 1 January 2026) excludes a bot used for "productivity and analysis related to source
  information"; AB 1609 (2026, customer-service chatbots) binds businesses with "more than five
  hundred million dollars ($500,000,000) in gross annual revenue"; AB 2355 (Gov. Code 84514) labels
  political committees' AI images, audio and video.

### The US Federal Trade Commission

- **FTC-1. The 2024 rule on reviews bans fake ones, AI-written ones included.** 16 CFR Part 465
  (https://www.ecfr.gov/current/title-16/chapter-I/subchapter-D/part-465; final rule 89 FR 68034, 22
  August 2024, "This rule is effective October 21, 2024."). 465.2(a) makes it unlawful "for a business
  to write, create, or sell a consumer review, consumer testimonial, or celebrity testimonial that
  materially misrepresents, expressly or by implication: (1) That the reviewer or testimonialist
  exists;". The preamble, footnote 35: "AI-generated reviews are covered by the final rule".
- **FTC-2. The Endorsement Guides** (16 CFR Part 255, 88 FR 48102, 26 July 2023) say nothing about AI
  authorship; they concern material connections in endorsements.
- **FTC-3. Section 5** (15 U.S.C. 45(a)(1), govinfo, US Code 2023 edition): "Unfair methods of
  competition in or affecting commerce, and unfair or deceptive acts or practices in or affecting
  commerce, are hereby declared unlawful."
- **FTC-4. The impersonation rule** (16 CFR Part 461, effective 1 April 2024) bars posing as a
  government or a business. Extending it to impersonating individuals, and to firms such as "an AI
  platform" that supply the means, was proposed (89 FR 15072, 1 March 2024); the means-and-instruments
  part was dropped "at this time" (89 FR 104905, 26 December 2024); a new advance notice about
  platforms was published 1 October 2026 (91 FR 62347, comments to 30 November 2026). No final rule.
- **FTC-5. No FTC rule requires disclosing that a post or message was AI-written.** A Federal
  Register search for FTC final rules mentioning "artificial intelligence" since 2024 returned three
  (COPPA, impersonation, reviews), none of them such a rule.

### Utah and Washington

- **UT-1.** Utah Code 13-77-103 (https://le.utah.gov/xcode/Title13/Chapter77/C13-77_2025050820250508.xml,
  enacted 2025): a supplier using generative AI "in connection with a consumer transaction" must
  disclose it "if the individual asks or otherwise prompts the supplier". Its definition covers
  systems producing output "with limited or no human oversight".
- **WA-1. Nothing in Washington law in force reaches AI-written text posts.** RCW 29B.35 (synthetic
  media in election communications) covers images, audio and video; RCW 19.435 (from 1 February 2027)
  covers providers with "over 1,000,000 monthly users" of AI that generates images, audio or video;
  RCW 19.440 (from 1 January 2027) covers companion chatbots and excludes productivity tools. Searched
  through the legislature's own sites; the RCW full-text search (a script page) was not run.

---

## What is still unknown, and what would settle it

1. **Is HumanityOS an EU "provider" of an AI system?** If the app offers an AI drafting helper to
   people in the EU, Article 50(2) may require machine-readable marking of drafted text from day one,
   with no review exception (EU-2, EU-8), unless the model's provider marks it upstream (EU-9, para
   74). The question is whether a free, donation-funded, non-commercial project "places on the market"
   (Article 3(10): "in the course of a commercial activity"). **Settles it:** a written question to
   the European Commission's AI Act Service Desk (free), or an EU lawyer.
2. **How substantive must an Outbox approval be?** The guidelines' bar (EU-9, paras 134 to 138) is
   written for newsrooms. **Settles it:** the same Service Desk question; meanwhile the design meets
   the bar on its face (a review screen with the sources, a fact-check tick, no AI change after
   approval, a named responsible person).
3. **YouTube: does shipping a client ID only inside the released program satisfy "embed your API
   Credentials in open source projects"?** **Settles it:** YouTube API Services support, or the
   compliance audit (free; takes weeks).
4. **X: may each person bring their own developer app, and what is "API Connect"?** **Settles it:** a
   question to X's developer support; reading the Public Utility programme's criteria when applying.
5. **Telegram: is an AI helper drafting from fetched messages "deployment" under API Terms 1.5?**
   **Settles it:** Telegram's developer support, or a lawyer. Until then the design treats it as not
   allowed.
6. **Bluesky: may a released desktop app use a loopback address to finish signing in?** **Settles
   it:** reading the reference OAuth client libraries' native examples, or asking the atproto
   maintainers on GitHub (free). A custom URI scheme works either way.
7. **LinkedIn: will it switch on secret-free sign-in for HumanityOS's app?** **Settles it:** creating
   the app (it needs a LinkedIn Page) and asking LinkedIn's developer support.
8. **Threads and Facebook: would a person's own Meta app in Live mode with Standard Access be
   allowed, and does a personal Outbox count as a "consumer app"?** **Settles it:** Meta's App Review
   answer to a real submission.
9. **Discord: does the "Public Client" setting work outside the Social SDK?** **Settles it:** a test
   with a throwaway application.
10. **Reddit's law-enforcement guidelines, and whether user-authorised clients keep access after March
    2027.** **Settles it:** reading both pages in a browser (minutes).
11. **Meta: are Messenger group chats end-to-end encrypted, and are Threads DMs?** Meta's own pages
    disagree or are silent. **Settles it:** a newer Meta help page, or Meta's privacy team.
12. **Small numbers not found:** a Facebook Page post's length limit, Messenger's text limit (only a
    third-party copy says 2,000), LinkedIn's comment length, Rumble's upload limits (behind sign-in),
    Patreon's post length, the true Instagram daily post limit (50 or 100). **Settles it:** reading
    each from the live API's own errors or quota endpoints during the rung that adds the platform.

## Sources we could not reach

- **help.x.com** (403 to the fetch tool): `using-x/automated-account-labels` (the label's wording),
  `using-x/about-chat` and `using-x/encrypted-direct-messages` (X Chat's fallback to unencrypted
  messages), `rules-and-policies/x-legal-faqs`. The automation and law-enforcement pages were read by
  a plain download.
- **transparency.x.com/en/reports/information-requests** (402), **x.com/en/privacy** (402),
  **privacy.x.com/en** (403): the report period and the privacy policy's words on DMs.
- **developer.x.com** pricing (a script shell): Basic and Pro prices, if any remain.
- **Threads' terms**: https://help.instagram.com/769983657850450 and its facebook.com twin (empty),
  https://www.threads.com/legal/terms (404).
- **Meta**: the Send API reference and the message-tags page (script shells, empty even in a
  browser); https://help.instagram.com/581066165581870 and /761121959519495 (empty);
  https://www.instagram.com/legal/terms/ (redirects to an empty page); the Groups API's end date; the
  government-requests report pages (script-drawn).
- **Google**: https://transparencyreport.google.com/user-data/overview (script-drawn, no period).
- **Reddit**: https://redditinc.com/policies/guideline-for-law-enforcement and
  https://redditinc.com/transparency (blocked to the research tools); the r/redditdev post on the
  migration timeline (403); https://old.reddit.com/dev/api (sign-in wall).
- **LinkedIn**: the old law-enforcement PDF URL (404); the current PDF has no text layer.
- **Discord**: the 2026 transparency report PDF (over the fetch tool's size limit).
- **Telegram**: https://t.me/transparency (a bot inside Telegram); the archived pre-2024 wording of
  privacy policy 8.3 (web.archive.org cannot be fetched).
- **Rumble**: https://player.rumble.com/developers/ and its Platform API pages (421), the developer
  index on rumbleplayer.com (a certificate for another name, not bypassed), the creator's API and
  upload pages (behind sign-in), and /s/law-enforcement, /s/transparency, /s/legal, /s/dsa (404).
- **Patreon**: https://privacy.patreon.com/policies (a script shell; read from the site's own data
  file instead); /policy/api, /policy/law-enforcement, /policy/transparency (404).
- **California**: leginfo.legislature.ca.gov's code pages (a challenge page); read from the bulk data.
- **The EU Code of Practice's own text**, AB 1609's operative date, SB 1000's urgency clause wording,
  and Utah's definition of "consumer transaction" were not read.

## What it means for this project

### Certain (from the sources above)

- A serverless, open-source desktop app can sign people in, post, read mentions and replies and
  read and send DMs on **Mastodon** and **Bluesky**, free and without anyone's approval. Bluesky
  needs one public static file describing the app on the project's website (BS-12), and its DMs
  need a separate permission (BS-7). Mastodon's stream (MA-16) is the only real-time feed in this
  set an app can open itself for a person's notifications and DMs.
- **No app can post to a personal Facebook profile** (FB-1). **Reddit** gives no new access (RD-1).
  **Rumble** and **Patreon** have no posting API (RU-1, PA-1). **YouTube** has no community-post or
  DM API (YT-3, YT-5). **Threads** has no DM API (TH-5).
- **Threads, Facebook Pages and Instagram** need App Review, Business Verification and the app
  secret, which Meta forbids in a client (TH-8, FB-8, IG-6). A project server holding the secret would
  break the design's rule that sign-ins never pass through our servers.
- **Discord** forbids acting as a person's account (DC-1) and forbids credentials in open source
  (DC-7); posting into channels works with a webhook or bot the person or group creates.
- **X** charges per call: $0.20 for every post carrying a link (X-1), which the "copies link back"
  rule would make every post.
- **LinkedIn** forbids using its API "to automate posting" (LI-4), so scheduled posts there are out;
  reading comments and DMs is closed (LI-3).
- **Telegram** forbids public credentials (TG-6) and, as written, using its data to deploy AI (TG-4).
- **Platform AI labels exist** on Threads (`is_made_with_ai`), Instagram (`is_ai_generated`) and
  YouTube (realistic content only, `containsSyntheticMedia`); none on X, Mastodon, Bluesky or LinkedIn,
  where the label goes in the text. **mastodon.social requires AI use to be disclosed** (MA-7).
- **Platforms limit what may be kept**: YouTube 30 days then refresh or delete (YT-7), X in step with
  deletions within 24 hours (X-15), Reddit deletions removed and 48 hours recommended (RD-5), LinkedIn
  only as long as necessary (LI-6), and every one that addresses it honours a person's request to
  delete.
- **DMs the platform cannot read** exist only in X Chat, Telegram Secret Chats and personal Messenger
  chats (X-22, TG-8, FB-10). Everywhere else the platform holds a readable copy, and those that
  publish guidelines hand content over on a warrant or court order (X-23, FB-11, YT-12, LI-10, BS-18).
- **The EU AI Act's Article 50 applies now** (EU-7), reaches deployers outside the EU whose output is
  published on the open internet (EU-6, EU-9 para 13), and does not exempt open source (EU-6).
- **No US federal or Washington State law requires labelling AI-written posts** (FTC-5, WA-1);
  California's bot law spares accounts a person runs post by post (CA-1).

### Judgement calls (the author's opinion; a reader may weigh the same sources differently)

- **Build Mastodon first, then Bluesky.** Mastodon's sign-in is simpler (no DPoP or PAR, an
  out-of-band code), it has the stream, and the fediverse shares the project's federation values;
  Bluesky follows as the second rung because of its reach. The operator decides the order.
- **The HumanityOS project's own posts are probably "professional" use** (EU-9 para 19), so the
  operator is a deployer; posts about the mission (poverty, privacy, policy) may be "public interest"
  text, release notes closer to product descriptions. Meeting the review exception's bar on its face
  (a real review screen, a fact-check, no AI change after approval, a named responsible person with a
  contact on the profile) AND labelling AI-drafted posts anyway is both lawful and in the Act's
  spirit; the label costs nothing.
- **A person using the app for their own life is outside the deployer duties** (EU-6, 2(10)), but the
  app's own design still has to meet any provider duty (EU-9 para 20), so the label stays on for
  everyone.
- **Automatic replies with no person's tap should never be on by default**, and if ever offered must
  say in the message itself that an AI wrote it and for whom (EU-1, EU-9 para 31); Bluesky counts
  them as abuse (BS-8), X requires approval (X-10), LinkedIn forbids bots messaging (LI-4).
- **Telegram's AI clause should switch the AI helper off for Telegram messages** until Telegram says
  otherwise (TG-4).
- **X should be "bring your own developer app"** (the person's own credits), or skipped, rather than
  the project paying per post; whether that is allowed is open (unknown 4).
- **For YouTube, the operator's own channel can use his own Google project with credentials kept in
  his vault**, which is not embedding them in open source; serving everyone waits for an answer
  (unknown 3).
- **The X "substitute service" clause and Instagram's "simply display" rule** are risks for a
  unified inbox; showing those platforms' messages only with actions on them (reply, mark done,
  open on the platform) is the safer reading.

**I am not a lawyer, and none of this is legal advice.** It is a dated reading of public pages so
that, when the question comes back, a reader can see exactly what was read, when, and in which
words. Before the Outbox posts for anyone but the operator, and before any AI helper drafts for
people in the EU, a qualified person should read the EU sections.
