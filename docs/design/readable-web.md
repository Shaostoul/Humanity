# The Readable Web

> Rung 6 (core) of the in-world screens ladder, shipped 2026-09-16. Supersedes
> the CEF kiosk design in `docs/native/in_game_browser.md` (kept as reference).
> The decision it implements is Brief 4 in `docs/design/decision-briefs.md`.

HumanityOS can open a real website inside the app and draw it itself: the
headings, paragraphs, links, images, lists, tables and code of the page, laid
out in egui, with links that navigate inside the same view. There is no
browser engine in the binary and no JavaScript runs. This document says what
that gives, what it deliberately does not give, what leaves the machine when
it is used, how the websites database records whether a site may be shown this
way, and what the next rung is.

## What renders, what does not

The reader keeps what a person reads and drops what a page does:

| Kept | Dropped |
|------|---------|
| `h1` to `h6`, paragraphs, line breaks | `script`, `style`, `noscript`, `template` |
| links (`a href`), bold, italic, inline code | forms and every control (`input`, `button`, `select`, `textarea`) |
| `ul`/`ol` lists, nested to any depth | `nav`, `footer`, `aside`, and the ARIA landmark roles that mean the same |
| images (`img`, lazy `data-src` too) | `iframe`, `object`, `embed`, `canvas`, `svg`, `video`, `audio` |
| `pre` code blocks, whitespace kept | anything `hidden`, `aria-hidden`, or styled `display:none` |
| tables (`thead`/`tbody`, `th` bold, captions) | 1x1 tracking pixels |
| block quotes, horizontal rules | class/id conventions listed in `data/web/readability.json` |

If the page marks its content with `<main>` or `<article>`, only that subtree
is read; otherwise the whole `body`. Relative links and image sources resolve
against the page URL (and `<base href>` when present), so `/wiki/Foo`,
`../x.png` and `//host/path` all come out absolute. Only `http` and `https`
links stay links; a `javascript:` or `mailto:` href becomes plain text.

A page whose readable body comes out empty (only navigation and footers) shows
a notice saying so and pointing at "Open in browser".

**What this means in practice.** Wikipedia, documentation sites, blogs, wikis,
our own pages (united-humanity.us, the forge, the GitHub repository pages) and
any site that puts its content in HTML all read well. A site that is only a
JavaScript application (the page HTML is an empty `<div id="root">`) shows the
empty-page notice. That is the trade, made on purpose, and the "Open in
browser" button on the toolbar hands those pages to the system browser.

Not yet: CSS layout (columns, floats), fonts, forms, video, non-UTF-8 pages
(they read with replacement characters), and `<picture>`/`srcset` selection
(the plain `src` is used).

## Why no JavaScript

Brief 4 chose "the readable web" over embedding Chromium/CEF, Servo or an OS
webview, and the reasons hold up in use:

- **No tracking, no popups, no consent walls.** With no script running there
  is nothing to fingerprint the reader, set third-party cookies, open windows
  or replay ads. The page is inert text.
- **Safe to composite on in-game screens.** A page is a `Vec<Block>`; drawing
  it on an in-world monitor is a rendering question, not a security question.
  A live browser engine on a world quad would be a live browser engine with
  game input forwarded to it.
- **Small.** html5ever and the `url` crate. No 200 MB runtime, no per-OS
  webview divergence, no second process.
- **Honest.** The view says what it is. Cooperating sites render; the rest get
  the escape hatch. Servo/Verso is re-evaluated yearly per Brief 4; if it
  matures it slots behind the same view.

## The opt-in, and what leaves the machine

In-app web reading is **off by default** and **independent of the privacy
tier**. A person who never turns it on never has the app fetch a web page:
the Browser page's cards open the system browser exactly as before. The switch
is Settings > Privacy > "Read websites inside HumanityOS"
(`AppConfig.readable_web`, persisted through the normal config path; a config
that predates the field reads as off). The Browser page shows a one-click
button to that switch while it is off, per the GUI-first rule.

When it is on and a page is opened, this is everything that leaves the
machine:

1. One HTTPS `GET` for the page URL, from the person's own connection, never
   through our relay. User-Agent: `HumanityOS/<version> readable-web`. No
   cookies (the ureq cookie feature is not compiled in, so there is no store
   to send from), no referer, no scripts, no fonts, no stylesheets.
2. One `GET` per image that scrolls into view, through the same image cache
   chat uses. An image below the fold is not requested until the reader
   scrolls to it: the view lays out a placeholder, asks egui whether that
   placeholder is inside the visible part of the scroll area, and only then
   dispatches the request (`PageDraw::image` in `web_view.rs`). The headless
   test `web_view_requests_an_image_only_when_it_scrolls_into_view` draws an
   eighty-paragraph page with one image at the bottom in a 400 px viewport
   and proves nothing is requested, then in a viewport tall enough to show
   the whole page and proves it is.

Nothing else. No third-party beacons, because nothing runs to send them.

Where those requests go is decided by the page, not by the address row: an
image whose `src` names another host is fetched from that host, and a
redirect is followed to whatever host it names (up to five hops, each one
scheme-checked but not host-checked). So opening a page can mean talking to
hosts other than the one you typed; what never happens is a request the
page did not name.

The fetch itself is gated and bounded (`src/web_reader/fetch.rs`):

- The scheme is checked **before any request**: only `http` and `https`.
  `javascript:`, `file:`, `data:`, `mailto:` and `ftp:` are refused with a
  status-line message. The same check runs on **every redirect hop**, which
  is why redirects are followed by hand (ureq's own follower would not ask).
- 10 s whole-request timeout, 5 hops of redirect at most, 4 MB body cap
  enforced by `Read::take` (so a wrong Content-Length cannot get around it).
- Images take a separate path, the chat image cache
  (`src/gui/widgets/image_cache.rs`), with its own bounds: a 16 MB cap on the
  bytes downloaded (`MAX_IMAGE_BYTES`; a declared Content-Length over the cap
  is refused before the body is read, and a body that streams past it with no
  length is cut off by `Read::take`), a 20 s timeout, and a pixel cap on the
  decoded texture. ureq follows redirects for images on its own; it only
  speaks http and https, so an image redirect cannot reach a scheme the page
  gate would refuse. The 4 MB figure above is the page only.
- A response that is not `text/html`, `application/xhtml` or `text/plain` is
  refused with the type named ("Not a readable page (server sent
  application/pdf)").
- The fetch runs on a background thread; the UI polls a channel. A slow or
  dark site never freezes the app.

## The widget and the page

`src/gui/widgets/web_view.rs` draws a `Page` with: a toolbar (Sites, Back,
Forward, Reload, the editable URL field with Go, Open in browser), a status
line (fetching, the page title, or the error in one sentence), the affiliate
disclosure when the site carries a tag (none do yet), and the page in a
scroll area. Links are clickable labels in the accent color; the layout
approach is the markdown reader's (one wrapped run per paragraph, even-width
grid for tables, card-colored frame for code). Every color is a theme token.

`src/gui/pages/browser.rs` is the Browser page: the site cards from the
database, grouped by category, and the web view when the opt-in is on and a
card or typed address was opened. A card's hover text shows the site's embed
review state so the review backlog is visible where the sites are.

`web/pages/web.html` is the web mirror: the same cards from the same file,
opening in a new window (a website cannot host the reader). The parity lint
(`tests/page_parity_lint.rs`) fails if either side stops reading
`data/web/sites.json`.

## The websites database

`data/web/sites.json` (schema `schemas/web_sites.toml`) is one record per site
the Browser page offers. It was seeded on 2026-09-16 by merging the native
page's `data/browser/bookmarks.json` and web.html's `DEFAULT_SITES` array, two
lists that had drifted apart (17 shared URLs out of 42); both are gone.

Each record: `id`, `name`, `url`, `category`, `description`, `icon`, and two
sub-records:

```
"embed": {
  "status": "needs_review",       needs_review | allowed | forbidden | unknown
  "basis": "not yet reviewed",    the terms clause the decision rests on
  "terms_url": null,
  "reviewed_on": null,            ISO date
  "reviewed_by": null             a person's name
},
"affiliate": {
  "program": null,
  "tag": null,
  "disclosure": ""                shown above every page while tag is set
}
```

### The legality workflow

The database decides nothing. It records the decision a person made after
reading the site's terms, and it refuses to let a decision be typed in without
the evidence:

1. Every site starts at `needs_review`. Our own domains (`own_domains` in the
   file: the live site, the forge, the GitHub repository pages) are `allowed`
   with basis "our own site".
2. To move a site out of `needs_review`, a person reads its terms of service
   and looks for clauses about framing, embedding, display inside other
   software, and (if relevant) affiliate-programme rules. They then record
   `basis` (the clause, quoted or closely paraphrased), `terms_url`,
   `reviewed_on` and `reviewed_by`, and set `status` to `allowed` or
   `forbidden`. If no terms address it, `unknown` with `basis` saying what was
   checked; `terms_url` may then be null.
3. `scripts/check-web-sites.js` (in `just preflight` and `just check-web-sites`)
   refuses a record whose status is not `needs_review` but lacks any of those
   fields, a site on our own domain that is not `allowed`, a duplicate id or
   URL, a non-http(s) URL, and an affiliate tag without a disclosure.

Affiliate tags require a disclosure string because of the transparency badge
idea in the old kiosk design: a person always sees when a link earns the
cooperative money, and the sentence says exactly what the tag does ("the
price is the same for you"). The web view shows it above the page for every
page of that site, not only the bookmarked one.

Whether embedding a given site, or joining an affiliate programme, is
acceptable is the operator's call to make and record; this increment records
the seed state honestly (34 of 38 awaiting review). The fetch path was run
against two non-affiliate URLs, `https://united-humanity.us` (also over plain
`http://`, which answers 301 and exercises the manual redirect hop) and
`https://en.wikipedia.org/wiki/Silverdale,_Washington`, first at review on
2026-09-16 and then as the permanent ignored test
`web_reader::fetch::tests::live_fetch_of_the_two_reference_pages`. It needs
the network, so it is skipped by the normal test run; run it on demand with
`cargo test --features native --lib -- --ignored web_reader::fetch::live`.

### Readability hints

`data/web/readability.json` (schema `schemas/web_readability.toml`) lists the
class names and ids that mark screen-only chrome on cooperating sites
(`noprint`, MediaWiki's `mw-editsection`, cookie banners, skip links). The
HTML-spec part of readability is fixed in `src/web_reader/parse.rs`; the file
holds only markup conventions, never a domain (the checker refuses one).

## Tests

All offline (`cargo test --features native --lib web_reader`, `web_view`,
`readable_web`):

- Parser, on fixture HTML under `tests/fixtures/web/`: headings and title in
  order; relative, root-relative, protocol-relative and absolute links resolve
  correctly and `javascript:` does not become a link; script, style,
  noscript, template, nav, footer, aside, forms, hidden and class-listed
  elements are gone; lists (with nesting depth), images (and the dropped
  tracking pixel), `pre`, rules, quotes and tables come out as typed blocks;
  `<main>` wins over the rest of the body; `<base href>` is honoured;
  whitespace collapses like a browser; a page of only nav and footer is empty
  with a notice that names the fix.
- Fetch gate: every blocked scheme is refused by `check_url` and by
  `fetch_page` with the same error (proof it is refused before the request),
  including through the background channel.
- Widget, headless: a page with a link is drawn with the app's own egui
  context, the link is clicked with the synthetic move/press/release
  sequence, and the view has queued navigation to the resolved absolute URL
  with no fetch dispatched. This test was proven red on 2026-09-16 (at
  review, and again in the review-fix pass) by disabling the click handler;
  it then fails with `left: None, right: Some(".../nearby.html")`.
- Widget, headless, images: an eighty-paragraph page with one image at the
  bottom is drawn in a 400 px viewport and the image cache must still be
  idle for it; drawn in a viewport tall enough for the whole page, the cache
  must be fetching it. Proven red by removing the `is_rect_visible` gate
  (the first assertion then fails with "fetched while it was off screen").
- Image cache cap: four tests against a one-shot loopback server prove a
  declared length over the cap is refused before the body is read, a body
  that streams past the cap with no length is refused, a body within the cap
  arrives whole, and exactly-at-the-cap passes while one byte over does not.
- Config: `readable_web` reads as off for a config that predates it and for a
  fresh install, and a deliberate on survives save + load through both GUI
  legs.

## On a wall

The same view renders on an in-world screen (rung 6, integration; the
screen architecture is `docs/design/in-world-screens.md`). A machine
instance whose `screen_source` is `web:<url>` gets a `WebProvider`
(`src/engine/screens/web.rs`) that owns one `WebViewState` of its own and
draws it into the screen's texture every framed tick. The shipped example is
`wall_screen_3` in `data/machines/home.ron`, on the console room's east
wall, showing `https://united-humanity.us`. The player looks at the wall,
the look ray becomes the view's pointer, and a click on a link navigates the
wall; Back, Forward, Reload, the address row and "Open in browser" are the
same toolbar minus "Sites" (a wall has no card list to return to).

**The off switch holds on the wall.** `readable_web` is read every frame.
While it is off, the wall shows a notice ("In-app web reading is off", the
url the screen would show, and the exact control: Settings > Privacy, "Read
websites inside HumanityOS") and the provider never calls the view, so no
navigation is queued and no fetch can be dispatched; the screen's status
reports `off`. Turning it on starts ONE navigation on the next frame (never
one per frame: the view's own fetch is asynchronous and is polled, not
re-issued). Turning it off again stops drawing the view at all. Everything
in "The opt-in, and what leaves the machine" above applies unchanged: the
same one GET for the page, the same lazy image requests as they scroll into
view, on the same bounded fetch path.

**What the rig proves.** `just verify-screens` (`scripts/verify-screens.js`)
boots the real release binary in a portable rig with `readable_web: true`
written into the rig's own config before boot, enters the world, parks the
camera in front of the web wall, and through `debug/screen_request.json`
waits for the page to report `ready` on our own host, snapshots it, clicks
the first link that stays on our own host (the screen reports the hrefs it
drew, in link order; the engine maps that link's rect to a point and sends
a normal hover, press and release on three frames through the screen's
event API, never a side path into the view; a page with no such link gets
no click at all, so the gate never fetches a third-party page), waits for
`ready` again on a NEW url that is still on our host, snapshots again, and
requires the two images to differ. The same run proves the inventory wall
reacts to a click with more than a pixel diff: it finds the Home container
header by its drawn text, hovers it FIRST so both snapshots carry the
pointer in the same place (egui's floating scrollbar fades in under a
pointer, which alone makes two frames differ), checks that a child row
drawn only while Home is open ("Garage") is found, snapshots, clicks, and
requires the click's answer to report the header's PointingHand cursor,
the child row to be gone, and the two images to differ; then it checks the
tasks wall drew a page and the log holds no panic. The status the rig
reads back is the provider's own (`url`, `title`, `status`), so a run
where the setting did not take fails with `off`, never passes by luck. The
gate only FOLLOWS LINKS on our own site; the page's own images and any
redirect go to the hosts the page names, exactly as in "The opt-in, and
what leaves the machine" above.

Run it on the dev machine (never in CI: it needs the GPU) after touching
the web view, the screen surface, the provider, or the IPC:

```
cargo build --features native --release
just verify-screens              # boots, clicks, judges; evidence in .probe-rig/screens/runs/
just verify-screens --dry-verdict .probe-rig/screens/runs/<stamp>/manifest.json
node scripts/verify-screens.js --self-test   # the verdict logic on the fixture manifests (no GPU)
```

Headless twins of the wall checks live in `src/engine/screens/web.rs`:
the off switch never fetches, one navigation per screen, a click at a
link's rect through the core navigates the wall view, a failed fetch
reports its reason.

## The ladder above

Rung 6 core and its wall integration are shipped. The sites database
records each site's review state (`embed.status`: needs_review, allowed,
forbidden, unknown, with the basis and the reviewer); today its only
consumer is the review label on the Browser page's site cards, and nothing
checks it when a `web:` source is placed on a screen. Still wanted:

- **A placement gate on `embed.status`:** a `forbidden` site is never
  placed on a screen and a `needs_review` one carries the review badge on
  the wall. Until it exists the only guard is that the shipped
  `wall_screen_3` shows our own site.
- From the old kiosk design: input from a VR controller ray, distance-based
  suspend of a wall's fetches, an affiliate dashboard once any programme is
  joined.

## Files

| Path | Role |
|------|------|
| `src/web_reader/mod.rs` | Document model (`Page`, `Block`, `Inline`), `WebError`, `ReadRules`, limits, user agent |
| `src/web_reader/dom.rs` | Arena DOM and the hand-written html5ever `TreeSink` |
| `src/web_reader/parse.rs` | Readability pass and block conversion, fixture tests |
| `src/web_reader/fetch.rs` | Scheme gate, capped fetch, manual redirects, background thread |
| `src/gui/widgets/web_view.rs` | The egui view: toolbar, history, status, page drawing |
| `src/gui/pages/browser.rs` | The Browser page: cards + the view |
| `src/engine/screens/web.rs` | The view on an in-world screen (`WebProvider`) |
| `scripts/verify-screens.js` | The runtime rig that clicks the wall screens (`just verify-screens`) |
| `src/gui/pages/settings.rs` | The Privacy-section opt-in toggle |
| `src/config.rs` | `AppConfig.readable_web` and its save/load legs |
| `data/web/sites.json`, `schemas/web_sites.toml` | The websites database |
| `data/web/readability.json`, `schemas/web_readability.toml` | Readability hints |
| `scripts/check-web-sites.js` | The gate on the database (preflight) |
| `web/pages/web.html` | The web mirror of the Browser page |
| `tests/fixtures/web/` | Parser fixtures |
