# What did the Grand Theft Auto VI leak subpoenas ask Discord and Microsoft for?

**Research date: 7 November 2026.** Everything below was read on that day. If you are reading this
later, check the date of each source against anything that has changed since, above all the two
court dockets, which were still open.

This is a reading of public court filings and public statements by an AI working on the project
(Claude), not legal advice. Nobody who wrote it is a lawyer.

## The question

`docs/outreach/privacy_by_architecture.md` opened with a story: a court order "forced Discord and
Microsoft to hand over personal information on roughly 100,000 users, real names, emails, phone
numbers, IP addresses, device identifiers, linked accounts, to help hunt down a single person who
leaked a video game." Is that true, and what exactly do the sources say: the number, who was
ordered, what was asked for or handed over, and why?

## Short answer

**The story is real, but four of its details went further than the record.** Corrected:

- **What it was.** On 20 August 2026 Take-Two Interactive Software, the publisher of Grand Theft
  Auto VI, asked the United States District Court for the Southern District of New York for two
  subpoenas under the Digital Millennium Copyright Act's subpoena section (17 U.S.C. 512(h)): one to
  Discord (case 1:26-mc-422) and one to Microsoft (case 1:26-mc-421), in the Discord declaration's
  words "to identify the user or users responsible for posting Take-Two’s copyrighted content
  online" (finding 3; the Microsoft declaration says "the user responsible"). Both were approved on
  21 August 2026: Judge Jennifer L. Rochon's signed order for Microsoft is on the docket (finding 6);
  for Discord, Take-Two told the court that the request was granted, the subpoena issued and served
  that day (finding 4; the court's own order there is a scanned page that could not be read).
- **What was asked for, not handed over.** The Discord subpoena asked, for "all user accounts that
  are/were members" of three Discord communities from 1 June 2026, for message logs, IP access logs,
  email addresses, phone numbers, connected and linked accounts, registration times and device
  identifiers (finding 2). The Microsoft subpoena asked for the records of Microsoft's own
  investigation of the "cyberleek" persona and, for members of the same Discord servers, much the
  same list plus OneDrive content about the game (finding 5). **"Real names" is in neither list.**
  **Nothing public shows that either company handed anything over**: Discord said on 24 August 2026
  that it had not yet been served and would "evaluate the validity and scope before responding"
  (finding 8); Take-Two told the court it was served (finding 4).
- **The number.** **No court document gives one.** "~100k Discord users" comes from a widely shared
  post of 22 August 2026 by a commentator, which itself put the largest server at 90,000 members
  (finding 10). What Discord's own data says, read on 7 November 2026: the largest of the three
  communities, the server of the Grand Theft Auto video creator DarkViperAU (the server ID named in
  the subpoena), has **117,129 members**; "! Odyssey" has 5,716 (finding 9). How many accounts were
  members between 1 June and August 2026 only Discord knows.
- **Why.** Take-Two's filings say the content infringed its copyrights in Grand Theft Auto VI, and
  its letter to the judge describes "an ongoing investigation into copyright infringement and the
  misappropriation of Take-Two’s confidential information" (findings 1 and 7). The subpoena names
  three Discord accounts (CYBERLEEK, CINEMATICROCKSTAR, Surfer24k) and the servers, not "a single
  person".

So the page may tell the story, with these details: subpoenas a court approved, asking (not shown
handing over) for data on every member of three communities, one with more than 117,000 members by
Discord's count, to find the few accounts that posted leaked Grand Theft Auto VI material.

## Findings

### 1. Take-Two's request for the Discord subpoena

- Source: *In re DMCA Subpoena to Discord, Inc.*, No. 1:26-mc-422 (S.D.N.Y.), Document 1, Take-Two's
  request to the clerk for issuance of a subpoena to Discord, https://storage.courtlistener.com/recap/gov.uscourts.nysd.671042/gov.uscourts.nysd.671042.1.0.pdf
- Source date: filed 20 August 2026 (the page header: "Filed 08/20/26").
- Quoted exactly:

  > Petitioner Take-Two Interactive Software, Inc. (“Take-Two”) through its undersigned counsel of record, hereby requests that the Clerk of this Court issue a subpoena to Discord, Inc. to identify alleged infringers at issue, pursuant to the Digital Millennium Copyright Act (“DMCA”), 17 U.S.C. § 512(h)

  > This content infringes copyrights held by Take-Two for GRAND THEFT AUTO VI, proprietary software owned by Take-Two.

- The accounts and servers it names: "CYBERLEEK, Discord User ID 532773089466122241",
  "CINEMATICROCKSTAR, Discord User ID 1183522740725108736", "Surfer24k (and replacement handles
  cyberleek_west and surwest), Discord User ID 1517326120867991592", "https://discord.gg/darkviperau"
  and "https://discord.gg/odyssey".

### 2. What the Discord subpoena asked for (its Attachment A)

- Source: the same case, Document 1-1, Exhibit A, the proposed subpoena on form AO 88B with
  "Attachment A, Documents to be produced under subpoena",
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671042/gov.uscourts.nysd.671042.1.1.pdf
- Source date: filed 20 August 2026; the form names a production date of "09/04/2026 10:00 am" at
  Kirkland & Ellis in New York.
- Quoted exactly, the request that reaches every member:

  > All identifying information associated with all user accounts that are/were members of the Discord community servers and channels listed below during the time period from June 1, 2026 to present. For each returned account, provide the following: backend server metadata or telemetry records, original message logs, IP access logs, email addresses, phone numbers, connected accounts, registration timestamps, linked connections (Google/Xbox/etc.), and any associated device identifiers (MachineGuid/MSA).

- The servers listed under it: "Ødyssey.gg" with the number 1517326120867991592, "! Odyssey"
  (1127436882800816149, discord.gg/odyssey) and "DarkViperAU" (268280696601051136,
  discord.gg/darkviperau). Observed, not explained in the filing: the number given for Ødyssey.gg is
  the same number given as Surfer24k's account ID, marked "(resolve via guild".
- Not in the list: real names. A Discord account does not ask for one; the list asks for what
  Discord holds.

### 3. The sworn purpose

- Source: the same case, Document 4, "Declaration of Dale Cendali",
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671042/gov.uscourts.nysd.671042.4.0.pdf
- Source date: filed and dated 20 August 2026.
- Quoted exactly (paragraph 3):

  > I submit this declaration in support of Take-Two’s request for issuance to Discord, Inc., of a subpoena, pursuant to the Digital Millennium Copyright Act (“DMCA”) 17 U.S.C. § 512(h) (hereinafter “DMCA Subpoena”), to identify the user or users responsible for posting Take-Two’s copyrighted content online.

- The Microsoft case's matching declaration (No. 1:26-mc-421, Document 4, filed 20 August 2026,
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671027/gov.uscourts.nysd.671027.4.0.pdf)
  says, in the same place, "to identify the user responsible for posting Take-Two’s copyrighted
  content online."

### 4. Granted, issued and served, in Take-Two's words; a second, narrower request

- Source: the same case, Document 7, Take-Two's second request to the clerk,
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671042/gov.uscourts.nysd.671042.7.0.pdf
- Source date: filed and dated 28 August 2026; the case number now carries the judge's initials
  ("1:26-mc-00422-ALC", Judge Andrew L. Carter, Jr., to whom the letter in finding 7 is addressed).
- Quoted exactly (footnote 1):

  > This is Take-Two’s second request for a DMCA subpoena to Discord. The Court granted Take-Two’s first request on August 21, 2026, the Clerk of this Court issued the subpoena that same day, and the subpoena was promptly served on Discord. (Dkt. 5).

- And, on the second request:

  > This second DMCA Subpoena request seeks different information than that sought from Take-Two’s first subpoena and provides additional clarifying information regarding one of the user IDs specified in the first subpoena.

- The court's own order (Document 5, filed 21 August 2026) and a later entry (Document 10, filed 31
  August 2026) are scanned pages with no text layer, so their words were not read (see "Sources we
  could not read"). This finding relies on Take-Two's account of Document 5, and on the Microsoft
  docket, where the matching order is readable (finding 6).

### 5. What the Microsoft subpoena asked for

- Source: *In re DMCA Subpoena to Microsoft Corporation*, No. 1:26-mc-421 (S.D.N.Y.), Document 1 (the
  request) and Document 1-1 (Exhibit A, with Attachment A),
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671027/gov.uscourts.nysd.671027.1.0.pdf and
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671027/gov.uscourts.nysd.671027.1.1.pdf
- Source date: both filed 20 August 2026.
- Why Microsoft, quoted exactly from Document 1:

  > The DMCA Subpoena is directed to Microsoft Corporation, the service provider of GitHub (a subsidiary of Microsoft Corporation). Github is the service provider of a website with the URL https://github.com/zyrexdz/cyberleek-leak-research to which the infringing content was posted.

- What it asked for, quoted exactly from Attachment A:

  > All internal Microsoft business records and investigative records associated with Microsoft’s internal investigation of the "cyberleek" persona sufficient to identify the user(s), person(s) and/or entity/ies associated with that persona.

  > All identifying information associated with all user accounts that are/were members communicating with the Discord server(s) listed below during the time period from June 1, 2026 to present; and for those accounts, provide any associated device/telemetry records where one or more of the tool/file artifacts below were present on the system. For each returned account, provide: the account ID, registration email, registration and last-login IP addresses, phone number, linked connections (Google/Xbox/etc.), any associated device identifiers (MachineGuid/MSA), and content related to Grand Theft Auto (“GTA”), Rockstar or Cyberleek from their OneDrive account.

- The same three servers and three accounts follow, the DarkViperAU server listed as "DarkViperAU editors'
  server (discord.gg/darkviperau) (invite; guild ID TBD by partner)".

### 6. The Microsoft order

- Source: the same Microsoft case, Document 5,
  https://storage.courtlistener.com/recap/gov.uscourts.nysd.671027/gov.uscourts.nysd.671027.5.0.pdf
- Source date: filed 21 August 2026 ("1:26-mc-00421-JLR"), dated "August 21, 2026", over the name
  "UNITED STATES DISTRICT JUDGE JENNIFER L. ROCHON".
- Quoted exactly:

  > This matter comes before the Court upon the ex parte application of petitioner Take-Two Interactive Software, Inc. along with the Declaration of Dale Cendali and supporting documents for the signing of a Subpoena directing Microsoft Corporation to produce the identity of entities or persons believed to be infringing on the copyright of Take-Two Interactive Software, Inc.

  > ORDERED that the clerk of this Court shall issue the Subpoena for Microsoft Corporation as sought by the petitioner.

- Observed: the order's own heading reads "[PROPOSED] ORDER GRANTING PETITIONER TAKE-TWO INTERACTIVE
  SOFTWARE, INC.’S RULE 7.1 DISCLOSURE STATEMENT", a slip in the proposed order's title; its text is
  the subpoena order quoted above. It was made "upon the ex parte application", on Take-Two's
  application alone.

### 7. Why Take-Two asked to seal the second request's list

- Source: the Discord case, Document 6, letter motion from Dale M. Cendali to Judge Andrew L. Carter,
  Jr., https://storage.courtlistener.com/recap/gov.uscourts.nysd.671042/gov.uscourts.nysd.671042.6.0.pdf
- Source date: filed 28 August 2026; its first page is dated "August 28, 2026" and its later page
  headers "August 27, 2026".
- Quoted exactly:

  > The Subpoena Attachment reveals highly sensitive, confidential information about an ongoing investigation into copyright infringement and the misappropriation of Take-Two’s confidential information.

### 8. Discord's statement

- Source: a post on X by Ryan K. Rigney, whose account carries Discord's business label,
  https://x.com/RKRigney/status/2091951509931954436 (read through X's public embed data,
  https://cdn.syndication.twimg.com/tweet-result?id=2091951509931954436&token=a, because X's own page
  needs a sign-in).
- Source date: 24 August 2026, 18:11:21 UTC (`created_at`), not edited (one entry in its edit history).
- Quoted exactly (its line breaks shown as spaces):

  > I can share an update on this: Discord has not yet been served with a subpoena from Take Two. When we do, we'll evaluate the validity and scope before responding.

- Where this disagrees with finding 4: Take-Two told the court on 28 August that the subpoena "was
  promptly served on Discord" after its issue on 21 August. Both can be true (service between 24 and
  28 August); the record read here does not say when.

### 9. How big the three communities are, by Discord's own count

- Source: Discord's public invite data for the two invite links the subpoena names,
  https://discord.com/api/v10/invites/darkviperau?with_counts=true and
  https://discord.com/api/v10/invites/odyssey?with_counts=true.
- Source date: live counts, read 7 November 2026 at 09:13 UTC. They are today's counts, not August's.
- Quoted exactly from the first: `"id":"268280696601051136","name":"DarkViperAU"`, `"description":"Community
  Discord for GTA 5 Content Creator DarkViperAU"`, `"member_count":117129`. From the second:
  `"id":"1127436882800816149","name":"! Odyssey"`, `"member_count":5716`.
- The DarkViperAU server's ID matches the one in the Discord subpoena (finding 2). The third
  community (Ødyssey.gg, reached through a private invite) was not looked up.

### 10. Where "roughly 100,000" came from

- Source: a post on X by "Pagan" (@Pagan_GMR), https://x.com/Pagan_GMR/status/2091193953282293919,
  quoted inside Rigney's post in finding 8 (read through the same embed data).
- Source date: 22 August 2026, 16:01:05 UTC.
- Quoted exactly:

  > Take-Two got court approval to pull personal info (emails, phone numbers, IPs, device IDs) from ~100k Discord users across multiple servers... including DarkViperAU’s 90k-member server.

- A commentator's count of server sizes, not a number from any filing. Reporting at the time said the
  same: Dexerto's headline of the time reads "Discord users outraged over GTA 6 leak subpoena seeking
  personal data from over 100k members"
  (https://www.dexerto.com/gta/discord-users-outraged-over-gta-6-leak-subpoena-seeking-personal-data-from-over-100k-members-3405630/,
  its address read from search results, the page itself not opened).

### 11. What kind of order a 512(h) subpoena is

- Source: 17 U.S.C. 512(h), as published by Cornell's Legal Information Institute,
  https://www.law.cornell.edu/uscode/text/17/512
- Source date: the page shows no date; its notes list the last amendment as Pub. L. 111-295 (9
  December 2010).
- Quoted exactly:

  > (1) Request.— A copyright owner or a person authorized to act on the owner’s behalf may request the clerk of any United States district court to issue a subpoena to a service provider for identification of an alleged infringer in accordance with this subsection.

  > (3) Contents of subpoena.— The subpoena shall authorize and order the service provider receiving the notification and the subpoena to expeditiously disclose to the copyright owner or person authorized by the copyright owner information sufficient to identify the alleged infringer of the material described in the notification to the extent such information is available to the service provider.

- So such a subpoena is an order to disclose, issued by the court's clerk on a proper filing (here,
  with a judge's order directing it), without a lawsuit against anyone. Whether a subpoena as broad as
  finding 2 is within what 512(h) allows ("information sufficient to identify the alleged infringer")
  is a question for a court and was not researched; a lawyer would answer it.

### Reporting used only to find the sources above

- Engadget, Ian Carlos Campbell, 21 August 2026,
  https://engadget.com/2241941/take-two-subpoenas-discord-and-microsoft-in-hunt-for-gta-vi-leaker
  (linked the two Document 1 filings).
- Kotaku, Ethan Gach, 24 August 2026,
  https://kotaku.com/discord-says-it-hasnt-been-served-with-a-subpoena-over-gta-6-leaks-yet-well-evaluate-the-validity-and-scope-before-responding-2000727642
  (linked the post in finding 8, and a Microsoft statement: Xbox's Scott Van Vliet on X, 21 August
  2026, 20:53:03 UTC, https://x.com/scottvanvliet/status/2090905042014576920, read through the embed
  data: "We are working closely with Take-Two and Rockstar Games to support efforts to protect
  creative works and intellectual property.").
- TorrentFreak, Ernesto Van der Sar, 31 August 2026,
  https://torrentfreak.com/take-two-says-gta-6-leak-probe-is-rapidly-evolving-wants-new-discord-demands-under-seal/
  (reported then that the court had not yet ruled on the sealing request and the second subpoena had
  not been issued; that is TorrentFreak's reading, not checked against Document 10).

## What is still unknown, and what would settle it

- **Whether Discord or Microsoft handed anything over, and what.** No filing read and no statement
  found says so. Settle it with the dockets of both cases after 31 August 2026 (a motion to quash or
  to compel would show there), read on CourtListener (free, where someone has already uploaded the page) or on PACER, the courts'
  own system (paid); or with each company's statement if asked.
- **The exact words of the Discord orders** (Documents 5 and 10, scanned pages). Settle it by opening
  them by eye or through optical character recognition, free on CourtListener; neither was possible
  here.
- **What the second Discord subpoena asks for.** Its Exhibit A (Document 7-1) is a scanned page and its
  Attachment A was asked to be sealed. Only the court's ruling, or the sealed text if unsealed, would
  settle it.
- **How many accounts the first subpoena reached.** Members between 1 June 2026 and the subpoena's
  date; only Discord's records can answer, and it has not said.
- **Whether a member-wide request like finding 2 fits 512(h).** A legal question for a court or a
  lawyer; not answered by anything read here.

## Sources we could not read

- https://www.courtlistener.com/ search and docket pages: HTTP 403 to automated reading. Hoped for:
  the docket entries' own descriptions of Documents 5 and 10 and anything filed after 31 August.
- https://www.courtlistener.com/api/rest/v4/search/ and the v3 API: they need a signed-in account.
- Documents 5 and 10 of 1:26-mc-422, and Document 7-1 (the second subpoena's Exhibit A): scanned
  images with no text layer, and no page renderer was available to read them.
- https://www.digitalcitizen.life/take-two-gta-6-leak-subpoenas-court-record/: HTTP 403. Hoped for: its
  summary of the docket's later entries, as a pointer.
- https://pcgamer.com/games/grand-theft-auto/as-take-two-seeks-ip-addresses-and-phone-numbers-of-thousands-in-gta-6-subpoena-discord-says-its-not-been-served-the-order-just-yet:
  the page came back cut off after its headline.
- Documents 8, 11 and 12 of 1:26-mc-422 and Documents 6 onwards of 1:26-mc-421: not in CourtListener's
  archive (404) on 7 November 2026, which means only that nobody has uploaded them there.

## What it means for this project

**Certain:**

- The story behind the sealed-sender work of 23 August 2026 (`docs/history/2026-08-23-sealed-sender-dms.md`)
  is real: a court-approved subpoena asked Discord for identifying data, message logs and network
  addresses on every member of three Discord communities, one of them with 117,129 members by
  Discord's count on 7 November 2026, and another asked Microsoft for much the same on the same
  members, to find the few accounts that posted Grand Theft Auto VI material.
- "Forced ... to hand over" is not supported: they were ordered to disclose; whether they did is not
  public. "Real names" was not asked for. "Roughly 100,000 users" is not in any filing.
- The page `privacy_by_architecture.md` (now `docs/outreach/how_your_privacy_is_built.md`) says only
  what these findings support, as of this date.

**Judgement calls:**

- That most members of those communities had nothing to do with the leak. The subpoena itself names
  three accounts among more than 100,000 members; that is the basis, and the page says so.
- That the episode is a fair example of why holding less data protects people. It is the project's
  argument, not a finding about Discord, which publishes rules for legal requests and said it would
  evaluate this one.

Not legal advice. Nobody who wrote this is a lawyer; anyone relying on it for a legal step should have
a lawyer read the filings themselves.
