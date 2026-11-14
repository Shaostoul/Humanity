# Office file formats and editing together: research notes

What this file is: research notes on what it would take for HumanityOS to open, edit and save
everyday office documents (Word's .docx, Excel's .xlsx, PowerPoint's .pptx, and the OpenDocument
.odt, .ods and .odp files LibreOffice uses) and to let several people edit one document together
across federated servers. It covers the standards and who may implement them (part A), the Rust
libraries that exist today (part B), converting through LibreOffice as a separate program (part
C), the libraries for editing together (part D) and how three open editors do it (part E), then
what it would take (the last section). The licensing and patent question is answered on its own in
the dated finding [`../../findings/2026-11-14-office-file-formats.md`](../../findings/2026-11-14-office-file-formats.md).

**Read on 14 November 2026 (Pacific).** Method: each site's `robots.txt` was read first, in a step
of its own, before any other page of that site (results at the end). Pages were read with curl
identifying itself as `Claude-User (research tool)` and turned into plain text for searching; one
page was cross-checked through WebFetch. GitHub facts came through `gh api` (reading, not
downloading); crates.io facts through its public API at one request per 1.1 seconds. No file other
than the pages themselves was downloaded (one zip's headers were checked, its body was not
fetched). Quotations are exact, except that curly quotes may be normalised to straight ones.

Words used below: a **format** is the agreed layout of a file; a **specification** (spec) is the
document that defines it; a **crate** is a Rust library; **round trip** means opening a file and
saving it again without losing the parts the program did not understand; a **CRDT** and
**operational transformation (OT)** are the two families of methods for merging edits that several
people made at once (explained in part D).

## A. The standards, and whether anyone may implement them

### Office Open XML (the .docx, .xlsx and .pptx formats)

| Fact | Source and its date | Exact words |
|---|---|---|
| ECMA-376 is the standard; current edition 5th, December 2021 | [ecma-international.org ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/), page modified 2022-01-26 | "5th edition, December 2021" |
| Its four parts and their dates | same page | "Part 1 "Fundamentals And Markup Language Reference", 5th edition, December 2016"; "Part 2 "Open Packaging Conventions", 5th edition, December 2021"; Part 3 "5th edition, December 2015"; Part 4 "5th edition, December 2016" |
| Only part 2 changed in the latest edition | same page | "ECMA-376 contains 4 parts but only part 2 has been adopted in the last edition of the Standard." |
| It is the same standard as ISO/IEC 29500 | same page | "ISO/IEC number 29500" |
| Free to download: each part is a direct link, no sign-in or price shown | same page ("Download part 1" to "Download part 4"); the part 1 zip answered HTTP 200, `application/zip`, 43,501,721 bytes, last modified 2 November 2022 (headers only, body not fetched) | (an observation, not a quotation) |
| ISO/IEC 29500 parts 1 to 4 edition dates, and whether ISO lists them as publicly available | not read: www.iso.org answered its own `robots.txt` with a 403 challenge page, and standards.iso.org's `robots.txt` disallows everything | (see Could not read) |

### OpenDocument (the .odt, .ods and .odp formats)

| Fact | Source and its date | Exact words |
|---|---|---|
| Current version: ODF 1.4, an OASIS Standard of 6 October 2025 | [docs.oasis-open.org ODF 1.4 namespace page](https://docs.oasis-open.org/office/OpenDocument/v1.4/OpenDocument-v1.4.html), dated 6 October 2025 | "Stage: OASIS Standard (OS)"; "Date: 6 October 2025" |
| Earlier versions in the same folder | [docs.oasis-open.org/office/OpenDocument/](https://docs.oasis-open.org/office/OpenDocument/), folder listing | "v1.3/ 27-Apr-2021"; "v1.4/ 06-Oct-2025" |
| The ODF committee's patent rule (its "IPR mode") | [ODF 1.4 Part 1](https://docs.oasis-open.org/office/OpenDocument/v1.4/OpenDocument-v1.4-part1-introduction.html), 6 October 2025 | "This specification is provided under the RF on Limited Terms Mode of the OASIS IPR Policy" |
| Every member of the committee agreed to it | [ODF TC IPR page](https://www.oasis-open.org/committees/office/ipr.php), undated page | "All participants in this TC have agreed to the terms of the RF on Limited Terms Mode of the OASIS IPR Policy ." |
| What RF (royalty-free) means there: a licence granted on request, without royalties | [OASIS IPR Policy, section 10.2.1](https://www.oasis-open.org/policies-guidelines/ipr/), page modified 2023-09-11 (latest listed revision approved 07/31/2013) | "upon request and subject to Section 11, it will grant to any OASIS Party or third party: a nonexclusive, worldwide, non-sublicensable, perpetual patent license (or an equivalent non-assertion covenant)" and "without payment of royalties or fees" |
| "Limited Terms" adds that no further conditions may be imposed | same policy, section 10.2.3 | "Obligated Parties may not impose any further conditions or restrictions beyond those specifically mentioned in Section 10.2.1" |
| Sun's covenant (2005) not to enforce its patents against any ODF implementation, with a reciprocity condition | ODF TC IPR page, statement dated September 29, 2005 | "it will not seek to enforce any of its enforceable U.S. or foreign patents against any implementation of the Open Document Format for Office Applications (OpenDocument) v1.0 Specification , or of any subsequent version thereof" |
| ODF is also ISO/IEC 26300 | [OASIS ODF TC page](https://www.oasis-open.org/tc-opendocument/), modified 2026-11-03 | "published as an ISO/IEC international standard (ISO/IEC 26300)" |
| OASIS's own summary of who may implement it | same page | "freely available specifications that anyone can implement without licensing fees or restrictions" |
| The spec text may be copied for implementing it | ODF 1.4 Part 1 notices | "derivative works that comment on or otherwise explain it or assist in its implementation may be prepared, copied, published, and distributed, in whole or in part, without restriction of any kind" |
| ISO/IEC 26300's current edition | not read (ISO barred, as above) | |

### Microsoft's Open Specification Promise (OSP)

| Fact | Source and its date | Exact words |
|---|---|---|
| The promise | [learn.microsoft.com, Open Specification Promise](https://learn.microsoft.com/en-us/openspecs/dev_center/ms-devcentlp/1c24c7c8-28b0-4ce1-a47d-95fe1ff504bc), "Published: September 12, 2006 \| Revised: February 24, 2023"; footer "Last updated on 2023-03-16" | "Microsoft irrevocably promises not to assert any Microsoft Necessary Claims against you for making, using, selling, offering for sale, importing or distributing any implementation to the extent it conforms to a Covered Specification" |
| It ends for anyone who sues Microsoft over the same specification | same page | "If you file, maintain or voluntarily participate in a patent infringement lawsuit against a Microsoft implementation of such Covered Specification, then this personal promise does not apply" |
| It covers only Microsoft's patents | same page | "This promise is not an assurance either (i) that any of Microsoft issued patent claims covers a Covered Implementation or are enforceable or (ii) that a Covered Implementation would not infringe patents or other intellectual property rights of any third party." |
| Covered: ECMA-376, ISO/IEC 29500:2008 and :2012, and ODF 1.0 to 1.3, with future versions while Microsoft takes part | same page, section "Open XML and Open Document File Formats" | lists "Office Open XML 1.0 - Ecma-376", "Office Open XML ISO/IEC 29500:2008", "Office Open XML ISO/IEC 29500:2012", "OpenDocument Format for Office Applications v1.3 OASIS"; "As long as Microsoft participates in their revision process to completion, Microsoft irrevocably commits to apply the Open Specification Promise to future versions of the below listed specifications." |
| Covered: Microsoft's own extension documents | same page, section "Office File Formats" | "[MS-DOCX]: Word Extensions to the Office Open XML File Format (.docx) Specification"; "[MS-PPTX]: PowerPoint Extensions to the Office Open XML File Format (.pptx) Specification"; "[MS-XLSX]: Excel Extensions to the Office Open XML SpreadsheetML File Format (.xlsx) Specification" |
| Covered as implementer notes: [MS-OI29500], for 29500:2008, :2012 and :2016 | same page, "Implementer Notes for Covered Specifications" | "Office Implementation Information for ISO/IEC 29500 Standards Support [MS-OI29500]"; "Office Open XML ISO/IEC 29500:2016" |
| Version rule | same page | "This promise applies to the identified version of the following specifications." |
| No signature needed | [OSP FAQ](https://learn.microsoft.com/en-us/openspecs/dev_center/ms-devcentlp/03347966-f8ff-4d53-a05e-63419d4132e2), "Last updated on 2023-03-16" | "No one needs to sign anything or even reference anything." |
| Open source and partial implementations | same FAQ | "It is intended to enable open source implementations"; "The OSP applies whether you have a full or partial implementation." |
| Current revisions of the Microsoft documents | [MS-DOCX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/b839fe1f-e1ca-4fa6-8c26-5954d0abbccd) 8/18/2026, revision 23.0; [MS-XLSX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/2c5dee00-eff2-4b22-92b6-0738acd4475e) 10/16/2026, 30.0; [MS-PPTX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/efd8bb2d-d888-4e2e-af25-cad476730c9f) 8/20/2024, 25.0; [MS-OI29500](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/1fd4a662-8623-49c0-82f0-18fa91b413b8) 8/18/2026, 25.0 | (each page's "Published Version" table) |
| The documents' copyright notice lets implementers copy schemas into their code | MS-DOCX page, notice section | "You can also distribute in your implementation, with or without modification, any schemas, IDLs, or code samples that are included in the documentation." |
| The documents' patent notice points back to the promise | same page | "a given Open Specifications document might be covered by the Microsoft Open Specifications Promise or the Microsoft Community Promise" |

### Ecma's patent rules

| Fact | Source and its date | Exact words |
|---|---|---|
| Ecma's aim | [Ecma Code of Conduct in Patent Matters](https://ecma-international.org/policies/by-ipr/code-of-conduct-in-patent-matters/), "Version 2 (approved by the Ecma GA in June 2016)", page modified 2022-01-12 | "Ecma desires to develop standards for which licenses for any essential patents are available on a non-discriminatory basis and on reasonable terms and conditions." |
| Option 1: free of charge | same | "The patent holder is prepared to grant licenses free of charge to other parties on a non-discriminatory basis on reasonable terms and conditions." |
| Option 2: reasonable terms, which may include a fee | same | "The patent holder is prepared to grant licenses to other parties on a non-discriminatory basis on reasonable terms and conditions." |
| A member who says nothing must still license reasonably | same, 4.3 | "is obliged to license any claims in patents or patent applications required to implement the Standard on a reasonable and non-discriminatory basis." |
| Declarations made for ECMA-376 (editions 1 to 3 only) | [Ecma list of patent statements](https://ecma-international.org/policies/ecma-list-of-patent-statements/), page modified 2021-01-29 | Adobe (22 November 2006), Microsoft (4 December 2008, editions 1 and 2), Intel (24 June 2011, edition 3), each marked "Yes" under "Prepared to grant license according to Ecma policy ?" |
| A royalty-free option exists but members opt in | [Ecma IPR policies](https://ecma-international.org/policies/by-ipr/), modified 2025-12-19 | "Ecma has a patent policy (Ecma Code of Conduct in patent matters) along with a Royalty-Free patent policy extension option" |

## B. Rust libraries for these formats

crates.io figures read through its API on 14 November 2026: the version is the newest stable one,
the date is that version's release date, downloads are all-time. "Licence" quotes crates.io's
licence field; the GitHub column is `gh api` (licence files opened where the two disagreed).

| Crate | Does | Version, date | Licence (crates.io) | Downloads | Repository since | Its own words |
|---|---|---|---|---|---|---|
| docx-rs | read and write .docx | 0.4.22, 2026-07-21 | "MIT" | 3,913,726 | 2019-09 | "Read and write .docx files with Rust, WebAssembly, and JavaScript." |
| docx-rust | read and write .docx | 0.1.11, 2026-01-22 | "MIT" | 2,356,861 | (repo cstkingkey/docx-rs) | "A Rust library for parsing and generating docx files." |
| rdocx | read, write, convert .docx | 0.17.0, 2026-11-09 | "MIT OR Apache-2.0" | 66,356 | 2026-02 | "High-level API for reading, writing, and converting DOCX documents" |
| calamine | READ only: xls, xlsx, xlsb, ods | 0.38.0, 2026-11-02 | "MIT" | 18,343,439 | 2016-06 | "As long as your files are *simple enough*, this library should just work." |
| rust_xlsxwriter | WRITE only: xlsx | 0.102.0, 2026-10-29 | "MIT OR Apache-2.0" (both licence files present) | 6,711,044 | 2022-06 | "a Rust crate for writing Excel files in the xlsx format." |
| umya-spreadsheet | read and write xlsx | 3.1.1, 2026-10-12 | "MIT" | 1,539,536 | 2020-08 | "a library written in pure Rust to read and write xlsx file." |
| spreadsheet-ods | read and write ods, with round trip | 1.0.4, 2026-03-20 | "MIT/Apache-2.0" (both files present) | 122,306 | 2020-06 | "Any unparsed parts are kept as blobs or raw xml and are written again." |
| office_oxide | read, write, edit docx, xlsx, pptx; read old doc, xls, ppt | 0.1.17, 2026-11-11 | "MIT OR Apache-2.0" (both files present) | 1,256,015 | 2026-03 | its README's format table marks .pptx "Yes" under Read, Write and Edit |
| ppt-rs | create, read, update .pptx | 0.2.37, 2026-10-24 | "Apache-2.0" | 107,234 | 2025-11 | "Also supports HTML import, edit round-trip, PDF export, and validation" |
| pptx | create .pptx | 0.1.0, 2026-02-25 | "MIT" | 24,172 | | "A Rust library for creating and manipulating PowerPoint (.pptx) files" |
| betteroffice-pptx-parse (part of BetterOffice) | docx, xlsx, pptx editing with collaboration | 0.5.1, 2026-10-27 | "Apache-2.0" | 134,133 | 2026-07 | "Native OOXML editing preserves untouched file parts losslessly when round-tripping." |
| lo_odf (part of libreoffice-rs) | write ODF text, spreadsheet, presentation | 0.5.3, 2026-08-16 | "MIT" | 9,292 | 2026-04 | "This is **not feature-parity with LibreOffice**." |
| quick-xml | XML reading and writing (the base layer) | 0.42.0, 2026-08-22 | "MIT" | 547,873,301 | | "High performance xml reader and writer" |
| zip | zip containers (every one of these formats is a zip) | 9.5.1, 2026-11-10 | "MIT" | 345,248,627 | | "Library to support the reading and writing of zip files." |
| typst | typesetting to PDF | 0.15.1, 2026-07-17 | "Apache-2.0" | 4,101,928 | | "A new markup-based typesetting system that is powerful and easy to learn." |
| printpdf | read and write PDF | 0.12.9, 2026-11-11 | "MIT" | 4,061,902 | | "Rust library for reading and writing PDF files" |
| lopdf | change PDF files | 0.47.0, 2026-11-04 | "MIT" | 30,206,315 | | "A Rust library for PDF document manipulation." |
| pdfium-render | render, read, write PDF through Google's Pdfium | 0.9.5, 2026-11-01 | "MIT OR Apache-2.0" | 3,943,378 | | "`pdfium-render` binds to a Pdfium library at run-time" (the C++ Pdfium library is separate; its own licence was not read) |
| mupdf (Rust wrapper of MuPDF) | render PDF | 0.8.0, 2026-06-22 | "AGPL-3.0" | 1,785,787 | | MuPDF's own COPYING file opens "GNU AFFERO GENERAL PUBLIC LICENSE" (GitHub mirror ArtifexSoftware/mupdf) |

What the table says in plain words:

- **.xlsx and .ods are well served.** Reading is mature (calamine, since 2016, 18 million downloads,
  though only for files "simple enough"), writing is mature (rust_xlsxwriter), and reading plus
  writing exists in umya-spreadsheet (.xlsx) and spreadsheet-ods (.ods, small but it keeps what it
  does not understand).
- **.docx has one established reader-writer** (docx-rs, since 2019) and newer ones.
- **.pptx has NO mature Rust library.** Every crate that writes it is under a year old (ppt-rs
  November 2025, office_oxide March 2026, BetterOffice July 2026).
- **.odt and .odp have NO mature Rust library.** The only crates found that read and write them
  are the libreoffice-rs family (April 2026, about 9,000 downloads), whose README claims
  "reading, writing, converting, and rendering" for ODT and ODP among others and says plainly it is
  not a full replacement. Other crates only pull the text out (anydoc, pptx-to-md).
- **PDF output is well served by permissive crates** (typst, printpdf, lopdf). MuPDF is AGPL, so
  it could only ever run as a separate program beside HumanityOS.
- **Every crate above except mupdf is permissive** (MIT, Apache-2.0 or both), so each could live
  inside the one program.

## C. Converting through LibreOffice, as a separate program

| Fact | Source and its date | Exact words |
|---|---|---|
| Headless mode | [LibreOffice Help, Command line parameters](https://help.libreoffice.org/latest/en-US/text/shared/guide/start_parameters.html), "LibreOffice 26.8 Help" (page metadata "datePublished" 2020) | "Starts in "headless mode" which allows using the application without user interface." |
| It is meant to be driven by other programs | same | "This special mode can be used when the application is controlled by external clients via the API ." |
| Conversion switch and examples | same | "--convert-to OutputFileExtension"; "--convert-to pdf *.doc"; "In absence of --outdir , current working directory is used for the result." |
| Licence | [libreoffice.org Licenses](https://www.libreoffice.org/licenses/), undated page | "LibreOffice is made available subject to the terms of the Mozilla Public License v2.0" |
| It also carries other licences | same | "also includes software which differs from version to version under a large variety of other Open Source licenses." |

In plain words: LibreOffice can be started with no window and asked to turn one file into another
(for example `.docx` to `.odt` or `.pdf`). Run as its own program that HumanityOS calls, its MPL-2.0
licence never mixes with HumanityOS's code, which fits the "beside, never linked" rule. The cost is
size and a second install; it is a fallback for the formats Rust cannot yet write well (.pptx,
.odt, .odp), not a core.

## D. Libraries for editing together

**CRDT and OT in plain words.** When two people type into the same document at the same moment,
their changes must be merged so that both end up seeing the same text. Two families of methods do
this:

- **Operational transformation (OT)** rewrites each incoming change to account for the changes
  that happened first (it "transforms index positions"). In practice it needs one central server
  that sees every change in order.
- **A CRDT (conflict-free replicated data type)** gives every character or item its own permanent
  identity, so changes can arrive in any order, from anyone, and still merge to the same result,
  with or without a server.

Both terms, from one primary source, the Yjs project's own README ([yjs/yjs](https://github.com/yjs/yjs), read through `gh api`, latest release v13.6.33 of 2026-09-23): "OT attempts to transform index positions to ensure convergence (all clients end up with the same content), while CRDTs use mathematical models that usually do not involve index transformations, like linked lists." And on servers: "OT approaches that support shared editing without a central source of truth (a central server) require too much bookkeeping to be viable in practice." Etherpad's own design notes show OT's central server in practice (part E).

| Library | Licence (licence file) | Latest | Rust? | What it says about itself (its own README or site) |
|---|---|---|---|---|
| Automerge | MIT ("Copyright (c) 2019-2021 the Automerge contributors") | crate 0.12.0, 2026-09-16; JS 3.5.0 same day | Rust core (the `automerge` crate), also C and JS | "a sync protocol for efficiently transmitting those changes over the network"; site: "a local-first sync engine for multiplayer apps that works offline, prevents conflicts, and runs fast"; "Your document's entire history lives locally"; [rich text page](https://automerge.org/docs/reference/documents/rich-text/) (undated): "As well as supporting plain text, Automerge supports rich text editing." |
| Yrs (y-crdt, the Rust port of Yjs) | MIT ("The MIT License (MIT)", Bartosz Sypytkowski and Kevin Jahns) | 0.28.1, 2026-11-04 | yes | "It aims to maintain behavior and binary protocol compatibility with Yjs, therefore projects using Yjs/Yrs should be able to interoperate with each other."; its feature table marks "YText: formatting attributes and deltas", "Snapshots" and "Undo Manager" as done for yrs (table headed yrs 0.21, older than the current release) |
| Yjs (JavaScript original) | MIT | v13.6.33, 2026-09-23 | no (JS) | "Yjs is **network agnostic** (p2p!), supports many existing **rich text editors**, **offline editing**, **version snapshots**, **undo/redo** and **shared cursors**." |
| Loro | MIT ("Copyright (c) 2023 Loro") | 1.17.1, 2026-10-22 | yes (also JS through WebAssembly, Swift) | lists "P2P Synchronization", "Automatic Merging", "Local Availability", "Rich Text CRDT", "Fast Time Travel Through History" |
| diamond-types | ISC (Cargo.toml; no licence file at the top of the repository) | crate 1.0.0, 2022-08-25; repository's Cargo.toml says 2.0.0 | yes | "concurrent editing of lists or strings (text documents) by multiple users in a P2P network without needing a centralized server"; "This version of diamond types only supports plain text editing."; "Note the package published to cargo is quite out of date" |
| cola | MIT | 0.5.1, 2025-07-06 | yes | "specialized for real-time collaborative editing of plain text documents"; "without relying on a central server to coordinate the edits." |
| y-octo (repository toeverything/y-octo) | "MIT" (crates.io) | 0.1.4, 2026-11-13 | yes | "High-performance and thread-safe CRDT implementation compatible with Yjs" (crates.io description; README not read) |
| crdts (rust-crdt) | "Apache-2.0" (crates.io) | 7.3.2, 2023-08-08 | yes | "Practical, serializable, thoroughly tested CRDTs" (general building blocks, no text editor type claimed; no release since 2023) |

A notable link between parts B and D: BetterOffice (Apache-2.0, July 2026) builds its live
co-editing of .docx, .xlsx and .pptx on Yrs. Its workspace `Cargo.toml` takes `yrs` from
"https://github.com/openooxml/y-crdt.git" (a copy kept by the same organisation, pinned to one
commit), and its README claims "Concurrent changes merge automatically, and offline edits sync when
peers reconnect."

## E. How three open editors handle collaboration and encryption

| Editor | Source and its date | Exact words | In plain words |
|---|---|---|---|
| CryptPad (AGPL-3.0, release 2026.10.1 of 2026-11-10) | [cryptpad.org/about](https://cryptpad.org/about/), undated page | "All data on CryptPad is encrypted in the browser. This means no readable data leaves the user's device." | Documents are locked on each person's device before they reach the server, so the server stores only scrambled data. |
| CryptPad, the limit it states | [cryptpad/cryptpad README](https://github.com/cryptpad/cryptpad) via `gh api` | "The code which performs the encryption is still loaded from the host server like any other web page, so you still need to trust the administrator" | A web page's encryption is only as honest as the server sending the page; a desktop app does not have this gap. |
| Etherpad (Apache-2.0, v3.3.17 of 2026-11-11) | [Easysync full description](https://github.com/ether/etherpad-lite/blob/develop/doc/public/easysync/easysync-full-description.tex) (the `.tex` source, via `gh api`), undated; README points to it | "There is a server that holds the current state of a document."; "clients cannot send each other messages directly, and must go through the server always." Concurrent edits are reconciled in its section "Follows", which computes adjusted changesets for each side. | Etherpad is OT: every change goes through one server, which transforms and orders it. |
| Collabora Online (MPL-2.0 COPYING file) | [Based on LibreOffice](https://www.collaboraonline.com/based-on-libreoffice/), modified 2026-10-23 | "today, we develop Collabora Office (the engine powering Collabora Online) independently, in our own codebase." | A LibreOffice-derived engine runs on the server and draws the document; the browser shows the result. |
| Collabora's parts | [online.mirror README](https://github.com/CollaboraOnline/online.mirror) via `gh api` | "engine/" is "The document rendering engine"; "kit/" is "The client which lives in its own chroot and renders documents" | The server must read every document in the clear to render it: not compatible with end-to-end encryption. |

## What it would take

**Per format, in Rust today** (each claim ties to a row in part B):

| Format | Read | Write | Round trip (keep what is not understood) |
|---|---|---|---|
| .xlsx | mature (calamine, umya-spreadsheet) | mature (rust_xlsxwriter, umya-spreadsheet) | possible with umya-spreadsheet (reads and writes one model); not claimed in its README |
| .ods | mature reader (calamine) | small but working (spreadsheet-ods) | claimed by spreadsheet-ods |
| .docx | established (docx-rs since 2019), newer rdocx, office_oxide | established (docx-rs) | claimed only by BetterOffice (young) |
| .pptx | young only (office_oxide, ppt-rs, BetterOffice) | young only | claimed by ppt-rs and BetterOffice (both young) |
| .odt | young only (libreoffice-rs claims it); otherwise text extraction (anydoc) | young only (lo_odf, about 9,000 downloads) | none claimed |
| .odp | young only (libreoffice-rs); otherwise text extraction (pptx-to-md reads .odp to Markdown) | young only (libreoffice-rs) | none claimed |
| PDF out | n/a | mature (typst, printpdf, lopdf) | n/a |

Gaps to fill: a real .odt and .odp reader and writer, and a .pptx writer with years behind it. Per
the house rule "build what does not exist", these would be HumanityOS crates on quick-xml and zip,
with LibreOffice headless (part C) as an optional separate download for conversion meanwhile.

**The CRDT candidates, ranked by fit** (permissive licence, Rust, works offline, rich text). The
order of the first three is a judgement; the facts behind it are in part D.

1. **Yrs.** MIT; Rust core; offline and peer to peer (the Yjs README, whose protocol Yrs keeps);
   rich text through formatting attributes; snapshots and undo. It speaks the same protocol as Yjs,
   so the website mirror could use the JavaScript original and still merge with the desktop app, and
   an Apache-2.0 office editor (BetterOffice) already builds .docx, .xlsx and .pptx co-editing on it.
2. **Loro.** MIT; Rust; P2P, "Local Availability", a rich text CRDT and full history ("Time Travel");
   version 1.x and very active. Its web build is WebAssembly of the same Rust code.
3. **Automerge.** MIT; Rust core; offline-first, rich text with marks and blocks, the whole history
   kept locally. Slightly smaller download count than Loro or Yrs.
4. **diamond-types.** ISC; Rust; plain text only, and its own README says the published crate is
   out of date.
5. **cola.** MIT; Rust; plain text only.
6. **crdts.** Apache-2.0; general building blocks; no release since 2023 and no text type claimed.

Why a CRDT and not OT for HumanityOS: OT needs one server that sees every change in the clear
(Etherpad's design), while a CRDT merges on each person's device. That lets every edit be signed
and end-to-end encrypted, CryptPad-style, and relayed by any federated server without the server
reading it (CryptPad's about page), and keeps editing working offline. A server-rendered editor
like Collabora cannot be end-to-end encrypted by its design.

## Robots.txt read

- www.ecma-international.org and ecma-international.org: one group "User-agent: *", "Crawl-delay: 10", nothing disallowed (requests were spaced out).
- www.iso.org: answered `robots.txt` itself with a 403 challenge page; treated as not readable, no ISO page read.
- standards.iso.org: "User-agent: *" / "Disallow: /"; barred, not read.
- www.oasis-open.org: "User-agent: *" allows "/" apart from listed paths (committee downloads, search, wp-admin); no Claude group. Pages read were not on the list.
- docs.oasis-open.org: only commented-out lines and sitemaps; no rules.
- learn.microsoft.com: "User-agent: *" with disallows under /answers/ and a few APIs; the openspecs pages are allowed.
- crates.io: "User-agent: *" / "Disallow:" (nothing barred).
- github.com: a group naming "Claude-User" with "Crawl-delay: 1" and the same disallows as everyone (archives, raw, blame and the like); facts were read through `gh api`, api.github.com has no robots.txt (404).
- docs.rs: "User-agent: *" barring /releases and semver redirect URLs only (no docs.rs page was needed in the end).
- help.libreoffice.org: "User-agent: *" bars numbered versions, "Allow: /latest/"; the /latest/ page was read.
- www.libreoffice.org: no robots.txt (404), so no rules.
- cryptpad.org: "User-agent: *" / "Allow: /".
- docs.cryptpad.org: the connection failed (no answer), so nothing on that host was read.
- etherpad.org: no robots.txt (the address returned an HTML not-found page); no page of the site was read, the repository was used instead.
- www.collaboraonline.com: "User-agent: *" / "Disallow:" (nothing barred).
- sdk.collaboraonline.com: names "Claude-User" and "ClaudeBot" with "Disallow: /", and "User-agent: *" / "Disallow: /"; barred, not read.
- automerge.org: only comments (a content-signals preamble with no signals set); no rules.
- docs.yjs.dev: "User-agent: *" "Allow: /" with "Content-Signal: ai-train=yes, search=yes, ai-input=yes" (not needed in the end).
- loro.dev: "User-agent: *" "Allow: /" (not needed in the end).
- mupdf.com: "User-agent: *" allows "/" but bars /docs/, /repos/ and /downloads/archive/.
- typst.app: "User-agent: *" bars app, project and sign-in paths (not needed in the end).
- webstore.iec.ch: an empty robots.txt (no rules); www.iec.ch answered 403.

## Could not read

- https://www.iso.org/ (any page): the edition dates of ISO/IEC 29500-1 to -4 and ISO/IEC 26300, and their declared-patent notes. Its robots.txt is behind a challenge page.
- https://standards.iso.org/ittf/PubliclyAvailableStandards/: whether ISO/IEC 29500 parts are on ISO's free "publicly available" list. Barred by robots.txt.
- https://webstore.iec.ch/en/catalogsearch/result/?q=29500: IEC's co-published catalogue entries for the same dates; the results are drawn by scripts and came back empty.
- https://docs.cryptpad.org/: CryptPad's technical documentation on how its office apps sync; the host did not answer.
- https://sdk.collaboraonline.com/: Collabora's developer documentation; barred by robots.txt.
- https://mupdf.com/licensing: 404; MuPDF's commercial licence route (its page links to artifex.com/licensing, not read).
- The Ecma patent declarations for ECMA-376 (PDFs from Adobe, Microsoft and Intel): which Ecma option (free of charge, or reasonable terms) each chose. Not downloaded; the brief allows pages only.
- The copyright notice inside the ECMA-376 zip files: whether Ecma's schemas may be copied into a program. Not downloaded.
- Google's Pdfium licence (needed only if pdfium-render were chosen).
