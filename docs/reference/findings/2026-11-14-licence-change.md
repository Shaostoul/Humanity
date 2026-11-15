# HumanityOS's licence changes from CC0 to plan B: what the new licences ask, and what they cannot do

**Researched 2026-11-14** (every source below was read on 14 November 2026, Pacific time, unless it
says it was carried from an earlier finding; each carries its own date). Read the licence texts
again before relying on this for anything new: they are stable, but the pages around them change.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** What it says the project should do is marked as a judgement
call.

## The question

Now that the operator has chosen plan B of
[`2026-10-25-licence-options.md`](2026-10-25-licence-options.md) (AGPL-3.0 for the code, CC BY-SA
4.0 for the Library and the docs, CC0 kept for plain data), what do those licences ask of the
people who use HumanityOS, which version of the AGPL should the project name, who else must agree,
and what can the change not do?

## The decision

The operator, 14 November 2026, verbatim: "Really sounding like we need to switch from CC0 because,
it keeps getting in the way of various things we could benefit from." To the recommendation of plan
B: "Let's do it." On the releases already made staying CC0: "I'm not really worried about old
releases staying CC0. We're going to be developing so much that no one would want to save an old
pre-release build."

What "gets in the way" was recorded in earlier findings: CC0 shuts share-alike material
(Wikipedia, Open Source Ecology) out of the Library
([`2026-10-25-open-source-ecology.md`](2026-10-25-open-source-ecology.md)), and SignPath
Foundation's free code-signing certificate needs "an OSI-approved Open Source license", which CC0 is
not ([`2026-11-14-app-distribution-costs.md`](2026-11-14-app-distribution-costs.md), section 1.6).

## Short answer

- **Code: GNU AGPL 3.0, "or any later version"** (SPDX `AGPL-3.0-or-later`, root `LICENSE`).
  Anyone may use, change, share and sell it; whoever runs a changed version that people use over a
  network must offer those people its source (finding 1). "Or later" because the project has no
  contributor agreement, so "-only" would freeze it on version 3 unless every contributor could be
  found and asked (findings 2 and 3). The OSI approved the AGPL-3.0 in 2008 (finding 4), which
  meets SignPath's licence rule for the code.
- **Library and docs: CC BY-SA 4.0** (`docs/LICENSE`, and `data/library/LICENSE`, which ships).
  Sharing an adaptation requires credit, a licence link, a note of changes, and the same licence or
  a compatible one (finding 5). Wikipedia's article text is CC BY-SA 4.0 and may now be copied into
  the Library on those terms (finding 7).
- **Plain data and the Humanity Accord: CC0 1.0**, unchanged (`data/LICENSE`, `docs/accord/LICENSE`;
  the Accord as a judgement call, below).
- **What the change cannot do:** it cannot take back anything already released under CC0 (finding
  6); it binds only the parts that carry human authorship (finding 9); and no licence here covers
  the name, which now has its own policy (finding 8,
  [`docs/legal/trademark-policy.md`](../../legal/trademark-policy.md)).
- **Nobody else's consent is needed:** every human author in the git history is the operator; the
  only co-authors are Claude models, whose maker assigns any rights in outputs to the user (finding
  10).

## Findings

### 1. What the AGPL-3.0 asks of someone who runs a changed server

- Source: https://www.gnu.org/licenses/agpl-3.0.txt ("Version 3, 19 November 2007"; server
  Last-Modified 30 September 2017; read 2026-11-14, SHA-256 `0d96a4ff...079abcb0`, copied byte for
  byte into the root `LICENSE`). gnu.org's robots.txt was read first: nothing under `/licenses/` is
  barred, for any reader.
- Section 13: "if you modify the Program, your modified version must prominently offer all users
  interacting with it remotely through a computer network (if your version supports such
  interaction) an opportunity to receive the Corresponding Source of your version by providing
  access to the Corresponding Source from a network server at no charge, through some standard or
  customary means of facilitating copying of software."
- Section 13, second paragraph: "you have permission to link or combine any covered work with a work
  licensed under version 3 of the GNU General Public License into a single combined work, and to
  convey the resulting work."
- Section 11: "Each contributor grants you a non-exclusive, worldwide, royalty-free patent license
  under the contributor's essential patent claims, to make, use, sell, offer for sale, import and
  otherwise run, modify and propagate the contents of its contributor version."
- Section 15 begins "THERE IS NO WARRANTY FOR THE PROGRAM, TO THE EXTENT PERMITTED BY APPLICABLE
  LAW." Section 16 begins "IN NO EVENT UNLESS REQUIRED BY APPLICABLE LAW OR AGREED TO IN WRITING WILL
  ANY COPYRIGHT HOLDER, OR ANY OTHER PARTY WHO MODIFIES AND/OR CONVEYS THE PROGRAM AS PERMITTED
  ABOVE, BE LIABLE TO YOU FOR DAMAGES". (CC0 disclaimed warranties but limited no liability, as
  [`2026-10-30-wip-warning-and-liability.md`](2026-10-30-wip-warning-and-liability.md) found; the
  AGPL does both.)
- The licence's own advice for a network program ("How to Apply These Terms"): "if your program is
  a web application, its interface could display a "Source" link that leads users to an archive of
  the code."

### 2. "Or any later version", and the proxy

- Source: the same text, section 14.
- "If the Program specifies that a certain numbered version of the GNU Affero General Public
  License "or any later version" applies to it, you have the option of following the terms and
  conditions either of that numbered version or of any later version published by the Free Software
  Foundation."
- On what a later version may be: "Such new versions will be similar in spirit to the present
  version, but may differ in detail to address new problems or concerns."
- The third option: "If the Program specifies that a proxy can decide which future versions of the
  GNU Affero General Public License can be used, that proxy's public statement of acceptance of a
  version permanently authorizes you to choose that version for the Program."
- And: "no additional obligations are imposed on any author or copyright holder as a result of your
  choosing to follow a later version."

### 3. The FSF's case for approving later versions at the outset

- Source: Richard Stallman, "For Clarity's Sake, Please Don't Say "Licensed under GNU GPL 2"!",
  https://www.gnu.org/licenses/identify-licenses-clearly.html (page dated 22 January 2022; read
  2026-11-14). It is the FSF's own argument, so it argues one side.
- "Publishing programs with license upgrade permission is vital to avoid incompatibility between
  programs released under different GPL versions."
- On "-only": "What if we release GNU GPL version 4 ten years from now, and by that time 50 others
  have added to your program, releasing their added code under GPL-3.0-only simply because you did?
  You could approve GPL 4 for the program you initially released, but it would be a big job to
  contact the 50 subsequent developers at that time to get their permission for GPL 4 usage of their
  additions."
- It lists the explicit identifiers, among them "AGPL-3.0-only or AGPL-3.0-or-later".

### 4. The OSI approved the AGPL-3.0

- Source: https://opensource.org/license/agpl-v3 (redirected to `/license/agpl-3.0`; read
  2026-11-14). opensource.org's robots.txt bars only `/wp-admin/`.
- "Version 3.0 Submitted: January 30, 2008 Submitter: Stefano Maffulli Approved: March 3, 2008";
  "SPDX short identifier: AGPL-3.0" (the older identifier; the GNU page in finding 3 gives the
  explicit `AGPL-3.0-only` and `AGPL-3.0-or-later`).
- SignPath's rule, carried from
  [`2026-11-14-app-distribution-costs.md`](2026-11-14-app-distribution-costs.md) (read the same day,
  not re-read): "OSS License: The project must use an OSI-approved Open Source license without
  commercial dual-licensing for all components."

### 5. What CC BY-SA 4.0 asks of an adaptation

- Source: https://creativecommons.org/licenses/by-sa/4.0/legalcode.txt (server Last-Modified 11
  September 2023; read 2026-11-14, copied byte for byte into `docs/LICENSE` and
  `data/library/LICENSE`). creativecommons.org's robots.txt bars nothing.
- Section 3(a)(1), when you share the material, in part: retain "identification of the creator(s)
  of the Licensed Material", "a notice that refers to this Public License", "a URI or hyperlink to
  the Licensed Material to the extent reasonably practicable"; "indicate if You modified the Licensed
  Material and retain an indication of any previous modifications"; and "indicate the Licensed
  Material is licensed under this Public License, and include the text of, or the URI or hyperlink
  to, this Public License."
- Section 3(b)(1), for an adaptation: "The Adapter's License You apply must be a Creative Commons
  license with the same License Elements, this version or later, or a BY-SA Compatible License."
- Section 3(b)(3): "You may not offer or impose any additional or different terms or conditions on,
  or apply any Effective Technological Measures to, Adapted Material that restrict exercise of the
  rights granted under the Adapter's License You apply."
- Compatibility with the GPL, carried from
  [`2026-10-25-licence-options.md`](2026-10-25-licence-options.md) finding 3 (read 2026-10-25, not
  re-read): "compatibility with the GPLv3 is one-way only, which means you may license your
  contributions to adaptations of BY-SA 4.0 materials under GPLv3, but you may not license your
  contributions to adaptations of GPLv3 projects under" BY-SA 4.0.

### 6. What stays CC0, and why the old releases cannot change

- Source: https://creativecommons.org/publicdomain/zero/1.0/legalcode.txt (server Last-Modified 11
  September 2023; read 2026-11-14). The repository's old root `LICENSE` is the same text, line for
  line (compared today), and now sits at `data/LICENSE` and `docs/accord/LICENSE`.
- Section 2: the waiver is made "fully intending that such Waiver shall not be subject to
  revocation, rescission, cancellation, termination, or any other legal or equitable action to
  disrupt the quiet enjoyment of the Work by the public as contemplated by Affirmer's express
  Statement of Purpose."

### 7. Wikipedia's text may now go into the Library, on its terms

- Source: https://en.wikipedia.org/wiki/Wikipedia:Copyrights ("This page was last edited on 5
  October 2026, at 15:56 (UTC)"; read 2026-11-14). en.wikipedia.org's robots.txt was read first: it
  names no Claude agent and does not bar this page.
- "Permission is granted to copy, distribute and/or modify Wikipedia's text under the terms of the
  Creative Commons Attribution-ShareAlike 4.0 International License and, unless otherwise noted, the
  GNU Free Documentation License".
- Attribution: "To re-distribute text on Wikipedia in any form, provide credit to the authors either
  by including a) a hyperlink (where possible) or URL to the page or pages you are re-using".
- "If you make modifications or additions to the page you re-use, you must license them under the
  Creative Commons Attribution-ShareAlike 4.0 International License or later."
- Images differ: "Images on Wikipedia are not always covered by the same license as article text."

### 8. No licence here covers the name

- AGPL-3.0, section 7, among the terms an author may add: "e) Declining to grant rights under
  trademark law for use of some trade names, trademarks, or service marks; or". And 7(c):
  "requiring that modified versions of such material be marked in reasonable ways as different from
  the original version".
- CC BY-SA 4.0, section 2(b)(2): "Patent and trademark rights are not licensed under this Public
  License."
- CC0 1.0, section 4(a): "No trademark or patent rights held by Affirmer are waived, abandoned,
  surrendered, licensed or otherwise affected by this document."

### 9. A copyleft licence binds only what copyright reaches

- Carried from [`2026-10-25-licence-options.md`](2026-10-25-licence-options.md) finding 6 (the US
  Copyright Office's Copyright and Artificial Intelligence, Part 2, January 2025; read 2026-10-25,
  not re-read): "in many circumstances these outputs will be copyrightable in whole or in
  part--where AI is used as a tool, and where a human has been able to determine the expressive
  elements they contain. Prompts alone, however, at this stage are unlikely to satisfy those
  requirements."

### 10. Who wrote HumanityOS, by the git history

- Source: `git log` of this repository on 2026-11-14 (6,621 commits from 16 January 2026), and
  GitHub's contributor list for `Shaostoul/Humanity` (`gh api .../contributors`, the same day).
- Commit authors: `Shaostoul <contact@shaostoul.com>` (6,397 commits), `Michael Boisson
  <michael@shaostoul.com>` (222) and `Michael "Shaostoul" Boisson <contact@shaostoul.com>` (2, made on
  GitHub's website): all three are the operator. GitHub lists one contributor, Shaostoul.
- `Co-authored-by` lines name only Claude models (Claude Opus 5.5, Fable 5 and 5.1, Opus 5, 4.8, 4.7
  and 4.6, Sonnet 5 and 4.6, and plain "Claude", all `noreply@anthropic.com`). No `Signed-off-by`
  line, and no pull request from anyone else's fork, was ever merged (the one merged pull request,
  #2, came from a branch of this repository).
- The AI's maker on outputs: Anthropic's Consumer Terms (https://www.anthropic.com/legal/consumer-terms,
  "Effective Oct 8, 2025"; read 2026-11-14, robots.txt allows all): "Subject to your compliance with
  our Terms, we assign to you all of our right, title, and interest—if any—in Outputs."
- Third-party code and media in the tree (vendored crates, fonts, models, recordings, map and star
  data) keep their own licences, listed in `LICENSES.md` and `CREDITS.md`; nothing here relicenses
  them. The 2026-09-06 attribution audit found "no GPL, LGPL, AGPL, CDDL, EPL, SSPL or BUSL anywhere
  in the Rust graph", the strongest term being MPL-2.0.

## What is NOT true, written down so it is not argued again

- **The change does not reach back.** Every version published before 14 November 2026 stays public
  domain for good (finding 6). Someone who wants a HumanityOS with no conditions can take an old
  version; the operator accepted that (the decision above).
- **The AGPL does not bind someone who runs HumanityOS unchanged**, or who changes it for their own
  use with no network users: section 13's duty starts with "if you modify the Program" and runs to
  "users interacting with it remotely" (finding 1).
- **No copyleft licence can bind work that carries no human authorship.** Where the AI wrote a part
  without a human determining its expressive elements, copyright (and so the AGPL or CC BY-SA) may
  not reach it, and anyone could take that part with no conditions (finding 9). How much of
  HumanityOS that is, only a lawyer or a court could say.
- **No licence here covers facts.** The data restates facts, which no copyright covers (Feist, as
  `schemas/source.toml` sets out); CC BY-SA on the Library covers its wording, not the facts in it.
- **Not every share-alike source becomes copyable.** CC BY-NC-SA sources (WHO, GBIF, iNaturalist in
  the registry) carry a non-commercial term that a Library anyone may sell from cannot carry, and
  OpenStreetMap's ODbL is a database licence; they stay `facts` or `fetch`. CC BY was not looked at.
- **GPL-3.0 code can now be built in; GPL-2.0-only code still cannot.** Finding 1's second
  paragraph allows GPLv3 combinations; nothing read today makes GPL-2.0-only code combinable with a
  version 3 licence.
- **The Library's text cannot be copied into the code**, and the code's text cannot be copied into
  the Library: CC BY-SA 4.0 goes into GPLv3 one way only (finding 5), and AGPL code is not CC BY-SA.
- **No licence covers the name or the logo** (finding 8); the trademark policy states the project's
  wishes and does not claim a registered mark.

## What is still unknown, and what would settle it

- **Whether SignPath counts the CC0 data and the CC BY-SA docs in the download as "components"**
  that need "an OSI-approved Open Source license" (finding 4). Settled by asking SignPath Foundation
  when applying. Free.
- **How much of HumanityOS carries human authorship** (finding 9). Only a lawyer, or a court.
- **Whether every Rust dependency may be combined into an AGPL-3.0 program.** The 2026-09-06 audit's
  negative result (no GPL family licence in the graph) was checked against CC0, not the AGPL; MPL-2.0
  and Apache-2.0 compatibility with version 3 were not re-read today. Settled by reading the FSF's
  licence list for each licence family in `Cargo.lock`, a short job.
- **Whether to add AGPL section 7 terms**: 7(e) declining trademark rights, and 7(c) requiring a
  changed version to be marked as different, would put the trademark policy's core into the licence
  notice itself. A drafting choice; the plain licence text is what ships today.
- **The OSE License for Distributive Economics** (unread since 2026-10-25): read it before
  distributing any Open Source Ecology design or file; copying OSE's CC BY-SA wiki text into the
  Library is what the registry now allows.

## Sources not reached

- None refused today. Not fetched: the FSF's licence list (wanted for the dependency check above),
  and Creative Commons' compatible-licences page (its 2026-10-25 reading is carried in finding 5).

## What it means for this project

**Certain (from the sources):**
- The root `LICENSE` is the AGPL-3.0 text; `docs/LICENSE` and `data/library/LICENSE` the CC BY-SA
  4.0 legal code; `data/LICENSE` and `docs/accord/LICENSE` the CC0 1.0 legal code. `LICENSING.md`
  maps every folder; `Cargo.toml` says `AGPL-3.0-or-later` for every crate.
- A changed HumanityOS server run for others must offer its users the source (finding 1); the app's
  Settings > Credits and the website's footer link the source and the licence map.
- Share-alike text from Wikipedia and Open Source Ecology may now be copied into the Library with
  credit, a licence link and a note of changes: `use = "library"` in `data/sources/registry.json`.
  Into the Library only, never into data/ outside it, never into code.
- No outside contributor's consent is needed: there is no outside contributor (finding 10).

**Judgement calls (the operator's to confirm):**
- **"-or-later" over "-only"**, for the reason in finding 3: with no contributor agreement, "-only"
  needs every future contributor found and asked before any move to a version 4. The cost is that a
  version the FSF writes later would apply without anyone here agreeing; section 14 limits it to
  versions "similar in spirit". The proxy option (finding 2) would let the project choose instead,
  at the price of naming a proxy.
- **The Humanity Accord stays CC0**, as an exception to "Library and docs CC BY-SA". The Accord states
  its own licence ("Released under CC0 public domain, freely adoptable by any sovereign nation or
  civilization") as part of its text, every version of it already released is CC0 for good, and a
  constitution meant for any community to adopt gains nothing from conditions. Moving it would mean
  amending that sentence.
- **The project's own media in assets/ (icons, textures, models, sounds, brand art) stays CC0** with
  the data, since plan B named code, Library and docs, and data, and media is closest to data. Shaders
  are code (AGPL).
- **How-to guides (`data/how_to/`) stay CC0** with the data, so share-alike text may not be copied into
  them; if OSE build steps should ever be adapted into a How-to, that folder would have to join the
  Library's licence first.

## The operator's answers to the switch's five questions (2026-11-14)

Asked the five questions the switch left open, each with a recommendation, the operator answered:
"Let's go with your recommendations." So:

1. **AGPL-3.0-or-later**, not -only: no contributor agreement exists, so "-only" would need every
   future contributor found and asked before any move to a version 4; AGPL section 14 limits a
   later version to one "similar in spirit".
2. **The Humanity Accord stays CC0** (docs/accord/LICENSE), as its own text releases it for any
   nation to adopt; every released version is CC0 for good anyway.
3. **The project's own media and the How-to guides stay CC0 with the game data**; the How-tos move
   to the Library's licence only if Open Source Ecology build steps are ever adapted into them.
4. **SignPath:** whether the CC0 data and CC BY-SA docs count as separate components under its
   "OSI-approved for all components" rule is asked of SignPath when applying (free).
5. **AGPL section 7 additional terms** (declining trademark rights, section 7(e); marking changed
   versions, section 7(c)) are added to the licence notice as a later drafting step; the plain AGPL
   text ships now.
