# May HumanityOS read and write Office Open XML (.docx, .xlsx, .pptx) and OpenDocument files without a licence, a royalty or a patent risk?

**Research date: 14 November 2026 (Pacific).** Every source below was read on that date. Each
carries its own published or last-updated date beside it, which is often much older. If you are
reading this later, compare those dates with whatever has changed since.

**I am not a lawyer. This is a reading of public sources, not legal advice.** Nobody qualified has
reviewed it. The technical side (which Rust libraries exist, how editing together works) is in the
research notes [`../research/2026-11-14-microsoft-services/office-formats-and-co-editing.md`](../research/2026-11-14-microsoft-services/office-formats-and-co-editing.md);
this file answers only the permission question.

## The question

May a CC0, free, open-source program (HumanityOS) open, edit and save Word, Excel and PowerPoint
files (Office Open XML: ECMA-376, also ISO/IEC 29500) and OpenDocument files (ODF, also ISO/IEC
26300) with code of its own, without signing a licence, paying a royalty, or carrying a real
patent risk?

## The short answer

**Very probably yes, with no licence to sign and no royalty to pay; the patent risk is low but not
zero.**

- **Both specifications are free to read.** Ecma links each part of ECMA-376 for download with no
  price or sign-in; OASIS publishes ODF 1.4 openly.
- **Microsoft has promised, irrevocably and without anyone signing anything, not to assert its
  patents** against implementations of ECMA-376, ISO/IEC 29500 (2008 and 2012 editions), ODF 1.0 to
  1.3, and its own extension documents for .docx, .xlsx and .pptx ([MS-DOCX], [MS-XLSX],
  [MS-PPTX]). Its FAQ says the promise covers open-source and partial implementations. It stops
  only for someone who sues Microsoft for patent infringement over the same specification.
- **Everyone who wrote ODF agreed to license their essential patents royalty-free**, with no
  further conditions (OASIS's "RF on Limited Terms" mode), and Sun gave a separate covenant in 2005.
- **What is not covered:** patents held by anyone outside these commitments. Microsoft's promise
  says in terms that it is no assurance about other people's patents. Ecma's rule lets a patent
  holder ask "reasonable" terms, which may include a fee; three companies (Adobe, Microsoft, Intel)
  declared for ECMA-376's early editions, and which option each chose was not read.
- **Not read at all:** ISO's own pages (its site refuses automated reading), so ISO's patent
  declarations for ISO/IEC 29500 and 26300 are unknown. A person with a browser can check them in
  minutes (below).

None of the sources read mentions a royalty or a licence programme for implementing either format.

## Finding 1: ECMA-376 is current at its 5th edition and free to download

- Source: [Ecma, ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/),
  page published 2021-12-08, modified 2022-01-26.
- Quotation: "5th edition, December 2021" and "ECMA-376 contains 4 parts but only part 2 has been
  adopted in the last edition of the Standard." The parts: Part 1 "5th edition, December 2016",
  Part 2 "5th edition, December 2021", Part 3 "5th edition, December 2015", Part 4 "5th edition,
  December 2016". The same page gives "ISO/IEC number 29500".
- Observation (not a quotation): the page links "Download part 1" to "Download part 4" directly; the
  part 1 zip answered HTTP 200 with no sign-in (43,501,721 bytes, last modified 2 November 2022;
  headers only checked).

## Finding 2: Microsoft's Open Specification Promise covers the formats and its own extensions

- Source: [Microsoft, Open Specification Promise](https://learn.microsoft.com/en-us/openspecs/dev_center/ms-devcentlp/1c24c7c8-28b0-4ce1-a47d-95fe1ff504bc),
  "Published: September 12, 2006 | Revised: February 24, 2023"; page footer "Last updated on
  2023-03-16".
- The promise: "Microsoft irrevocably promises not to assert any Microsoft Necessary Claims against
  you for making, using, selling, offering for sale, importing or distributing any implementation
  to the extent it conforms to a Covered Specification ("Covered Implementation"), subject to the
  following."
- Who receives it: "This is a personal promise directly from Microsoft to you".
- When it ends: "If you file, maintain or voluntarily participate in a patent infringement lawsuit
  against a Microsoft implementation of such Covered Specification, then this personal promise does
  not apply with respect to any Covered Implementation of the same Covered Specification made or
  used by you."
- Its limit: "This promise is not an assurance either (i) that any of Microsoft issued patent claims
  covers a Covered Implementation or are enforceable or (ii) that a Covered Implementation would not
  infringe patents or other intellectual property rights of any third party."
- Covered, under "Open XML and Open Document File Formats": "Office Open XML 1.0 - Ecma-376",
  "Office Open XML ISO/IEC 29500:2008", "Office Open XML ISO/IEC 29500:2012", and "OpenDocument
  Format for Office Applications" v1.0, v1.1 (with ISO/IEC 26300:2006 and its corrigenda and
  amendment), v1.2 and "v1.3 OASIS".
- Future versions: "As long as Microsoft participates in their revision process to completion,
  Microsoft irrevocably commits to apply the Open Specification Promise to future versions of the
  below listed specifications."
- Version rule: "This promise applies to the identified version of the following specifications."
- Covered, under "Office File Formats": "[MS-DOCX]: Word Extensions to the Office Open XML File
  Format (.docx) Specification", "[MS-XLSX]: Excel Extensions to the Office Open XML SpreadsheetML
  File Format (.xlsx) Specification", "[MS-PPTX]: PowerPoint Extensions to the Office Open XML File
  Format (.pptx) Specification". These list no version.
- Covered as implementer notes: "Office Implementation Information for ISO/IEC 29500 Standards
  Support [MS-OI29500]", applying to "Office Open XML ISO/IEC 29500:2008", ":2012" and
  "Office Open XML ISO/IEC 29500:2016".

## Finding 3: the promise needs no signature and covers open-source and partial work

- Source: [Microsoft, Open Specification Promise FAQ](https://learn.microsoft.com/en-us/openspecs/dev_center/ms-devcentlp/03347966-f8ff-4d53-a05e-63419d4132e2),
  "Last updated on 2023-03-16".
- Quotations: "No one needs to sign anything or even reference anything."; "It is intended to
  enable open source implementations"; "The OSP applies whether you have a full or partial
  implementation."; on the GPL: "The OSP applies to developers, distributors, and users of Covered
  Implementations without regard to the development model that created such implementations, or
  the type of copyright licenses under which they are distributed".
- The same FAQ adds a caution in Microsoft's own words: "Because open source software licenses can
  vary you may want to consult with your legal counsel to understand your particular legal
  environment."

## Finding 4: the Microsoft extension documents are current and point back to the promise

- Sources and dates: [MS-DOCX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/b839fe1f-e1ca-4fa6-8c26-5954d0abbccd),
  published version 8/18/2026 (revision 23.0); [MS-XLSX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/2c5dee00-eff2-4b22-92b6-0738acd4475e)
  10/16/2026 (30.0); [MS-PPTX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/efd8bb2d-d888-4e2e-af25-cad476730c9f)
  8/20/2024 (25.0); [MS-OI29500](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/1fd4a662-8623-49c0-82f0-18fa91b413b8)
  8/18/2026 (25.0).
- Patents notice (MS-DOCX page): "Neither this notice nor Microsoft's delivery of this
  documentation grants any licenses under those patents or any other Microsoft patents. However, a
  given Open Specifications document might be covered by the Microsoft Open Specifications Promise
  or the Microsoft Community Promise."
- Copyright notice (same page): "you can make copies of it in order to develop implementations of
  the technologies that are described in this documentation" and "You can also distribute in your
  implementation, with or without modification, any schemas, IDLs, or code samples that are
  included in the documentation."

## Finding 5: ODF's committee works under a royalty-free patent mode

- Source: [OASIS, OpenDocument Version 1.4 Part 1](https://docs.oasis-open.org/office/OpenDocument/v1.4/OpenDocument-v1.4-part1-introduction.html),
  an OASIS Standard dated 6 October 2025 (its [namespace page](https://docs.oasis-open.org/office/OpenDocument/v1.4/OpenDocument-v1.4.html):
  "Stage: OASIS Standard (OS)", "Date: 6 October 2025").
- Quotation: "This specification is provided under the RF on Limited Terms Mode of the OASIS IPR
  Policy , the mode chosen when the Technical Committee was established."
- Source: [OASIS ODF TC, IPR page](https://www.oasis-open.org/committees/office/ipr.php), undated
  page. Quotation: "All participants in this TC have agreed to the terms of the RF on Limited Terms
  Mode of the OASIS IPR Policy ."
- Source: [OASIS IPR Policy](https://www.oasis-open.org/policies-guidelines/ipr/), page modified
  2023-09-11; latest revision listed "Approved 07/31/2013". Section 10.2.1: each obligated party
  "covenants that, upon request and subject to Section 11, it will grant to any OASIS Party or third
  party: a nonexclusive, worldwide, non-sublicensable, perpetual patent license (or an equivalent
  non-assertion covenant)" and "without payment of royalties or fees". Section 10.2.3: "Obligated
  Parties may not impose any further conditions or restrictions beyond those specifically mentioned
  in Section 10.2.1". Section 10.2.1 also allows a reciprocity term: "such license may include a term
  requiring the Licensee to grant a reciprocal license to its Essential Claims (if any)".
- The same ODF 1.4 Part 1, Appendix A, lists among those who "have participated in the creation of
  this specification" five people at Microsoft, among them "Doug Mahugh, Microsoft Corporation".

## Finding 6: Sun's 2005 covenant for every ODF version

- Source: ODF TC IPR page (above), "Sun OpenDocument Patent Statement, submitted by Sun Microsystems,
  Inc., September 29, 2005".
- Quotation: "Sun irrevocably covenants that, subject solely to the reciprocity requirement
  described below, it will not seek to enforce any of its enforceable U.S. or foreign patents
  against any implementation of the Open Document Format for Office Applications (OpenDocument)
  v1.0 Specification , or of any subsequent version thereof".
- Its reciprocity: the covenant does not apply to anyone "that asserts, threatens or seeks at any
  time to enforce its own or another party's U.S. or foreign patents or patent rights against any
  OpenDocument Implementation."

## Finding 7: OASIS describes ODF as free to implement and as ISO/IEC 26300

- Source: [OASIS OpenDocument TC page](https://www.oasis-open.org/tc-opendocument/), page modified
  2026-11-03.
- Quotations: "published as an ISO/IEC international standard (ISO/IEC 26300)"; "freely available
  specifications that anyone can implement without licensing fees or restrictions"; it is
  "implemented by numerous applications including LibreOffice, Apache OpenOffice, Microsoft Office,
  Google Docs, and many others."
- Weight: this is OASIS's own summary, not its patent policy; Finding 5 is the binding text.
- Copying the spec text: ODF 1.4 Part 1 notices allow "derivative works that comment on or otherwise
  explain it or assist in its implementation" to be "copied, published, and distributed, in whole or
  in part, without restriction of any kind, provided that the above copyright notice and this
  section are included".

## Finding 8: Ecma's patent rule is "reasonable terms", free only if the holder chose it

- Source: [Ecma Code of Conduct in Patent Matters](https://ecma-international.org/policies/by-ipr/code-of-conduct-in-patent-matters/),
  "Version 2 (approved by the Ecma GA in June 2016)", page modified 2022-01-12.
- Aim: "Ecma desires to develop standards for which licenses for any essential patents are
  available on a non-discriminatory basis and on reasonable terms and conditions."
- Option 2.1: "The patent holder is prepared to grant licenses free of charge to other parties on a
  non-discriminatory basis on reasonable terms and conditions."
- Option 2.2: "The patent holder is prepared to grant licenses to other parties on a
  non-discriminatory basis on reasonable terms and conditions."
- A participating member who files nothing (4.3): "is obliged to license any claims in patents or
  patent applications required to implement the Standard on a reasonable and non-discriminatory
  basis."
- Ecma gives no legal opinions (6): "Ecma International shall not provide legal opinions about
  evidence, validity or enforceability of patents, or whether a claim is required to implement a
  standard."
- Source: [Ecma list of patent statements](https://ecma-international.org/policies/ecma-list-of-patent-statements/),
  page modified 2021-01-29. ECMA-376 rows: Adobe Systems (22 November 2006, editions 1 to 3),
  Microsoft (4 December 2008, editions 1 and 2), Intel (24 June 2011, edition 3), each "Yes" under
  "Prepared to grant license according to Ecma policy ?". No row for the 4th or 5th edition. The
  list does not say which option each chose; that is in each declaration PDF, not read.

## What is still unknown, and what would settle it

1. **ISO's patent declarations and edition dates for ISO/IEC 29500 (parts 1 to 4) and ISO/IEC
   26300.** The ISO pages refuse automated reading. Settle: a person opens each standard's page on
   iso.org and the ISO patent database, and copies the declarations and dates into a dated
   addendum. Cost: about fifteen minutes, free.
2. **Which option (free, or reasonable terms) Adobe, Microsoft and Intel chose for ECMA-376.**
   Settle: read the three declaration PDFs linked from Ecma's list (a download, so the operator is
   asked first). Cost: minutes, free.
3. **Whether Microsoft's promise reaches ECMA-376's 5th edition, ISO/IEC 29500:2016 and ODF 1.4.**
   The listed versions stop at 29500:2012 and ODF 1.3; the promise extends to "future versions"
   only "As long as Microsoft participates in their revision process to completion". Microsoft names
   29500:2016 in its implementer notes, and Microsoft staff are acknowledged in ODF 1.4. Whether that
   is "participation to completion" is a reading of contract-like words. Settle: ask a lawyer, or
   ask Microsoft in writing (the documentation's notice names a contact for written licences). Cost:
   a lawyer's hour, or a free letter and a wait. In practice it matters little: ODF's own RF mode
   (Finding 5) binds Microsoft as a participant regardless.
4. **Patents held by anyone outside these commitments.** No source can settle this; Microsoft's
   promise disclaims it. Settle: a freedom-to-operate search by a patent lawyer, which costs
   thousands of dollars and is only worth it if someone ever raises a specific patent. A lawyer
   would need: the features we implement, the jurisdictions we distribute in, and any letter
   received.
5. **Whether Ecma's XML schema files may be copied into the app.** The copyright notice inside the
   ECMA-376 zip was not read. Microsoft's documents allow their schemas in implementations
   (Finding 4) and OASIS allows copies with its notice (Finding 7). Settle: read the notice page of
   ECMA-376 Part 1 (a 43 MB download, asked first). Cost: minutes. Writing our own reader without
   copying the schemas avoids the question.
6. **Trademarks.** Whether the app may say "Word", "Excel" or "PowerPoint" in its interface was not
   researched. Settle: a separate dated finding on Microsoft's trademark guidelines. Naming files by
   their extension (.docx) sidesteps it, and the FAQ says implementers "do not need to make any
   mention of or reference to Microsoft".

## Sources I could not reach

- https://www.iso.org/ (every page): ISO/IEC 29500 and 26300 edition dates and patent declarations.
  Its `robots.txt` is itself behind a 403 challenge page, so nothing on the host was read.
- https://standards.iso.org/ittf/PubliclyAvailableStandards/: whether these standards are on ISO's
  free list. Its `robots.txt` says "Disallow: /" for everyone.
- https://webstore.iec.ch/en/catalogsearch/result/?q=29500: IEC's catalogue (IEC co-publishes these
  standards). The results are drawn by scripts and came back empty.
- The ECMA-376 patent declaration PDFs from Adobe, Microsoft and Intel (linked from Ecma's list): the
  licensing option each chose. Not downloaded.
- The copyright notice inside the ECMA-376 zip files: schema copying. Not downloaded.

## What it means for this project

**Certain (from the quotations above):**

- Both specifications can be read without paying: ECMA-376 by direct download, ODF from OASIS.
- Microsoft has made an irrevocable, no-signature promise not to assert its necessary patent claims
  against conforming implementations, full or partial, of ECMA-376, ISO/IEC 29500:2008 and :2012,
  ODF 1.0 to 1.3, and [MS-DOCX], [MS-XLSX] and [MS-PPTX], open source included. It is personal to
  each implementer, distributor and user, so HumanityOS does not have to pass it on (CC0 could not),
  and it ends only for someone who sues Microsoft over the same specification.
- Every ODF committee participant is bound to license essential patents royalty-free, on request,
  with no further conditions; Sun added a covenant for every ODF version.
- Ecma's rule allows a patent holder to ask "reasonable" terms, which need not be free.
- Microsoft's promise says nothing about anyone else's patents.
- No source read names a royalty for implementing either format.

**Judgement calls (mine, to be weighed, not facts):**

- The patent risk of reading and writing these formats with our own code is low. Many programs
  already implement them, open-source ones among them (OASIS's list in Finding 7; its TC page
  headlines ODF 1.4 as "Marking 20 Years of Interoperable Document Innovation"), and the format
  owners have made public, irrevocable commitments.
- HumanityOS holds no patents and will not sue anyone, so the "stops if you sue" clauses cost the
  project nothing.
- Implement the plain standards first, then Microsoft's [MS-DOCX], [MS-XLSX] and [MS-PPTX]
  extensions where real files need them for a faithful round trip; all three are on the promise's
  list.
- Write our own readers and writers (or use the permissive crates in the research notes) rather
  than copying Ecma's schema files until the copyright notice is read.
- Call the formats by their extensions in the interface until a trademark finding exists.
- A lawyer is needed only if a specific patent holder ever writes to the project; what a lawyer
  would need is listed in item 4 above.

**I am not a lawyer, and this is not legal advice.** It records what the named sources said on 14
November 2026 so a later reader can tell whether the world changed or we misread it.
