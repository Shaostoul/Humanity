# Digital Public Goods application: the evidence, indicator by indicator

**Prepared 2026-11-20. Not submitted.** The Digital Public Goods Alliance (DPGA) takes an
application only from a person authorised to speak for the project, so the operator submits it.
This file holds everything the application form asks, the evidence a reviewer can open for each
answer, and an answer for each field written so a reviewer can check it in under a minute. Why we
are applying, and the first check of the standard:
[the 2026-11-18 finding](../reference/findings/2026-11-18-digital-public-goods.md).

Each indicator below has five parts: the standard's own words, what the form asks, where the
evidence is, an answer ready to paste, and a verdict. A verdict is **ready**, **ready, with a limit
said** (the answer is true and names what the code does not do yet), or **thin** (a reviewer could
fairly fail it today; what would close it is named).

Every repository path below is public at `https://github.com/Shaostoul/Humanity/blob/main/`
followed by the path. The answers ready to paste use full addresses, because a reviewer reads them
outside the repository.

## Before you start: what is thin, and what only you can answer

### The thin spots

| Indicator | What a reviewer could fail | What closes it |
|---|---|---|
| 9b, illegal content | The content policy is merged ([docs/legal/content-policy.md](../legal/content-policy.md), 2026-11-20, in the Library and linked from Settings > Safety in both apps, the website's footer and its rules page), and it says plainly what the app does not do yet. "Hide and keep" is on the Reports page in both apps (2026-11-20, [docs/design/hide-and-keep.md](../design/hide-and-keep.md) rungs 1 to 5): the post and its files out of view at once, one copy sealed to the server's owner for the year a CyberTipline report must be preserved, nobody able to view it, the owner alone able to export, hold, destroy or restore it, every step signed. It needs the server's owner to be set, which is still done on the server itself; until then an operator follows the policy's manual steps. Reports are deleted 90 days after a decision unless they hold a kept copy. No response time has been measured, and the policy sets no target. | Setting the server's owner in the app (the operator's choice of mechanism, `docs/design/in-app-ops.md`). A report's retention for the child-danger reason is still an open decision (`docs/PRIORITIES.md`, "Report duties"). |
| 9c, harassment | The content policy does not name harassment: it covers threats, blackmail and calls to harm a group of people, and leaves "every other report" to the admins' proportionate choice. Harassment is named in the rules of the project's own server, [united-humanity.us/rules](https://united-humanity.us/rules) ("No harassment, hate speech, doxxing or targeted attacks"), with the moderators' real powers and the order of responses. Those rules are one server's; since 2026-11-20 [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md) applies them to the project's GitHub repository and Discord too, defines harassment, and says how to report and appeal. The Humanity Accord states principles, not rules with consequences. | Closed by the code of conduct, for the project's own spaces. Other people's servers write their own rules; the content policy still leaves harassment to each server's admins. |
| 3 and 4, ownership and dependencies | The Windows download ships two Microsoft files, `dxcompiler.dll` and `dxil.dll`, copied from the Windows SDK (`.github/workflows/build-desktop.yml`), with no licence notice anywhere in the project. The compiler's source is open (its repository's LICENSE.TXT is the "University of Illinois/NCSA Open Source License", read 2026-11-20), and that licence asks that a binary copy carry its copyright notice; the code comments call it MIT. Whether `dxil.dll`, as shipped in the SDK, carries the same terms is not settled (`docs/reference/attribution-audit-2026-09-06.md`, item 7). | The dated finding is written, [2026-11-21-dxc-redistribution.md](../reference/findings/2026-11-21-dxc-redistribution.md), and it is negative for the copies shipped so far: they fall under the Windows SDK licence and are not on its redistribution list. DXC's own release zip turned out to carry a Microsoft licence beside the NCSA one (read 2026-11-20), so the project now builds the pair itself from DXC's NCSA source (`.github/workflows/build-dxc.yml`) and ships LICENSE.TXT beside each file, from the next release on. Until that release is out, the answer below should not imply the shipped files are under an open licence. |
| 8, standards | Closed 2026-11-20: the post-quantum code now passes NIST's published ACVP vectors for FIPS 203 and 204 (240 vectors, in the Rust build and on the website, run in CI), as well as the tests across our own implementations. | Nothing; keep the vectors running in CI. |
| 9a, privacy and security | No independent human security audit has been done since the post-quantum switch; the reviews so far are AI reviews and one contributor's review of the cryptography (2026-11-16). Two known limits are open: direct messages have no forward secrecy yet; a private group's server can see its name, members and who posted when. (A third, the wallet key being the recovery phrase's raw entropy, was closed on 2026-11-19: it is now derived under its own label.) | Say it plainly (the answer below does). The first is decided and designed (`docs/design/dm-forward-secrecy.md`); the second is named in the privacy policy. |
| 7, laws | The laws were read by AI from primary sources, not by a lawyer. No DMCA designated agent is registered (none until the project has an address to give, through a fiscal sponsor or a nonprofit of its own). | Say it plainly (the answer below does). |

Not an indicator, but the form asks: the app is English only in practice, it runs on two servers
that are both the maintainer's, and no other organisation uses it yet. The answers say so.

### Questions only you can answer

1. **The owner's name on the form.** Recommended: "Shaostoul and the HumanityOS contributors", the
   words of the copyright line in LICENSING.md and the README. The form keeps the submitter's name
   and contact private (the DPGA's submission guide: "Your name and contact information will
   remain private"); what you write in the answers is published.
2. **"Type of organisation" and "Where is the owner based?"** Yours to pick. The privacy policy
   already says the server is in the United States, and the website says there is no company or
   nonprofit behind the project.
3. **The category.** Recommended: **Open Software** only. The Library and the data could also be
   entered as Open Content and Open Data, but each category is reviewed on its own documentation
   rules, and one category keeps the first application simple.
4. **Which goals to select.** Recommended, in this order: 4, 6, 3, 2, 16 (the evidence is in "The
   Sustainable Development Goals" below). Goal 1, the mission, can be added, but its evidence is
   indirect; the short description can carry it instead.
5. **When to submit.** The content policy is merged (2026-11-20), so every indicator now has an
   answer. Recommended: submit now, with the limits said. Hide and keep reached both apps on 2026-11-20 and
   the 9b removal sentence says so; re-read it before submitting, because an application under
   review cannot be edited.
6. **A response time for reports.** The form asks for "the average response time for assessment
   and/or action". The content policy sets no target. Recommended: say that none has been measured
   because the platform has not launched, and that reports wait for each server's volunteer admins;
   promise no number one person cannot keep.
7. **"Do you own all of the code, content and/or data?"** Recommended: **No**, with the evidence
   under indicator 3 (contributors keep their copyright; other people's work keeps its own open
   licence).

**The operator's answers, 2026-11-20:**

- 3 (Open Software only), 4 (goals 4, 6, 3, 2, 16), 6 (no response time promised) and 7 (No):
  agreed, as recommended.
- 5: submit soon, "maybe today or this weekend", after a few things are wrapped up.
- 1 and 2: he asked for advice. Given: the answers (which are published) carry the copyright line's
  name, "Shaostoul and the HumanityOS contributors"; the private submitter fields may carry his legal
  name, which the DPGA says it keeps private. For the type of organisation, the form's choices (seen 2026-11-20) are Academic, NGO/Non
  Profit, Multilateral, Government, Private and Other: **Other** (no nonprofit is registered, and
  Private reads as a company). Where the owner is based: the
  United States.
- 9c: a project-wide [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md) (2026-11-20) applies the rules of
  united-humanity.us to the GitHub repository and the Discord, names harassment and says how to report
  it and appeal.

## The sources read for this file

Read on 2026-11-20, between about 01:00 and 01:35 Pacific time (09:00 to 09:35 UTC), each site's
robots.txt first, in a step of its own.

| Source | Its own date | Read |
|---|---|---|
| The standard's page, https://www.digitalpublicgoods.net/standard | not dated | It names the nine indicators. The requirement text in its panels loads through the site's `/api/` path, which its robots.txt disallows, so the panels were not read. |
| The standard's text, `standard.md` in https://github.com/DPGAlliance/DPG-Standard (through GitHub's API) | Its header still says version 1.1.4, 2021-01-04; the file was last changed 2025-03-20; the changelog's latest entry is 1.1.7, 2025-06-26 | The review policy links this file as the standard every application is assessed against. All indicator quotations below are from it. |
| The form's questions, `standard-questions.md`, same repository | last changed 2025-06-24 | The questions the form at app.digitalpublicgoods.net asks, field by field. |
| The review policy, `docs/dpg-review-policy.md` in https://github.com/DPGAlliance/dpg-resources | last changed 2025-01-23 | Who may submit, how review works, renewal. |
| The submission guide, https://www.digitalpublicgoods.net/digital-public-goods/submission-guide; the FAQ, https://www.digitalpublicgoods.net/digital-public-goods/faqs; the eligibility tool, https://www.digitalpublicgoods.net/eligibility | not dated (footer 2026) | The evidence each indicator wants, and the steps. |
| The DPG Wiki: "2. Open Licensing" (edited 21 October 2026), "3. Clear Ownership" and "9. Do No Harm by Design" (both edited 16 May 2025), https://github.com/DPGAlliance/dpg-resources/wiki | as stated | github.com's robots.txt was read first; it does not disallow wiki pages. |
| The UN's goal pages, https://sdgs.un.org/goals/goal1 to goal16 (goals 1, 2, 3, 4, 6, 7 and 16) | not dated | The target texts quoted below. Read 11 seconds apart (the site asks for 10). |
| `LICENSE.TXT` of https://github.com/microsoft/DirectXShaderCompiler (through GitHub's API) | not dated; the latest release is v1.9.2609, 2026-09-29 | For the Windows DLLs (indicators 3 and 4). |

The standard, its questions and its wiki are licensed CC BY-SA 4.0 (each repository's licence), and
are quoted in this file with that credit: *Digital Public Goods Standard, by the Digital Public Goods
Alliance, CC BY-SA 4.0.* This file is under CC BY-SA 4.0 too, like the rest of `docs/`.

Two differences between the DPGA's own documents, so they surprise nobody:

- `standard.md`, indicator 2, still says licences without commercial reuse are accepted for
  content. The Open Licensing wiki page (21 October 2026) accepts only CC BY, CC BY-SA and CC0 for
  content, in line with changelog 1.1.7 ("requiring only fully open-licensed content collections and
  data sets"). Every licence of ours is on both lists.
- `standard.md`, indicator 9b, says "inappropriate and illegal content"; the form and the wiki say
  "inappropriate, misleading, and/or illegal content". Answer the form's wording.

## The form's first page: general information

The form asks these before the indicators, and asks that the details and the code be in English
(they are).

| Field | Answer |
|---|---|
| Solution name | HumanityOS |
| Aliases | HOS (confirm; the repository is named Humanity) |
| Solution category | Open Software (question 3 above) |
| Short description (published on the registry) | A free, open-source app for chat, planning, trade and shared decisions, with a library of practical guides on food, water, first aid and repair. Your identity is a key on your own device: no sign-up, post-quantum encrypted messages, and anyone can run a server. |
| Website | https://united-humanity.us |
| Alternate email address | yours |
| Where the source is hosted | https://github.com/Shaostoul/Humanity, and the project's own copy, which follows every change: https://git.united-humanity.us/shaostoul/Humanity |

## Indicator 1: Relevance to the Sustainable Development Goals

**The standard:** "Digital public goods must demonstrate relevance to advancing the Sustainable
Development Goals (SDGs)."

**The form asks:** which goals are most relevant, and for each, a couple of sentences relating the
solution to that goal's targets.

**Evidence and answers:** the section "The Sustainable Development Goals" below, one goal at a
time, each tied to features that exist today.

**Verdict: ready.**

## Indicator 2: Use of approved open licenses

**The standard:** "Digital public goods must demonstrate the use of an approved open license. For
open-source software, only OSI approved licenses are accepted. For open content collections the use
of a Creative Commons license is required. DPGs are encouraged to use a license that allows for both
derivatives and commercial reuse (CC-BY and CC-BY-SA), or dedicate content to the public domain
(CC0); licenses that do not allow for commercial reuse (CC-BY-NC and CC-BY-NC-SA) are also accepted.
For open data, an Open Data Commons approved license is required. See The full license list for
reference."

**The form asks:** which licences, and links to the public pages that hold them.

**Evidence:**

- The folder-by-folder map: [LICENSING.md](../../LICENSING.md). The full texts:
  [LICENSE](../../LICENSE) (the code, AGPL-3.0-or-later), [docs/LICENSE](../LICENSE) (the Library
  and documentation, CC BY-SA 4.0), [data/LICENSE](../../data/LICENSE) (the data, CC0 1.0),
  [docs/accord/LICENSE](../accord/LICENSE) (the Humanity Accord, CC0 1.0).
- Other people's work that ships with the app keeps its own licence, each with its obligation:
  [LICENSES.md](../../LICENSES.md) and [CREDITS.md](../../CREDITS.md).
- The DPG Wiki's list of accepted licences (read 2026-11-20) includes AGPL-3.0, CC-BY-SA-4.0,
  CC0-1.0, ODbL-1.0, CC-BY-SA-3.0-IGO and CC-BY-4.0, which covers ours and the shipped outside data.
- The Rust dependencies: `cargo metadata`, read 2026-11-20, lists 843 third-party crates, and every
  one declares an open licence (most MIT or Apache-2.0; also BSD, ISC, Zlib, MPL-2.0, Unicode-3.0,
  CC0, BSL-1.0, CDLA-Permissive-2.0 for a certificate list, and OFL-1.1 with the Ubuntu Font Licence
  for the bundled fonts). No crate is proprietary. Since 2026-11-20 the 746 of them linked into the
  six programs a release ships carry their copyright and licence notices with every download, in
  [THIRD-PARTY-NOTICES.txt](../../THIRD-PARTY-NOTICES.txt) (generated from `Cargo.lock` by
  `scripts/gen-third-party-notices.js`, shown in the app under Settings > Credits), and
  `scripts/tests/third-party-notices-fresh.test.js` fails in CI when that file falls behind
  `Cargo.lock` or a linked package arrives under a licence nobody has reviewed. The review:
  [the dependency licences finding](../reference/findings/2026-11-20-dependency-licences-and-agpl.md).

**Answer, ready to paste:**

> HumanityOS's code is licensed AGPL-3.0-or-later (OSI approved): https://github.com/Shaostoul/Humanity/blob/main/LICENSE.
> Its Library and documentation are CC BY-SA 4.0: https://github.com/Shaostoul/Humanity/blob/main/docs/LICENSE.
> Its data and the Humanity Accord are CC0 1.0: https://github.com/Shaostoul/Humanity/blob/main/data/LICENSE.
> LICENSING.md maps every folder to its licence: https://github.com/Shaostoul/Humanity/blob/main/LICENSING.md.
> Outside data shipped with the app keeps its own open licence (OpenStreetMap ODbL 1.0, the Gaia
> star catalogue CC BY-SA 3.0 IGO, some planet textures CC BY 4.0), each listed with its obligation in
> https://github.com/Shaostoul/Humanity/blob/main/LICENSES.md. Outside code keeps its own open licence
> with its notice in every download: the Rust libraries in
> https://github.com/Shaostoul/Humanity/blob/main/THIRD-PARTY-NOTICES.txt, and the DirectX Shader
> Compiler (NCSA licence), which the project builds from Microsoft's open source. Every release before
> 14 November 2026 was dedicated to the public domain under CC0 and stays there.

**Verdict: ready.** Since v0.1566.0 (2026-11-20) the Windows DLLs ship as our own build of DXC's open source with its NCSA licence beside each file (the thin spots table).

## Indicator 3: Clear Ownership

**The standard:** "Ownership of assets that the digital public good produces must be clearly defined
and documented. For example, through copyright, trademark or other publicly available information."

**The form asks:** who owns it (names of people if there is no organisation); links to the
copyright, trademark or ownership documents; the type of organisation; the country; whether you own
all the code, content and data; and if not, evidence of the right to redistribute it.

**Evidence:**

- [LICENSING.md](../../LICENSING.md), "Who holds the copyright": "Copyright 2026 Shaostoul and the
  HumanityOS contributors." Each contributor keeps the copyright in what they wrote; there is no
  copyright assignment and no contributor licence agreement, so nobody can relicense the whole
  project alone.
- The same line closes the [README](../../README.md), the repository's front page. Every page of the
  website links LICENSING.md from its footer (`web/shared/shell.js`).
- The name: [docs/legal/trademark-policy.md](../legal/trademark-policy.md) (a fork is welcome under
  its own name; the name means the software this project publishes).
- How a contribution is licensed: LICENSING.md, "Contributing", and
  [CONTRIBUTING.md](../../CONTRIBUTING.md), "The licence of what you contribute": a contribution is
  accepted under its folder's licence.
- The DPG Wiki ("3. Clear Ownership") asks software projects to name the accountable person or
  organisation on the licence, the website and the repository's README. The README and LICENSING.md
  do; the website does through its footer link.

**Answer, ready to paste** (owner field: "Shaostoul and the HumanityOS contributors"):

> HumanityOS has no company or nonprofit behind it. The copyright is held by Shaostoul, the
> maintainer, and the project's contributors, each in what they wrote, as stated in
> https://github.com/Shaostoul/Humanity/blob/main/LICENSING.md ("Who holds the copyright") and at the
> end of the README. There is no copyright assignment, so no one, the maintainer included, can
> relicense the project alone. The name is covered by a published policy:
> https://github.com/Shaostoul/Humanity/blob/main/docs/legal/trademark-policy.md.

**"Do you own all of the code, content and/or data?"** No.

**Answer to "Do you have the right to re-distribute?", ready to paste:**

> Yes. Contributions are accepted under the licence of the folder they land in (AGPL-3.0-or-later,
> CC BY-SA 4.0 or CC0; LICENSING.md, "Contributing"), so every contributor grants the same licence
> the project publishes. Other people's work (map data, star catalogues, models, recordings, fonts,
> the emoji pictures, vendored code) is redistributed under its own open licence, each listed with
> its obligation in https://github.com/Shaostoul/Humanity/blob/main/LICENSES.md and
> https://github.com/Shaostoul/Humanity/blob/main/CREDITS.md, with the licence file beside each one.
> The Rust libraries built into the programs carry their notices in
> https://github.com/Shaostoul/Humanity/blob/main/THIRD-PARTY-NOTICES.txt, and the DirectX Shader
> Compiler in the Windows download is built by the project from Microsoft's open source (NCSA), its
> licence beside each file.
> Much of the code is written with AI assistance under the maintainer's direction and review;
> LICENSING.md says plainly that a licence covers only the parts that carry human authorship.

**Verdict: ready.** The Windows DLLs carry their licence since v0.1566.0 (2026-11-20).

## Indicator 4: Platform Independence

**The standard:** "When the digital public good has mandatory dependencies that create more
restrictions than the original license, proving independence from the closed component(s) and/or
indicating the existence of functional, open alternatives that can be used without significant
changes to the core product is required."

**The form asks:** the core technologies; whether any closed component creates a proprietary
dependency; if so, the open alternatives and how little has to change to use them.

**Evidence:**

- The stack, in the README's "How it works under the hood": Rust, wgpu, egui, hecs, rapier3d and
  kira for the desktop app; Rust, axum, tokio and SQLite for the server (the relay); plain HTML,
  JavaScript and CSS for the website, with no build step; RustCrypto's `ml-dsa` and `ml-kem` and the
  vendored `@noble/post-quantum` for the cryptography (`docs/reference/cryptography.md`).
- The latest release, v0.1565.0 (2026-11-20), ships the desktop app for Windows, macOS (Apple
  silicon and Intel) and Linux, and the server for Linux:
  https://github.com/Shaostoul/Humanity/releases. One source tree builds them all.
- Graphics: on Linux the app draws through Vulkan, for which open drivers exist; on Windows through
  DirectX 12 and on macOS through Metal, the systems' own interfaces (`src/renderer/mod.rs`).
- The Windows DLLs are optional: the app loads `dxcompiler.dll` and `dxil.dll` only when both sit
  beside it and otherwise compiles its shaders with the compiler Windows already has, more slowly
  (`src/renderer/mod.rs`, `dxc_dlls_beside_exe`). Linux and macOS builds never use them.
- Optional outside services, none needed to run: a Solana server and CoinGecko for the optional
  wallet; the browser makers' push services for optional notifications; an S3-compatible bucket as
  one of three backup destinations (a folder, or another HumanityOS server, are the others;
  `docs/admin/continuous-backup.md`). No AI model ships in the app.

**Answer, ready to paste** (closed components: **No**):

> Core technologies: Rust (desktop app and server), wgpu, egui and rapier3d (desktop), axum, tokio
> and SQLite (server), plain HTML/JavaScript/CSS (website, no build step), and RustCrypto's ML-DSA
> and ML-KEM crates. Every dependency is open source (843 Rust crates, all under open licences such
> as MIT, Apache-2.0, BSD and MPL-2.0). The same source builds for Linux, Windows and macOS; on Linux
> the app can run entirely on open components (Vulkan with open drivers), and the server runs on
> Linux with no graphics hardware. On Windows and macOS it uses the operating system's own graphics
> interface. The Windows download includes Microsoft's DirectX Shader Compiler files (the compiler's
> source is open) for faster start-up; the app runs without them. Outside services are optional (a
> Solana node for the optional wallet, browser push services for optional notifications), and no AI
> model is bundled.

**Verdict: ready.** The DLL notice (thin spots table) is the only loose end, and it is a notice,
not a closed dependency.

## Indicator 5: Documentation

**The standard:** "Digital public goods require documentation of the source code, use cases,
and/or functional requirements. For content collections, this should include all
relevant/compatible apps, software, or hardware required to access the content collection, and
instructions regarding how to use it. For software solutions, this should be technical
documentation that would allow a technical person unfamiliar with the project to launch and run the
software. For data projects, this should be documentation that describes all the fields in the set,
and provides context on how the dataset was collected, and how it should be interpreted."

**The form asks:** links to the documentation (for software: developer documents, technical
architecture, user guides).

**Evidence:**

- To run it: the [README](../../README.md), "Get started" (try it in a browser, the desktop
  download, a server); [docs/admin/SELF-HOSTING.md](../admin/SELF-HOSTING.md) (a server from the
  release binary or from source, configuration, backups, becoming the first admin).
- To build and change it: [CONTRIBUTING.md](../../CONTRIBUTING.md), "Quick Start" (clone, build and
  run the server, open the chat, build the desktop app);
  [docs/contributor/00-START-HERE.md](../contributor/00-START-HERE.md) and the architecture and
  module-map documents beside it; [docs/crates/README.md](../crates/README.md) (one card per crate:
  what it owns, its interface, how to test it); [docs/ai/code-map.md](../ai/code-map.md) (one line
  for every source file, generated); [docs/design/cross-platform-dev.md](../design/cross-platform-dev.md)
  (developing on macOS and Linux, after a macOS contributor's build failed and was fixed on
  2026-11-16).
- The data's fields: `schemas/` (35 TOML schemas that define the data files' shapes) and
  [docs/contributor/validate_data.md](../contributor/validate_data.md) (the data validator).
- For people using it: [docs/user/getting-started.md](../user/getting-started.md), the in-app
  Library, and plain-language hints on every control in both apps.

**Answer, ready to paste:**

> Running it: the README's "Get started" covers the browser, the desktop download and a server
> (https://github.com/Shaostoul/Humanity#readme), and
> https://github.com/Shaostoul/Humanity/blob/main/docs/admin/SELF-HOSTING.md takes a server from the
> release binary to live. Building it: https://github.com/Shaostoul/Humanity/blob/main/CONTRIBUTING.md
> (build and run from source), the contributor guide starting at
> https://github.com/Shaostoul/Humanity/blob/main/docs/contributor/00-START-HERE.md, one card per
> crate in https://github.com/Shaostoul/Humanity/blob/main/docs/crates/README.md, and a generated
> map of every source file in https://github.com/Shaostoul/Humanity/blob/main/docs/ai/code-map.md.
> Data: the data files' shapes are defined in schemas/ and checked by a data validator. Users: a
> getting-started guide, the in-app Library, and a plain-language hint on every control.

**Verdict: ready.** A single "your first hour" guide that walks a stranger from clone to a running
server and desktop app would make it plainer still (the 2026-11-18 finding said the same).

## Indicator 6: Mechanism for Extracting Data and Content

**The standard:** "Digital public goods with non-personally identifiable information (PII) design
for possibility of extracting or importing non-PII data and content from the system in a
non-proprietary format."

**The form asks:** whether the solution collects, uses or generates non-PII data or content (yes),
and how it is exported or imported in a non-proprietary format.

**Evidence:**

- `data/`: the reference and game data the app reads, as CSV, TOML, RON and JSON files, loaded at
  start and reloaded while the app runs; a mod is an edit to them (the README's "Layout"; LICENSING.md
  puts them under CC0).
- `data/library/`: the Library as Markdown files with a JSON catalogue.
- The server's public signed objects (proposals, votes, credentials) as JSON over its REST API
  (`/api/v2/objects`, `/api/v2/proposals`; `src/relay/api_v2_objects.rs`,
  `src/relay/api_v2_governance.rs`).
- A person's own data: "Export my data" downloads everything the server holds about them as JSON
  (`docs/outreach/privacy_policy.md`, "Your choices").

**Answer, ready to paste:**

> Yes. HumanityOS's data ships as plain files in open formats: CSV, TOML, JSON and RON (a documented
> open text format) in the data/ folder, and the in-app Library as Markdown with a JSON catalogue, all
> CC0 or CC BY-SA and readable with any text editor. Importing is editing or adding those files; the
> app reloads them while it runs. Public records on a server (proposals, votes, credentials) are
> served as JSON over a REST API, and each person can export everything a server holds about them as
> JSON from the app.

**Verdict: ready.**

## Indicator 7: Adherence to Privacy and Applicable Laws

**The standard:** "Digital public goods must be designed and developed to comply with privacy and
other applicable laws"

The standard's note on 7 to 9: "Evidence for requirements 7-9 can only be given by someone
authorized to speak on behalf of the project. We collect title, name and contact information to
confirm this authority."

**The form asks:** a list of the privacy and other laws the solution complies with, and links to the
privacy policy, terms or other legal documents that show it.

**Evidence:**

- The privacy policy: https://united-humanity.us/privacy (source
  [docs/outreach/privacy_policy.md](privacy_policy.md), checked against the server's code on 7 and
  8 November 2026, with its own "What is not right yet" section).
- The laws behind it, read from primary sources and dated:
  [the privacy notice finding](../reference/findings/2026-11-07-privacy-notice.md) (EU GDPR, UK GDPR
  with the Data Protection Act 2018, California's CCPA/CPRA and CalOPPA, Washington's My Health My
  Data Act, US COPPA);
  [children's online safety rules](../reference/findings/2026-10-10-childrens-online-safety-rules.md)
  (US, UK, EU);
  [report duties for child sexual abuse material](../reference/findings/2026-10-10-report-duties-child-abuse-material.md)
  (18 U.S.C. 2258A, with UK and EU notes).
- What the software will not do because of a law, a patent or a platform's rule, and which it is:
  [docs/user/rights/laws_that_limit_this_software.md](../user/rights/laws_that_limit_this_software.md).
- A copyright report on both apps ("Report a copyright problem", which works signed out) and a
  public takedown log (`docs/FEATURES.md`).

**Answer, ready to paste:**

> HumanityOS collects as little personal data as it can: no name, email, phone number, birth date or
> password, no cookies, no analytics or tracking scripts, and no IP address in the server's
> database. The project researched which laws could apply, from primary sources, and wrote down what
> each asks, with dates: the EU GDPR and UK GDPR (with the Data Protection Act 2018), California's
> CCPA/CPRA and CalOPPA, Washington's My Health My Data Act and US COPPA
> (https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-11-07-privacy-notice.md);
> children's online safety rules in the US, UK and EU; and a provider's duties on child sexual abuse
> material under 18 U.S.C. 2258A. The privacy notice at https://united-humanity.us/privacy is written
> to the contents lists of GDPR Article 13 and CalOPPA, and was checked against the server's code.
> Anyone can see and export what a server holds about them and erase it themselves. These are the
> project's own readings of the sources, not a lawyer's opinion.

**Verdict: ready, with a limit said.** Two limits a reviewer may ask about: no lawyer has reviewed
the readings, and no DMCA designated agent is registered (it needs a mailing address, waiting on a
legal home for the project; `docs/PRIORITIES.md`). Do not list accessibility laws: there are
accessibility settings (font size to 200%, high contrast, reduced motion), but no WCAG audit has
been done (`docs/STATUS.md`).

## Indicator 8: Adherence to Standards & Best Practices

**The standard:** "Digital public goods must be designed and developed to align with relevant
standards, best practices, and/or principles."

**The form asks:** the open standards followed, with evidence such as a validator or test suite; and
the best practices and principles followed, with evidence.

**Evidence, standards:**

| Standard | Where | How it is checked |
|---|---|---|
| FIPS 204, ML-DSA-65 (identity and signatures) and FIPS 203, ML-KEM-768 (sealing messages) | `docs/reference/cryptography.md`; RustCrypto `ml-dsa` 0.1.1 and `ml-kem` 0.3.3 | Known-answer tests between the Rust, website and relay implementations (`dilithium_cross_language_kat`, `kyber_cross_language_kat`, `scripts/pq-kat.mjs`). NIST's ACVP vectors for both (240 vectors from ACVP-Server 975de31) pass in the Rust build and on the website, in CI (`scripts/tests/pq-acvp.test.js`, the `humanity-protocol` conformance tests). |
| BIP39 (the 24-word recovery phrase) | `docs/reference/cryptography.md` | In use on both apps |
| WebRTC with DTLS-SRTP (calls) | `docs/reference/cryptography.md` | `just verify-browser-webrtc`: the desktop app against Chromium's own WebRTC, 2026-11-07 |
| RFC 8291 and RFC 8292 (Web Push encryption and VAPID) | `src/relay/web_push.rs` | Checked byte for byte against RFC 8291's worked example |
| RFC 5389 and RFC 5766 (STUN, and a closed subset of TURN) | `src/relay/stun_wire.rs`, `src/relay/call_forwarder.rs` | RFC 5769's test vectors |
| AWS Signature Version 4 (the S3 backup destination) | `src/relay/backup_stream/s3.rs` | AWS's published test vectors |
| SPDX licence identifiers; OpenStreetMap data under ODbL | [LICENSING.md](../../LICENSING.md), [LICENSES.md](../../LICENSES.md) | By hand |

**Evidence, practices:** a monthly dependency audit with a written log (`cargo audit`; the run of
2026-11-06 found three advisories and cleared them the same day, ending at no known vulnerabilities:
[docs/SECURITY-CADENCE.md](../SECURITY-CADENCE.md)); a build held at zero compiler warnings and
about 5,200 tests for the desktop app (the README's "Tests"); a desktop updater that installs only a
release carrying a valid hybrid Ed25519 and ML-DSA-65 signature (`docs/reference/cryptography.md`);
privacy settings that start at their most private; an incident playbook
([docs/INCIDENT-PLAYBOOK.md](../INCIDENT-PLAYBOOK.md)); every change public in the git history.

**Answer, ready to paste:**

> Open standards: FIPS 204 (ML-DSA-65) for identity and signatures and FIPS 203 (ML-KEM-768) for
> sealing private messages, checked against NIST's published ACVP test vectors and with known-answer
> tests across the Rust, website and server implementations; BIP39 for the recovery phrase; WebRTC (DTLS-SRTP) for calls, tested against
> Chromium; RFC 8291 and 8292 for Web Push, tested against the RFC's worked example; RFC 5389 and
> 5766 for STUN and TURN, tested with RFC 5769's vectors; SPDX licence identifiers. The cryptography
> is documented in one table: https://github.com/Shaostoul/Humanity/blob/main/docs/reference/cryptography.md.
> Practices: a monthly dependency audit with a public log
> (https://github.com/Shaostoul/Humanity/blob/main/docs/SECURITY-CADENCE.md), a zero-warning build
> with thousands of automated tests, an updater that installs only releases with a valid hybrid
> post-quantum signature, private-by-default settings, and an open development history.

**Verdict: ready.** The NIST ACVP vectors are in (2026-11-20) and named in the first line.

## Indicator 9: Do No Harm By Design

**The standard:** "Digital public goods must be designed to anticipate, prevent, and do no harm by
design."

### 9a: Data Privacy & Security

**The standard:** "Digital public goods that collect, store and distribute personally identifiable
(PII) data, must demonstrate how they ensure the privacy, security and integrity of this data in
addition to the steps taken to prevent adverse impacts resulting from its collection, storage and
distribution."

**The form asks:** which of four options fits (pick **"PII data can be collected and stored and
distributed"**: a server keeps what people publish, and linked servers receive the public parts);
the types of PII; and how privacy, security and integrity are ensured.

**Evidence:** the privacy policy's table of what is kept, why and for how long
([docs/outreach/privacy_policy.md](privacy_policy.md)); the cryptography table
([docs/reference/cryptography.md](../reference/cryptography.md)); the plain explainers
[who_can_read_your_messages.md](who_can_read_your_messages.md) and
[how_your_privacy_is_built.md](how_your_privacy_is_built.md); the open limits in
[docs/reference/research/2026-11-16-pqc-review/README.md](../reference/research/2026-11-16-pqc-review/README.md).

**Types of PII, ready to paste:**

> A chosen display name (no real name is asked for) and a public key; profile fields a person chooses
> to fill in; messages, reactions and files posted in rooms; sealed private messages (unreadable to
> the server); IP addresses in the web server's logs (deleted after about 3 days) and in the chat
> server's log only when a rate limit trips; a browser push address if notifications are turned on;
> with a bug report, the browser's name and window size.

**Answer, ready to paste:**

> Identity is a key made on the person's own device from a 24-word recovery phrase (ML-DSA-65,
> FIPS 204); nobody is asked for a name, email, phone number, birth date or password. Private
> messages are sealed on the sender's device (ML-KEM-768, FIPS 203, then AES-256-GCM) with the sender
> inside the encryption, so the server stores neither the words nor who sent them, and it deletes
> undelivered mail after 30 days. Files in private conversations are encrypted on the device. Each
> key a message is sealed to comes from that person's own signed key card, checked by the sending
> app, never from a server's list. Location and camera details are stripped from pictures and videos
> on the device and again on the server. The server's database holds no IP addresses; its backups
> are encrypted. Settings start at their most private (not shown as online, not listed in the
> directory). Each person can export everything a server holds about them, and erase it, without
> asking anyone. Known limits, stated publicly: a private group's server can see the group's name,
> its members and who posted when (not what was said); and private messages do not yet have forward
> secrecy, which is decided and designed. Security reviews so far are AI-assisted reviews and a
> review of the cryptography by a contributor who works in post-quantum cryptography (November 2026);
> no paid independent audit has been done yet. Details: https://github.com/Shaostoul/Humanity/blob/main/docs/reference/cryptography.md
> and https://united-humanity.us/privacy.

**Verdict: ready, with a limit said.**

### 9b: Inappropriate & Illegal Content

**The standard:** "Digital public goods that collect, store or distribute content must have policies
identifying inappropriate and illegal content such as child sexual abuse materials in addition to
processes for detecting, moderating, reporting and removing inappropriate/ illegal content."

**The form asks:** which of four options fits (pick **"Content is collected and stored and
distributed"**); the types of content; and (1) how inappropriate, misleading and illegal content is
identified, (2) the processes for detecting, moderating, reporting and removing it, with the average
response time.

**Evidence that exists today** (`docs/FEATURES.md`, "Safety: who can reach me, Block, reports,
warnings, protected setup"; `docs/design/blocking-and-safe-mode.md`, sections 6 and 8):

- **The content policy**, [docs/legal/content-policy.md](../legal/content-policy.md) (2026-11-20;
  in the Library at https://united-humanity.us/library#content-policy; linked from Settings > Safety
  in both apps, the website's footer and its rules page). It names what is never allowed, child
  sexual abuse material first, then the Humanity Accord's absolute prohibitions, then anything
  illegal where the server runs; how to report on HumanityOS and straight to the authorities; the
  operator's six steps on a report that a child may be involved (emergency number first, never open
  the file, take it out of public view without destroying it, report to the CyberTipline, keep it
  locked away for a year, tell nobody else); and, in its own list, what the app does not do for an
  operator yet. The law behind it:
  [the 2026-11-20 finding](../reference/findings/2026-11-20-csam-reporting.md).
- **Hide and keep** is built in both apps (rungs 1 to 5, 2026-11-20):
  [docs/design/hide-and-keep.md](../design/hide-and-keep.md). It needs the server's owner to be set,
  still done on the server itself; until then, the policy's manual steps.
- A report dialog on both apps with eleven reasons, among them "A child may be in danger", "Someone
  is in danger", "Unwanted sexual messages", "Harassment or bullying" and "Threats of violence"
  (`data/safety/report_reasons.json`).
- Reports are signed by the reporter; a reported private message is checked against its sender's own
  signature, so a report cannot be forged against someone. Admins and moderators handle them on a
  Reports page.
- Admins and moderators are never shown a reported file or link (`without_files_or_links`); a report
  that carries an encrypted file's key is refused whole, so no volunteer is ever handed abuse imagery
  (`docs/reference/cryptography.md`, the Reports row).
- For a child or a person in danger, the report dialog shows the emergency number and the child
  abuse reporting body for a country the person picks: 18 countries, each from a dated source; in
  the United States, NCMEC's CyberTipline (`data/safety/outside_help.json`).
- Moderation, with the real powers listed on the server's rules page
  (https://united-humanity.us/rules): a moderator can delete a message, with the pictures and files
  its author posted only in it; "Delete the post" on a report removes the post and keeps the report's
  copy of its words; kick, mute and ban run through a signed admin action registry with an admin log
  (`docs/FEATURES.md`, "Admin action registry"); admins and the owner can erase another person's
  data, and that person is told an admin did it. A public copyright takedown log.
- Warnings checked on the person's own device for scams and requests for the recovery phrase, and a
  send that holds the person's own recovery phrase is stopped.

**Types of content, ready to paste:**

> Text messages, reactions and threads in rooms; sealed private and group messages (unreadable to the
> server); pictures, video and other files; profiles; marketplace listings with pictures and reviews;
> signed proposals, votes and credentials; bug reports; buildings in the shared game world.

**Answer, ready to paste:**

> Policy: the HumanityOS content policy
> (https://github.com/Shaostoul/Humanity/blob/main/docs/legal/content-policy.md, also in the in-app
> Library and linked from Settings > Safety in both apps and from every page of the website) names
> what is never allowed: child sexual abuse material and any sexual contact with a child or attempt
> at it, first; then the Humanity Accord's absolute prohibitions (sharing sexual images without
> consent, trafficking, threats, blackmail, fraud, calls to harm a group of people); then anything
> illegal where the server runs. It tells people how to report on HumanityOS and straight to the
> authorities (the CyberTipline takes reports from any country), and never to save or forward an
> abusive image. On a report that a child may be involved, the operator calls the emergency number
> if a child is in danger, never opens the file, takes it out of public view without destroying it,
> bans the key, reports to the CyberTipline (or the country's own line), and keeps what was
> reported locked away for one year.
> Reporting: every message and person can be reported from both apps, with reasons including "A child
> may be in danger" and "Someone is in danger". For those two the dialog first shows the official
> emergency number and child abuse reporting body for the person's country (NCMEC's CyberTipline in
> the US; 18 countries listed with sources). Reports are signed, and reported private messages are
> verified against the sender's own signature, so reports cannot be forged. Moderators and admins act
> on a Reports page: delete the post; mute, kick or ban the member (signed admin actions, recorded in
> an admin log); or erase the account's data. Moderators are never shown reported files or links,
> and a report carrying a file is refused, so no volunteer is exposed to abuse imagery. A one-click
> "hide and keep" action takes a reported post and its files out of view at once and keeps a copy
> for the year, sealed so that only the server's owner can open it and nobody can view it in the
> app; the owner alone can export it into a file for the hotline or the police, place a legal hold,
> destroy it early or restore it, and every step is signed and logged. It needs the server's owner
> to be set, which is still done on the server itself; until it is, the policy gives operators the
> manual steps that do the same, and says so publicly. Detection: private messages are
> end-to-end encrypted, so no one, admins included, can scan them; action rests on reports from the
> people in a conversation, and warnings for scams and requests for the recovery phrase run on each
> person's own device. Response time: HumanityOS has not launched, so none has been measured yet;
> reports wait for each server's volunteer admins and moderators.

**Verdict: ready, with a limit said.** Hide and keep reached both apps (rung 5, 2026-11-20) and the
sentence that begins "A one-click" says so; the limit left is that a server's owner is still set on
the server itself, not in the app, and the answer says so. The retention of a child-danger
report (90 days after a decision today) is still an open decision.

### 9c: Protection from Harassment

**The standard:** "If the digital public good facilitates interactions with or between users or
contributors there must be a process for users and contributors to protect themselves against
grief, abuse, and harassment. The project must have system(s) to address the safety and security of
underage users."

**The form asks:** whether it facilitates interactions (**Yes**), and (1) policies such as a code of
conduct and the processes people use to protect themselves, (2) the processes for the safety and
security of underage users.

**Evidence** (`docs/FEATURES.md`, the Safety entry; `docs/design/blocking-and-safe-mode.md`,
section 6):

- **Who can reach me**: for messages, calls and trades, each person picks Nobody, People I choose,
  Friends, Friends and people in my groups, or Anyone; enforced by the server for everyone, admins
  included. The defaults are messages and trades from friends and calls from people I choose
  (`src/net/reach.rs`, `ReachSettings::default`). A stranger can send a contact request that shows
  only their name.
- **Block**: hides everything from that person and takes back what they were allowed; they are not
  told; it applies on all of the person's own devices.
- **Calls through the server** by default, so nobody in a call learns another person's network
  address.
- **The protected setup**, applied by a parent on a child's device and locked with a PIN
  (`data/gui/safety_presets.json`): direct messages and trades from friends only, calls only from
  people chosen, no pictures or files shown from anyone who is not a friend, public rooms hidden,
  calls always through the server; adding a friend, joining or starting a group, inviting someone to
  a group, joining a voice room, or changing the settings needs the PIN. Nothing about the setup is
  sent to any server.
- What the app says on screen about its limits (`data/gui/safety_presets.json`, read by both apps),
  among them: "It does not check anyone's age, and no setting can make an app, or the people in it,
  safe." and "No regulator or outside body has reviewed HumanityOS under any children's privacy or
  online safety law. Our dated research notes are public."
- The privacy policy: the service is for a general audience and not meant for children under 13; a
  parent can have a child's data erased.
- Conduct: the rules of the project's own server, https://united-humanity.us/rules (source
  `web/pages/rules.html`), open with "Respect every person. No harassment, hate speech, doxxing or
  targeted attacks.", list what each moderator power really does, and set the order of responses (a
  word, then mute, then kick, then ban). The content policy links them. They are that server's
  rules: there is no project-wide code of conduct, none for contributors, and the content policy
  itself does not name harassment (thin spots table).

**Answer, ready to paste:**

> Each person decides who can message, call or trade with them (nobody, chosen people, friends,
> friends and group members, or anyone), and the server enforces it for everyone, admins included.
> By default only friends can message or trade with a person and only people they chose can call;
> a stranger can send a contact request that shows only their name. Block
> hides everything from a person, withdraws what they were allowed, and is not announced to them.
> Calls go through the server by default, so no one in a call sees another's network address. Anyone
> can report a person or a message from both apps. For underage users: HumanityOS has no sign-up and
> collects no ID, so it cannot check ages, and it says so on screen. Instead a parent can apply a
> protected setup on a child's device, locked with a PIN: direct messages from friends only, no
> pictures or files shown from non-friends, public rooms hidden, calls always through the server, and
> adding a friend, joining a group or voice room, or changing the settings needs the PIN. Nothing
> about the setup is sent to any server. The app says on screen what the setup does not do, and that
> no regulator or outside body has reviewed it. The service is for a general audience and not meant
> for children under 13; a parent can ask an admin to erase what a server holds about a child.
> Conduct: the project's own server publishes its rules, beginning "No harassment, hate speech,
> doxxing or targeted attacks", with each moderator power and the order of responses (a word, mute,
> kick, ban): https://united-humanity.us/rules. What is never allowed anywhere is in the content
> policy: https://github.com/Shaostoul/Humanity/blob/main/docs/legal/content-policy.md.

**Verdict: ready for the protection systems; thin on a code of conduct** (thin spots table): the
rules that name harassment are one server's, and nothing covers contributors. The
protected setup must not be called "child safe", "kid safe" or "compliant with" anything: the
2026-10-10 finding says nothing read supports those words.

## The Sustainable Development Goals

The goals below are served by features that exist today, each with where to see it. Planned work
is left out. The target words are quoted from the UN's own goal pages (read 2026-11-20). The form
wants only the most relevant goals; the recommended selection is 4, 6, 3, 2 and 16 (question 4
above).

### Goal 4: Quality education

**Targets.** 4.4: "By 2030, substantially increase the number of youth and adults who have relevant
skills, including technical and vocational skills, for employment, decent jobs and
entrepreneurship". 4.7 asks that learners acquire, among other things, knowledge of "human rights"
and "promotion of a culture of peace and non-violence".

**What exists today.** The Library, inside the desktop app (it works offline: the guides ship in
`data/library/`) and on the website (https://united-humanity.us/library). Its "Learn" section holds
137 practical guides: Grow Food (34), Wild Food (4), Water (12), Shelter and Power (17), When
Somebody Is Hurt (14), Making and Repair (39) and Where You Are (17). Each guide ends with the
primary sources it was written from (`data/library/index.json`; the writing rules in
`docs/contributor/library-writer-brief.md`). Six step-by-step how-tos with timers and checks
(`data/how_to/`, https://united-humanity.us/how-to). The Humanity Accord's documents on rights,
consent and conflict resolution sit on the Library's "Your Rights and Consent" and "How We Decide"
shelves. Everything is CC BY-SA 4.0 or CC0, so a school or a library may copy, translate and adapt
it.

**Answer, ready to paste:**

> HumanityOS includes a free Library of 137 practical skill guides (growing and preserving food,
> safe water and sanitation, first aid and hygiene, building, power, tools and repair), each citing
> the primary sources it draws on, plus step-by-step how-tos. It works offline inside the desktop
> app and on the website, and it is openly licensed (CC BY-SA 4.0), so anyone may copy, translate
> and adapt it. This serves target 4.4 (technical and vocational skills); the Humanity Accord's
> guides on rights, consent and conflict resolution serve target 4.7.

**Limits.** English only. The guides were written with AI help under a sourcing rule; no outside
expert review is recorded. It is a reference, not a course with assessment.

### Goal 6: Clean water and sanitation

**Targets.** 6.1: "By 2030, achieve universal and equitable access to safe and affordable drinking
water for all". 6.2: "By 2030, achieve access to adequate and equitable sanitation and hygiene for
all and end open defecation, paying special attention to the needs of women and girls and those in
vulnerable situations".

**What exists today.** The Library's Water shelf (12 guides), among them Making Water Safe to Drink,
Testing Water, Storing Water Safely, Collecting Rainwater, Wells and Groundwater, Greywater, and
Toilets and Human Waste; Handwashing and Hygiene on the health shelf. The how-to "Boil water to make
it safe to drink" (`data/how_to/water/boil_water_safe.json`). The water guides cite the US CDC, the
US EPA and the WHO (for example `data/library/making_water_safe_to_drink.md`, "Sources").

**Answer, ready to paste:**

> HumanityOS's Library teaches how to make water safe to drink, test and store it, collect
> rainwater, and handle greywater and human waste safely, drawing on WHO, CDC and EPA guidance,
> with a step-by-step how-to for boiling water. It is free, works offline, and may be copied and
> translated freely. This serves targets 6.1 (safe drinking water) and 6.2 (sanitation and hygiene).

**Limits.** Knowledge, not infrastructure. Four of the six how-tos, this one among them, are marked
as able to cause serious harm if wrong; they are sourced and were reviewed twice by AI, and wait for
a person's review before they show as checked (`docs/FEATURES.md`, "How-to").

### Goal 3: Good health and well-being

**Targets.** 3.3: "By 2030, end the epidemics of AIDS, tuberculosis, malaria and neglected tropical
diseases and combat hepatitis, water-borne diseases and other communicable diseases". 3.d:
"Strengthen the capacity of all countries, in particular developing countries, for early warning,
risk reduction and management of national and global health risks".

**What exists today.** The Library's When Somebody Is Hurt shelf (14 guides): First Aid Until Help
Arrives, Bleeding and Wounds, Treating Burns, Preventing and Spotting Infection, The Germs That
Matter, When Food or Water Makes You Sick, Rats, Flies, Mosquitoes and Ticks, Cold and Hypothermia,
When Heat Becomes Dangerous, Mental Health Under Strain, and others. The how-tos "Mix oral
rehydration solution from a packet" (`data/how_to/health/mix_ors_packet.json`) and "Test the seals
on home-canned jars" (`data/how_to/food/test_jar_seals.json`). Health details a person records stay
on their own device and are never sent to a server (the privacy policy, "Things we never ask for").

**Answer, ready to paste:**

> HumanityOS's Library includes guides on first aid, wounds, burns, infection, hygiene, the germs
> behind water- and food-borne illness, disease-carrying pests, heat and cold, and mental health
> under strain, plus how-tos for mixing oral rehydration solution and checking home-canned food. It
> is free and works offline. This serves targets 3.3 (water-borne and communicable diseases) and 3.d
> (risk reduction).

**Limits.** Not medical advice, and the guides say when to get help. The rehydration and canning
how-tos wait for a person's review, as under Goal 6.

### Goal 2: Zero hunger

**Targets.** 2.3 asks to "double the agricultural productivity and incomes of small-scale food
producers", including through access to "knowledge, financial services, markets". 2.4 asks for
"sustainable food production systems" and "resilient agricultural practices".

**What exists today.** The Library's Grow Food shelf (34 guides: Your First Tomato, Starting Seeds,
Saving Your Own Seeds, Growing Food You Can Live On, Nutrition That Keeps You Working, and the
preserving guides on canning, drying, fermenting, salting and smoking) and Wild Food (4). The
marketplace: signed listings, reviews and seller ratings, where the platform introduces buyers and
sellers and never handles the payment (the README's "What you can do today";
https://united-humanity.us/market).

**Answer, ready to paste:**

> HumanityOS teaches small-scale food growing and preservation (seed starting and saving, growing a
> household's food, canning, drying, fermenting, curing) through its free Library, and its
> marketplace lets growers list goods with signed listings and reviews, without the platform ever
> handling the payment. This serves targets 2.3 (knowledge and markets for small-scale producers)
> and 2.4 (resilient practices).

**Limits.** No one uses the marketplace yet; this is relevance by design, not a measured outcome.

### Goal 16: Peace, justice and strong institutions

**Targets.** 16.10: "Ensure public access to information and protect fundamental freedoms, in
accordance with national legislation and international agreements". 16.7: "Ensure responsive,
inclusive, participatory and representative decision-making at all levels". 16.6: "Develop
effective, accountable and transparent institutions at all levels".

**What exists today.**

- For 16.10: end-to-end encrypted private messages; no sign-up; settings that start at their most
  private; anyone can run a server, and servers link to each other, so no single server is the only
  way in; a privacy policy that names what is kept; plain-language summaries of laws with their
  sources on the Laws page (six jurisdictions so far, `data/laws/laws.json`,
  https://united-humanity.us/laws).
- For 16.7: proposals and votes on both apps, each vote a signed record checked by the server and
  weighted by a published trust score capped at 0.95 (`docs/FEATURES.md`, "Governance" and "Native
  Governance page"; `data/identity/trust_weights.ron`; https://united-humanity.us/governance).
- For 16.6: organisations with members, roles and a signed audit log (`docs/FEATURES.md`,
  "Organisations"); member moderation (kick, mute, ban) as signed admin actions recorded in an admin
  log; a public takedown log for copyright reports; all of the project's development in a public
  history.

**Answer, ready to paste:**

> HumanityOS protects private communication (end-to-end encrypted, post-quantum, no sign-up,
> private by default) and lets any community run its own server, linked to others, so no single
> operator controls access (target 16.10). Communities can make proposals and vote with signed,
> verifiable votes, weighted by a published and capped trust score (16.7). Organisations keep
> a signed audit log of their decisions, and member moderation is signed and logged (16.6).

**Limits.** Server linking has run only between two servers, both the maintainer's. Votes are
weighted by a published trust score, capped, rather than one person one vote; ranked choice and
quorums are not built.

### Goals that can be added, with weaker evidence

- **Goal 1, no poverty** (the project's purpose). Target 1.4 names access to "appropriate new
  technology"; 1.5 asks to "build the resilience of the poor and those in vulnerable situations"
  against "shocks and disasters". Evidence: the app is free, with no ads, runs on
  Windows, macOS, Linux and in any browser, and works offline on the desktop; the Library's guides
  include Helping Your Neighbours After a Disaster and The Hazards Where You Live. The link to
  reducing poverty is indirect, so it is recommended for the short description rather than the
  selection.
- **Goal 7, affordable and clean energy.** Target 7.1, "By 2030, ensure universal access to
  affordable, reliable and modern energy services". Evidence: the guides Your First Solar Power,
  Batteries and Storage, Water and Wind Power, and Choosing and Running a Generator. Guides only, so
  weaker than the goals above.

## The form's last page: scale of the solution

| Field | Answer |
|---|---|
| Where it is deployed | United States: two servers, united-humanity.us and public.guide, both run by the maintainer, linked to each other. Desktop downloads for Windows, macOS and Linux reach anyone. |
| Who else uses it | No organisation has adopted it yet. Say so. |
| Designed for other languages? | Yes. Every word in both apps comes from a language file by key (`data/i18n/en.json`), and a missing word falls back to English. |
| Available languages | English. Spanish, French, Chinese and Japanese files exist, with 37 phrases each against 7,492 in English (counted 2026-11-20), so list English only. |
| Programmes | Yours to answer. |
| Highlights | The devlog, one card per release: https://united-humanity.us/devlog. The Library: https://united-humanity.us/library. The releases: https://github.com/Shaostoul/Humanity/releases. The mission: https://united-humanity.us/mission. |

## Submitting: how the DPGA's process works

From the DPGA's own pages (the sources table above).

**Who may submit.** Only an authorised representative of the solution, on behalf of the people or
organisation that own it. The DPGA records the applicant's name and email address and requires the
email to be verified before an application can be submitted. Evidence for indicators 7 to 9 comes
from someone authorised to speak for the project, whose title, name and contact the DPGA takes.

**What becomes public.** The content of a submitted application is published under the Unlicense
(public domain). The applicant's name and contact stay private. So write nothing in the answers you
would not publish; the answers in this file are written to be published.

**One sitting, in order:**

1. Hide and keep reached both apps on 2026-11-20 and the 9b sentence that begins "A one-click"
   says so ([docs/design/hide-and-keep.md](../design/hide-and-keep.md)); re-read it once.
2. Optional, about five minutes: the eligibility tool, https://digitalpublicgoods.net/eligibility.
   It asks whether you are the developer or owner, then one question per indicator, and either says
   the solution is eligible or gives pointers.
3. Sign up at https://app.digitalpublicgoods.net/signup with the email you want the DPGA to use, and
   verify it.
4. Start the application and paste, in order: the general information, indicators 1 to 9c, and the
   scale page, from this file. The application is assessed against the version of the standard in
   force when it is created.
5. Read it over before pressing submit: an application under review cannot be changed.
6. Note the application ID. Its status shows on the dashboard and at
   `https://app.digitalpublicgoods.net/a/<ID>`; the public page is `https://digitalpublicgoods.net/r/<ID>`.

**What happens after.**

- Two reviewers read it, and may send an indicator to an outside expert (the status then reads
  "Under Consultation"). The review policy expects 4 to 8 weeks; the submission guide says they try
  to finish within 30 days.
- If they need more, a "Clarifications Required" notice appears on the dashboard: open it, answer
  the marked indicators, save, and submit. An application whose clarifications do not arrive in the
  time allowed is declared Ineligible, so watch the email.
- The result is either **DPG** (every indicator met) or **Ineligible**. An Ineligible application
  cannot be revived: fix what failed and submit a new one. Questions go to
  support@digitalpublicgoods.net.
- A DPG is listed on the registry, https://digitalpublicgoods.net/registry, and may use the DPG name
  and badge. The status lasts one year. A renewal application is created 60 days before it expires;
  update it on the dashboard, or it is resubmitted with the old answers.

## Keeping this file true

Update this file in the same piece of work when any of these lands: a change to the content policy
(9b), hide and keep (9b), the NIST conformance tests (8), the wallet key's own derivation
and forward secrecy (9a), the Windows DLLs' notice (3 and 4), a code of conduct (9c), or a second
language. Before a renewal, re-read the standard and the form's questions: the DPGA changes both.
