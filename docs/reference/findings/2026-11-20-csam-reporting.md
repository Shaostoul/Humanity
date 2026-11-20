# What US federal law asks of a server operator who learns of child sexual abuse material (re-checked for the content policy)

**Research date: 20 November 2026** (UTC; the sources were read between about 03:25 and 03:40
UTC, which is the evening of 19 November in Pacific time). Everything quoted below was read on that
day. If you are reading this later, compare each source's own date with anything that has changed
since.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** Where it says what the project should do, that is the project's
own reading, marked as such, and a lawyer could disagree with any of it.

**What this finding is to the earlier one.** It re-checks the United States part of
[`2026-10-10-report-duties-child-abuse-material.md`](2026-10-10-report-duties-child-abuse-material.md)
against the official, current text of the US Code, answers that finding's open question about
amendments after December 2025, and gathers what a written content policy may and may not promise
(the Digital Public Goods Standard's indicator 9b,
[`2026-11-18-digital-public-goods.md`](2026-11-18-digital-public-goods.md)). The earlier finding's
United Kingdom and European Union parts were **not** re-checked and stand as written on their date.
Its "Observed in the code" notes were re-checked and still hold (below).

## The question

When the operator of a HumanityOS server obtains actual knowledge of apparent child sexual abuse
material, what does US federal law ask of them (18 U.S.C. 2258A: the report to NCMEC's
CyberTipline, what a report holds, the preservation period and its amendments, including the
REPORT Act of 2024), what protection does it give (2258B), and does it require anyone to monitor or
scan (2258A(f))?

## Short answer

**Nothing moved since 10 October 2026, and one open question is closed.** The House's official
copy of the US Code, current through Public Law 119-142 (10 November 2026), shows that section
2258A was last amended by Public Law 119-60 of 18 December 2025, which changed only what NCMEC
passes on to law enforcement. Sections 2258B and 2258E were last amended in 2024 and 2018.

- **The duty.** A "provider" that obtains **actual knowledge** of facts showing an apparent child
  sexual abuse material offence (or the sex trafficking or enticement of a child) must, "as soon as
  reasonably possible", give the CyberTipline its contact details and **report** to it.
- **What a report holds** is largely the provider's choice ("at the sole discretion of the
  provider").
- **Preserve.** A completed report counts as a request to **preserve** what it contained "for 1
  year", with reasonably accessible context, in a secure place with access limited, and (since 7 May
  2025) in line with NIST's Cybersecurity Framework.
- **Who may be told.** What a provider reported may go only to NCMEC, to law enforcement or "as
  necessary to respond to legal process".
- **No duty to look.** Nothing in 2258A requires a provider to monitor anyone or to "search,
  screen, or scan".
- **The shield** (2258B) covers the reporting and preserving only, not misconduct and not acting
  "for a purpose unrelated" to those duties, and it comes with a duty to keep access to the images
  to a minimum.
- **Fines** for a knowing and willful failure to report: up to $600,000 for a first failure by a
  provider with fewer than 100,000,000 monthly active users.

**Still unsettled, and a lawyer's question:** whether a volunteer running a small HumanityOS server
is a "provider", and whether a user's report on its own is "actual knowledge". The definition has
no size, money or "to the public" condition; no official source says how small a service can be
and still not be one. The project's reading, as before: act as if the duty applies.

## How each finding is recorded

For each finding: the source, its own date, and a quotation of the words that carry the meaning.
Statute text is from the Office of the Law Revision Counsel's US Code site (uscode.house.gov), the
official current Code, whose pages each say "Text contains those laws in effect through Public Law
119-142 (November 10, 2026)." That site now shows a notice that it "is currently operating in a
temporary format", and its section pages moved to addresses of the form
`https://uscode.house.gov/us/usc/t18/s2258A/` (the older `view.xhtml?req=granuleid:...` addresses
answered 404 on the research date). The Code's text puts an invisible word-joiner character after
each dash; it is left out of the quotations. Every quotation of an outside source was checked by
script against the downloaded page, with tags removed and spaces collapsed.

---

## Findings

### 1. The current text, and the amendments since 2024

- 18 U.S.C. 2258A, https://uscode.house.gov/us/usc/t18/s2258A/ (current through Pub. L. 119-142,
  10 November 2026). Its source credit, the list of every law that changed it:
  > (Added Pub. L. 110-401, title V, § 501(a) , Oct. 13, 2008 , 122 Stat. 4243 ; amended Pub. L. 115-395, § 2 , Dec. 21, 2018 , 132 Stat. 5287 ; Pub. L. 118-59 , §§ 3, 4(a), May 7, 2024 , 138 Stat. 1016 ; Pub. L. 119-60, div. H, title LXXXII, § 8202(i) , Dec. 18, 2025 , 139 Stat. 1844 .)

  (The spaces before the commas are the page's own, between its links.) Pub. L. 118-59 is the
  REPORT Act. The Code's note on the 2025 change:
  > 2025—Subsec. (c). Pub. L. 119-60 inserted “and all supplemental data included in the report” after “each report made under subsection (a)(1)” in introductory provisions.

  Subsection (c) is what NCMEC must make available to law enforcement; it adds nothing to what a
  provider must do. **This closes the earlier finding's open item** ("Whether 2258A was amended
  after December 2025"), which it could check only on Cornell's unofficial copy because the official
  site was down.
- 18 U.S.C. 2258B, https://uscode.house.gov/us/usc/t18/s2258B/ (same currency):
  > amended Pub. L. 115-395, § 3 , Dec. 21, 2018 , 132 Stat. 5292 ; Pub. L. 118-59, § 2(a) , May 7, 2024 , 138 Stat. 1014 .)
- 18 U.S.C. 2258E, https://uscode.house.gov/us/usc/t18/s2258E/ (same currency):
  > amended Pub. L. 115-395, § 6 , Dec. 21, 2018 , 132 Stat. 5294 .)

### 2. Who counts as a "provider" (unchanged)

- 18 U.S.C. 2258E(6), same source:
  > the term “provider” means an electronic communication service provider or remote computing service;
- 18 U.S.C. 2510(15), https://uscode.house.gov/us/usc/t18/s2510/ (same currency):
  > (15) “electronic communication service” means any service which provides to users thereof the ability to send or receive wire or electronic communications;
- 18 U.S.C. 2711(2), https://uscode.house.gov/us/usc/t18/s2711/ (same currency):
  > (2) the term “remote computing service” means the provision to the public of computer storage or processing services by means of an electronic communications system;

  As the earlier finding noted: a remote computing service is provided "to the public"; an
  electronic communication service, as defined, need not be. Neither definition has a size, money
  or business condition.
- NCMEC's page on child sexual abuse material, https://www.missingkids.org/theissues/csam (no date on
  the page; read 2026-11-20):
  > U.S. federal law requires that U.S.-based ESPs report instances of apparent child pornography that they become aware of on their systems to NCMEC’s CyberTipline.

  and
  > To date, over 1,400 companies are registered to make reports to NCMEC’s CyberTipline

  ("ESP" is NCMEC's word, electronic service provider. "U.S.-based" and "companies" are NCMEC's
  description, not the statute's words.) The page's question "Are you an ESP who would like to register with
  NCMEC?" is followed by a "Click here" link to the address espteam@ncmec.org.
- **The fines are set by size, so small providers are inside the law.** 18 U.S.C. 2258A(e)(1):
  > in the case of an initial knowing and willful failure to make a report, not more than $850,000 in the case of a provider with not less than 100,000,000 monthly active users or $600,000 in the case of a provider with less than 100,000,000 monthly active users; and

  Subsection (d)(1): "The Attorney General shall enforce this section."

### 3. What starts the duty, and what the duty is

- 18 U.S.C. 2258A(a)(1)(A), same source. A provider:
  > (i) shall, as soon as reasonably possible after obtaining actual knowledge of any facts or circumstances described in paragraph (2)(A), take the actions described in subparagraph (B); and

  > (ii) may, after obtaining actual knowledge of any facts or circumstances described in paragraph (2)(B), take the actions described in subparagraph (B).
- The facts, (a)(2)(A):
  > any facts or circumstances from which there is an apparent violation of section 2251, 2251A, 2252, 2252A, 2252B, or 2260 that involves child pornography, of section 1591 (if the violation involves a minor), or of

  followed by "2422(b)." (the Code marks the missing word "So in original. Probably should be
  followed by “section”."). Section 1591 is sex trafficking and 2422(b) is enticing a minor; the
  REPORT Act added both. (a)(2)(B), which a provider **may** report, covers facts that "indicate a
  violation of any of the sections described in subparagraph (A) involving child pornography may be
  planned or imminent."
- The two actions, (a)(1)(B):
  > (i) providing to the CyberTipline of NCMEC, or any successor to the CyberTipline operated by NCMEC, the mailing address, telephone number, facsimile number, electronic mailing address of, and individual point of contact for, such provider; and

  > (ii) making a report of such facts or circumstances to the CyberTipline, or any successor to the CyberTipline operated by NCMEC.
- "Actual knowledge" is still not defined anywhere in sections 2258A to 2258E (read in full on the
  research date). No official source found says whether a user's report on its own gives a provider
  actual knowledge.

### 4. What a report contains

18 U.S.C. 2258A(b), same source: the facts and circumstances in each report
> may, at the sole discretion of the provider, include the following information:

and then five kinds: information about the person involved (which "may, to the extent reasonably
practicable, include the electronic mail address, Internet Protocol address, uniform resource
locator, payment information (excluding personally identifiable information), or any other
identifying information, including self-reported identifying information"), when and how the
content was uploaded or discovered "including a date and time stamp and time zone", geographic
location, "Any visual depiction of apparent child pornography or other content relating to the
incident such report is regarding.", and the complete communication.

How a provider sends one: NCMEC's *CyberTipline Reporting API Technical Documentation*,
https://report.cybertip.org/ispws/documentation (server `Last-Modified: Wed, 07 Oct 2026 18:05:24
GMT`):
> There are two ways for companies to submit reports of apparent child sexual exploitation to NCMEC: they can submit a report using a web form or using this web service API, which allows a more automated reporting experience.

> While ESPs have a statutory duty to report apparent child pornography to NCMEC’s CyberTipline (see 18 U.S.C. § 2258A), the reporting of data via the web service, other than the incident type and date/time of incident, is voluntary and undertaken at the ESPs' initiative.

and "A username and password must be requested from and supplied by NCMEC."

### 5. Preservation: one year, locked away

18 U.S.C. 2258A(h), same source:

- (h)(1):
  > For the purposes of this section, a completed submission by a provider of a report to the CyberTipline under subsection (a)(1) shall be treated as a request to preserve the contents provided in the report for 1 year after the submission to the CyberTipline.

  The Code's note: "Subsec. (h)(1). Pub. L. 118-59, § 3(1) , substituted “1 year” for “90 days”."
- (h)(2):
  > Pursuant to paragraph (1), a provider shall preserve any visual depictions, data, or other digital files that are reasonably accessible and may provide context or additional information about the reported material or person.
- (h)(3):
  > A provider preserving materials under this section shall maintain the materials in a secure location and take appropriate steps to limit access by agents or employees of the service to the materials to that access necessary to comply with the requirements of this subsection.
- (h)(5) allows keeping the reported contents "for longer than 1 year after the submission to the
  CyberTipline for the purpose of reducing the proliferation of online child sexual exploitation or
  preventing the online sexual exploitation of children."
- (h)(6):
  > Not later than 1 year after the date of enactment of this paragraph, a provider of a report to the CyberTipline under subsection (a)(1) shall preserve materials under this subsection in a manner that is consistent with the most recent version of the Cybersecurity Framework developed by the National Institute of Standards and Technology, or any successor thereto.

  The Code's note: "The date of enactment of this paragraph, referred to in subsec. (h)(6), is the
  date of enactment of Pub. L. 118-59 , which was approved May 7, 2024." So the method applies from
  7 May 2025.

**Negative finding:** the statute starts the preservation clock at the completed report. It says
nothing about what to keep between learning of material and reporting it, and nothing in 2258A
tells a provider to delete anything.

**Longer, or sooner: who can change the year** (added the same day, about 08:20 UTC, after a
review question on the content policy's wording). Three rules move the end of the year, and they
point in both directions:

- **Longer, by the provider's choice:** (h)(5), above.
- **Longer, when the government asks:** (h)(4) says "Nothing in this section shall be construed as
  replacing, amending, or otherwise interfering with the authorities and duties under section
  2703." Section 2703(f), https://uscode.house.gov/us/usc/t18/s2703/ (same currency):
  > (1) In general .— A provider of wire or electronic communication services or a remote computing service, upon the request of a governmental entity, shall take all necessary steps to preserve records and other evidence in its possession pending the issuance of a court order or other process.

  > (2) Period of retention .— Records referred to in paragraph (1) shall be retained for a period of 90 days, which shall be extended for an additional 90-day period upon a renewed request by the governmental entity.
- **Sooner, when law enforcement asks:** 18 U.S.C. 2258B(c)(2), quoted in finding 8 below: a
  provider shall "ensure that any such visual depiction is permanently destroyed, upon a request
  from a law enforcement agency to destroy the visual depiction."

So keeping it a year and then deleting it has two exceptions: keep it while a government preservation
request runs, and destroy it early if law enforcement asks for that.

### 6. No duty to monitor or scan

18 U.S.C. 2258A(f), same source, headed "Protection of Privacy":
> Nothing in this section shall be construed to require a provider to—

> (1) monitor any user, subscriber, or customer of that provider;

> (2) monitor the content of any communication of any person described in paragraph (1); or

> (3) affirmatively search, screen, or scan for facts or circumstances described in sections (a) and (b).

**Negative finding:** section 2258A asks for no scanning, no filtering and no weakening of
end-to-end encryption; its duty starts from knowledge. (Other laws were not checked for this; the
earlier finding found the same in the EU's Digital Services Act, Article 8.)

### 7. Who a provider may tell

18 U.S.C. 2258A(g)(4), same source:
> A provider that submits a report under subsection (a)(1) may disclose by mail, electronic transmission, or other reasonable means, information, including visual depictions contained in the report, in a manner consistent with permitted disclosures under paragraphs (3) through (8) of section 2702(b) only to a law enforcement agency described in subparagraph (A), (B), or (C) of paragraph (3), to NCMEC, or as necessary to respond to legal process.

The general rule on stored messages, 18 U.S.C. 2702, https://uscode.house.gov/us/usc/t18/s2702/
(same currency), lets a provider divulge a communication, among other cases,
> (3) with the lawful consent of the originator or an addressee or intended recipient of such communication, or the subscriber in the case of remote computing service;

and
> (6) to the National Center for Missing and Exploited Children, in connection with a report submitted thereto under section 2258A;

(The person who received a message choosing to hand it over, which is how a HumanityOS report's
evidence works, is the first of these.)

### 8. The limited liability, and its edges

18 U.S.C. 2258B, same source.

- (a):
  > a civil claim or criminal charge against a provider or domain name registrar, including any director, officer, employee, or agent of such provider or domain name registrar arising from the performance of the reporting or preservation responsibilities of such provider or domain name registrar under this section, section 2258A, or section 2258C may not be brought in any Federal or State court.
- (b): that does not apply if the provider or its people "engaged in intentional misconduct", or
  acted or failed to act "with actual malice", "with reckless disregard to a substantial risk of
  causing physical injury without legal justification", or
  > for a purpose unrelated to the performance of any responsibility or function under sections

  followed by "2258A, 2258C, 2702, or 2703." (the Code again marks "So in original").
- (c), "Minimizing Access":
  > (1) minimize the number of employees that are provided access to any visual depiction provided under section 2258A or 2258C; and

  > (2) ensure that any such visual depiction is permanently destroyed, upon a request from a law enforcement agency to destroy the visual depiction.

**Negative finding:** the shield does not cover looking at, copying or sharing material for any
reason other than reporting and preserving it.

### 9. What every person must not do, provider or not (unchanged; re-read)

- 18 U.S.C. 2252A(a)(5)(B), https://uscode.house.gov/us/usc/t18/s2252A/ (same currency) punishes
  whoever
  > knowingly possesses, or knowingly accesses with intent to view, any book, magazine, periodical, film, videotape, computer disk, or any other material that contains an image of child pornography

- The defence for a person who ends up holding a few images, 2252A(d):
  > (1) possessed less than three images of child pornography; and

  > (2) promptly and in good faith, and without retaining or allowing any person, other than a law enforcement agency, to access any image or copy thereof—

  > (A) took reasonable steps to destroy each such image; or

  > (B) reported the matter to a law enforcement agency and afforded that agency access to each such image.
- What counts, 18 U.S.C. 2256, https://uscode.house.gov/us/usc/t18/s2256/ (same currency):
  > (1) “minor” means any person under the age of eighteen years;

  and child pornography includes a depiction where
  > (B) such visual depiction is a digital image, computer image, or computer-generated image that is, or is indistinguishable from, that of a minor engaging in sexually explicit conduct; or

- Drawings and made-up children, 18 U.S.C. 1466A, https://uscode.house.gov/us/usc/t18/s1466A/
  (same currency, read for the first time here): it covers "a visual depiction of any kind,
  including a drawing, cartoon, sculpture, or painting" that depicts a minor engaging in sexually
  explicit conduct and is obscene (among other cases), and
  > It is not a required element of any offense under this section that the minor depicted actually exist.

  Whether a given drawing is "obscene" is a legal test this finding does not try to apply; the
  content policy does not need it, because it forbids all sexual depictions of children, drawn or
  computer-made included.

### 10. Where people are sent to report

- NCMEC's CyberTipline, https://report.cybertip.org/ (no date; "Copyright © 2026"):
  > If you or someone you know is in immediate danger, please call 911 or your local police immediately.

  and the call centre, "1-800-THE-LOST (1-800-843-5678)", which the page says is available "24
  hours a day, 7 days a week".
- Its FAQ, https://report.cybertip.org/faqs (no date; "Copyright © 2026"):
  > No, anyone can make a report of child sexual exploitation or abuse.

  (the answer to "Do I have to live in the United States to make a report?");
  > No, you don’t have to share your name or contact information to make a report.

  among what should be reported:
  > Someone chatting online with a child about sex;

  on screenshots:
  > No, you cannot attach them to the CyberTipline report you are making but it is important to make a note in the report that you do have them.

  and on what happens:
  > Every CyberTipline report is made available to law enforcement for their independent review and assessment.
- The Justice Department's Child Exploitation and Obscenity Section,
  https://www.justice.gov/criminal/criminal-ceos/report-violations ("Updated August 11, 2023"):
  to report child pornography, file a report on NCMEC's website or call 1-800-843-5678;
  > Your report will be forwarded to a law enforcement agency for investigation and action.

  > If you have an emergency that requires an immediate law enforcement response, please call 911 or contact your local Police Department or Sheriff’s Department.

  Outside the United States, the app's report dialog already shows each listed country's emergency
  number and official hotline, from
  [`2026-10-09-outside-help-lines.md`](2026-10-09-outside-help-lines.md)
  (`data/safety/outside_help.json`).

### 11. Bills that could change this

The STOP CSAM Act of 2025 would change provider duties. Its official bill status records on
govinfo:

- S. 1829, https://www.govinfo.gov/bulkdata/BILLSTATUS/119/s/BILLSTATUS-119s1829.xml (record
  updated 2026-11-10), latest action 2025-06-26:
  > Placed on Senate Legislative Calendar under General Orders. Calendar No. 106.
- H.R. 3921, the identical House bill,
  https://www.govinfo.gov/bulkdata/BILLSTATUS/119/hr/BILLSTATUS-119hr3921.xml (record updated
  2026-10-02), latest action 2025-06-11:
  > Referred to the House Committee on the Judiciary.

Neither is law. Their contents were not read. Other bills were not searched for.

### 12. Telling the person whose post was hidden (added the same day, about 08:40 UTC)

The question, from the operator's requirements for a "hide and keep" action: should the person
whose post is taken down and reported be told, and how, given that tipping them off could matter
to an investigation?

- **Nothing in 2258A to 2258E forbids or requires telling them** (read in full on the research
  date). The disclosure limits in 2258A(g)(4) concern the report's contents, not the fact that a
  post was removed.
- **A court can order silence, case by case.** 18 U.S.C. 2705(b),
  https://uscode.house.gov/us/usc/t18/s2705/ (same currency): a governmental entity
  > may apply to a court for an order commanding a provider of electronic communications service or remote computing service to whom a warrant, subpoena, or court order is directed, for such period as the court deems appropriate, not to notify any other person of the existence of the warrant, subpoena, or court order.

  Such an order binds only when one has been served; there is no standing rule of that kind for a
  CyberTipline report.
- **NCMEC asks whether the user was told.** Its reporting API documentation (finding 4's source,
  `Last-Modified` 7 October 2026), for an account the provider disabled, has the fields
  "userNotified" ("Has user been notified that account was temporarily disabled.", and the same for
  "permanently disabled") and "userNotifiedDate". So whether the user was told is something NCMEC
  wants recorded in a report; the documentation gives no advice on whether to tell them.
- **Not found:** any official guidance from NCMEC or the Justice Department on whether a provider
  should tell the user. NCMEC's PDF guidance is barred to automated readers by its robots.txt, so it
  could not be checked.

**Negative finding:** no official source read settles it. It is a question for the operator (and,
ideally, for NCMEC's provider team or a lawyer).

---

## Where the sources pull in different directions

Unchanged from the earlier finding: the public is told to report and not keep material (NCMEC: do
not attach screenshots, say you have them), the individual's defence accepts destroying the images
or reporting to law enforcement with nobody else seeing them (2252A(d)), and a provider that has
reported is told to preserve for a year (2258A(h)). These are addressed to different people. The
project's reading, as a judgement: a server operator follows the provider rules (report, then keep
it locked away), and a member who came across something follows the public guidance (report, do
not share, do not keep a copy).

## What is still unknown, and what would settle it

- **Whether a volunteer running a small HumanityOS server is a "provider".** Not answered by any
  official source read. What would settle it: a lawyer's opinion (a real answer, at a lawyer's
  cost), or a short written question to NCMEC's provider team at espteam@ncmec.org asking whether
  they register individuals or volunteer groups running a chat server (an email, free, possibly not
  a legal answer). Court decisions on who provides an electronic communication service were not
  researched. Sending that email is the operator's to decide (an outside message).
- **Whether the project itself, as the software's authors, is a provider for servers other people
  run.** The duty attaches to a provider of a service; the statute does not speak of software
  publishers. The project's reading is that each server's operator is responsible for their own
  server and the project's maintainer for the servers he runs; a lawyer would confirm or correct it.
- **Whether a user's report, with no file and no admin having seen anything, is "actual
  knowledge".** Not defined in the statute. A lawyer reading the case law.
- **State law** (reporting and mandatory-reporter laws in the state where a server runs): not read.
- **The TAKE IT DOWN Act** (Pub. L. 119-12; its removal process was due by 19 May 2026): whether a
  HumanityOS server is a "covered platform" is still not researched and needs its own finding.
- **The United Kingdom and the European Union:** not re-checked since 10 October 2026.
- **Whether to tell the person whose post was hidden and reported** (finding 12): no official
  guidance found. What would settle it: the question in the NCMEC draft
  (`docs/legal/ncmec-esp-question-draft.md`), or a lawyer.

## Sources that could not be reached, or were not read

- `https://uscode.house.gov/view.xhtml?req=granuleid:USC-prelim-title18-section2258A&num=0&edition=prelim`
  (and the same for 2258B and 2258E): HTTP 404. The site's section pages moved to
  `/us/usc/t18/s<section>/`, which answered.
- NCMEC's REPORT Act guidelines,
  https://www.missingkids.org/content/dam/missingkids/pdfs/NCMEC-REPORT-Act-Guidelines.pdf, quoted
  by the earlier finding: **not read this time**, because missingkids.org's robots.txt now says
  "Disallow: /content/dam/missingkids/pdfs/" for every reader. Wanted: whether the guidelines
  changed. Nothing in this finding depends on them.
- Cornell's Legal Information Institute and congress.gov were not needed: the official Code site
  answered.

## What it means for this project

### Certain (stated by the sources above)

1. Section 2258A has not changed in any way that matters to a server operator since the REPORT Act
   of 2024; the earlier finding's US duties stand.
2. A provider with actual knowledge must give the CyberTipline its contact details and report "as
   soon as reasonably possible"; text alone can be reportable ("Someone chatting online with a child
   about sex").
3. After a CyberTipline report, the reported contents and reasonably accessible context are kept for
   1 year, securely, with access limited, in line with the NIST Cybersecurity Framework; longer
   while a government preservation request under 2703(f) runs (90 days, renewable once for 90
   more), and destroyed early if law enforcement asks for that (2258B(c)(2)).
4. What was reported goes only to NCMEC, law enforcement or in answer to legal process.
5. No duty to monitor or scan (2258A(f)).
6. Anyone who knowingly possesses or accesses such material with intent to view it commits a federal
   crime; computer-generated images that are indistinguishable from a real child count, and 1466A
   reaches obscene drawings of children that need not depict a real child.
7. Anyone, anywhere, may report to the CyberTipline, without giving a name; the public form takes no
   attachments; in immediate danger, call 911 or the local police.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **Every HumanityOS server operator should act as if they are a provider.** The definition has no
   size condition, small providers are named in the fines, and both the provider route and the
   individual's defence start with prompt reporting and nobody else seeing the material.
2. **The content policy may promise:** that such material is never allowed; that the app never scans
   messages and that sealed messages cannot be read by a server (true in the code, and lawful under
   2258A(f)); that action on a sealed conversation starts from a report by someone in it; that the
   project's own servers report what they learn of to the CyberTipline and keep what they reported
   locked away for a year; and where to report directly.
3. **The content policy must not promise:** that the app detects such material by itself; that
   server admins are a child-protection service or the police; that any other operator's server
   behaves like the project's (each operator answers for their own); or that the app keeps reported
   material for the year by itself (it does not yet; next section).
4. **The order of an operator's steps** stays as the earlier finding set it out: emergency first;
   never open, copy, screenshot or forward the file, nor show it to other admins; take it out of
   public view without destroying it; report to the CyberTipline (or the country's own line); keep
   what was reported for a year, locked away; tell nobody else what it contains.

### Observed in the code on 20 November 2026 (read, not changed, by this finding)

The three gaps the earlier finding found are all still there, and they are why the content policy
says some steps are manual for now:

- **"Delete the post" removes the post and leaves its file.** `src/relay/handlers/reports.rs`,
  `handle_decide`, runs `state.db.delete_message` for a `delete_post` decision;
  `src/relay/storage/channels.rs` `delete_message` is a plain `DELETE FROM messages`, so the text is
  gone (not preserved) and an uploaded file stays under `data/uploads/` at its address.
- **Reports are culled 90 days after a decision** (`docs/design/blocking-and-safe-mode.md` section
  10e; `src/relay/storage/expiry.rs`); open reports are not culled. 2258A(h) asks for a year after a
  CyberTipline report.
- **Admins are never shown files or links in a report** (`without_files_or_links` in
  `reports.rs`), and a direct or group message carrying a file cannot be evidence at all
  (`FILE_IN_EVIDENCE`): this already fits 2252A and 2258B(c).

The fix the earlier finding suggested (a separate decision for `child_danger` reports that hides
the post and its file from everyone, keeps them where only the operator can reach them, exempts them
from every automatic deletion and keeps them a year) is still the operator's open decision in
`docs/PRIORITIES.md` ("Report duties").

---

This document is a reading of public sources on 20 November 2026. It is not legal advice, and
nobody who wrote it is a lawyer. Laws, guidance and report addresses change; check the dates above,
and ask a lawyer before relying on any of it for a decision with legal consequences.
