# Sketchfab's CC0 and CC BY models: how its terms bind downloads, what the NoAI tag means, and how per-model credit must look

**Research date: 9 November 2026** (Pacific time, early morning). Every source below was read that
day by an AI research agent working for the project, with the user agent
`HumanityOS-research/1.0 (AI research agent reading licence pages; https://united-humanity.us)`,
one request at a time, after reading `robots.txt` for sketchfab.com and help.sketchfab.com (both
allow these pages). Public API records were read; **no model was downloaded** (downloads need a
logged-in account, F3).

**I am not a lawyer, and this is not legal advice.**

Builds on the 2026-11-08 asset scan,
[`../research/2026-11-08-asset-packs/README.md`](../research/2026-11-08-asset-packs/README.md)
(sections 4.7, 5 and 7.2), which listed about 40 CC BY 4.0 candidates for the shower, washer, water
heater, pump, inverter and air-handling gaps, and the hazards it saw (brand replicas, fan art,
AI-generated uploads, NoAI tags). Index of this day's asset findings:
[`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md). The CC BY 4.0
reading is the one in [`2026-11-08-nvidia-simready-assets.md`](2026-11-08-nvidia-simready-assets.md),
F2.

## The question

May HumanityOS download Sketchfab models offered under CC0 or CC BY 4.0, convert them (often with AI
agents running the conversion) and ship them; how do Sketchfab's own terms bind such downloads; what
does the "NoAI" tag mean legally; and what must a per-model credit say?

## Short answer

- **ALLOWED: CC0 and CC BY 4.0 models, one by one.** Sketchfab's terms let uploaders offer models
  under Creative Commons licences, and the downloader agrees "to adhere to the terms of the license
  that applies at the time of download" (F2, F3). Sketchfab's help centre: "the license you received
  it under will always prevail" (F6).
- **NOT allowed: Standard, Free Standard and Editorial models.** The Standard licence forbids making
  the asset available "as a stand-alone file" or in a way that lets others extract it, which an open,
  moddable data folder does; Editorial bars commercial and promotional use and alteration (F2).
  NonCommercial and NoDerivatives CC models are out too.
- **Downloads need the operator's logged-in account.** The download API requires the user to "be
  authenticated", and that user is "responsible for complying with the terms of the license" (F3).
  Account holders must be over 18 (F3).
- **NoAI is a contract term on Sketchfab, not part of the CC licence.** The terms say "You shall not
  collect, aggregate, mine, scrape, or otherwise use NoAI Content" in, for or "as inputs to
  Generative AI Programs" (F4). Sketchfab's own help centre says that for a CC model, "only the CC
  license applies once the model is distributed outside of Sketchfab.com" (F4). But whoever downloads
  it from Sketchfab has agreed to the terms, and an AI agent that reads a model's files to convert
  them is plausibly "Generative AI Programs" using it "as inputs". **Avoid NoAI models**: both the
  `noai` tag and a NoAI statement in the description (F4).
- **Credit per model**: the title, the author's name, links to the model and to the licence, and a
  note of changes; Sketchfab's own pattern is "This work is based on Model 2 by Sketchfab licensed
  under CC BY 4.0." with the title and author as links (F6).
- **The service changed hands.** The terms (effective 27 October 2026) name "KitBash SF Operations
  LLC."; the licence agreement page still says "Sketchfab is a division of Epic Games, Inc." (F1).
  Neither changes a CC licence already granted by the artist.
- **Per-item hazards stay** (brands, fan art, AI-generated uploads): CC BY licenses only what the
  uploader owns, and Sketchfab's terms make the uploader alone responsible for that (F5).
- **Recommendation: yes, for gap items nothing better covers, picked by hand with the checklist
  below**, downloaded by the operator, credited per model.

## Findings

### F1. Who runs Sketchfab, and the date of its terms

- https://sketchfab.com/terms (read 2026-11-09):
  > Effective October 27, 2026

  > KitBash SF Operations LLC. (“Sketchfab,” “we” or “us”) is pleased to provide you with access to the Sketchfab platform,

  and on amendments (section 12): "Registered Users will receive notification of forthcoming change
  one (1) month before the Agreement as amended is effective."
- https://sketchfab.com/licenses (read 2026-11-09; no date shown; footer "© 2026, KitBash SF
  Operations LLC."), definition 1.9 of its Standard licence agreement:
  > Sketchfab is a division of Epic Games, Inc. (“Epic”).

  The two pages disagree on the owner; the licence agreement looks not yet updated.
- Epic's announcement (https://www.epicgames.com/site/news/kitbash-acquires-artstation-and-sketchfab)
  answered with a robot check (HTTP 403); not worked around. The 2026-11-08 scan read it as
  announced on 10 August 2026.

### F2. The licences a Sketchfab model can carry

The public licence list, https://api.sketchfab.com/v3/licenses (read 2026-11-09), gives each label
and its stated requirement:

| Slug | Label | Requirement as stated | Usable here? |
|---|---|---|---|
| `cc0` | CC0 Public Domain | "Credit is not mandatory. Commercial use is allowed." | yes |
| `by` | CC Attribution | "Author must be credited. Commercial use is allowed." (links CC BY 4.0) | yes, with credit |
| `by-sa` | CC Attribution-ShareAlike | "Modified versions must have the same license." | only if our changed version stays CC BY-SA |
| `by-nd` | CC Attribution-NoDerivs | "Modified versions can not be distributed." | no (we change models) |
| `by-nc`, `by-nc-sa`, `by-nc-nd` | NonCommercial | "No commercial use." | no |
| `free-st`, `st` | Free Standard, Standard | "Under basic restrictions, use worldwide, on all types of media, commercially or not, and in all types of derivative works" | no (below) |
| `ed` | Editorial | "Use only in connection with events that are newsworthy or of public interest" | no |

**Why Standard is out**, from the licence agreement on https://sketchfab.com/licenses: section 2.1
grants a "nonexclusive, non-sublicensable, nontransferable, worldwide, revocable, paid, Single-Seat
license", and section 2.2 forbids, among other things, to

> (b) sell, license, distribute or otherwise make available the Licensed Material as a stand-alone file (or group of files) or in a way that allows third parties to use, download, extract or access the Licensed Material as a stand-alone file (or group of files);

A glTF file in a public repository and an open data folder is exactly that. The page's plain summary
says Editorial assets "cannot be used for any commercial or promotional use" and, in its section 2.4,
"shall not otherwise be altered under any circumstances."

### F3. How Sketchfab's terms bind a download

From https://sketchfab.com/terms (effective 27 October 2026):

- **The licence at the time of download governs**, 4.2.2:
  > By downloading User Content made available by other Users, you agree to adhere to the terms of the license that applies at the time of download.
- **But 4.1 also says "most-current version"**:
  > You agree that your use of any User Content downloaded from the Services will always be subject to the most-current version of the License Agreement or Creative Commons license, as applicable (additional details of which are provided below); accordingly, you agree that your continued use of any such User Content constitutes your acceptance of the terms of any new version of the License Agreement or Creative Commons license, applicable to such User Content, as we may update from time to time.

  The two clauses pull different ways. For CC material the licence itself settles it: CC BY 4.0
  grants an "irrevocable license" (section 2(a)(1) of the legal code,
  https://creativecommons.org/licenses/by/4.0/legalcode.txt, Last-Modified 11 September 2023), CC
  licences are versioned (a later version is a different licence the artist would have to choose),
  and Sketchfab's own help centre says the licence received prevails (F6). Sketchfab is not the
  licensor of an artist's CC model and cannot rewrite its terms.
- **Downloads are authenticated**, 4.2.4:
  > Users accessing and downloading User Content through the download API operated by Sketchfab shall be authenticated and it is expressly agreed that: (i) the use of the download API is governed by these Terms and each User’s plan; and (ii) the authenticated User accessing and downloading the User Content shall be responsible for complying with the terms of the license governing said User Content.
- **Account holders are adults**, 1.4: "THE SERVICES ARE NOT INTENDED FOR CHILDREN UNDER 18" and users
  represent they are "more than 18 years of age". An AI agent cannot create or use an account (this
  project's safety rules forbid an agent creating accounts), so downloads are an operator step.
- **No harvesting to redistribute**, section 5 (restrictions):
  > Except where explicitly allowed by this Agreement, collect, aggregate, mine, scrape, or otherwise use any Sketchfab Content or User Content for the purposes of publishing, selling, distributing or otherwise making the content available to others to use, download or copy; or

  Section 4.2.2 explicitly allows CC licensing, and CC BY allows sharing, so redistributing a model
  we chose under its CC licence is within that exception. Bulk harvesting many models into a
  collection is the use this clause is aimed at.
- **Removal**, 4.6: Sketchfab may "remove User Content from the Services and terminate any licenses
  thereto". For a CC model the licensor is the artist, not Sketchfab, and CC BY 4.0 section 6(c)
  says a licensor ceasing to distribute "will not terminate this Public License".

### F4. The NoAI tag

**The terms**, section 15(a):

> You are able to tag your projects containing your User Content with “NoAI” if you would like such content to be prohibited from use with AI. This tag will not be applied to your uploads or your User Content by default; you must actively designate your projects containing your User Content for the “NoAI” tag to apply. Any User Content that has been tagged, labeled, or otherwise marked “NoAI” via the functionality provided by the platform will be known as “NoAI Content.”

> For purposes of this Agreement, “Generative AI Programs” means artificial intelligence, machine learning, deep learning, neural networks, or similar technologies designed to automate the generation of or aid in the creation of new content, including but not limited to audio, visual, or text-based content. You shall not collect, aggregate, mine, scrape, or otherwise use NoAI Content (i) in datasets utilized by Generative AI Programs; (ii) in the development of Generative AI Programs; or (iii) as inputs to Generative AI Programs.

The same restriction appears in section 5's list of things users may not do.

**Sketchfab's own explanation**, help article "Generative AI Policies, Tagging, and NoAI Protection",
https://help.sketchfab.com/en/articles/16152133-generative-ai-policies-tagging-and-noai-protection
("Written by Elaine", "October 8, 2026"):

> If you have made your model available under a Creative Commons license, the Sketchfab Terms of Use (which govern the use of the NoAI tag) will not apply to further distribution of your model outside of the Sketchfab platform. This means that if you make your model available under a Creative Commons license and use the NoAI tag, only the CC license applies once the model is distributed outside of Sketchfab.com. The NoAI tag does not stay with models when they leave the platform.

> You will need to use the Standard License if you would like to distribute your model for free and ensure the NoAI tag continues to protect your work after it is downloaded.

It quotes Creative Commons' FAQ that platform terms "do not form part of the license for the work
and may not be applicable to distributions of the work outside of the platform." (A web search
summary claimed the article says the tag can no longer be applied to new CC uploads; the article as
read says no such thing.)

**How NoAI shows in the data** (API records read 2026-11-09):

- As a tag: `epilogueronin`'s "Diesel generator"
  (https://api.sketchfab.com/v3/models/7a0b72c7d01944a8aafab59d44eb120e), licence `by`, carries the
  tag `noai` among "power generator electricity substancepainter substance industrial".
- As a statement only: Glowbox 3D's "Water Heater (used)"
  (https://api.sketchfab.com/v3/models/f1a7274c1a4c4d849cb2e67859763532), licence `by`, published
  2020-11-04, has **no** `noai` tag, but its description reads:
  > **NoAI: This model may not be used in datasets for, in the development of, or as inputs to generative AI programs.**

  Whether a statement in a description is "otherwise marked "NoAI" via the functionality provided by
  the platform" is unclear; it is plainly the artist's wish either way.

**What it means legally, as far as the sources go:**

1. **Copyright:** the CC BY 4.0 licence has no NoAI condition. Creative Commons' preamble to the
   legal code treats such wishes as requests: "A licensor may make special requests, such as asking
   that all changes be marked or described. Although not required by our licenses, you are
   encouraged to respect those requests where reasonable."
2. **Contract:** the person who downloads from Sketchfab agrees to its terms, including section 15(a).
   Sketchfab says the terms do not follow the model once it is distributed outside the platform, but
   that speaks to later recipients, not plainly to the downloader's own use.
3. **Our pipeline:** an AI coding agent is a "neural network" technology "designed to automate the
   generation of or aid in the creation of new content". If such an agent opens a model's files,
   reads its glTF, or looks at its textures to check for brands, that is arguably using the content
   "as inputs to Generative AI Programs". A conversion script a person runs, with no AI reading the
   file, arguably is not. This is a reading, not a settled answer.

### F5. AI-generated uploads and the uploader's own rights

- Section 4.4(vi): uploaders whose content is licensed for redistribution and "was created using
  Generative AI Programs" must apply "the “CreatedWithAI” tag"; section 15(b) repeats it. The help
  article adds that "some AI-generated models may still appear as Human Created if they have not been
  properly flagged by the creator."
- Section 4.1 and 4.4: the uploader alone is responsible for the rights in what they upload ("you
  own or otherwise control all of the rights to your User Content"). Sketchfab does not check. Even
  for the Standard licence it sells, its licence agreement says "SKETCHFAB MAKES NO REPRESENTATIONS OR
  WARRANTIES REGARDING LICENSOR’S RIGHT OR AUTHORITY TO GRANT ANY RIGHTS IN OR TO ANY LICENSED
  MATERIAL". A CC BY label on a replica of a branded appliance, or on fan art of someone else's
  character, licenses only what the uploader owned.

### F6. How per-model credit must look

- **Sketchfab's pattern**, help article "Crediting users for 3D model downloads",
  https://help.sketchfab.com/en/articles/16152215-crediting-users-for-3d-model-downloads ("Written by
  Elaine", "October 7, 2026"):
  > The simplest way to credit a Sketchfab user for the free downloadable 3D model they've offered and you have used in your own work is to include some text and links next to your new work.

  > "This work is based on Model 2 by Sketchfab licensed under CC BY 4.0."

  and:
  > If the model author changes the license after you download it, that's ok, but the license you received it under will always prevail.

  > By downloading a CC model from a Sketchfab user, you agree to credit that user wherever you publicly reuse their work.
- **What CC BY 4.0 itself requires**, section 3(a)(1): keep the creator's identification "in any
  reasonable manner requested by the Licensor", any copyright notice, a notice referring to the
  licence and to the disclaimer of warranties, "a URI or hyperlink to the Licensed Material to the
  extent reasonably practicable"; "indicate if You modified the Licensed Material and retain an
  indication of any previous modifications"; and the licence's text or link. Section 3(a)(2): "in any
  reasonable manner based on the medium, means, and context".
- **What a download contains** (help article "Downloading 3D models",
  https://help.sketchfab.com/en/articles/16152217-downloading-3d-models, "October 7, 2026"): "the
  original file that the author uploaded", "a transcoded glTF version", "various GLB versions" and "a
  transcoded USDZ version". Whether the zip carries a licence or credit file was not checked (it
  needs a download).
- **A chain to credit, seen in practice:** Gazebo Fuel's Open Robotics `Oven` is built on Francesco
  Coldesina's Sketchfab "gas oven" (CC BY 4.0, published 2017-10-28), and Khronos's
  `CommercialRefrigerator` on Sean Thomas's Sketchfab "Commercial Fridge"; both credit the Sketchfab
  original, as [`2026-11-09-gazebo-fuel-models.md`](2026-11-09-gazebo-fuel-models.md) and
  [`2026-11-09-reference-3d-collections.md`](2026-11-09-reference-3d-collections.md) record.

## The per-model checklist (what each pick must pass before download)

1. Licence label is `cc0` or `by` (record it and the date; screenshot or save the API record).
2. No `noai` tag, and no NoAI statement in the description.
3. No CreatedWithAI tag, and nothing in the description saying it was generated (the scan found one
   "generated by the state-of-the-art 3D GenAI tool, Rodin Gen-1").
4. Not a replica of a branded or manufacturer product (no maker in the title, tags or description;
   no logo in the thumbnails), and not an upload by a manufacturer or distributor.
5. Not fan art of someone else's character, vehicle or world.
6. The author's display name and profile link recorded for the credit.
7. After download, a look at every texture for logos, labels or printed names.

## What is still unknown, and what would settle it

1. **Whether a NoAI statement in a description (no tag) is "NoAI Content"** under section 15(a).
   Settled by asking Sketchfab support (free; the help article names its email and chat), or simply
   avoided by treating both as NoAI, as recommended.
2. **Whether an AI agent running or supervising a conversion uses a model "as inputs to Generative
   AI Programs".** Settled only by a lawyer or by Sketchfab's written answer. Avoided by not using
   NoAI models.
3. **Whether a Sketchfab download carries a licence file.** Settled by the first download (an
   operator step); keep whatever comes with it beside the converted model.
4. **The exact acquisition terms** (Epic's announcement refused automated reading). Not needed: the
   current terms name KitBash, and CC licences already granted by artists do not depend on who runs
   the site.

## Sources I could not reach

- https://www.epicgames.com/site/news/kitbash-acquires-artstation-and-sketchfab: robot check (HTTP
  403). Wanted: the acquisition date and whether licences changed.
- https://sketchfab.com/blogs/community/restricting-generative-ai-use-of-free-models and
  https://sketchfab.com/blogs/community/introducing-the-noai-createdwithai-tags/: both answered with
  "we need to verify that you're not a robot" (HTTP 202 with a script check). Wanted: the history of
  the NoAI tag. The help article covered the substance.

## What it means for this project

### Certain

- CC0 and CC BY 4.0 Sketchfab models may be converted, changed and shipped, CC BY ones with per-model
  credit and a note of changes (F2, F6; CC BY 4.0 section 3).
- Standard, Free Standard, Editorial, NonCommercial and NoDerivatives models may not be shipped in the
  open data folder (F2).
- Downloads require a logged-in adult's account; the account holder answers for licence compliance
  (F3).
- CC BY licenses no trademarks (section 2(b)(2)) and only what the uploader owned (F5).
- CC BY models are not CC0 and must be listed as CC BY 4.0 in `LICENSES.md`.

### Judgement calls

- **Avoid every NoAI model**, tag or statement. This project's conversions are run and checked by AI
  agents, the artists asked plainly, and alternatives exist for every NoAI item the scan found (for
  the tank water heater, model our own; the scan found no other open one).
- **Avoid CreatedWithAI models**: provenance and the uploader's rights are both uncertain.
- **Sketchfab last, not first**: take Poly Haven, Fuel, SimReady and ABO before Sketchfab, and use
  Sketchfab for what only it has (the worn shower room kit, a washer, pumps, an inverter).
- **Hand-picked, operator-downloaded, never harvested**: one model at a time, through the website or
  the download API with his login, honouring section 5.

### How the project would credit it

- **In each converted folder**: a `SOURCE.md`, Sketchfab's own pattern made concrete: "This work is
  based on "Shower Room - Assets" (https://sketchfab.com/3d-models/shower-room-assets-8e13a866ee184bed95bb5b59ddee62cd)
  by Evan Petrov (https://sketchfab.com/EvanPetrov), licensed under CC BY 4.0
  (https://creativecommons.org/licenses/by/4.0/). Downloaded <date>. Converted to glTF by HumanityOS;
  [changes]. Provided as is, without warranties." plus the CC BY 4.0 legal code as `LICENSE.txt` and
  any licence file that came in the download.
- **One file for the set**, `assets/models/LICENSE-sketchfab-cc.md`, listing every model, author,
  link, licence and download date.
- **`data/credits.ron`**: a row `sketchfab_cc` with `licence: "CC BY 4.0 (per model; some CC0)"`, a
  notice such as "3D models by Sketchfab artists, CC BY 4.0, modified; see the list of models and
  authors", `attribution_required: true`, `shown_in: ["Credits"]` (and the item viewer, if it shows
  credits).
- **`CREDITS.md`**: every artist by name, with model and link, under a "3D models" heading;
  **`LICENSES.md`**: an entry beside SimReady's.
- **Never**: Sketchfab's logo or wording suggesting Sketchfab or an artist endorses HumanityOS.

### The question for the operator

**May CC0 and CC BY 4.0 Sketchfab models fill the gaps nothing else covers (shower room, washer,
pumps, inverter), downloaded by you with your Sketchfab account, one at a time, after each passes the
checklist above?**

Recommendation: **yes**, with NoAI and CreatedWithAI models left out entirely, brands and fan art
excluded, every texture checked after download, and each artist credited by name in the model's
folder, `CREDITS.md` and the Credits page.

---

**Again: I am not a lawyer, and this is not legal advice.** It records what the sources said on
9 November 2026, so that a later reader can tell whether the sources changed or this reading was
wrong.
