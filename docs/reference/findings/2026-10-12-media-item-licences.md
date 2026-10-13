# Licences for Media items: what each choice means, what the default should be, and what a server needs to host them

**Researched 2026-10-12** (sources read between about 17:15 and 17:45 Pacific
time on 12 October 2026). Everything below was read on that day. The Creative
Commons licences quoted here have not changed since 2013, but platform terms,
the EU rules and court decisions do change; if you are reading this later,
check each source's own date against anything that has changed since.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such, and a
lawyer could disagree with any of it.

## The question

When a member uploads a picture, video, 3D model or file to a HumanityOS
server's Media page, what does each candidate licence (CC BY, CC BY-SA, CC
BY-NC, CC0, "All rights reserved") permit and require, which should be the
default, and what permission do the server's operator, and the project for its
own game and videos, need beyond the item's licence?

## Short answer

- **Recommended default: "All rights reserved"** (the project's reading). It is
  what the law already gives every uploader with no statement at all (US law;
  Berne: "not be subject to any formality"). It can be loosened later; a
  Creative Commons licence can never be taken back for copies already taken,
  and on a members-only item it would let any member republish the item
  publicly.
- **Every server needs its own, separate permission from each uploader**, in
  its terms: a limited, non-exclusive licence to store, copy, resize, show and
  serve the item to the audience the uploader chose, ending when it is removed.
  Large platforms do this (practice, not law). It also means the server never
  depends on how "NonCommercial" is read. **No HumanityOS server has such terms
  today.**
- **NonCommercial is unclear for a donation-funded project.** CC says the test
  is the use's purpose, not who the user is, and a survey it published found
  "no "pass" for not-for-profit organizations". A promotional video asking for
  donations is the riskiest case. **NC and "All rights reserved" items cannot go
  into the game as they stand**, because the game is CC0 and anyone may sell
  copies of it.
- **A separate opt-in "Let HumanityOS use this in the game and its videos, with
  credit" is a normal, sound pattern** (CC calls it dual licensing, or CC+). It
  must say who receives it (HumanityOS is not a legal entity), what it covers,
  how credit is given, whether forks may redistribute it, and whether and how it
  can be withdrawn: under US case law a free, non-exclusive permission is
  "revocable absent consideration" unless drafted otherwise.
- **For files a member uploads but does not own**, US safe harbour needs a
  designated agent registered with the Copyright Office ($6, renewed every
  three years, a street address), a public takedown address, a repeat-infringer
  policy, and expeditious removal. UK regulation 19 and EU DSA Article 6 work on
  the same knowledge-and-removal idea; DSA Article 16 also needs an electronic
  notice form.

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Long passages are trimmed to the words
that matter; nothing inside a quotation is reworded. Every quotation below was
checked by script against the text read on the day.

- **creativecommons.org** (licence deeds, legal code, FAQ, wiki) was fetched
  directly and its text extracted. The legal code pages carry no date; they say
  "Version 4.0" and point to an errata page. CC's own wiki page *License
  Versions* (https://wiki.creativecommons.org/wiki/License_Versions , "last
  edited on 4 January 2016") calls 4.0 "the current version 4.0, published
  November 2013", and nothing read on the day pointed to a later version.
- **US statutes** were read from the US Code, 2024 Edition, on govinfo.gov
  (`https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-<chapter>-<section>.htm`).
- **US Copyright Office** pages and circulars were read from copyright.gov
  (circular PDFs converted to text).
- **Berne Convention**: WIPO Lex, the authentic text as amended on 28 September
  1979.
- **UK**: legislation.gov.uk (revised text) and GOV.UK. The page sets each
  paragraph label, such as "(a)", apart from its words by layout; a space is
  shown after the label here.
- **EU**: EUR-Lex answered with a bot check (HTTP 202, empty page), so the DSA
  and the DSM Directive were read from the EU Publications Office's own copy of
  the Official Journal text (`https://publications.europa.eu/resource/celex/<CELEX>.ENG.xhtml`).
- **Court opinions** were read on CourtListener in a browser (the site refused
  direct requests).
- **Platform terms** (YouTube, GitHub, Flickr, Sketchfab) and one open-source
  platform's code (Pixelfed) are **practice, not law**, and are marked so.

---

## 1. What each candidate licence permits and requires

### 1.1 What CC BY, CC BY-SA and CC BY-NC (all version 4.0) have in common

Sources: the legal code of each,
https://creativecommons.org/licenses/by/4.0/legalcode.en ,
https://creativecommons.org/licenses/by-sa/4.0/legalcode.en ,
https://creativecommons.org/licenses/by-nc/4.0/legalcode.en (undated pages;
version 4.0, published November 2013; read 2026-10-12).

- **The grant** (section 2(a)(1), worded the same in all three):
  > the Licensor hereby grants You a worldwide, royalty-free, non-sublicensable, non-exclusive, irrevocable license to exercise the Licensed Rights in the Licensed Material to:

  For BY and BY-SA the list is "reproduce and Share the Licensed Material, in
  whole or in part; and" "produce, reproduce, and Share Adapted Material." BY-NC
  adds "for NonCommercial purposes only" to both.
- **Attribution** (section 3(a)(1)). Anyone who shares the material must
  > retain the following if it is supplied by the Licensor with the Licensed Material:

  namely
  > identification of the creator(s) of the Licensed Material and any others designated to receive attribution, in any reasonable manner requested by the Licensor (including by pseudonym if designated);

  plus a copyright notice, a notice referring to the licence, a notice referring
  to the disclaimer of warranties and a link to the material "to the extent
  reasonably practicable"; they must also say if they changed it, and say it is
  under this licence with its text or a link. How (section 3(a)(2)):
  > You may satisfy the conditions in Section 3(a)(1) in any reasonable manner based on the medium, means, and context in which You Share the Licensed Material.

  And the creator can ask to be left off (section 3(a)(3)):
  > If requested by the Licensor, You must remove any of the information required by Section 3(a)(1)(A) to the extent reasonably practicable.
- **Changing format is not changing the work** (section 2(a)(4)). This is what
  covers thumbnails, resized previews and transcoded video:
  > The Licensor authorizes You to exercise the Licensed Rights in all media and formats whether now known or hereafter created, and to make technical modifications necessary to do so.

  > For purposes of this Public License, simply making modifications authorized by this Section 2(a)(4) never produces Adapted Material.
- **No added restrictions** (section 2(a)(5)(B); 2(a)(5)(C) in BY-SA):
  > You may not offer or impose any additional or different terms or conditions on, or apply any Effective Technological Measures to, the Licensed Material if doing so restricts exercise of the Licensed Rights by any recipient of the Licensed Material.
- **No endorsement** (section 2(a)(6)): nothing in the licence is "permission to
  assert or imply" that a user is "connected with, or sponsored, endorsed, or
  granted official status by, the Licensor".
- **What is NOT licensed** (section 2(b)(1) and (2)), a negative finding that
  matters for pictures of people:
  > Moral rights, such as the right of integrity, are not licensed under this Public License, nor are publicity, privacy, and/or other similar personality rights;

  > Patent and trademark rights are not licensed under this Public License.
- **Music put to moving pictures is always an adaptation** (section 1(a)):
  > where the Licensed Material is a musical work, performance, or sound recording, Adapted Material is always produced where the Licensed Material is synched in timed relation with a moving image.
- **Only the owner can apply one.** The legal code's preamble, "Considerations
  for licensors":
  > Licensors should also secure all rights necessary before applying our licenses so that the public can reuse the material as expected.

  The CC FAQ (https://creativecommons.org/faq/ , page stamped "2025-05-13
  14:17:56 UTC"):
  > You should not apply a license to material that you do not own or that you are not authorized to license.

### 1.2 CC BY (Attribution)

- The deed, https://creativecommons.org/licenses/by/4.0/deed.en (undated):
  > Share — copy and redistribute the material in any medium or format for any purpose, even commercially.
- Its one further condition, on adaptations (legal code section 3(a)(4)):
  > If You Share Adapted Material You produce, the Adapter's License You apply must not prevent recipients of the Adapted Material from complying with this Public License.
- **What this means for us.** Anyone, including the project, may use a CC BY
  item in a game or a promotional video, for any purpose, with credit given in
  a reasonable manner for the medium.

### 1.3 CC BY-SA (Attribution-ShareAlike)

- Legal code section 3(b), which applies "if You Share Adapted Material You
  produce":
  > The Adapter’s License You apply must be a Creative Commons license with the same License Elements, this version or later, or a BY-SA Compatible License.

  > You may not offer or impose any additional or different terms or conditions on, or apply any Effective Technological Measures to, Adapted Material that restrict exercise of the rights granted under the Adapter's License You apply.
- The deed, https://creativecommons.org/licenses/by-sa/4.0/deed.en (undated):
  > ShareAlike — If you remix, transform, or build upon the material, you must distribute your contributions under the same license as the original.
- **Unchanged inclusion is not an adaptation.** CC FAQ, "If I create a
  collection that includes a work offered under a CC license, which license(s)
  may I choose for the collection?":
  > All Creative Commons licenses (including the version 4.0 licenses) allow licensed material to be included in collections such as anthologies, encyclopedias, and broadcasts. You may choose a license for the collection, however this does not change the license applicable to the original material.
- **What counts as an adaptation** is mostly left to copyright law. CC FAQ,
  "When is my use considered an adaptation?":
  > Whether a modification of licensed material is considered an adaptation for the purpose of CC licenses depends primarily on the applicable copyright law.
- **What this means for us.** A BY-SA model shipped unchanged in the game stays
  BY-SA inside an otherwise CC0 game and must be marked as such; a retextured
  or altered version must itself be released BY-SA. Whether a promotional video
  that shows a BY-SA model is "Adapted Material" (and so must itself be BY-SA)
  is not answered by any source read (see "Unknown"). For music it is answered:
  synching it to video always is.

### 1.4 CC BY-NC (Attribution-NonCommercial)

- The definition (legal code section 1(i)):
  > NonCommercial means not primarily intended for or directed towards commercial advantage or monetary compensation. For purposes of this Public License, the exchange of the Licensed Material for other material subject to Copyright and Similar Rights by digital file-sharing or similar means is NonCommercial provided there is no payment of monetary compensation in connection with the exchange.
- The deed, https://creativecommons.org/licenses/by-nc/4.0/deed.en (undated):
  > NonCommercial — You may not use the material for commercial purposes.
- Royalties are kept for commercial use (section 2(b)(3)): the licensor
  "expressly reserves any right to collect such royalties, including when the
  Licensed Material is used other than for NonCommercial purposes."
- How NC is read: section 4 below.

### 1.5 CC0 1.0 (public domain dedication)

Source: https://creativecommons.org/publicdomain/zero/1.0/legalcode.en and the
deed https://creativecommons.org/publicdomain/zero/1.0/deed.en (both undated;
read 2026-10-12).

- **It is not a licence.** CC FAQ:
  > Please note that CC0 is not a license; it is a public domain dedication.
- **The waiver** (section 2), trimmed:
  > Affirmer hereby overtly, fully, permanently, irrevocably and unconditionally waives, abandons, and surrenders all of Affirmer's Copyright and Related Rights and associated claims and causes of action

  "for any purpose whatsoever, including without limitation commercial,
  advertising or promotional purposes", and the Affirmer makes it
  > fully intending that such Waiver shall not be subject to revocation, rescission, cancellation, termination, or any other legal or equitable action to disrupt the quiet enjoyment of the Work by the public
- **The fallback licence** (section 3), for places where a waiver does not work:
  > Should any part of the Waiver for any reason be judged legally invalid or ineffective under applicable law, then the Waiver shall be preserved to the maximum extent permitted taking into account Affirmer's express Statement of Purpose.

  and, to that extent,
  > Affirmer hereby grants to each affected person a royalty-free, non transferable, non sublicensable, non exclusive, irrevocable and unconditional license to exercise Affirmer's Copyright and Related Rights in the Work

  on the same four terms (worldwide, maximum duration, any medium, any purpose).
- **What it leaves alone** (section 4(a)): "No trademark or patent rights held
  by Affirmer are waived, abandoned, surrendered, licensed or otherwise affected
  by this document." The deed:
  > In no way are the patent or trademark rights of any person affected by CC0, nor are the rights that other persons may have in the work or in how the work is used, such as publicity or privacy rights.
- **Negative finding: CC0 cannot require credit.** It has no attribution
  condition. The deed only asks that a user "should not imply endorsement by
  the author or the affirmer", and the deed's own notice says "The Commons Deed
  is not a legal instrument." Giving credit for a CC0 item is courtesy, not a
  duty.
- HumanityOS itself is released under CC0 1.0 (`LICENSE`).

### 1.6 What other platforms offer (practice, not law)

- **Sketchfab** (3D models), help article *Making your models downloadable*,
  https://help.sketchfab.com/en/articles/16152220 (page data "dateModified"
  2026-10-07):
  > When you make your model free to download, you'll need to choose between a Standard License or a Creative Commons license to place it under:

  The CC choices it lists are CC BY, CC BY-NC, CC BY-ND and CC BY-SA. It adds:
  "You can change the license at any time from the model page."
- **Pixelfed** (open-source, self-hosted photo sharing; the closest
  architectural match to a HumanityOS server), source code read through
  GitHub's API on 2026-10-12. `app/Util/Media/License.php` (last commit
  2026-10-02) offers All Rights Reserved, the Public Domain Mark, CC0 and all
  six CC 4.0 licences, describing the first as:
  > You, the copyright holder, reserve all rights provided by copyright law, such as the right to make copies, distribute your work, perform your work, license, or otherwise exploit your work; no rights are waived under this license.

  and `app/Http/Controllers/ComposeController.php` (last commit 2026-10-08)
  sets `'default_license' => 1`, which is that All Rights Reserved entry.
- **Flickr**, help article *Copyright Licenses on Flickr*,
  https://www.flickrhelp.com/hc/en-us/articles/40048459562388 ("Updated 1 year
  ago"), lists All Rights Reserved, Public Domain Work, CC0 and the six CC 4.0
  licences. Its sentence on defaults is ambiguous:
  > Flickr now defaults to Creative Commons 4.0 for all newly uploaded content. If your account previously defaulted to one of the six CC 2.0 licenses, it has been automatically upgraded to the equivalent 4.0 license.

  Read with its second sentence, this most likely means CC choices now default
  to version 4.0, not that every upload is now CC; the page does not say what a
  new account's default is.
- **CC on game art.** CC FAQ:
  > While we recommend against using a CC license on software itself, CC licenses may be used for software documentation, as well as for separate artistic elements such as game art or music.

---

## 2. Is "All rights reserved" the default with no statement at all?

Yes. Copyright arises when the work is fixed, with no notice, registration or
statement.

- **US Copyright Office**, *Frequently Asked Questions: Copyright in General*,
  https://www.copyright.gov/help/faq/faq-general.html (undated; read
  2026-10-12), "When is my work protected?":
  > Your work is under copyright protection the moment it is created and fixed in a tangible form that it is perceptible either directly or with the aid of a machine or device.

  and "Do I have to register with your office to be protected?":
  > No. In general, registration is voluntary. Copyright exists from the moment the work is created. You will have to register, however, if you wish to bring a lawsuit for infringement of a U.S. work.
- **Circular 1, *Copyright Basics***, https://www.copyright.gov/circs/circ01.pdf
  ("CIRCULAR 1 REVISED: 09/2021"):
  > Copyright protection in the United States exists automatically from the moment the original work of authorship is fixed.

  > Applying a copyright notice to a work has not been required since March 1, 1989, but may still provide practical and legal benefits.
- **The statute.** 17 U.S.C. 102(a):
  > Copyright protection subsists, in accordance with this title, in original works of authorship fixed in any tangible medium of expression, now known or later developed, from which they can be perceived, reproduced, or otherwise communicated, either directly or with the aid of a machine or device.

  17 U.S.C. 201(a): "Copyright in a work protected under this title vests
  initially in the author or authors of the work." 3D models fit a listed
  category: 17 U.S.C. 101 says "Pictorial, graphic, and sculptural works"
  include "two-dimensional and three-dimensional works of fine, graphic, and
  applied art, photographs, prints and art reproductions, maps, globes, charts,
  diagrams, models, and technical drawings".
- **What the owner holds.** 17 U.S.C. 106: the owner "has the exclusive rights
  to do and to authorize" among others "(1) to reproduce the copyrighted work in
  copies or phonorecords;" "(2) to prepare derivative works based upon the
  copyrighted work;" "(3) to distribute copies or phonorecords of the
  copyrighted work to the public" and, for pictorial, graphic and sculptural
  works and the images of a film, "(5) ... to display the copyrighted work
  publicly".
- **The Berne Convention**, article 5(2), WIPO Lex,
  https://www.wipo.int/wipolex/en/text/283698 (Paris Act of 24 July 1971, as
  amended on 28 September 1979):
  > The enjoyment and the exercise of these rights shall not be subject to any formality; such enjoyment and such exercise shall be independent of the existence of protection in the country of origin of the work.

  WIPO's summary, https://www.wipo.int/treaties/en/ip/berne/summary_berne.html
  (undated), calls this the principle of "automatic" protection:
  > (b) Protection must not be conditional upon compliance with any formality (principle of "automatic" protection) [2].
- **The UK.** GOV.UK, *How copyright protects your work*,
  https://www.gov.uk/copyright (first published 8 August 2014, updated 18
  November 2015 per GOV.UK's content API):
  > You get copyright protection automatically - you do not have to apply or pay a fee. There is not a register of copyright works in the UK.
- **Negative finding: the words "All rights reserved" add nothing in law.**
  Circular 3, *Copyright Notice*, https://www.copyright.gov/circs/circ03.pdf
  ("CIRCULAR 3 REVISED: 03/2021"), lists a notice's elements as the symbol or
  the word "copyright", the year of first publication and the owner's name, and
  says notice itself is optional:
  > Copyright notice is optional for unpublished works, foreign works, or works published on or after March 1, 1989.

  Neither Circular 1 nor Circular 3 contains the phrase "all rights reserved"
  (searched). In the Media page it is a plain description of the absence of any
  public licence, not a legal act. (Project's reading.)
- **Negative finding: the label does not create copyright where there is
  none.** *Meshwerks, Inc. v. Toyota Motor Sales U.S.A., Inc.*, 528 F.3d 1258
  (10th Cir., filed 17 June 2008),
  https://www.courtlistener.com/opinion/170844/meshwerks-inc-v-toyota-motor-sales-usa-inc/ ,
  held that digital wire-frame models of real cars were not original:
  > Originality is the sine qua non of copyright.

  > Put another way, Meshwerks’ models depict nothing more than unadorned Toyota vehicles — the car as car.

  A faithful scan of a real object may carry little or no copyright whatever
  label the uploader picks. The same opinion says it did "not doubt for an
  instant" that digital modelling can produce protectable work.

---

## 3. Can a Creative Commons licence be revoked?

No, not for anyone who already has a copy. The licensor can stop offering it,
and can ask to have their name removed.

- **The legal code.** The grant is "irrevocable" (section 1.1 above). The
  preamble: "Our licenses are irrevocable." Section 6(c):
  > For the avoidance of doubt, the Licensor may also offer the Licensed Material under separate terms or conditions or stop distributing the Licensed Material at any time; however, doing so will not terminate this Public License.

  Section 6(a): the licence "applies for the term of the Copyright and Similar
  Rights licensed here", but a user who breaks it loses it automatically;
  section 6(b) restores it if the breach is cured "within 30 days of Your
  discovery of the violation".
- **The deeds** (BY, BY-SA, BY-NC):
  > The licensor cannot revoke these freedoms as long as you follow the license terms.
- **The CC FAQ**, "What if I change my mind about using a CC license?":
  > CC licenses are not revocable. Once something has been published under a CC license, licensees may continue using it according to the license terms for the duration of applicable copyright and similar rights. As a licensor, you may stop distributing under the CC license at any time, but anyone who has access to a copy of the material may continue to redistribute it under the CC license terms.

  and, for the person using the work:
  > The CC licenses are irrevocable. This means that once you receive material under a CC license, you will always have the right to use it under those license terms, even if the licensor changes his or her mind and stops distributing under the CC license terms.

  and on breach:
  > However, CC licenses are not revocable once granted unless there has been a breach, and even then the license is terminated only for the breaching licensee.
- **CC0** says the same of its waiver ("shall not be subject to revocation,
  rescission, cancellation, termination", section 1.5 above), and its fallback
  licence is "irrevocable and unconditional".
- **Members only does not limit a CC licence.** CC FAQ, on technological
  measures:
  > Likewise, limiting recipients to a particular set of users (for example, by requiring a username and password to enter a site) does not restrict further use of the content by the recipients.

  and
  > may post material under any CC license on a site restricted to members of a certain school, or to paying customers, but you may not place effective technological measures (including DRM) on the files that prevents them from sharing the material elsewhere.

  So a members-only item under any CC licence may be republished publicly by
  any member who received it. (A negative finding for the Media page's
  "members only" choice.)
- **How platforms describe it (practice).** Flickr, *Change your photo's
  license in Flickr*,
  https://www.flickrhelp.com/hc/en-us/articles/4404078674324 ("Updated 2 months
  ago"):
  > You can change the license as often as you like, but downloaded content always retains the license assigned to it, even if the license is changed on Flickr.

  Sketchfab says a licence can be changed "at any time" (section 1.6) without
  that warning.
- **A US-law wrinkle.** US courts treat a free, non-exclusive licence as
  revocable unless something was given for it (section 6.3 below). CC 4.0 is
  drafted around this; the paragraph before section 1 of the legal code:
  > To the extent this Public License may be interpreted as a contract, You are granted the Licensed Rights in consideration of Your acceptance of these terms and conditions, and the Licensor grants You such rights in consideration of benefits the Licensor receives from making the Licensed Material available under these terms and conditions.

  No court decision on revoking a CC licence for want of consideration was
  found (not searched exhaustively).
- **A statutory exception that no contract removes.** 17 U.S.C. 203(a) lets an
  author (or heirs) end "the exclusive or nonexclusive grant of a transfer or
  license of copyright or of any right under a copyright, executed by the
  author on or after January 1, 1978" during a five-year window starting 35
  years after the grant, and (a)(5):
  > Termination of the grant may be effected notwithstanding any agreement to the contrary, including an agreement to make a will or to make any future grant.

  Whether this reaches a public licence such as CC BY, offered to everyone, was
  not answered by any source read; the CC FAQ does not mention it.
- **What this means for us.** Choosing a CC licence, or CC0, is permanent for
  every copy that leaves the server while it is in force. Changing an item from
  CC to "All rights reserved" later only stops new copies being taken from that
  server under CC. The uploader can still require their name to be taken off
  adaptations (section 3(a)(3)).

---

## 4. What "NonCommercial" covers, and the project's own uses

### 4.1 CC's own words

- **CC's interpretation page**, *NonCommercial interpretation*,
  https://wiki.creativecommons.org/wiki/NonCommercial_interpretation ("This page
  was last edited on 15 October 2017"), which opens "THIS PAGE IS FOR
  INFORMATIONAL PURPOSES ONLY":
  > The definition is intent-based and intentionally flexible in recognition of the many possible factual situations and business models that may exist now or develop later.

  > The inclusion of “primarily” in the definition recognizes that no activity is completely disconnected from commercial activity; it is only the primary purpose of the reuse that needs to be considered.

  > NonCommercial turns on the use, not the identity of the reuser.

  > The NC licenses limit reusers to NonCommercial uses of the work only, which includes when the work is used in a collection or when it is adapted. For example, an NC essay may not be included as part of a collection in a commercially distributed book of essays, even if it is only a small portion of the book.

  and on attempts to pin it down in terms of service:
  > Some licensors or website providers state expectations or interpretations about what NC means. Those explanations never form part of the CC license, even if included in terms of service or another resource designed to contractually bind reusers.

  Also: "NC licenses do not qualify as “open licenses” under the Open
  Definition, and works licensed under an NC license are not considered Free
  Cultural Works."
- **The CC FAQ**, "Does my use violate the NonCommercial clause of the
  licenses?":
  > Please note that CC’s definition does not turn on the type of user: if you are a nonprofit or charitable organization, your use of an NC-licensed work could still run afoul of the NC restriction, and if you are a for-profit entity, your use of an NC-licensed work does not necessarily mean you have violated the term.

  > CC cannot advise you on what is and is not commercial use. If you are unsure, you should either contact the rights holder for clarification, or search for works that permit commercial uses.
- **CC's 2009 study**, *Defining "Noncommercial": A Study of How the Online
  Population Understands "Noncommercial Use"* (September 2009),
  https://mirrors.creativecommons.org/defining-noncommercial/Defining_Noncommercial_fullreport.pdf .
  The FAQ says its results "are not intended to serve as CC’s official
  interpretation", so this is evidence of how people read NC, not of what it
  means:
  > Both creators and users generally consider uses that earn users money or involve online advertising to be commercial, while uses by organizations, by individuals, or for charitable purposes are less commercial but not decidedly noncommercial.

  > Notably, while many more respondents rate all the studied uses by for-profits "definitely" commercial, some respondents rate many uses by not-for-profits "definitely" commercial, as well: there is no "pass" for not-for-profit organizations.
- **A court reading NC.** *Great Minds v. FedEx Office & Print Services, Inc.*,
  886 F.3d 91 (2d Cir., 21 March 2018),
  https://www.courtlistener.com/opinion/4479341/great-minds-v-fedex-office-print-servs-inc/ .
  The licence was CC BY-NC-SA 4.0 ("Great Minds used the “Creative Commons
  Attribution—Non Commercial—Share Alike 4.0 International Public License,”");
  schools using it non-commercially paid FedEx to make copies. The court:
  > We hold that a copyright holder must state in its license any limitation it might wish to impose precluding such an engagement.

  The question the court decided was who may help a non-commercial user, not
  whether donations or promotion are commercial; it read the licence by its
  words and declined to add limits.

### 4.2 The project's uses, one by one (project's reading)

HumanityOS is not a registered nonprofit: per
`2026-10-04-sponsor-a-can-tax-status.md`, gifts to the maintainer are personal
gifts to a person. That makes the "not-for-profit" argument weaker still.

1. **Hosting and showing an NC item to the audience its uploader chose, on a
   free server.** Very likely NonCommercial: nothing is sold, and the
   definition itself counts file-sharing without payment as NonCommercial. If
   the uploader owns the item, the server's own hosting licence (section 5)
   covers it anyway, so the server never needs to rely on this reading.
2. **Featuring items on the server's front page "for visitors and the donation
   story"** (`docs/design/media-showcase.md`, "Featured"). Less clear: a page
   that asks for money and uses the item to draw people to it could be
   "directed towards ... monetary compensation". Covered if the server's
   hosting licence names featuring on that server.
3. **Putting an NC model in the game.** The game is given away, but its licence
   (CC0) lets anyone sell copies, and NC follows the item into collections.
   An NC model would have to be carved out of the CC0 grant, and any seller of
   the game would have to strip it out. **Negative finding: NC items do not fit
   a CC0 game.** Keep them out unless the uploader also gives the separate
   permission (section 6) in a form that covers redistribution.
4. **Using an NC item in a promotional video that asks for donations.** Plainly
   unclear, and the riskiest case: the video's purpose is to bring in money,
   CC says nonprofits get no automatic pass, the survey found no "pass" for
   them, and HumanityOS is not even a registered one. **Do not rely on NC for
   promotional videos; ask for the separate permission.**

What is genuinely unclear, with no source found that answers it: whether
donations count as "monetary compensation" for NC; whether a project whose
maintainer is supported by donations has a "commercial advantage"; and whether
a free game with no sales and no advertising is commercial because its licence
lets others sell it.

---

## 5. What a server operator needs to host members' uploads

### 5.1 What hosting does, in copyright terms

Storing a file is reproduction, serving it is distribution, and showing it on a
page is public display (17 U.S.C. 106, section 2 above). "Publicly" includes
sending it over the internet to people at different times, 17 U.S.C. 101:
> (2) to transmit or otherwise communicate a performance or display of the work to a place specified by clause (1) or to the public, by means of any device or process, whether the members of the public capable of receiving the performance or display receive it in the same place or in separate places and at the same time or at different times.

So a server needs either the owner's permission or a legal exception for every
item it holds.

### 5.2 Permission need not be written, but an unwritten one has fuzzy edges

- 17 U.S.C. 101: a "transfer of copyright ownership" is an assignment or
  exclusive licence, "but not including a nonexclusive license". 17 U.S.C.
  204(a): "A transfer of copyright ownership, other than by operation of law,
  is not valid unless an instrument of conveyance, or a note or memorandum of
  the transfer, is in writing and signed by the owner of the rights conveyed or
  such owner's duly authorized agent."
- Circular 1 (09/2021):
  > Transferring a right on a nonexclusive basis does not require a written agreement.
- *Asset Marketing Systems, Inc. v. Gagnon*, 542 F.3d 748 (9th Cir., filed 9
  September 2008),
  https://www.courtlistener.com/opinion/1436728/asset-marketing-systems-inc-v-gagnon/ :
  > Though exclusive licenses must be in writing, 17 U.S.C. § 204, grants of nonexclusive licenses need not be in writing, and may be granted orally or by implication.

  and on how far an implied licence reaches:
  > Gagnon had to express an intent to retain control over the programs and limit AMS’s license if he intended to do so.
- UK, Copyright, Designs and Patents Act 1988, section 90(3),
  https://www.legislation.gov.uk/ukpga/1988/48/section/90 (revised text, "no
  known outstanding effects"): "An assignment of copyright is not effective
  unless it is in writing signed by or on behalf of the assignor." Section
  92(1) defines an "exclusive licence" as "a licence in writing"; neither
  section asks for writing for a non-exclusive one. Section 90(4): "A licence
  granted by a copyright owner is binding on every successor in title to his
  interest in the copyright", with an exception for a good-faith purchaser
  without notice.
- **What this means for us** (project's reading). An owner who uploads to a
  page whose whole purpose is showing things almost certainly gives an implied
  licence to show it to the audience they chose. But an implied licence's
  edges (thumbnails? featuring? backups? other servers?) are whatever a court
  later infers from conduct, and *Gagnon* shows an implied licence can be read
  broadly against the owner. Writing the hosting licence down protects both
  sides.

### 5.3 A CC licence covers the server; "All rights reserved" does not

- For an item its owner put under CC BY, BY-SA or BY-NC, the licence itself lets
  anyone, the server included, reproduce and share it (section 1), with credit,
  in any format (thumbnails are "technical modifications"), and for BY-NC only
  for NonCommercial purposes.
- For an "All rights reserved" item there is no public permission at all; the
  server rests entirely on the uploader's own permission (5.2, 5.4).
- **A member cannot license a platform for someone else's CC work.** CC FAQ,
  "Do I need to worry about website terms of service when I share CC-licensed
  content on social media platforms?":
  > Creative Commons licenses do not allow sublicensing, which means you cannot grant a license to a platform with respect to the rights in any CC-licensed content you do not own.

  For such an item the server relies on the item's own CC licence, and must
  keep to its conditions (credit; for NC, non-commercial use).

### 5.4 What platforms' terms do (practice, not law)

- **CC's own summary**, same FAQ entry:
  > Generally speaking, most social media platforms allow users to keep the copyright they have in their content and ask users to grant them a copyright license to use that content for purposes of providing their service. In some cases, platforms may also ask for a license to use your content in advertising and other promotions of their service.

  > However, it is unusual for social media platforms to require you to own or control the copyright on all content you share on their sites. Instead, they often simply require that you have the rights to post it.
- **YouTube**, Terms of Service, https://www.youtube.com/t/terms ("Dated:
  December 15, 2023"; "Effective as of December 15, 2023"):
  > You retain ownership rights in your Content.

  > By providing Content to the Service, you grant to YouTube a worldwide, non-exclusive, royalty-free, sublicensable and transferable license to use that Content (including to reproduce, distribute, prepare derivative works, display and perform it) in connection with the Service and YouTube’s (and its successors' and Affiliates') business, including for the purpose of promoting and redistributing part or all of the Service.

  > The licenses granted by you continue for a commercially reasonable period of time after you remove or delete your Content from the Service.
- **GitHub**, Terms of Service, section D.4, read from
  https://raw.githubusercontent.com/github/docs/main/content/site-policy/github-terms/github-terms-of-service.md
  ("Effective date: April 27, 2026"):
  > You grant GitHub and our Affiliates the right to store, host, archive, parse, display, and make copies of Your Content as necessary to provide, develop, and improve the Service

  The sentence continues into training artificial intelligence models. Section
  D.5: "If you post Content you did not create or own, you are responsible for
  ensuring it is licensed under terms that permit these uses."
- **What this means for us** (project's reading). The pattern is universal: the
  member keeps the copyright and gives the service a non-exclusive licence to
  run the service. YouTube's and GitHub's grants are far wider than a server
  that wants to protect its members needs (sublicensable, transferable,
  promotion of the business, AI training, "a commercially reasonable period"
  after removal). A HumanityOS server can ask for much less.

### 5.5 US safe harbour for files a member does not own (17 U.S.C. 512)

Source: 17 U.S.C. 512, US Code 2024 Edition,
https://www.govinfo.gov/content/pkg/USCODE-2024-title17/html/USCODE-2024-title17-chap5-sec512.htm .
Section 512 limits a service provider's liability for what users store; it is
not a licence and does not help with the provider's own uses.

- **Who:** 512(k)(1)(B): "the term "service provider" means a provider of
  online services or network access, or the operator of facilities therefor".
- **The conditions, 512(c)(1)**: no liability "for infringement of copyright by
  reason of the storage at the direction of a user of material that resides on
  a system or network controlled or operated by or for the service provider, if
  the service provider—"
  > (A)(i) does not have actual knowledge that the material or an activity using the material on the system or network is infringing;

  > (ii) in the absence of such actual knowledge, is not aware of facts or circumstances from which infringing activity is apparent; or

  > (iii) upon obtaining such knowledge or awareness, acts expeditiously to remove, or disable access to, the material;

  > (B) does not receive a financial benefit directly attributable to the infringing activity, in a case in which the service provider has the right and ability to control such activity; and

  > (C) upon notification of claimed infringement as described in paragraph (3), responds expeditiously to remove, or disable access to, the material that is claimed to be infringing or to be the subject of infringing activity.
- **A designated agent, 512(c)(2):**
  > The limitations on liability established in this subsection apply to a service provider only if the service provider has designated an agent to receive notifications of claimed infringement described in paragraph (3), by making available through its service, including on its website in a location accessible to the public, and by providing to the Copyright Office, substantially the following information:

  "(A) the name, address, phone number, and electronic mail address of the
  agent."
- **What a valid notice holds, 512(c)(3)(A):** a signature of someone
  authorised to act for the owner; identification of the work and of the
  material, with enough to locate it; contact details; and
  > (v) A statement that the complaining party has a good faith belief that use of the material in the manner complained of is not authorized by the copyright owner, its agent, or the law.

  > (vi) A statement that the information in the notification is accurate, and under penalty of perjury, that the complaining party is authorized to act on behalf of the owner of an exclusive right that is allegedly infringed.
- **Putting it back, 512(g):** a provider is not liable for taking material down
  in good faith (512(g)(1)), but for material removed on a notice it keeps that
  protection only if it "takes reasonable steps promptly to notify the
  subscriber", forwards a valid counter notification, and
  > (C) replaces the removed material and ceases disabling access to it not less than 10, nor more than 14, business days following receipt of the counter notice, unless its designated agent first receives notice from the person who submitted the notification under subsection (c)(1)(C) that such person has filed an action seeking a court order to restrain the subscriber from engaging in infringing activity relating to the material on the service provider's system or network.
- **Repeat infringers, 512(i)(1):** the limits apply "only if the service
  provider—"
  > (A) has adopted and reasonably implemented, and informs subscribers and account holders of the service provider's system or network of, a policy that provides for the termination in appropriate circumstances of subscribers and account holders of the service provider's system or network who are repeat infringers; and
- **No duty to watch, 512(m)(1):** the safe harbours are not conditioned on "a
  service provider monitoring its service or affirmatively seeking facts
  indicating infringing activity".
- **False notices, 512(f):** anyone who "knowingly materially misrepresents"
  that material is infringing, or was removed by mistake, "shall be liable for
  any damages".
- **The Copyright Office's directory.** *DMCA Designated Agent Directory*,
  https://www.copyright.gov/dmca-directory/ (undated), and its FAQ,
  https://www.copyright.gov/dmca-directory/faq.html (undated; says the rule
  "was amended in 2017 and 2020"):
  > The current fee is $6 per designation, amendment, or resubmission.

  > A service provider’s designation will expire and become invalid three years after it is registered with the Office, unless the service provider renews such designation by either amending it to correct or update all relevant information or resubmitting it without amendment to confirm the designation’s continued accuracy.

  > A service provider is required to supply its full legal name, physical street address (not a post office box), any alternate names used by the service provider, and the name, organization, physical mail address (street address or post office box), telephone number, and email address of its designated agent.

  A post office box for the service provider is allowed only with prior
  approval "in exceptional circumstances, such where there is a demonstrable
  threat to an individual’s personal safety or security". A one-person
  service may use the same person for every role: "if the service provider is
  run by a single person who will be managing the service provider’s account
  and acting as its designated agent, then it would be okay for the contact
  information for the primary contact, service provider, and designated agent
  to all be the same." On lapsing: "Service providers risk losing the safe
  harbor protections of section 512 of the DMCA".
- **What this means for us.** Each server is its own "service provider": the
  operator's two servers (united-humanity.us and public.guide) need a
  designation each, or one designation listing both as alternate names if they
  are the same provider (the FAQ allows "website names and addresses" as
  alternate names), and anyone running their own server needs their own. The
  service provider's street address is published, which is a privacy cost for
  a person running a server from home; the agent's address may be a post
  office box, and the FAQ says the agent may be "a third-party entity
  generally (e.g., “ACME Takedown Service”)". Whether donations are "a
  financial benefit directly attributable to the infringing activity" is not
  answered by the text; the project's reading is that general donations are
  not directly attributable to any one file.

### 5.6 The UK

- The Electronic Commerce (EC Directive) Regulations 2002, regulation 19,
  https://www.legislation.gov.uk/uksi/2002/2013/regulation/19 (revised; "There
  are currently no known outstanding effects"):
  > Where an information society service is provided which consists of the storage of information provided by a recipient of the service, the service provider (if he otherwise would) shall not be liable for damages or for any other pecuniary remedy or for any criminal sanction as a result of that storage where—

  > (a) the service provider—

  > (i) does not have actual knowledge of unlawful activity or information and, where a claim for damages is made, is not aware of facts or circumstances from which it would have been apparent to the service provider that the activity or information was unlawful; or

  > (ii) upon obtaining such knowledge or awareness, acts expeditiously to remove or to disable access to the information, and

  > (b) the recipient of the service was not acting under the authority or the control of the service provider.
- Regulation 2, https://www.legislation.gov.uk/uksi/2002/2013/regulation/2 ,
  defines "information society services" by the Directive, "summarised in
  recital 17 of the Directive as covering “any service normally provided for
  remuneration, at a distance, by means of electronic equipment for the
  processing (including digital compression) and storage of data, and at the
  individual request of a recipient of a service”".
- There is no registered-agent requirement in regulation 19.
- **Open:** whether a free, donation-supported server is a service "normally
  provided for remuneration" (see "Unknown").

### 5.7 The EU

- **Digital Services Act**, Regulation (EU) 2022/2065 of 19 October 2022 (OJ L
  277, 27.10.2022), Article 6(1):
  > Where an information society service is provided that consists of the storage of information provided by a recipient of the service, the service provider shall not be liable for the information stored at the request of a recipient of the service, on condition that the provider:

  "(a) does not have actual knowledge of illegal activity or illegal content
  and, as regards claims for damages, is not aware of facts or circumstances
  from which the illegal activity or illegal content is apparent; or (b) upon
  obtaining such knowledge or awareness, acts expeditiously to remove or to
  disable access to the illegal content." Article 8: no "general obligation to
  monitor".
- **A notice form is required of every hosting provider**, Article 16(1):
  > Providers of hosting services shall put mechanisms in place to allow any individual or entity to notify them of the presence on their service of specific items of information that the individual or entity considers to be illegal content. Those mechanisms shall be easy to access and user-friendly, and shall allow for the submission of notices exclusively by electronic means.

  Article 16(2) lists what the form must collect (reasons, exact location, the
  notifier's name and email, a statement of good faith); 16(4) and (5) require a
  confirmation of receipt and a decision with "information on the possibilities
  for redress". Article 17 requires a statement of reasons to the uploader
  when something is removed.
- **Reach**, Article 2(1): the Regulation "shall apply to intermediary services
  offered to recipients of the service that have their place of establishment
  or are located in the Union, irrespective of where the providers of those
  intermediary services have their place of establishment." Article 3(d) and
  (e) make "to offer services in the Union" depend on "a substantial connection
  to the Union", such as "a significant number of recipients of the service in
  one or more Member States in relation to its or their population" or "the
  targeting of activities towards one or more Member States". Article 13(1)
  then requires a provider with no EU establishment that offers services there
  to "designate, in writing, a legal or natural person to act as their legal
  representative". Whether a small US server reaches that threshold was not
  researched.
- **The upload-filter rule probably does not apply.** Directive (EU) 2019/790
  of 17 April 2019 (OJ L 130, 17.5.2019), Article 2(6):
  > ‘online content-sharing service provider’ means a provider of an information society service of which the main or one of the main purposes is to store and give the public access to a large amount of copyright-protected works or other protected subject matter uploaded by its users, which it organises and promotes for profit-making purposes.

  It goes on: "Providers of services, such as not-for-profit online
  encyclopedias, not-for-profit educational and scientific repositories, open
  source software-developing and-sharing platforms, ... are not ‘online
  content-sharing service providers’". Only such providers fall under Article
  17, which makes them need "an authorisation from the rightholders". The
  project's reading: a community server that is not organised "for
  profit-making purposes" and does not hold "a large amount" of works is not
  one, so the ordinary hosting rule above applies. National laws implementing
  the Directive were not read.

### 5.8 The metadata strip and "copyright management information"

- 17 U.S.C. 1202(b):
  > No person shall, without the authority of the copyright owner or the law—

  > (1) intentionally remove or alter any copyright management information,

  ending: "knowing, or, with respect to civil remedies under section 1203,
  having reasonable grounds to know, that it will induce, enable, facilitate,
  or conceal an infringement of any right under this title." 1202(c) defines
  copyright management information as information "conveyed in connection with
  copies ... of a work ... including in digital form", including "(2) The name
  of, and other identifying information about, the author of a work.", "(3)
  The name of, and other identifying information about, the copyright owner of
  the work, including the information set forth in a notice of copyright." and
  "(6) Terms and conditions for use of the work."
- Where pictures carry it: the IPTC Photo Metadata Standard 2025.1 (version
  2025.1 rev 1, 2025-11-26),
  https://www.iptc.org/std/photometadata/specification/IPTC-PhotoMetadata ,
  field "Copyright Notice":
  > Contains any necessary copyright notice for claiming the intellectual property for this photograph and should identify the current owner of the copyright for the photograph.

  stored in XMP as "dc:rights" and in the older IIM block; the "Creator" field
  "Contains the name of the photographer". (Exif has similar fields; the Exif
  standard itself was not read.)
- **What this means for us** (project's reading). The Media page's metadata
  strip (`web/shared/strip-metadata.js`, a port of
  `src/relay/core/strip_metadata.rs`) removes, for JPEG, "APP1 (Exif, where the
  GPS position lives, and XMP), APP13 (Photoshop and IPTC captions and place
  names)", which is where these notices live. When the uploader owns the work
  and agrees to the strip, it is not done "without the authority of the
  copyright owner". When a member uploads someone else's photo, it removes the
  real owner's notice. Liability needs knowledge that removal will help an
  infringement, which a blanket privacy strip is unlikely to meet, but the
  careful course is to show creator and licence on the item page and to keep
  (or let the uploader keep) the creator and copyright fields while still
  removing location and camera details.

---

## 6. A separate permission for HumanityOS's own use

### 6.1 Giving one party more than the public licence is normal

- **CC FAQ**, "Can I enter into separate or supplemental agreements with users
  of my work?":
  > Yes. CC licenses are nonexclusive. Licensors always have the option of entering into separate arrangements for the sharing of their material in addition to applying a CC license. However, those different arrangements are not “CC” or “Creative Commons” licenses.

  > Or, for example, you may offer your material to the public under a NonCommercial license, but offer commercial permissions to fee-paying customers.

  > It should be noted that any agreements you make with other parties only have an effect on the other parties to that agreement, and do not apply to anyone else receiving the licensed material.

  with one warning:
  > While you may offer separate terms and conditions to other parties, you should not do so in such a way that would neutralize terms of the CC license.

  And "Can I waive license terms or conditions?": "Yes. You may always choose
  to waive some license terms or conditions."
- **CC+**, https://wiki.creativecommons.org/wiki/CCPlus ("last edited on 21
  December 2017"):
  > CC+ denotes the combination of a CC official license (unmodified and verbatim) + another separate and independent agreement granting more permissions.

  > Note that in order to use CC+ the additional permissions must be set forth in a separate document or resource -- the official legal code within the "corners" of the license cannot be added to or changed.
- **CC's NC page**: "Also, licensees are always free to contact licensors to ask
  permission to use the work for commercial purposes."
- **For an "All rights reserved" item** the separate permission is simply the
  only permission the project has. That is the ordinary case of a
  non-exclusive licence to a named party (17 U.S.C. 101, section 5.2): no
  writing is required, though a record of it is what proves it later.

### 6.2 What it can never cover

- Only the uploader's own rights. CC's licences and CC0 leave "publicity,
  privacy, and/or other similar personality rights" untouched (section 1.1), and
  the same is true of any permission the uploader gives: **a person shown in a
  photo or video has not agreed to appear in the project's promotion just
  because the uploader did.** Publicity law varies by state and country and
  was not researched here.
- Nothing the uploader does not own (section 5.3).

### 6.3 Can it be withdrawn?

- *Lulirama Ltd., Inc. v. Axcess Broadcast Services, Inc.*, 128 F.3d 872 (5th
  Cir., 10 November 1997),
  https://www.courtlistener.com/opinion/747750/lulirama-ltd-inc-v-axcess-broadcast-services-inc/ :
  > A nonexclusive license may be irrevocable if supported by consideration. See 3 NIMMER, supra, § 10.02[B] ("[N]onexclusive licenses are revocable absent consideration.")
- *Gagnon* (above):
  > Furthermore, because AMS paid consideration, this license is irrevocable.

  > “[A] nonexclusive license supported by consideration is a contract.”
- 17 U.S.C. 203, section 3 above: even a grant drafted as permanent can be ended
  by the author in the window from 35 to 40 years after it, "notwithstanding
  any agreement to the contrary", if that section reaches it. Its 203(b)(1)
  shows how the law treats work already made: "A derivative work prepared
  under authority of the grant before its termination may continue to be
  utilized under the terms of the grant after its termination, but this
  privilege does not extend to the preparation after the termination of other
  derivative works".
- **What this means for us** (project's reading). A tick-box permission given
  for nothing in return is probably revocable at will under US law unless it is
  drafted and accepted as an agreement. That is not necessarily bad; it may be
  the right promise to members. But it must be decided and written down, not
  left to a court: for example, "you can withdraw this at any time; things
  already made with it (released game versions, published videos) may stay as
  they are, but nothing new will use it". **Negative finding: "irrevocable
  forever" is not available to the project in the US for a grant by the author
  if 17 U.S.C. 203 applies.**

### 6.4 What the permission must say to be clear (project's reading)

1. **Who receives it.** HumanityOS is not a company or a charity; gifts go to
   the maintainer as a person (2026-10-04 finding). A permission to
   "HumanityOS" needs a real grantee: for example the maintainer by name "and
   whoever maintains the HumanityOS project after him". Whether a permission
   to an unincorporated project and its future maintainers holds together as
   intended is a question for a lawyer.
2. **What it covers.** Name the uses: inside the HumanityOS game and its
   website; in the project's own videos, posts and pages about HumanityOS,
   including ones that ask for donations; resizing, cropping, retexturing and
   cutting as needed.
3. **Forks and redistribution.** The game is CC0: anyone may copy, change and
   sell it. A permission only to "HumanityOS" would not let a fork, a mirror or
   another server's operator pass the model on inside the game. Either the
   permission says that anyone may distribute the item as part of HumanityOS
   and its forks (close to a public licence), or only items under CC0, CC BY or
   CC BY-SA go into the game, under their own licence, marked and credited.
4. **Credit.** The name the uploader chose, where it appears (the in-game
   Credits page, the item's page, a video's description), and "where
   practicable", since a three-second clip cannot always carry a caption.
5. **Withdrawal.** Whether it can be withdrawn, how, and what happens to things
   already made (6.3).
6. **No endorsement**, matching the CC licences: using an item does not mean
   the uploader endorses the project, nor the project the uploader.
7. **A promise from the uploader** that it is theirs to give, and that people
   shown in it agree to appear.
8. **Non-exclusive and royalty-free**, so the uploader keeps every other right,
   including selling the work.

---

## Where the sources pull in different directions

**Irrevocable, or revocable absent consideration?** CC's legal code, deeds and
FAQ say CC licences are irrevocable; US cases (*Lulirama*, citing Nimmer, and
*Gagnon*) say a non-exclusive licence given for nothing is revocable. CC 4.0's
"in consideration of" paragraph is written to meet exactly this, and no case
was found where a CC licensor revoked. The project weighs CC's text as the
working answer for CC items and the case law as the warning for its own
tick-box permission, which has no such drafting yet.

**Changing a licence later.** Sketchfab says it can be done at any time; Flickr says
the same but adds that downloaded copies keep the licence they came with. The
CC text sides with Flickr's fuller statement, and a HumanityOS licence picker
should too.

**What NonCommercial means.** CC says it is intent-based and turns on the use;
CC's survey shows people disagree, especially about non-profit and charitable
uses; the one appeals court read it narrowly by its words. Nobody official has
said whether donation-driven promotion is commercial.

## What is still unknown, and what would settle it

- **Whether donation-driven uses (promotional videos, a featured front page) are
  "NonCommercial".** Settles it: a lawyer's opinion, or a court case (none
  found). Cheaper and enough in practice: never rely on NC for those uses; ask
  for the separate permission.
- **Whether a promotional video showing a BY-SA model is "Adapted Material"**
  that must itself be BY-SA. Settles it: a lawyer, or CC's own legal team (CC
  answers general questions by email at info@creativecommons.org, a source's
  view, not binding). Avoided entirely by the separate permission.
- **Whether a free, tick-box permission is revocable**, and whether 17 U.S.C.
  203 reaches public licences or such permissions. Settles it: a lawyer. The
  project can make the first question moot by deciding the answer and writing
  it into the permission.
- **Who the grantee is** for a permission to "HumanityOS", and how it passes to
  later maintainers. Settles it: a lawyer; or forming a legal entity for the
  project, which is a larger decision.
- **Whether a free, donation-supported server is an "information society
  service"** ("normally provided for remuneration") under the UK regulations
  and the DSA, and whether a small US server offers services in the Union in
  the DSA's sense (Article 3(d) and (e)).
  Settles it: a UK or EU lawyer; the CJEU's case law on remuneration was not
  read.
- **Whether "financial benefit directly attributable"** in 512(c)(1)(B) can ever
  include general donations. Settles it: a lawyer, or US case law on that
  clause (not read).
- **Flickr's default for a new account.** Settles it: creating a test account,
  or asking Flickr support. Practice only, so low value.
- **Exif's own copyright and artist fields** (the Exif standard, CIPA DC-008,
  was not read). Settles it: reading the standard; a short task.
- **Publicity and model-release rules** for people shown in members' photos and
  videos used in promotion. Settles it: a separate dated finding.
- **The text of 37 C.F.R. 201.38** (the agent rule) was not read directly; the
  Copyright Office's FAQ was. Settles it: reading the eCFR section.

## Sources that could not be reached

- **EUR-Lex** (https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32022R2065
  and the same for 32019L0790, and the ELI address) answered HTTP 202 with an
  empty page (a bot check). The texts were read from the EU Publications
  Office's copy instead. Wanted from EUR-Lex: the consolidated version, to
  confirm no later amendment.
- **flickrhelp.com** refused tool requests (HTTP 403); its two articles were
  read in a browser.
- **courtlistener.com** opinion pages returned nothing to direct requests; the
  four opinions were read in a browser, and only the passages quoted were
  extracted.
- **The CC0 and CC 4.0 legal code pages carry no publication date**; the dates
  given come from CC's own *License Versions* wiki page.
- **Not looked for:** the eCFR text of 37 C.F.R. 201.38; national implementations
  of Directive 2019/790; the UK Online Safety Act's bearing on copyright
  notices; publicity-rights statutes.

## What it means for HumanityOS

### Certain (stated by the sources above)

1. Every upload is protected by copyright from the moment it was made, with no
   statement needed; "All rights reserved" describes that state and adds no
   legal effect of its own.
2. CC BY, CC BY-SA, CC BY-NC and CC0 cannot be taken back for copies already
   taken. A server can stop offering an item under CC; it cannot recall it.
3. A CC licence on a members-only item lets any member who receives it
   republish it anywhere.
4. A member can choose a licence only for what they own. For anyone else's
   work, the item's own licence (or the law) is all there is, and the member
   cannot license the server for it.
5. CC0 cannot require credit; the three CC licences require reasonable credit
   and let the creator have their name removed from adaptations.
6. NonCommercial turns on the purpose of the use; nonprofits get no automatic
   pass, and CC will not say where the line falls.
7. Non-exclusive licences, including a server's hosting licence and the
   project's separate permission, need not be written, but writing fixes their
   scope; in US case law a non-exclusive licence without consideration is
   revocable.
8. US safe harbour for members' uploads requires a designated agent registered
   with the Copyright Office ($6, renewed within three years, the service
   provider's street address published), expeditious removal on notice, a
   counter-notice process, and a repeat-infringer policy the members are told
   about. EU hosting providers must offer an electronic notice form.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **Default every item to "All rights reserved"**, shown in plain words (for
   example "Only me: people may look, not reuse"). It protects the uploader
   best, it is what they already have, it can be loosened later, and it is
   Pixelfed's default too. Offer, per item: CC BY 4.0, CC BY-SA 4.0, CC BY-NC 4.0
   and CC0 1.0, each with a one-line plain explanation and a link to its deed,
   plus **"Someone else's work"** with fields for whose it is and under what
   licence or permission it is shared.
2. **Warn before any CC choice**, once and plainly: "Anyone who saves a copy
   keeps these rights for good, even if you change your mind later." On a
   members-only item add: "Members who receive it may share it publicly."
   Changing a CC item back to "All rights reserved" should say that copies
   already taken keep the old licence (Flickr's wording is a good model).
3. **Give every server a hosting licence in its terms**, granted by each
   uploader to that server's operator: non-exclusive, royalty-free, only to
   store, back up, copy, make previews and thumbnails, convert formats, show
   and serve the item to the audience the uploader chose, and feature it on
   that same server; nothing outside the server; ending when the item is
   removed, apart from backups that expire on their normal schedule. Ship it as
   default text in the software that each operator adopts and may edit, since
   every server is its own service provider. Keep it separate from the item's
   public licence, so hosting never depends on how NC is read.
4. **Make "Let HumanityOS use this in the game and in its own videos, with
   credit" a separate, unticked opt-in**, with the eight points in section 6.4
   written into its short text. Ask for it whatever the public licence, since
   it is the only clean route for NC and "All rights reserved" items and for
   promotional videos. Decide withdrawal before shipping it; the suggested
   answer is "withdraw any time; what is already released or published may
   stay".
5. **Only CC0, CC BY and CC BY-SA items, or items whose separate permission
   covers redistribution with the game and its forks, go into the game.** Each
   gets a row in `data/credits.ron` and `LICENSES.md` saying it is not CC0,
   following the pattern those files already use for third-party sources. NC
   and "All rights reserved" items stay out.
6. **Register a DMCA agent for united-humanity.us and public.guide before the
   Media page is open to the public**, consider a third-party agent service or
   the safety waiver if publishing a home street address is a problem, and add
   to the server rules a takedown address, the counter-notice process and a
   repeat-infringer policy. Give the EU notice form the fields Article 16(2)
   asks for; the existing report flow
   (`docs/design/blocking-and-safe-mode.md` section 10e) is the natural home.
7. **Keep credit outside the file and consider keeping it inside too**: show
   the creator and licence on every item page, and change the metadata strip so
   it keeps creator and copyright fields (removing location and camera details
   as now), or ask the uploader.
8. **Ask before using any photo of a person in promotion**, separately from the
   uploader's permission.
9. **Re-check this finding** before the Media page goes public, before the
   first member's model goes into the game, and whenever Creative Commons
   publishes a new licence version.

### Observed in the code on 12 October 2026 (relevant to the judgement calls)

Read, not changed, while writing this; a reviewer should confirm.

- `docs/design/media-showcase.md` plans a per-item "public" or "members" choice
  and an admin "Featured" list described as "the "showcase" for visitors and
  the donation story". It says nothing about licences.
- `web/pages/rules.html`, the only rules page found, has no copyright, licence
  or takedown wording.
- No mention of a DMCA agent, designated agent or takedown process was found
  under `docs/` apart from unrelated passages about takedowns of the project's
  own releases.
- `web/shared/strip-metadata.js` removes JPEG APP1 (Exif and XMP) and APP13
  (IPTC), PNG text and Exif chunks, and WebP EXIF and XMP chunks, and
  neutralises video metadata boxes.
- `LICENSE` is CC0 1.0 Universal. `LICENSES.md` and `data/credits.ron` already
  record third-party material with its obligations, and `src/credits.rs` tests
  that every source needing attribution names where it is shown.

---

This document is a reading of public sources on 12 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. Ask a lawyer before relying
on any of it for a decision with legal consequences, starting with the terms
each server adopts and the wording of the separate permission.
