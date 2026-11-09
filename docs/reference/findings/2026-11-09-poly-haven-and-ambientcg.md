# Poly Haven and ambientCG: may the project ship their models, materials and HDRIs, and what do their sites and APIs add

**Research date: 9 November 2026** (Pacific time, early morning; local clock Mon Nov 9 04:00 PST
2026 when the first page was read). Every source below was read that day by an AI research agent
working for the project, with the user agent `HumanityOS-research/1.0 (AI research agent reading
licence pages; https://united-humanity.us)`, one request at a time, after reading each site's
`robots.txt`. Nothing was downloaded except pages and JSON metadata: no model, texture or HDRI.
Each source carries its own date where it shows one.

**I am not a lawyer, and this is not legal advice.** It is a reading of public licences and terms,
written so a later reader can check every word against the same sources.

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(sections 4.1, 4.2 and 7.1), which ranked these two first for props, materials and HDRIs and asked
this finding to re-quote both licences and confirm that no API term reaches the shipped files.
The index of this day's asset findings is
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md).

## The question

May HumanityOS download Poly Haven's and ambientCG's models, textures and HDRIs, convert and change
them with its own tools (often run by AI agents), and ship them in its open, moddable data folder,
and do either site's terms of service or API terms add any condition to that?

## Short answer

- **ALLOWED, both: use, change and ship them, with no credit required.** Poly Haven: "Our assets are
  all licensed as CC0"; ambientCG: "All ambientCG assets are provided under the Creative Commons CC0
  1.0 Universal License" and "You can include the raw files in your project, for example a video
  game." (F1, F5).
- **No API term reaches the shipped files.** Poly Haven's API terms (last changed 18 July 2026) say
  their attribution clause "does not apply to the CC0 assets themselves once downloaded or
  self-hosted" (F3). ambientCG publishes no terms of service or API terms at all; its API pages ask
  nothing of users beyond a warning that one person runs it (F6).
- **Two conditions apply to HOW files are fetched from Poly Haven, not to the files.** Its website
  terms forbid "Web scraping or data mining without express permission", so scripts must use the
  API; and every API call must carry a user agent "that matches your software name" (F2, F3).
- **One condition would apply if the app itself ever used Poly Haven's live API** (an in-app asset
  browser, say): the screen must make clear the content comes from Poly Haven, "lightweight", with
  no implied endorsement (F3).
- **NOT licensed: anything that is not an asset.** Poly Haven's descriptions, example renders, logos
  and thumbnails are reserved ("excluding the CC0 assets themselves", ToS 4.1); and CC0 waives no
  trademark and clears no one else's rights (CC0 sections 4(a) and 4(c), F4). So write our own item
  text, and look at each chosen asset for printed brands, and each HDRI for readable signs or
  identifiable people.
- **Credit anyway** (both ask for it as a courtesy, and the project already credits both in
  `CREDITS.md`): exact lines are below.
- **Recommendation: yes, both, with the conditions below.** They are the first sources to reach for.

## Findings

### F1. Poly Haven's licence page

https://polyhaven.com/license (read 2026-11-09; the page shows no date; it carries the licence and
the website terms of service on one page). `robots.txt` (Last-Modified 9 November 2026, 08:12 GMT)
allows these pages.

> All assets (HDRIs, textures and 3D models) on this site are the original work of Poly Haven staff, or artists who willingly and directly donate/sell their work to Poly Haven.

> Our assets are all licensed as CC0, which is effectively Public Domain even in jurisdictions that do not support the Public Domain.

and, under "In other words:":

> You can use our assets for any purpose, including commercial work.

> You do not need to give credit or attribution when using them (although it is appreciated).

> You can redistribute them, share them around, include them when sharing your own work, or even in a product you sell.

The same answer, one level down, on an asset page: https://polyhaven.com/a/electric_stove (read
2026-11-09) shows "Author: Kuutti Siitonen" and "CC0" "License"; the API record
https://api.polyhaven.com/info/electric_stove gives `"authors":{"Kuutti Siitonen":"All"}`,
`"donated":true`, `"date_published":1702252800` (11 December 2023) and `"files_hash"` for the
version. **No licence file travels with a download**: the API's file list for that model
(https://api.polyhaven.com/files/electric_stove) holds the glTF, its `.bin` and texture images,
nothing else. The licence lives on the website only.

### F2. Poly Haven's website terms of service (same page)

> Although our assets are shared under the CC0 license, and our code is open source, we maintain a standard Terms of Service agreement to prevent any misuse of this website (polyhaven.com).

Section 3.2 lists what may not be done, including:

> Web scraping or data mining without express permission.

Section 4.1:

> 4.1. All content on this website, excluding the CC0 assets themselves, is protected by intellectual property laws. You may not reproduce, distribute, or otherwise use this content without prior written permission from the website or intellectual property owner.

Section 4.2's examples: "Logos", "User avatar images", "Asset example renders", "User submitted
renders", "Text and copy". Section 5: "The use of the public API hosted at api.polyhaven.com is
governed by separate API Terms." Governing law is South Africa (section 10).

The site's own `llms.txt` (https://polyhaven.com/llms.txt, Last-Modified 9 November 2026, 07:59
GMT), written for automated readers, says the same in plain words:

> If you want lists of assets, their metadata or their download links, please use the API rather than scraping the site.

and lists the website terms as one that "explicitly forbids web scraping or data mining, use the
API instead."

### F3. Poly Haven's API terms

The full terms: https://github.com/Poly-Haven/Public-API/blob/master/ToS.md (read from the raw file
2026-11-09). Its history (GitHub API) shows it was added on 2023-07-25 ("Add Terms of Service") and
last changed on 2026-07-18 ("The API is now freely available for any purpose, including
commercial."). Opening line:

> While our 3D assets themselves are published under the CC0 license, and the code for this API is licensed as AGPL, access to this API through our servers (api.polyhaven.com) is covered separately by these terms of service.

- **Free, any purpose**, 2.1: "The API is free to access and use by anyone - individuals,
  organizations, and companies alike - for any purpose, including commercial use, at no charge."
- **User agent**, 2.4:
  > 2.4. Requirements: When using the API inside your software, website or service, all API calls must be made with a unique "Referer" header or user-agent that matches your software name, such that we can track all the requests from your app together.
- **Attribution, live API only**, 2.5:
  > 2.5. Attribution: This clause applies only to use of the live API service - it does not apply to the CC0 assets themselves once downloaded or self-hosted. CC0 assets carry no attribution requirement whatsoever, now or ever.

  It goes on: "if you use the live API inside your software, website, or service to surface Poly
  Haven content, you must make it clear to your users where that content comes from", gives "a
  label within your interface (a title, menu item, or button that names Poly Haven)" or "a "Powered
  by Poly Haven" credit" as examples, and ends:
  > This credit must be lightweight (a simple credit line or logo) and must not state or imply that your product is made by, owned by, or officially endorsed by Poly Haven.
- **Rights in the API's data**, 3.2: "Users acknowledge that the data provided through the API are
  subject to copyright, trademark, and other intellectual property rights held by Poly Haven or
  their respective owners." Read with 2.5 and the website terms 4.1, this covers the metadata and
  text around the assets, not the CC0 files.
- **No warranty, may change**, 4.1 and 5.2: the API may be changed or ended "without prior notice",
  and Poly Haven "makes no warranties or representations regarding the accuracy, completeness, or
  reliability of the API data or assets."

The summary page https://polyhaven.com/our-api (signed "-Greg", "18 July 2026") repeats it:

> This is a condition of using the live API service, not of the assets themselves: the assets are CC0.

and "There's no premium tier, no revenue share, and no login or API key required for the default
endpoints." It also names, among known uses, "training data for AI/ML pipelines". The FAQ
(https://docs.polyhaven.com/en/faq, no date) answers "Can I train an AI with your content?" with
"Yes." and says "All of our assets are made by humans with love, we do not use any generative AI in
the creation of our work." So AI agents running our conversions raise no issue here.

### F4. What CC0 does not do (the legal code)

https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt (served with Last-Modified
11 September 2023). Section 4:

> a. No trademark or patent rights held by Affirmer are waived, abandoned,
>    surrendered, licensed or otherwise affected by this document.

> c. Affirmer disclaims responsibility for clearing rights of other persons
>    that may apply to the Work or any use thereof, including without
>    limitation any person's Copyright and Related Rights in the Work.

Section 4(b): the work is offered "as-is", with no warranty of "non infringement". So a CC0 asset
is clean of its maker's copyright, but a brand printed on a texture, a sign photographed into an
HDRI, or an identifiable person standing in one, is not cleared by the dedication. Poly Haven's FAQ
adds a request, not a licence term: "The only thing you can't do is claim to be the original author
or re-license them." and, for products, "Please make it clear to your customers that these assets
are free and public domain, and if possible include a link to Poly Haven."

### F5. ambientCG's licence page

https://docs.ambientcg.com/license/ (served with Last-Modified 31 October 2026, 10:37 GMT;
`robots.txt` on ambientcg.com, Last-Modified 4 April 2026, reads `User-agent: *` `Allow: /` apart
from `/hx/`; the docs host has none, a 404).

> All ambientCG assets are provided under the Creative Commons CC0 1.0 Universal License.
> This applies to the downloadable asset files and the material preview renders shown for each asset on the site.

(In the page the first sentence breaks across two lines, with the licence name in bold.) Under
"License Details":

> You can copy, modify, distribute and perform the assets, even for commercial purposes, all without asking permission.

> You can include the raw files in your project, for example a video game.

and under "Giving credit":

> You don't need to give credit but I would of course appreciate it, if you did it anyways.

with suggested text:

> Created using <asset name> from ambientCG.com,
> licensed under the Creative Commons CC0 1.0 Universal License.

Unlike Poly Haven, ambientCG puts its preview renders under CC0 too. The API record for one asset
(https://ambientcg.com/api/v3/assets?id=PavingStones036&include=downloads,license) lists only its
zip downloads and sizes (1K to 8K, JPG and PNG); it carries no licence field.

### F6. ambientCG's API and the absence of other terms

- https://docs.ambientcg.com/api/ (Last-Modified 31 October 2026): "This section covers the
  ambientCG API which you can use to search and download assets using code." Then a warning, quoted
  exactly, doubled word included:
  > This is not a problem in itself but I would like to make it clear that all of this is operated and and filled with content by just one person.

  > It was built with hobbyists and educators in mind and the API is therefore potentially not as reliable or stable as needed for an 'enterprise-level' application.
- https://docs.ambientcg.com/api/v3/ (Last-Modified 31 October 2026): read-only GET endpoints; an
  API token only unlocks a Patreon supporter's own benefits (early access, exclusive files). No
  key is needed for public assets, and no condition of use is stated.
- **No terms of service exist that I could find.** The docs' whole navigation is License, Asset
  Types, Creation Methods, Patreon Rewards, API, Technical Updates, Logo and Legal/Imprint; the last
  (https://docs.ambientcg.com/legal/) is a German legal imprint naming the operator, Lennart Demes,
  and a privacy statement. Nothing restricts automated reading or bulk downloads.
- How assets are made, https://docs.ambientcg.com/creation-methods/: "Approximated",
  "Photogrammetry", "Procedural" ("usually in Adobe Substance 3D Designer"), "Multi Angle
  Lighting", "Plain Photo", "Stitched HDRI", "Stitched HDRI (Edited)" and "3D Photogrammetry".
  Generative AI is not among them (the page says nothing either way).

## What is still unknown, and what would settle it

1. **Whether a chosen asset shows a brand, a readable sign or a person.** CC0 does not clear those
   (F4). Settled by looking at each chosen model's colour texture and each chosen HDRI before it is
   converted: minutes per asset. Sky-only HDRIs and plain materials need little more than a glance.
2. **Whether Poly Haven would call a scripted fetch of specific files through the API "data
   mining".** The API terms invite "bulk-downloading" (the API page names it) and the website terms
   send automated readers to the API, so fetching chosen assets through the API with a named user
   agent is the sanctioned path. Settled finally by asking Poly Haven (free; their contact page).
   Not needed to proceed.
3. **Nothing open on ambientCG** beyond item 1.

## Sources I could not reach

- https://polyhaven.com/tos returned 404: the terms live on the licence page instead (F2).
- docs.ambientcg.com has no `robots.txt` (404); ambientcg.com's own allows everything.

## What it means for this project

### Certain

- Every Poly Haven and ambientCG asset may be converted, decimated, re-textured, merged and shipped
  in `assets/` or `data/`, in releases, in the open repository and on the website, with no credit
  and no licence file required (F1, F5).
- They stay CC0 inside the CC0 project; nothing about them needs a `LICENSES.md` obligation entry.
- Fetch scripts must use the Poly Haven API, not the website, and send a user agent naming our
  software (F2, F3). For ambientCG the API is the convenient route; no rule requires it.
- If the app ever browses Poly Haven's live API for players, that screen must name Poly Haven, and
  the credit must be lightweight and imply no endorsement (F3). Files we downloaded and ship do not
  carry that duty (F3).
- Poly Haven's descriptions, renders, thumbnails and logo are not CC0 (F2): our item text and icons
  must be our own (or ambientCG's previews, which are CC0, F5).

### Judgement calls

- **Credit both anyway**, as both ask and as the project already does for the CC0 model packs
  (`assets/models/LICENSE-cc0-model-packs.md`); being remembered is the point.
- **Record provenance per asset**, so a later reader can tell what was taken and when: the asset
  slug or ID, its author (Poly Haven's API names one per asset), the download date and Poly Haven's
  `files_hash` or ambientCG's file name. That costs one line per asset.
- **A brand and people glance per asset** (unknown 1) as part of choosing it, the same step the
  SimReady finding asks for.
- **Prefer the API's metadata for real dimensions** (Poly Haven gives millimetres) over guessing
  scale; that is data about the asset, used, not republished.

### How the project would credit it

- **Beside converted files**: one `assets/models/LICENSE-cc0-poly-haven.md` (and the same for
  ambientCG materials where they land) in the style of `LICENSE-cc0-model-packs.md`: "The following
  folders are converted from Poly Haven (https://polyhaven.com) assets, CC0 1.0 Universal
  (https://creativecommons.org/publicdomain/zero/1.0/). No attribution is required; we credit the
  authors anyway." Then one line per asset: `electric_stove` by Kuutti Siitonen,
  https://polyhaven.com/a/electric_stove, downloaded <date>, files_hash <hash>, changes: <list>.
- **For ambientCG**, its own suggested line: "Created using <asset name> from ambientCG.com,
  licensed under the Creative Commons CC0 1.0 Universal License."
- **`data/credits.ron`**: two rows (`poly_haven`, `ambientcg`) with `licence: "CC0 1.0"`,
  `attribution_required: false`, `shown_in: ["Credits"]`, so the in-app Credits page lists them
  (today only `CREDITS.md` does).
- **`CREDITS.md`**: the "Asset libraries" lines exist; widen Poly Haven's from "plant models and
  textures" to "models, textures and HDRIs" when the first non-plant asset ships.
- **Never**: Poly Haven's logo or wording that suggests either site endorses HumanityOS.

### The question for the operator

**Shall Poly Haven and ambientCG be the project's first-choice sources for props, materials and
HDRIs, starting with Poly Haven's `electric_stove` for the stove and oven gap?**

Recommendation: **yes**, both, with: fetching through the APIs with a named user agent; our own item
text (never Poly Haven's descriptions or renders); a brand, sign and person glance at each chosen
asset; a per-asset provenance line and courtesy credit as above; and `credits.ron` rows so the
Credits page shows them.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
