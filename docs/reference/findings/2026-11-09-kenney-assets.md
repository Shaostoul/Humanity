# Kenney's game assets: CC0 from the primary pages, the one reservation, and a site that asks AI agents not to read it

**Research date: 9 November 2026** (Pacific time, early morning). Read that day by an AI research
agent working for the project, with the user agent `HumanityOS-research/1.0 (AI research agent
reading licence pages; https://united-humanity.us)`. Nothing was downloaded.

**I am not a lawyer, and this is not legal advice.**

**Read this first: kenney.nl asks AI agents not to read it.** Its `robots.txt` (Last-Modified
27 October 2026) disallows everything to named AI crawlers and fetchers, `ClaudeBot` and
`Claude-User` among them, and disallows `/assets/` and `/downloads/` to every automated reader. I
read two pages (`/support` and `/assets/furniture-kit`) **before** reading `robots.txt`; that was
the wrong order, and I stopped reading the site as soon as I saw it. The 2026-11-08 scan read the
same two pages. So this finding quotes those two pages once, and recommends that any further check
of Kenney's terms be made by a person in a browser. Lesson recorded below.

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(sections 4.3 and 7.7). Index of this day's asset findings:
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md).

## The question

Are Kenney's game assets (the Furniture Kit first) CC0 according to Kenney's own pages, are there
asset-specific exceptions, and what does the project already ship from Kenney?

## Short answer

- **ALLOWED: CC0.** Kenney's support page: "all game assets on the asset pages are public domain
  licensed (CC0)"; the Furniture Kit's page shows its licence as "Creative Commons CC0" (F1, F2).
- **Attribution is not required**; if given, Kenney asks it be "mentioning "Kenney"" (F1).
- **The one reservation: the logo.** "Do not use our logo, as it is reserved for official projects
  by our studio." (F1). No asset-specific exception was found on the pages read.
- **The licence file inside each download was not read** (it needs a download), and the support page
  points to it "for full details". The project shipped 15 Furniture Kit models without keeping that
  file (F3).
- **Kenney's terms of service were not read**: its own `robots.txt` asks AI agents not to (F4).
- **Recommendation: keep using Kenney's CC0 packs as stand-ins**, keep each pack's own licence file
  beside its models from the next download on, and have a person confirm the file and the terms.

## Findings

### F1. The support page

https://kenney.nl/support (read 2026-11-09, before `robots.txt` was read; the page shows "Copyright
© 2010 - 2026" and no date for its answers):

> Yes, all game assets on the asset pages are public domain licensed (CC0). You’re free to use them, even in commercial projects. For full details, see the included license file or visit the asset page.

> Attribution is not required, but if you choose to give credit you can do so by mentioning "Kenney". Do not use our logo, as it is reserved for official projects by our studio.

(the answers to "Can I use the game assets in a (commercial) project?" and "Is attribution required
when using the game assets?"). The footer links a "Terms of Service" page, not read (F4).

### F2. The Furniture Kit's page

https://kenney.nl/assets/furniture-kit (read 2026-11-09, before `robots.txt` was read; that path is
disallowed to all automated readers): "License" "Creative Commons CC0", "Files" "140×", "1.0"
"Released in 2018", tags "furniture interior table chair bed", category "3D". The page lists no
model names; the scan's list of names (`kitchenStove`, `washer`, `shower` and others) came from a
third-party mirror and is still unconfirmed against Kenney's own download.

### F3. What the project already holds

- `assets/models/LICENSE-cc0-model-packs.md` credits "the **Furniture Kit** by **Kenney**
  (https://kenney.nl/assets/furniture-kit), CC0 1.0 Universal" for the folders under
  `assets/models/furniture/` (15 folders today: bathroom mirror, double bed, three bookcases, cabinet
  bed, cushion chair, coat rack, desk, kitchen cabinet, kitchen fridge, lounge sofa, rug, two tables;
  the 2026-11-08 scan said 14).
- `assets/audio/LICENSE.md` credits Kenney audio packs (Interface Sounds, UI Audio, Impact Sounds,
  RPG Audio) as CC0, "Imported 2026-07-25 (operator-approved)".
- **Neither keeps Kenney's own licence file** from the download, and no Kenney zip is kept under
  `E:\HumanityOS\` (the folder where the project keeps other downloaded sources). So the only record
  of the licence is the support page and our own note.

### F4. Kenney's robots.txt

https://kenney.nl/robots.txt (Last-Modified 27 October 2026), in full apart from blank lines:
`User-agent:` `GPTBot`, `ChatGPT-User`, `OAI-SearchBot`, `ClaudeBot`, `Claude-User`,
`Google-Extended`, `CCBot`, `Bytespider` and `PerplexityBot`, each `Disallow: /`; then
`User-agent: *` with `Disallow: /assets/`, `Disallow: /downloads/`, `Disallow: /files/`.

`robots.txt` is a request, not a licence term, and the assets stay CC0 whoever reads the page. But
it is the site owner's clear wish that AI agents not read the site, and this project's research
rule is to respect such a refusal rather than work around it. Kenney's itch.io page for the kit was
tried as a substitute: the first request returned HTTP 429 (too many requests), and a later one 404
(the kit is not published there under that name).

## What is still unknown, and what would settle it

1. **The licence file inside the Furniture Kit download, and any exception in it.** Settled by a
   person downloading the kit from kenney.nl (free) and reading its `License.txt`, then keeping that
   file beside `assets/models/furniture/`. That download also confirms the model names the scan took
   from a mirror.
2. **Kenney's terms of service.** Settled by a person reading https://kenney.nl in a browser. Not
   needed to keep using CC0 files already obtained: a CC0 dedication cannot be withdrawn from copies
   already given under it (CC0 1.0, section 2, "irrevocably").

## Sources I could not reach

- https://kenney.nl/ (any further page, including its Terms of Service): **not read on purpose**,
  because its `robots.txt` disallows AI agents (F4).
- https://kenney.itch.io/furniture-kit: HTTP 429, then 404 (F4).

## What it means for this project

### Certain

- Kenney's pages describe every game asset on them as CC0 with no attribution required (F1, F2).
- Kenney's logo is reserved and must not appear in the project (F1).
- The project ships 15 Furniture Kit models and Kenney audio as CC0, credited in its own files (F3).

### Judgement calls

- **Kenney stays a stand-in source.** The models are low-poly and stylised; the realistic-first rule
  makes them placeholders until Poly Haven, Fuel, SimReady or our own models replace them.
- **Keep the pack's own licence file from now on.** It is the instrument that travels with the bytes
  (the character-assets finding's 2026-11-09 addendum showed a site page and a download's file can
  disagree; there, the Quaternius zip said CC0 while the site's licence page said otherwise).
- **No more automated reading of kenney.nl.** Respect the site's request; a person does the two
  checks above when the next Kenney pack is wanted.

### How the project would credit it

As today, in `assets/models/LICENSE-cc0-model-packs.md` and `assets/audio/LICENSE.md`, using
Kenney's own wording: "Kenney" (https://kenney.nl), CC0 1.0. Optionally a `data/credits.ron` row
(`kenney`, `licence: "CC0 1.0"`, `attribution_required: false`, `shown_in: ["Credits"]`) so the
in-app Credits page names Kenney beside the other libraries. Never Kenney's logo.

### The question for the operator

**Next time a Kenney pack is downloaded, will you (or a person you choose) read its `License.txt` and
Kenney's Terms of Service in a browser, and keep the licence file beside the models?**

Recommendation: **yes**; and keep using the 15 Furniture Kit models and the audio as CC0 meanwhile.
Kenney's six gap items (stove, shower, bathtub, washer, dryer, microwave) are worth taking as
stand-ins only if nothing realistic is ready, since Poly Haven's stove and Fuel's oven, shower and
pump already cover most of them.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
