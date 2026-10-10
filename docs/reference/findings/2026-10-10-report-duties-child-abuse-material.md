# What a server operator must do, and must not do, when a report says child sexual abuse material may be involved

**Research date: 10 October 2026** (sources read between about 00:20 and 01:05
Pacific time, which is 07:20 to 08:05 UTC on the same day). Everything below
was read on that day. Laws in this area were amended in 2018, 2024 and 2025 in
the United States and came into force in April 2026 in the United Kingdom; if
you are reading this later, check each source's own date against anything that
has changed since.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such, and a
lawyer could disagree with any of it.

## The question

When a person reports someone to a HumanityOS server's admins and the report
says child sexual abuse material may be involved, what does the server's
operator have to do, and have to avoid doing, under the law of the United
States (where the project's own server is), and in brief under the law of the
United Kingdom and the European Union? (Design:
`docs/design/blocking-and-safe-mode.md` sections 6.6, 8 and 10e; open item in
`docs/PRIORITIES.md`, "Legal research needed before reports go further".)

## Short answer

**United States.** A "provider" (a provider of an electronic communication
service or a remote computing service) that gains **actual knowledge** of facts
showing an apparent child sexual abuse material offence, or the enticement or
sex trafficking of a child, must report it to NCMEC's **CyberTipline** "as soon
as reasonably possible" and give the CyberTipline its contact details. After
reporting it must **preserve** what it reported, plus reasonably accessible
context, **for 1 year**, securely and with access limited. It need not monitor
or scan anyone. It may share the report's contents only with NCMEC, law
enforcement or in answer to legal process. Its legal protection covers only the
reporting and preserving. Separately, anyone who knowingly receives, keeps,
passes on or **accesses the material with intent to view it** commits a federal
crime; the defence in 18 U.S.C. 2252A(d) for a person who ends up holding a few
images requires destroying them or reporting to law enforcement promptly,
without letting anyone else see them.

**Whether a volunteer running a small server for friends is a "provider" is not
answered by any official source found.** The definition has no size, money or
business condition; the Justice Department's manual describes it in terms of a
"company or government entity"; NCMEC speaks of "U.S.-based ESPs". Treat it
as unknown, and act as if the duty applies.

**United Kingdom.** Since 7 April 2026, providers of regulated user-to-user
services (which can be an individual) must report detected child sexual
exploitation and abuse content to the National Crime Agency, unless it is
already covered by reports to a foreign agency such as NCMEC; "detected"
includes being told by another person. **European Union.** A hosting provider
loses its liability shield unless it removes or disables illegal content
expeditiously once it knows, and must promptly tell law enforcement of a
suspected offence threatening a person's life or safety; the Digital Services
Act's recitals name child sexual abuse offences as an example.

**For HumanityOS, in the project's reading:** admins should never be shown
files; a report about a child should point the reporter to the official hotline
first; and an operator who learns of material should not open, copy or pass it
on, should take it out of view without destroying it, should report it to the
CyberTipline (or their country's equivalent), and should keep what was reported
for a year in a locked-away place. The app today hard-deletes a reported post
and leaves its uploaded file at its public address, and keeps reports for 90
days; both need changing for this case (see the end of this document).

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Statute quotations are from the official
text: the US Code as published by the Government Publishing Office (govinfo,
"United States Code, 2024 Edition", issued 31 December 2024), UK legislation
from legislation.gov.uk (The National Archives), and EU law from EUR-Lex.
Long passages are trimmed to the words that matter; nothing inside a quotation
is reworded. US Code text marks a few typos in the original with a footnote
"So in original"; those footnote markers are left out of the quotations here.

Every quotation of an outside source was checked by script against the
downloaded page (tags removed, spaces collapsed), except the CEOP Education
page, which blocked direct requests and was read through a fetching tool that
renders the page; its quotations could not be checked byte for byte and are
marked. (The bill status line in US-13 was checked against the XML record.)

---

## United States

### US-1. Who counts as a "provider"

**The statute's definition has no size, payment or business condition.**

- 18 U.S.C. 2258E, the definitions for sections 2258A to 2258E:
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2258E.htm
  (2024 Edition; last amended 2018).
  > the term "provider" means an electronic communication service provider or remote computing service

  The same section says "electronic communication service" has the meaning in
  section 2510, and "remote computing service" the meaning in section 2711.
- 18 U.S.C. 2510(15):
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap119-sec2510.htm
  > "electronic communication service" means any service which provides to users thereof the ability to send or receive wire or electronic communications
- 18 U.S.C. 2711(2):
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap121-sec2711.htm
  > the term "remote computing service" means the provision to the public of computer storage or processing services by means of an electronic communications system

  Note the difference: a remote computing service must be provided "to the
  public"; an electronic communication service, as defined, need not be.
- The Justice Department's manual for its own prosecutors, *Searching and
  Seizing Computers and Obtaining Electronic Evidence in Criminal
  Investigations*, third edition (2009),
  https://www.justice.gov/criminal/cybercrime/docs/ssmanual2009.pdf (redirects
  to a copy under justice.gov/d9/criminal-ccips/legacy/2015/01/14/), chapter 3:
  > Any company or government entity that provides others with the means to communicate electronically can be a "provider of electronic communication service" relating to the communications it provides, regardless of the entity's primary business or function.

  The manual says of itself that it provides "suggestions to Department of
  Justice attorneys" and creates no rights. It says "company or government
  entity" and does not discuss an individual running a service.
- NCMEC's page on child sexual abuse material,
  https://www.missingkids.org/theissues/csam (no date on the page):
  > U.S. federal law requires that U.S.-based ESPs report instances of apparent child pornography that they become aware of on their systems to NCMEC’s CyberTipline.

  ("ESP" is NCMEC's term, electronic service provider.) The statute itself does
  not say "U.S.-based"; that is NCMEC's description.
- The 2024 fines (US-8 below) are set by size, with a separate figure for "a
  provider with less than 100,000,000 monthly active users", so small providers
  are plainly within the law. Nothing found says how small a service can be
  before it is no provider at all.

### US-2. What triggers the duty: "actual knowledge"

- 18 U.S.C. 2258A(a), https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2258A.htm
  (2024 Edition, which includes the REPORT Act of 7 May 2024). A provider:
  > (i) shall, as soon as reasonably possible after obtaining actual knowledge of any facts or circumstances described in paragraph (2)(A), take the actions described in subparagraph (B); and

  and the facts in paragraph (2)(A) are:
  > any facts or circumstances from which there is an apparent violation of section 2251, 2251A, 2252, 2252A, 2252B, or 2260 that involves child pornography, of section 1591 (if the violation involves a minor), or of 2422(b)

  Section 1591 is sex trafficking; 2422(b) is enticing a minor, by mail or any
  means of interstate commerce, to engage in sexual activity for which anyone
  can be charged. Both were added by the REPORT Act in 2024 (the Code's
  amendment note: "Pub. L. 118–59, §4(a)(1), inserted ", of section 1591 (if
  the violation involves a minor), or of 2422(b)" after "child pornography"").
- Reporting what is only planned or imminent is allowed, not required. A
  provider:
  > may, after obtaining actual knowledge of any facts or circumstances described in paragraph (2)(B), take the actions described in subparagraph (B).

  where (2)(B) covers facts that "indicate a violation of any of the sections
  described in subparagraph (A) involving child pornography may be planned or
  imminent."
- "Actual knowledge" is not defined in sections 2258A to 2258E. No official
  source found says whether a user's report, on its own, gives a provider
  actual knowledge. (Compare the UK and EU below, which do say.)
- **Text alone can be enough to be reportable.** NCMEC's guidance to providers
  on the REPORT Act, *Guidelines on Identifiers of Online Enticement and Child
  Sex Trafficking*,
  https://www.missingkids.org/content/dam/missingkids/pdfs/NCMEC-REPORT-Act-Guidelines.pdf
  ("Released October 29, 2024"), on online enticement: it "is one of eight
  CyberTipline reporting categories, and may include reports concerning
  grooming; engaging in sexualized conversation or conduct with a child". And
  NCMEC's public FAQ, https://report.cybertip.org/faqs (no date; copyright
  2026), lists among what should be reported: "Someone chatting online with a
  child about sex;". The guidelines also say: "The information provided in this
  document does not, and is not intended to, constitute legal advice."

### US-3. What must be reported, where, and how

- 18 U.S.C. 2258A(a)(1)(B), the two required actions:
  > (i) providing to the CyberTipline of NCMEC, or any successor to the CyberTipline operated by NCMEC, the mailing address, telephone number, facsimile number, electronic mailing address of, and individual point of contact for, such provider; and

  > (ii) making a report of such facts or circumstances to the CyberTipline, or any successor to the CyberTipline operated by NCMEC.
- What goes in the report is largely the provider's choice. 2258A(b): the facts
  and circumstances in each report
  > may, at the sole discretion of the provider, include the following information:

  followed by five optional kinds: information about the person involved (email
  address, IP address, URL and other identifiers), when and how the content was
  uploaded or discovered with a time stamp and time zone, geographic location,
  "Any visual depiction of apparent child pornography or other content relating
  to the incident", and "The complete communication".
- How providers submit: NCMEC's *CyberTipline Reporting API Technical
  Documentation*, https://report.cybertip.org/ispws/documentation (server
  `Last-Modified` 26 August 2026):
  > There are two ways for companies to submit reports of apparent child sexual exploitation to NCMEC: they can submit a report using a web form or using this web service API, which allows a more automated reporting experience.

  > While ESPs have a statutory duty to report apparent child pornography to NCMEC’s CyberTipline (see 18 U.S.C. § 2258A), the reporting of data via the web service, other than the incident type and date/time of incident, is voluntary and undertaken at the ESPs' initiative.

  and, on access: "A username and password must be requested from and supplied
  by NCMEC."
- How a provider registers: the NCMEC guidelines above say "NCMEC recommends
  that providers not already registered to report to the CyberTipline contact
  ESPteam@ncmec.org to learn more about registering." NCMEC's CSAM page links
  the question "Are you an ESP who would like to register with NCMEC?" to the
  same address. The same page says registered companies "also receive notices
  from NCMEC about suspected CSAM on their servers".
- The 2025 change: Cornell's Legal Information Institute copy of 2258A
  (https://www.law.cornell.edu/uscode/text/18/2258A, not an official source,
  read because the House's official site was down) shows one later amendment,
  by Pub. L. 119-60 of 18 December 2025: in subsection (c), NCMEC now makes
  each report available to law enforcement "and all supplemental data included
  in the report". It does not change what a provider must do. Not checked
  against an official copy (see "Sources that could not be reached").

### US-4. What must be preserved, and for how long

18 U.S.C. 2258A(h), same source (the REPORT Act changed 90 days to 1 year; the
Code's note: "Subsec. (h)(1). Pub. L. 118–59, §3(1), substituted "1 year" for
"90 days".").

- (h)(1): a completed report to the CyberTipline
  > shall be treated as a request to preserve the contents provided in the report for 1 year after the submission to the CyberTipline.
- (h)(2), the surrounding material:
  > a provider shall preserve any visual depictions, data, or other digital files that are reasonably accessible and may provide context or additional information about the reported material or person.
- (h)(3), how it is kept: a provider
  > shall maintain the materials in a secure location and take appropriate steps to limit access by agents or employees of the service to the materials to that access necessary to comply with the requirements of this subsection.
- (h)(5): a provider "may voluntarily preserve the contents provided in the
  report (including any comingled content described in paragraph (2)) for
  longer than 1 year" to reduce or prevent online sexual exploitation of
  children.
- (h)(6), from 7 May 2025 (one year after the REPORT Act): a provider
  > shall preserve materials under this subsection in a manner that is consistent with the most recent version of the Cybersecurity Framework developed by the National Institute of Standards and Technology, or any successor thereto.

The duty to preserve starts with the completed report. Nothing in the statute
says what to keep between learning of material and reporting it; see "Where the
sources pull in different directions" below.

### US-5. No duty to look

18 U.S.C. 2258A(f), same source:

> Nothing in this section shall be construed to require a provider to

> (1) monitor any user, subscriber, or customer of that provider;

> (2) monitor the content of any communication of any person described in paragraph (1); or

> (3) affirmatively search, screen, or scan for facts or circumstances described in sections (a) and (b).

### US-6. Who a provider may tell

- 18 U.S.C. 2258A(g)(4), same source:
  > A provider that submits a report under subsection (a)(1) may disclose by mail, electronic transmission, or other reasonable means, information, including visual depictions contained in the report, in a manner consistent with permitted disclosures under paragraphs (3) through (8) of section 2702(b) only to a law enforcement agency described in subparagraph (A), (B), or (C) of paragraph (3), to NCMEC, or as necessary to respond to legal process.
- 18 U.S.C. 2702, the general rule on disclosing stored messages,
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap121-sec2702.htm
  (2024 Edition). The rule for a service "to the public":
  > a person or entity providing an electronic communication service to the public shall not knowingly divulge to any person or entity the contents of a communication while in electronic storage by that service; and

  with exceptions including 2702(b)(6), "to the National Center for Missing and
  Exploited Children, in connection with a report submitted thereto under
  section 2258A;" and 2702(b)(3), "with the lawful consent of the originator or
  an addressee or intended recipient of such communication". (The person who
  received a message choosing to hand it over, which is how HumanityOS report
  evidence works, falls under the second.)

### US-7. The limited liability, and what it does not cover

18 U.S.C. 2258B,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2258B.htm
(2024 Edition; amended 2024).

- (a), what is protected:
  > a civil claim or criminal charge against a provider or domain name registrar, including any director, officer, employee, or agent of such provider or domain name registrar arising from the performance of the reporting or preservation responsibilities of such provider or domain name registrar under this section, section 2258A, or section 2258C may not be brought in any Federal or State court.
- (b), what is not: the protection does not apply if the provider or its people
  "engaged in intentional misconduct", or acted or failed to act "with actual
  malice", with reckless disregard of a substantial risk of physical injury, or
  "for a purpose unrelated to the performance of any responsibility or function
  under sections 2258A, 2258C, 2702, or 2703."
- (c), "Minimizing Access": a provider shall
  > (1) minimize the number of employees that are provided access to any visual depiction provided under section 2258A or 2258C; and

  > (2) ensure that any such visual depiction is permanently destroyed, upon a request from a law enforcement agency to destroy the visual depiction.

In plain words: the shield covers reporting and preserving. Looking at,
copying or sharing material for any other reason is outside it.

### US-8. Penalties for not reporting

18 U.S.C. 2258A(e), same source: a provider that "knowingly and willfully fails
to make a report required under subsection (a)(1)" is fined, for a first
failure,

> not more than $850,000 in the case of a provider with not less than 100,000,000 monthly active users or $600,000 in the case of a provider with less than 100,000,000 monthly active users; and

and for a second or later failure up to $1,000,000 or $850,000. The REPORT Act
raised these from $150,000 and $300,000. Subsection (d)(1): "The Attorney
General shall enforce this section."

### US-9. What anyone, staff and admins included, must not do

These offences apply to every person, provider or not.

- 18 U.S.C. 2252A,
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2252A.htm
  (2024 Edition; last amended 23 December 2024). It punishes whoever
  "knowingly receives or distributes" child pornography (a)(2), whoever
  knowingly reproduces it for distribution (a)(3)(A), and whoever (a)(5)(B)
  > knowingly possesses, or knowingly accesses with intent to view, any book, magazine, periodical, film, videotape, computer disk, or any other material that contains an image of child pornography

  The penalty for (a)(5) is up to 10 years, more in aggravated cases.
- The statutory defence for holding a few images, 2252A(d) (section 2252,
  the companion offence, was not read for its own defence): it is an
  affirmative defence to a possession charge that the defendant
  > (1) possessed less than three images of child pornography; and

  > (2) promptly and in good faith, and without retaining or allowing any person, other than a law enforcement agency, to access any image or copy thereof

  > (A) took reasonable steps to destroy each such image; or

  > (B) reported the matter to a law enforcement agency and afforded that agency access to each such image.
- What counts, 18 U.S.C. 2256(8),
  https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2256.htm:
  any visual depiction of sexually explicit conduct where, among other cases,
  > such visual depiction is a digital image, computer image, or computer-generated image that is, or is indistinguishable from, that of a minor engaging in sexually explicit conduct; or
- The Justice Department's *Citizen's Guide to U.S. Federal Law on Child
  Pornography*,
  https://www.justice.gov/criminal/criminal-ceos/citizens-guide-us-federal-law-child-pornography
  ("Updated August 11, 2023"):
  > Images of child pornography are not protected under First Amendment rights, and are illegal contraband under federal law.

### US-10. What NCMEC tells the public who report

NCMEC's CyberTipline FAQ, https://report.cybertip.org/faqs (no date; copyright
2026):

- On screenshots: "I have screenshots of what I saw, can I send them in with
  the report?" Answer:
  > No, you cannot attach them to the CyberTipline report you are making but it is important to make a note in the report that you do have them.
- On who may report: "No, anyone can make a report of child sexual
  exploitation or abuse."
- On what happens: "Every CyberTipline report is made available to law
  enforcement for their independent review and assessment."
- The report page, https://report.cybertip.org/ (no date): "If you or someone
  you know is in immediate danger, please call 911 or your local police
  immediately."

The public web form takes no attachments; only registered providers upload files
(US-3).

### US-11. Where the government sends people

The Justice Department's Child Exploitation and Obscenity Section,
https://www.justice.gov/criminal/criminal-ceos/report-violations ("Updated
August 11, 2023"): to report the possession, distribution, receipt or production
of child pornography, "file a report on the National Center for Missing &
Exploited Children (NCMEC)'s website", or call 1-800-843-5678; "Your report will
be forwarded to a law enforcement agency for investigation and action." And:

> If you have an emergency that requires an immediate law enforcement response, please call 911 or contact your local Police Department or Sheriff’s Department.

### US-12. A related duty, outside the question: the TAKE IT DOWN Act

Not about child sexual abuse material as such, but it can land on the same
report. Public Law 119-12, enacted 19 May 2025,
https://www.govinfo.gov/content/pkg/PLAW-119publ12/html/PLAW-119publ12.htm,
section 3: within one year of enactment (so by 19 May 2026) a "covered platform"
must run a process for a person to ask for removal of an intimate image of them
published without consent, and on a valid request must act

> as soon as possible, but not later than 48 hours after receiving such request

to "(A) remove the intimate visual depiction; and" make reasonable efforts to
remove known identical copies. A covered platform is "a website, online service,
online application, or mobile application" that "serves the public" and "that
primarily provides a forum for user-generated content, including messages,
videos, images, games, and audio files;". The Federal Trade Commission enforces
it, including "with respect to organizations that are not organized to carry on
business for their own profit or that of their members." Whether a small
HumanityOS server is a covered platform was **not researched**; it needs its
own finding.

### US-13. Bills that could change this

The STOP CSAM Act of 2025 (S. 1829) would change provider duties. Its official
bill status record,
https://www.govinfo.gov/bulkdata/BILLSTATUS/119/s/BILLSTATUS-119s1829.xml
(record updated 2026-09-16), shows its latest Senate action on 2025-06-26:
"Placed on Senate Legislative Calendar under General Orders. Calendar No. 106."
It is not law. Its contents were not read.

---

## United Kingdom (brief)

All from legislation.gov.uk. Each Online Safety Act 2023 section below was
shown as "up to date with all changes known to be in force on or before" a date
between 03 and 10 October 2026 (section 66: 03 October). The section 66 page
also listed changes made by a 2026 Act ("2026 c. 20") to section 102 and
Schedule 7 that were not yet applied to the text; those were not researched.

- **The duty**, Online Safety Act 2023 section 66(1),
  https://www.legislation.gov.uk/ukpga/2023/50/section/66 :
  > A UK provider of a regulated user-to-user service must operate the service using systems and processes which secure (so far as possible) that the provider reports all detected and unreported CSEA content present on the service to the NCA.

  Section 66(2) applies the same to a non-UK provider for "UK-linked" content.
- **In force since 7 April 2026**: The Online Safety Act 2023 (Commencement No.
  7) Regulations 2026, S.I. 2026/262,
  https://www.legislation.gov.uk/uksi/2026/262/made, under "Provisions coming
  into force on 7th April 2026": "section 66(1) and (2) (requirement to report
  CSEA content: regulated user-to-user services);".
- **Being told counts.** Section 70(4),
  https://www.legislation.gov.uk/ukpga/2023/50/section/70 :
  > CSEA content is “detected” by a provider when the provider becomes aware of the content, whether by means of the provider’s systems or processes or as a result of another person alerting the provider.
- **Reports to NCMEC can cover it.** Content is "unreported" under section
  70(5) only "if the reporting of that content is not covered by arrangements
  (mandatory or voluntary)", including those "by which the provider reports
  content relating to child sexual exploitation or abuse to a foreign agency".
  The explanatory note to the reporting regulations (below), which says of
  itself "(This note is not part of the Regulations)", names NCMEC:
  > If these providers already have arrangements in place for reporting CSEA content to a foreign agency which is exercising functions similar to the NCA (such as the National Center for Missing & Exploited Children in the United States of America), then this content is not required by section 66 to be reported to the NCA.
- **An individual can be the provider.** Section 226(3),
  https://www.legislation.gov.uk/ukpga/2023/50/section/226 :
  > If no entity has control over who can use the user-to-user part of a user-to-user service, but an individual or individuals have control over who can use that part, the provider of the service is to be treated as being that individual or those individuals.
- **Which services are regulated.** Section 4(5),
  https://www.legislation.gov.uk/ukpga/2023/50/section/4 : a service "has links
  with the United Kingdom" if "(a) the service has a significant number of
  United Kingdom users, or" "(b) United Kingdom users form one of the target
  markets for the service (or the only target market)." Section 4(6) adds
  services usable in the UK that present a material risk of significant harm to
  people there. The phrase "significant number" is not defined in the section
  read.
- **How, how fast, and what to keep.** The Online Safety (CSEA Content
  Reporting by Regulated User-to-User Service Providers) Regulations 2026, S.I.
  2026/268, https://www.legislation.gov.uk/uksi/2026/268/made ("Laid before
  Parliament 12th March 2026", "Coming into force 7th April 2026"):
  regulation 4(1), "A provider and any third party provider must register with
  the NCA prior to submitting their first report pursuant to the reporting
  duty."; regulation 6(6), "Priority level 1: where there is information which
  suggests that there is an immediate threat to a child’s life, or immediate
  risk of serious harm to a child;", sent under regulation 6(7) "for priority
  level 1, immediately;"; regulation 8, after a report the provider must
  retain for one year "the detected CSEA content," the information submitted,
  and "any relevant data associated with the user who uploaded, created,
  shared or received the CSEA content,".
- **What not to do.** The Protection of Children Act 1978 section 1,
  https://www.legislation.gov.uk/ukpga/1978/37/section/1 (England and Wales),
  makes it an offence to take or make, distribute or show, or possess with a
  view to distributing, an indecent photograph or pseudo-photograph of a child.
  Section 1B, https://www.legislation.gov.uk/ukpga/1978/37/section/1B , is a
  defence to the making offence for a defendant who proves it was necessary to
  make it "for the purposes of the prevention, detection or investigation of
  crime, or for the purposes of criminal proceedings, in any part of the
  world". Scotland and Northern Ireland have their own laws, not read.
- **Official guidance to people who come across material.** CEOP Education
  (CEOP's education site; footer "© Crown copyright"; CEOP's own report page
  calls CEOP "a law enforcement agency", see the 2026-10-09 finding),
  *What to do if you come across child sexual abuse material*,
  https://www.ceopeducation.co.uk/professionals/blogs/what-to-do-if-you-come-across-child-sexual-abuse-material/
  (the page shows "Published Tuesday, February 23, 2021"; its text mentions
  AI-generated material, so it has probably been revised since). Read through a
  rendering tool, not script-checked:
  > Sharing CSAM is illegal and you will not need to keep it as evidence for a report.

  > Adults should not view CSAM.

  and, for material found on a work device, "do not make any changes to the
  device or its contents" and contact the police.

## European Union (brief)

The Digital Services Act, Regulation (EU) 2022/2065,
https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32022R2065
(Official Journal L 277, 27 October 2022; "This Regulation shall apply from 17
February 2024." EUR-Lex lists it as in force with no amending acts, only
corrections in other languages).

- **Liability shield, Article 6(1):** a hosting provider is not liable for what
  users store on condition that it
  > (b) upon obtaining such knowledge or awareness, acts expeditiously to remove or to disable access to the illegal content.
- **A notice can give knowledge, Article 16(3):**
  > Notices referred to in this Article shall be considered to give rise to actual knowledge or awareness for the purposes of Article 6 in respect of the specific item of information concerned where they allow a diligent provider of hosting services to identify the illegality of the relevant activity or information without a detailed legal examination.

  Article 16(1) requires hosting providers to let "any individual or entity"
  notify them of illegal content, and 16(2)(c) excuses the notifier's name and
  email "in the case of information considered to involve one of the offences
  referred to in Articles 3 to 7 of Directive 2011/93/EU" (the child sexual
  abuse directive).
- **Telling the police, Article 18(1):**
  > Where a provider of hosting services becomes aware of any information giving rise to a suspicion that a criminal offence involving a threat to the life or safety of a person or persons has taken place, is taking place or is likely to take place, it shall promptly inform the law enforcement or judicial authorities of the Member State or Member States concerned of its suspicion and provide all relevant information available.

  Recital 56 gives as examples offences under "Directive 2011/93/EU or Directive
  (EU) 2017/541", and says: "This Regulation does not provide the legal basis
  for profiling of recipients of the services with a view to the possible
  identification of criminal offences by providers of hosting services." Where
  the country cannot be identified, Article 18(2) says to inform the authorities
  where the provider is established "or inform Europol, or both."
- **No general monitoring, Article 8:** "No general obligation to monitor the
  information which providers of intermediary services transmit or store, nor
  actively to seek facts or circumstances indicating illegal activity shall be
  imposed on those providers."
- **Size does not exempt these.** Article 19's exclusion for micro and small
  enterprises covers only Section 3 (online platforms): "This Section, with the
  exception of Article 24(3) thereof, shall not apply to providers of online
  platforms that qualify as micro or small enterprises". Articles 16 and 18 are
  in Section 2 and apply to all hosting providers.
- **But the Act covers services "normally provided for remuneration".**
  Article 3(a) defines an information society service by reference to Directive
  (EU) 2015/1535, Article 1(1)(b),
  https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32015L1535 :
  > ‘service’ means any Information Society service, that is to say, any service normally provided for remuneration, at a distance, by electronic means and at the individual request of a recipient of services.

  Whether a free volunteer server meets that is unknown (see below).
- **What not to do.** Directive 2011/93/EU, Article 5,
  https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32011L0093
  (OJ L 335, 17 December 2011; in force; EUR-Lex lists a 2024 proposal to repeal
  and replace it, 52024PC0060, not researched): "Knowingly obtaining access, by
  means of information and communication technology, to child pornography shall
  be punishable by a maximum term of imprisonment of at least 1 year." and
  "Distribution, dissemination or transmission of child pornography shall be
  punishable by a maximum term of imprisonment of at least 2 years." Each member
  state writes these into its own law; national laws were not read.

---

## Where the sources pull in different directions

**Delete it, or keep it?** CEOP Education tells members of the public to note
where the material is, report it and delete it ("you will not need to keep it
as evidence for a report"). The US statute tells a provider that has reported
to preserve the reported contents and their context for a year (2258A(h)), and
the UK regulations tell a provider that has reported to retain "the detected
CSEA content" for a year (regulation 8). The US defence for an individual
(2252A(d)) accepts either destroying the images or reporting to law enforcement
and giving it access, provided nobody else sees them.

These are not really in conflict: they are addressed to different people. The
public guidance is for someone who has come across material and has no duty to
report; the preservation rules are for a provider that has made a report. The
researcher's reading, offered as a judgement: an operator who may be a provider
should follow the provider rules (report, then preserve, access-limited), and
should not delete material before reporting it. A person who is not running a
service (for example a member who saw something) should follow the public
guidance: report, do not share, do not keep a copy.

**Must a user's report alone count as knowledge?** The UK says yes in terms
(section 70(4), "as a result of another person alerting the provider"). The EU
says yes where a diligent provider can see the illegality "without a detailed
legal examination" (Article 16(3)). The US statute does not say; nothing found
answers it.

## What is still unknown, and what would settle it

- **Whether a volunteer running a small server for friends is a "provider" in
  the US.** No official source found answers it. The definition of electronic
  communication service has no size, money, business or "to the public"
  condition; the DOJ manual speaks of companies and government entities; NCMEC
  speaks of "U.S.-based ESPs" and registers "companies". What would settle it:
  a short written question to NCMEC's ESP team (ESPteam@ncmec.org) asking
  whether they register individuals or volunteer groups that run a chat server,
  which costs an email and may not be a legal answer; or a lawyer's opinion,
  which would be one. Court decisions on who is an electronic communication
  service were not researched.
- **Whether a report, with no file and no admin having seen anything, is
  "actual knowledge" in the US.** Not defined in the statute. Settled only by a
  lawyer reading the case law, which was not researched.
- **Whether a server operator outside the US may register with and report to
  NCMEC, and whether doing so satisfies their own country's law.** The UK
  accepts reports to a foreign agency such as NCMEC (explanatory note above).
  NCMEC's FAQ says anyone may make a public report. Whether NCMEC registers
  non-US providers, and what other countries accept, was not researched.
- **Whether a free, volunteer-run server is an "information society service"
  under EU law** ("normally provided for remuneration"). Court of Justice case
  law on remuneration was not read. A lawyer in an EU member state would know.
- **What "a significant number of United Kingdom users" means** (Online Safety
  Act section 4(5)). Ofcom's guidance was not read; it would answer this.
- **State law in the US.** States have their own reporting and mandatory
  reporter laws; none were read. The project's server's state law is the one to
  read first.
- **Scotland and Northern Ireland offences**, and **every EU member state's
  national law**: not read.
- **The EU's temporary rules on voluntary detection** (Regulation (EU)
  2021/1232 and its extensions) and the **proposed EU child sexual abuse
  regulation**: not researched. They concern scanning, which HumanityOS does not
  do, so they are unlikely to change the answer here, but this is not checked.
- **Whether a HumanityOS server is a "covered platform" under the TAKE IT DOWN
  Act** (US-12): needs its own finding.
- **Whether 2258A was amended after December 2025.** The House's official US
  Code site was down on the research date; the 2025 amendment was seen only on
  Cornell's copy. Re-read https://uscode.house.gov/ when it is back.

## Sources that could not be reached

- https://uscode.house.gov/ (the Office of the Law Revision Counsel's official
  current US Code): every request between about 00:20 and 01:00 Pacific time
  returned an "Under Maintenance" page or a server error. Wanted: the current
  text with every amendment through 2026. Used instead: the GPO 2024 Edition
  on govinfo (official, through 2024) and Cornell LII (unofficial) for the
  2025 amendment.
- https://www.congress.gov/bill/119th-congress/senate-bill/1829 : HTTP 403 to
  both direct and tool requests. Used instead: govinfo's bill status XML.
- https://www.ceopeducation.co.uk/ : a bot check on direct requests (no attempt
  was made to get past it); read through a rendering fetch tool, so its
  quotations are not script-verified.
- https://www.westyorkshire.police.uk/ask-the-police/question/Q135 (a police
  answer on receiving indecent images by email): HTTP 403. Wanted a second UK
  police source on what not to do.
- https://report.cybertip.org/faq : HTTP 404 (the FAQ is at `/faqs`).

## What it means for this project

### Certain (stated by the sources above)

1. In the US, a provider with actual knowledge of an apparent child sexual
   abuse material offence, or of enticing a child (2422(b)) or child sex
   trafficking (1591), must give the CyberTipline its contact details and
   report "as soon as reasonably possible" (US-2, US-3). Enticement can be shown
   by text alone; NCMEC lists "Someone chatting online with a child about sex"
   as reportable.
2. Nothing requires a provider to monitor or scan (US-5; EU Article 8). The
   duty starts when knowledge arrives, for example through a report.
3. After a CyberTipline report, the reported contents and reasonably accessible
   context must be kept for 1 year, securely, with access limited (US-4); the
   UK regulations say the same for reports to the NCA (one year).
4. A provider may pass on what it reported only to NCMEC, law enforcement or in
   answer to legal process (US-6), and must keep staff access to the images to
   a minimum and destroy them when law enforcement asks (US-7).
5. Anyone who knowingly receives, distributes, possesses or accesses such
   material with intent to view it commits a federal crime (US-9). The EU
   directive makes "Knowingly obtaining access" an offence in every member
   state's law.
6. In the UK, an individual can be a provider, a user's report counts as
   detection, and reports already made to NCMEC can cover the UK duty. In the
   EU, a notice can give a hosting provider actual knowledge, and it must
   remove or disable access expeditiously and tell the police of a suspected
   offence against a person's life or safety.
7. The official places to send people: the CyberTipline (US, accepts reports
   from anyone, anywhere), and 911 or local police when someone is in immediate
   danger (NCMEC, DOJ). Other countries' official lines are in
   `docs/reference/findings/2026-10-09-outside-help-lines.md`.

### Judgement calls (the project's own reading; a lawyer could disagree)

Each point names the source it leans on.

1. **Admins are never shown files, and that should cover public posts too.**
   Viewing is itself the act the law punishes (2252A(a)(5)(B), "knowingly
   accesses with intent to view"; EU directive, "Knowingly obtaining
   access"), the reporting shield does not cover looking for any other
   purpose (2258B(b)), access must be minimised (2258B(c), 2258A(h)(3)), and
   the UK's official
   advice is "Adults should not view CSAM." The relay already refuses DM and
   group evidence carrying a file. See the code note below for the public-post
   case.
2. **A report about a child should point the reporter to the official hotline
   first, and say plainly that admins are volunteers.** The CyberTipline takes
   reports from anyone, needs no files, and passes every report to law
   enforcement (US-10, US-11); a server admin cannot do any of that. Section 10e
   already says the admins are "volunteers, not police". The dialog should also
   tell the reporter not to screenshot, save or forward images, and if they
   already have them, to say so in the hotline report rather than attach them
   (NCMEC FAQ: "it is important to make a note in the report that you do have
   them"; CEOP: "Sharing CSAM is illegal").
3. **An operator who learns of material should, in this order:** (a) if a
   child is in immediate danger, call 911 or the local emergency number (NCMEC,
   DOJ); (b) not open, download, copy, screenshot or forward the file, and not
   show it to other admins (2252A; 2258B(b) and (c)); (c) take it out of view
   for everyone without destroying it (EU Article 6(1)(b), "remove or to disable
   access"; US 2258A(h), which assumes the material still exists to be
   preserved); (d) report to the CyberTipline, through the provider route if
   registered (ESPteam@ncmec.org) or the public form otherwise (US-3), or to
   the country's own line; (e) keep what was reported, and the report record,
   for one year in a place only the operator can reach, then delete it, or
   sooner if law enforcement asks (2258A(h); 2258B(c)(2); UK regulation 8);
   (f) tell nobody else what it contains (2258A(g)(4)). This is the safe default
   for every operator, because whether a volunteer is a "provider" is unknown,
   and because both the provider route and the individual's defence (2252A(d))
   start with prompt reporting and nobody else seeing the material.
4. **Text-only evidence can still carry the duty.** Because enticement and
   "chatting online with a child about sex" are reportable, refusing files does
   not take the operator out of the reporting duty. A `child_danger` report with
   checked DM text in which an adult sexualises a child may itself be the
   "facts or circumstances" of 2258A(a)(2)(A) (US-2). The Reports page should
   therefore show the operator what to do with such a report (point 3), not
   only Dismiss, Warn, Mute, Kick and Ban. This keeps the GUI-first rule: the
   steps live in the app, though the report to NCMEC is made on NCMEC's own
   site, since automatic API reporting needs NCMEC-issued credentials.
5. **Nothing here asks the app to scan or to weaken end-to-end encryption.**
   The US, UK and EU sources read all start the duty from knowledge, and the US
   and EU expressly impose no duty to monitor (US-5; Article 8).
6. **Do not promise users more than volunteers can deliver.** The app should
   not describe server admins as a child-protection service; that matches the
   project rule on saying plainly what the app does not do.

### Observed in the code on 10 October 2026 (relevant to the judgement calls)

Read, not changed, while writing this; a reviewer should confirm.

- **A reported public post can carry an uploaded image's address.** In
  `src/relay/handlers/reports.rs`, `Item::carries_file` returns `false` for a
  `post` item; only DM and group text is checked for the `[[hum:file:` marker.
  Public posts carry uploads as plain `/uploads/...` addresses that the web
  chat turns into inline images (`web/chat/app.js`, the image pattern near line
  2321). The web Reports page shows evidence text escaped, as plain text
  (`web/chat/chat-reports.js`, `reportsViewHtml`), so the image is not shown,
  but its address is. The native Reports page was not checked.
- **`delete_post` hard-deletes the row and leaves the file.** The decision runs
  `delete_message` (`src/relay/storage/channels.rs`), a plain `DELETE FROM
  messages`; the uploaded file stays under `data/uploads/` at its public
  address. For a child-abuse report that is the wrong way round on both counts:
  the file stays reachable (EU Article 6), while the post that would need
  preserving after a CyberTipline report is gone (2258A(h)).
- **Retention is shorter than the preservation period.** Section 10e keeps a
  decided report for 90 days; 2258A(h) and UK regulation 8 ask for one year
  after a report to the hotline. Automatic deletion elsewhere (expiring old
  channel messages, and the per-user upload limit in
  `src/relay/storage/uploads.rs`, which deletes the oldest uploads) could also
  remove material that should be kept.
- What would fit the sources, as a suggestion only: a separate decision for
  `child_danger` reports (for example *Hide and keep for the hotline*) that
  hides the post and its file from everyone, moves them where only the
  operator can reach
  them, exempts them from every automatic deletion, keeps them one year, and
  shows the operator the steps in point 3.

---

This document is a reading of public sources on 10 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. Laws, guidance and report
addresses change; check the dates above, and ask a lawyer before relying on any
of it for a decision with legal consequences.
