# May HumanityOS read the MP4 container itself, to play the AV1 video inside it?

**Research date: 3 November 2026.** Every source below was read on that date. Each carries its
own published or last-updated date beside it, which is often much older. If you are reading this
later, compare those dates with whatever has changed since.

**I am not a lawyer. This is a reading of public sources, not legal advice.** Nobody qualified has
reviewed it. It extends [2026-09-19-video-codec-licensing.md](2026-09-19-video-codec-licensing.md),
which covers the codecs (H.264 and AAC) and not the container they travel in, and changes none of
its conclusions. The codecs we decode ourselves, AV1 and Opus, are covered by
[`docs/design/media-player.md`](../../design/media-player.md); the container is the new question.

## The question

A video posted in chat (BUG-228) is an MP4 file holding AV1 video. Our player decodes AV1 already,
but reads only the WebM/Matroska container. Is reading the MP4 container (ISO/IEC 14496-12, the ISO
base media file format, and ISO/IEC 14496-14, the MP4 file format) free to implement and ship in
free, open-source software, or does a patent or a pool cover the container itself, as opposed to
the codecs inside it?

## The short answer

**Yes, probably: no one licenses the MP4 container today, and the patents once tied to it have
expired.** None of the three video patent pools lists a programme for a file format (Via LA, Access
Advance and Sisvel, all read today). The MPEG-4 Systems pool, which once covered the standard that
defined the first MP4 file format, had stopped offering licences by 2011 and is gone from Via LA's
list. The two patents an analysis of that pool tied to the base media file format both expired in
2018. AOMedia's binding of AV1 into the format is one of its Final Deliverables, released under its
royalty-free patent commitments. Firefox has shipped an MP4 reader of its own for years.

**One thing is open.** The 2026 edition of ISO/IEC 14496-12 says ISO and IEC "had received notice
of (a) patent(s)" that may be needed to implement it. Who declared, for what, and on which terms is
in ISO's patent database, which refuses automated reading. A person with a browser can read it in
minutes (below, "What is still unknown").

**The crate we would use is permissive:** `mp4-atom` 0.17.0 is MIT OR Apache-2.0 (its licence
files opened and read).

## Finding 1: the standard itself says patents were declared

Source: ISO/IEC 14496-12:2026, "Information technology — Coding of audio-visual objects — Part 12:
ISO base media file format", Eighth edition, 2026-04. Read as the publisher's free preview hosted by
the Czech standards agency, <https://www.technickenormy.cz/publicdoc/iec_previews/4213250.pdf>.

- The Foreword says: "ISO and IEC had received notice of (a) patent(s) which may be required to
  implement this document."
- It points to the database for the details: "may be obtained from the patent database available
  at www.iso.org/patents".
- The text is copyrighted ("All rights reserved"). That restricts copying the document, not
  writing a program that reads files in the format; we copy nothing from it.

## Finding 2: who had declared, as of 2010

Source: a reader comment by "brouhaha" on LWN.net, "Ogg and the multimedia container format
struggle", 8 April 2010, <https://lwn.net/Articles/382611/>. A secondary source, sixteen years
old, and a list of names only.

- For ISO/IEC 14496-12:2008: "The list of companies registered with the ISO and IEC in that
  standard is:" Apple, Matsushita Electric Industrial Co., Ltd., and Telefonaktiebolaget LM
  Ericsson. No licensing terms are given.

## Finding 3: the old MPEG-4 Systems pool reached the first MP4 file format

Sources:
- "Final Terms of MPEG-4 Visual and Systems Patent Portfolio Licenses Decided", Design & Reuse,
  15 July 2002, <https://us.design-reuse.com/news/3580/final-terms-mpeg-4-visual-systems-patent-portfolio-licenses-decided.html>.
- Library of Congress, "MPEG-4 File Format, Version 1" (fdd000037), last significant update
  2011-12-01, <https://loc.gov/preservation/digital/formats/fdd/fdd000037.shtml>.

- The pool covered the "MPEG-4 Systems Standard (ISO/IEC 14496-1:2001 [Part 1:Systems]" and its
  first amendment, at "US $0.15 per activated decoder" with an annual cap.
- The first MP4 file format was defined in "the 2001 edition of Part 1 of the MPEG-4 standard",
  so the pool's standard and the file format's first version were the same document. Version 2
  moved to Part 14.

## Finding 4: that pool stopped licensing, and its file-format patents have expired

Sources:
- W3C public-web-and-tv list, "Re: HTML5 Last Call May 2011 & DASH/Adaptive Streaming",
  16 February 2011: Mark Watson (Netflix),
  <https://lists.w3.org/Archives/Public/public-web-and-tv/2011Feb/0033.html>, and Silvia
  Pfeiffer's reply, <https://lists.w3.org/Archives/Public/public-web-and-tv/2011Feb/0035.html>.
- Cyril Concolato (Telecom ParisTech, a GPAC author), "MPEG-4 Systems Patent Pool Analysis",
  undated, <https://concolato.wp.imt.fr/?p=297>.
- Google Patents, US6134243A, <https://patents.google.com/patent/US6134243A/en>, and US6707944B1,
  <https://patents.google.com/patent/US6707944B1/en>.

- Watson: "To my knowledge there are no IP issues with the ISO Base Media File Format", and "The
  situation with mp4 may be different."
- Pfeiffer disagrees in part: "The systems pools show there are patents on the ISO Base Media File
  Format", adding "MPEG-4 systems still has a patent pool, even though licenses are not offered
  any more."
- Concolato names the base-format patents in the pool. Apple's US 6,134,243 relates to hint tracks
  only: "So this does not seem essential for clients nor for servers not relying on hint tracks."
  ETRI's US 6,707,944 concerns the degradation-priority tools, "only applicable when one uses one
  of these tools."
- Both are "Expired - Lifetime" on Google Patents: US 6,134,243 (filed 1998-08-25) with an
  anticipated expiration of 2018-08-25; US 6,707,944 (filed 1998-04-14), 2018-04-14.
- Where the two 2011 sources disagree: Watson speaks from practice (nobody was paying), Pfeiffer
  from the pool's existence. Concolato's analysis supports both: there were patents, and they did
  not reach a reader that uses neither hint tracks nor degradation priority. A player like ours
  uses neither. That weighting is a judgement.

## Finding 5: no pool lists the container today

Sources, all read 2026-11-03, none of the pages dated:
- Via LA, "Licensing Programs", <https://www.via-la.com/licensing-programs/>.
- Access Advance, home page, <https://accessadvance.com/>.
- Sisvel, "Licensing programmes", <https://www.sisvel.com/licensing-programmes>.
- Library of Congress, "MPEG-4 File Format, Version 2" (fdd000155), last significant update
  2025-04-11, <https://loc.gov/preservation/digital/formats/fdd/fdd000155.shtml>.

- Via LA lists fifteen programmes, among them "MPEG-2 Systems" but no MPEG-4 Systems and nothing
  naming a file format, MP4, ISOBMFF, CMAF or HEIF.
- Access Advance lists HEVC Advance, VVC Advance, the Multi-Codec Bridging Agreement, the Video
  Distribution Patent Pool and VCL Advance; none names a file format.
- Sisvel lists no file-format programme. It does list "Video coding platform: AV1", whose page
  says: "The licence offered covers any consumer product making use of the AV1 specification."
  That is about the codec, and it is outside this finding (see "What is still unknown").
- The Library of Congress says "MPEG-4 Visual, Systems, and Advanced Video Coding licensing is
  managed by VIA Licensing Alliance". It names no separate licensing for the container.
- **A negative finding, recorded as such:** nothing read today offers, or asks for, a licence for
  reading the MP4 container.

## Finding 6: the AV1 binding is an AOMedia deliverable under its royalty-free patent terms

Sources:
- AOMedia, "AV1 Codec ISO Media File Format Binding", v1.3.0, "AOM Final Deliverable, 3 April
  2024", <https://aomediacodec.github.io/av1-isobmff/v1.3.0.html>.
- AOMedia, Legal, <https://aomedia.org/about/legal/> (page footer 2026; no date on the text).
- Alliance for Open Media Patent License 1.0, <https://aomedia.org/license/patent-license/>
  (undated).

- The binding: "This document specifies the storage format for AV1 bitstreams in ISOBMFF tracks
  as well as in CMAF files." Its licence line: "Licensing information is available at
  http://aomedia.org/license/".
- AOMedia's legal page: "Participants in AOMedia Working Groups have adopted the Alliance for Open
  Media Patent License 1.0", committing their essential claims "in Final Deliverables adopted by
  that Working Group" as if each were a W3C Recommendation, which is the W3C's royalty-free mode.
- The licence itself is "no-charge, royalty-free", for an "Implementation", meaning "any
  implementation, including the Reference Implementation, that is an Encoder and/or a Decoder."
- What the binding says a sample is (the facts the reader is built on): "the sample data SHALL be a
  sequence of OBUs forming a Temporal Unit"; a sync sample "contains a Sequence Header OBU before
  the first Frame Header OBU"; and Temporal Delimiter OBUs "SHOULD NOT be used." So each sample is
  handed to the AV1 decoder as it is, which is what our decoder already takes from WebM.

## Finding 7: Opus in the same container

Source: "Encapsulation of Opus in ISO Base Media File Format", Version 0.6.8 (incomplete), last
updated 28 April 2016, <https://opus-codec.org/docs/opus_in_isobmff.html>. No licence statement on
the page; Opus's own royalty-free status is in `docs/design/media-player.md`.

- The sample entry carries an `OpusSpecificBox` (`dOps`): version, channel count, PreSkip, input
  sample rate, output gain, channel mapping.
- PreSkip "is not used for discarding the priming samples at the whole playback at all since it is
  informative only, and that task falls on the Edit List Box." So the reader takes the priming
  samples to drop from the sound track's edit list, and from PreSkip only when there is none.

## Finding 8: others ship MP4 readers in free software

Source: Mozilla, mp4parse-rust, README, <https://github.com/mozilla/mp4parse-rust> (last commit
2026-10-27, per GitHub's API).

- "Our primary interest is writing a pure-rust replacement for the track metadata parser needed
  by Firefox." It is MPL-2.0, and "Firefox will ship specific git revisions from the master
  branch." Firefox, a free and open-source browser, reads MP4 with it.

## Finding 9: the Rust MP4 readers on crates.io, and their licences

Source: the crates.io API (read 2026-11-03) for the version, its date and the declared licence;
for the four downloaded, the licence file in the published crate, opened and read.

| Crate | Latest, released | Licence | What it is |
|---|---|---|---|
| `mp4-atom` | 0.17.0, 2026-10-30 | MIT OR Apache-2.0 (`LICENSE-MIT`: "Copyright (c) 2024 Luke Curley"; `LICENSE-APACHE`) | Every box decoded to a type (AV1, Opus, AAC sample entries; the sample tables; fragments). Turning the tables into a list of samples is left to the caller. |
| `re_mp4` | 0.5.1, 2026-07-08 | MIT (`LICENSE`: "Copyright (c) 2020 Alfred Gutierrez", "Copyright (c) 2024 rerun.io") | Rerun's reader: a sample list for every track, AV1 and AAC, no Opus sample entry. |
| `mp4parse` | 0.17.0, 2023-05-29 | MPL-2.0 (`LICENSE`) | Firefox's parser. The crates.io release is three and a half years behind the repository. |
| `mp4box` | 0.14.0, 2026-09-16 | MIT (`LICENSE`) | A new parser and editor by the `mp4` crate's author, few users yet. |
| `mp4` | 0.14.0, 2023-08-01 | MIT (declared; not downloaded) | The original reader and writer; no AV1 sample entry. |
| `symphonia-format-isomp4` | 0.6.1, 2026-08-13 | MPL-2.0 (declared; not downloaded) | Sound only. |

- `mp4-atom`'s README: "This library provides encoding for the ISO Base Media File Format
  (ISO/IEC 14496-12)." Its box reader carries the comment "Don't use `with_capacity` on an
  untrusted size", and reads a box only as far as the bytes really present.

## What is still unknown, and what would settle it

1. **Who declared patents against ISO/IEC 14496-12:2026, for which parts, and on which terms.**
   *Settled by:* opening <https://www.iso.org/patents> in a browser and reading the rows for
   14496-12 (and 14496-14): the holder, the date, and the licensing option ticked (option 1 is free
   of charge, option 2 is reasonable and non-discriminatory). Free, about ten minutes, by a person:
   ISO answers automated readers with "403 Forbidden". If a declaration is option 2 for a part a
   player needs and is not yet expired, write a new finding and ask a lawyer.
2. **The Matsushita and Ericsson declarations of 2010.** Which patents, and whether they have
   expired. *Settled by:* the same database rows, then each patent's page on Google Patents.
3. **Sisvel's AV1 programme**, a codec question this finding does not answer: whether a royalty
   claimed on "any consumer product making use of the AV1 specification" reaches free software
   that decodes AV1 (our player already does, for WebM). *Settled by:* reading Sisvel's AV1
   licence agreement, linked from its programme page, into a finding of its own.

## Sources I could not reach

- <https://www.iso.org/iso-standards-and-patents.html>, ISO's patents page and its declaration
  spreadsheet: "403 Forbidden". Wanted: the 14496-12 and 14496-14 rows.
- <https://patents.iec.ch>, the IEC's patent database: "403 Forbidden". Same reason.
- <https://www.iso.org/standard/79110.html>, ISO's page for ISO/IEC 14496-14:2020: "403
  Forbidden". Wanted: its status and its Foreword's patent sentence (BSI's catalogue, read instead,
  lists it as published 27 Jan 2020, "Current, Under Review").
- <https://mpegif.org/lists-sub/pipermail/news/2003-February/000125.html>, the MPEG-4 Industry
  Forum's 2003 announcement of the Systems licence: HTTP 525. Wanted: the licence's own wording on
  the file format.
- <https://standards.iso.org/ittf/PubliclyAvailableStandards/>: the list of free ISO standards is
  closed ("The ISO/IEC Information Technology Task Force (ITTF) web site is now closed"), so
  whether 14496-12 is still free to download was not established.

## What this means for HumanityOS

### Certain

- No pool read today offers or asks for a licence to read the MP4 container. The pool that once
  covered the first MP4 file format no longer licenses, and the two base-format patents named in
  the analysis of it expired in 2018.
- The 2026 edition of 14496-12 says patents were declared against it; their holders and terms are
  not known to us yet.
- The AV1-in-MP4 binding is an AOMedia Final Deliverable under its royalty-free patent commitments.
- `mp4-atom` is MIT OR Apache-2.0, compatible with everything else the player uses.

### Judgement calls, mine, and arguable

- **Build it.** A reader of the container's core (the file type, the movie header, the tracks, the
  sample tables, the fragments, and the AV1, Opus and AAC sample entries) is what Firefox, Chromium
  and ffmpeg have shipped in free software for years, and no source read today describes a claim
  made against any of them for reading the container. The parts most likely to carry the newer declarations are the format's newer tools
  (image items, hint tracks for streaming servers, degradation priority), none of which a player
  reading a downloaded film touches.
- **Reading is lower exposure than the codecs ever were.** Every claim history found is about
  encoders, decoders and devices; no source read describes a royalty asked of a program that only
  reads boxes.
- **Read the ISO database rows before 1.0.** It is a ten-minute check that would turn the one open
  point into a certain one, and it is the obvious first thing anyone objecting would point at.

**I am not a lawyer, and this is not legal advice.** If anyone writes to the project about the MP4
container, this file is what we looked at, and when.
