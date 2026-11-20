# The licences of the packages HumanityOS links, held against distributing an AGPL-3.0 program

**Researched 2026-11-20** (every source below was read on 20 November 2026, Pacific time). The
licence texts quoted are the copies the packages themselves carry, read from the local cargo
registry, at the versions `Cargo.lock` pinned that day; a later version of a package can change its
licence, so re-run the scan below before relying on this for a later lock.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** What it says the project should do is marked as a judgement call.

## The question

May every package linked into HumanityOS's programs be distributed as part of a program whose own
code is under the GNU Affero General Public License version 3 or later (LICENSING.md), and what does
each licence ask in return?

## The short answer

Yes, as far as the licences' own words go, with one family to note. Of the 746 packages
`scripts/gen-third-party-notices.js` finds in the six programs a release ships (2026-11-20), none is
under a GPL-family, unknown, proprietary or no-licence term. 712 can be taken under a licence that
asks only for its notice to be kept (MIT, MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib,
0BSD, Unlicense, CC0-1.0, BSL-1.0). The other 34:

| Licence | Packages | What they are used for | What its text says about combining |
|---|---|---|---|
| MPL-2.0 | symphonia 0.6.1 and its nine codec and format crates; triple_buffer 9.0.0 | decoding audio files (FLAC, MP3, Vorbis, OGG, WAV) for the music and sounds, directly and through kira; triple_buffer inside kira | names the AGPL 3.0 as a "Secondary License" it may be combined with (finding 1); no source file carries the notice that would opt out |
| Unicode-3.0 | the ICU4X stack (icu_collections, icu_locale_core, icu_normalizer and _data, icu_properties and _data, icu_provider, litemap, potential_utf, tinystr, writeable, yoke and yoke-derive, zerofrom and zerofrom-derive, zerotrie, zerovec and zerovec-derive), and unicode-ident and unicode-normalization (each also MIT OR Apache-2.0) | international web addresses (idna, under the url crate that reqwest and tower-http use); the recovery phrase's Unicode normalisation (unicode-normalization, under bip39) | notice-only permission (finding 2) |
| CDLA-Permissive-2.0 | webpki-roots 0.26.11 and 1.0.11 | the list of trusted certificate authorities for secure connections (reqwest, tokio-tungstenite) | share the data with the agreement's text (finding 3) |
| OFL-1.1 AND Ubuntu-font-1.0 (and MIT OR Apache-2.0) | epaint_default_fonts 0.31.1 | the fonts egui draws every word of the app with, compiled into the program | the fonts may be bundled with any software but stay under their own licence (finding 4) |

Every one of them asks for its notice or licence text to travel with the program, which
THIRD-PARTY-NOTICES.txt (shipped beside the program since 2026-11-20) now does.

## Findings

### 1. MPL-2.0 permits combination with the AGPL 3.0

Source: the Mozilla Public License, version 2.0, as symphonia-core 0.6.1 carries it (`LICENSE` in
the crate; the licence text carries no date of its own). Section 1.12:

> "Secondary License"
>     means either the GNU General Public License, Version 2.0, the GNU
>     Lesser General Public License, Version 2.1, the GNU Affero General
>     Public License, Version 3.0, or any later versions of those
>     licenses.

Section 3.3:

> You may create and distribute a Larger Work under terms of Your choice,
> provided that You also comply with the requirements of this License for
> the Covered Software. If the Larger Work is a combination of Covered
> Software with a work governed by one or more Secondary Licenses, and the
> Covered Software is not Incompatible With Secondary Licenses, this
> License permits You to additionally distribute such Covered Software
> under the terms of such Secondary License(s)

Checked the same day: no file under `src/` of any symphonia crate or of triple_buffer 9.0.0 holds the
words "Incompatible With Secondary Licenses" (the Exhibit B notice that would opt out). symphonia's
source files open with "This Source Code Form is subject to the terms of the Mozilla Public License,
v. 2.0", Exhibit A.

### 2. Unicode-3.0 asks only for its notice

Source: `LICENSE` of icu_collections 2.4.0, headed "UNICODE LICENSE V3":

> Permission is hereby granted, free of charge, to any person obtaining a
> copy of data files and any associated documentation (the "Data Files") or
> software and any associated documentation (the "Software") to deal in the
> Data Files or Software without restriction, including without limitation
> the rights to use, copy, modify, merge, publish, distribute, and/or sell
> copies of the Data Files or Software, and to permit persons to whom the
> Data Files or Software are furnished to do so, provided that either (a)
> this copyright and permission notice appear with all copies of the Data
> Files or Software, or (b) this copyright and permission notice appear in
> associated Documentation.

### 3. CDLA-Permissive-2.0 asks only that its text go with the data

Source: `LICENSE` of webpki-roots 1.0.11, "Community Data License Agreement - Permissive - Version
2.0":

> 2.1. A Data Recipient may share Data, with or without modifications, so
> long as the Data Recipient makes available the text of this agreement
> with the shared Data.

and

> 3.1. This agreement does not impose any restriction or obligations
> with respect to the use, modification, or sharing of Results.

### 4. The two font licences allow bundling with any software, and keep the fonts under themselves

Source: `fonts/OFL.txt` of epaint_default_fonts 0.31.1, "SIL OPEN FONT LICENSE Version 1.1 - 26
February 2007":

> 2) Original or Modified Versions of the Font Software may be bundled,
> redistributed and/or sold with any software, provided that each copy
> contains the above copyright notice and this license.

> 5) The Font Software, modified or unmodified, in part or in whole,
> must be distributed entirely under this license, and must not be
> distributed under any other license.

Source: `fonts/UFL.txt` of the same crate, "UBUNTU FONT LICENCE Version 1.0":

> 1) Each copy of the Font Software must contain the above copyright
> notice and this licence.

> 4) The Font Software, modified or unmodified, in part or in whole, must
> be distributed entirely under this licence, and must not be distributed
> under any other licence.

And the AGPL's own word on a work that travels beside a covered one, section 5 of `LICENSE`:

> A compilation of a covered work with other separate and independent
> works, which are not by their nature extensions of the covered work,
> and which are not combined with it such as to form a larger program,
> in or on a volume of a storage or distribution medium, is called an
> "aggregate" if the compilation and its resulting copyright are not
> used to limit the access or legal rights of the compilation's users
> beyond what the individual works permit.

### 5. Apache-2.0 code may go into a GPLv3 work

Source: https://www.apache.org/licenses/GPL-compatibility.html (the page shows "Copyright © 2026 The
Apache Software Foundation" and no other date; robots.txt allows it):

> Apache 2 software can therefore be included in GPLv3 projects

and, the other way:

> However, GPLv3 software cannot be included in Apache projects.

## What is still unknown, and what would settle it

- **Whether a font compiled INTO the program (epaint_default_fonts uses `include_bytes!`) is an
  "aggregate" in the AGPL's sense or part of a "larger program".** The OFL and UFL allow bundling
  with any software; the AGPL's aggregate clause speaks of works "not combined with it such as to
  form a larger program". Settled by the FSF's licence list and GPL FAQ entries on fonts (which
  could not be read today, below), or by a lawyer.
- **Whether the FSF lists CDLA-Permissive-2.0 and Unicode-3.0 as GPL-compatible.** Their texts
  impose only a notice condition, of the kind the AGPL's section 7(b) lets a work add ("Requiring
  preservation of specified reasonable legal notices or author attributions in that material", read
  in `LICENSE` the same day); the FSF's own verdict would settle it. Same source, below.
- **Apache-2.0 into the AGPL 3.0 by name.** The ASF's page speaks of GPLv3, not the AGPL 3.0. The
  FSF's licence list names each licence's compatibility directly.

## Sources not reached

- https://www.gnu.org/robots.txt and so the FSF's licence list
  (https://www.gnu.org/licenses/license-list.html): HTTP 429 "Too Many Requests" twice on
  2026-11-20, so the list was not read (the rule: robots.txt first, and a refusal is not worked
  around). Wanted for the FSF's verdict on each licence family above, by name; it would also close
  the open question in `docs/reference/findings/2026-11-14-licence-change.md`.

## What it means for this project

**Certain (from the sources):**
- No linked package is under a licence that forbids combining it with an AGPL-3.0 program by its
  own words: MPL-2.0 names the AGPL 3.0 as a Secondary License; Unicode-3.0 and CDLA-Permissive-2.0
  ask only for their notice or text; the two font licences allow bundling with any software.
- Each asks for its notice or text to travel with the program; THIRD-PARTY-NOTICES.txt carries every
  licence and notice file each package ships, beside the program and in Settings > Credits.
- The fonts stay under the OFL and the UFL whatever licence the program is under; the AGPL does not
  relicense them, and nothing in LICENSING.md claims it does ("Other people's work ... Their own
  licence, never ours").

**Judgement calls:**
- The MPL-2.0 symphonia crates are used unchanged from crates.io, so the MPL's file-level copyleft
  asks nothing more than offering their source, which crates.io and the notices' repository links
  do. A change to a symphonia file would have to be shared under the MPL.
- Nothing here needs a dependency changed. If the FSF's list, once read, called the font arrangement
  a problem, the fix would be egui's choice of fonts (data the app loads), not the code's licence.
- Re-run the scan whenever `Cargo.lock` gains a licence family not in the table above:
  `node scripts/gen-third-party-notices.js --report` prints every licence expression with its count.
