# Children's privacy and online safety rules, and what HumanityOS may say about its protected setup

**Researched 2026-10-10** (sources read between about 01:55 and 04:00 Pacific
time on 10 October 2026). Everything below was read on that day. This area of
law moved fast in 2025 and 2026: the US children's privacy rule was amended in
2025 with a compliance date in April 2026, two federal bills passed one chamber
each in 2026, state laws are in and out of court, and UK and EU rules took
effect in 2025 and 2026. If you are reading this later, check each source's own
date against anything that has changed since.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such, and a
lawyer could disagree with any of it.

## The question

May HumanityOS (a free, open-source chat-and-game app with no sign-up, where
identity is a key made on the person's own device, no email, phone number,
birth date or name is collected, direct messages are end-to-end encrypted, and
anyone can run a server) offer a parent-applied, PIN-locked "Protected setup"
(design: `docs/design/blocking-and-safe-mode.md` sections 6.1 to 6.6), and what
may it truthfully call it, given US COPPA and the FTC Act, US state laws, the
UK Children's code and Online Safety Act, and the EU Digital Services Act and
GDPR?

## Short answer

- **Nothing read forbids the protected setup; nothing read supports calling it
  "kid safe", "child safe" or "compliant with" anything.** The FTC treats safety
  and parental-control claims as claims that must be true and backed up
  (Messenger Kids 2023, NGL 2024, Apitor 2025; all allegations).
- **US, COPPA:** no source says whether it reaches a free, donation-funded app
  run by an individual. If it does, HumanityOS does collect "personal
  information" as defined (a key others message you by, IP addresses, photos,
  voice, public posts). The setup tells no server anything, but presenting the
  app as being for children is evidence of "intended audience" (2025 rule).
- **UK:** the Children's code probably applies and asks that a child be told
  when parental controls are on; the Online Safety Act covers chat run by
  individuals, if a server has UK links (unknown). **EU:** DSA Article 28
  exempts micro and small enterprises and does not cover private messaging.
- **California AB 1043** (from 1 January 2027) may require the desktop app to
  ask the operating system for an age signal; it needs its own finding.

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Long passages are trimmed to the words
that matter; nothing inside a quotation is reworded. US statutes are quoted
from the Government Publishing Office's "United States Code, 2024 Edition" on
govinfo; the COPPA Rule from the eCFR (its version history lists 23 June 2025
as the latest amendment, with title 16 current to 17 August 2026); UK
legislation from legislation.gov.uk; EU law from EUR-Lex.

Quotations from pages downloaded directly (govinfo, eCFR, the Federal
Register, legislation.gov.uk, ico.org.uk, gov.uk, EUR-Lex, the Commission's
digital-strategy site, the Texas and Utah legislatures, the Ninth Circuit and
the Supreme Court) were checked by script against the downloaded text (tags
removed, spaces collapsed). ftc.gov and the California legislature's site
refused direct requests, so their pages were read in a browser and each
quotation was checked against the rendered page text in that browser. Two
sources are secondary and are marked where used.

---

## 1. United States: COPPA

COPPA is the Children's Online Privacy Protection Act, 15 U.S.C. 6501 to 6506
(1998), and the FTC's rule under it, 16 CFR Part 312. "Child" means under 13.

### US-1. Who the law reaches: an "operator", running a service "for commercial purposes"

- 15 U.S.C. 6501(2),
  https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap91-sec6501.htm
  (2024 Edition; enacted 1998). An operator is a person who runs a website or
  online service and collects or maintains personal information from its users,
  > where such website or online service is operated for commercial purposes

  and the term
  > does not include any nonprofit entity that would otherwise be exempt from coverage under section 45 of this title.

  "Person" includes "any individual" (6501(11)).
- The rule says the same: 16 CFR 312.2, definition of "Operator",
  https://www.ecfr.gov/current/title-16/chapter-I/subchapter-C/part-312 (as
  amended 22 April 2025, effective 23 June 2025).
- The FTC's own FAQ, *Complying with COPPA: Frequently Asked Questions*,
  https://www.ftc.gov/business-guidance/resources/complying-coppa-frequently-asked-questions
  (dated "July 2020", with a note "Edited January 2025" and a banner saying the
  rule was amended on 22 April 2025), question B.5:
  > In general, because many types of nonprofit entities are not subject to Section 5 of the FTC Act, these entities are not subject to the Rule.

  and, in the same answer, "the FTC encourages such entities to post privacy
  policies online and to provide COPPA’s protections to their child visitors."
- **What this means for us.** HumanityOS is not a company and not a 501(c)(3);
  gifts go to the maintainer personally (`docs/design/funding.md`). So the
  nonprofit-entity exclusion is not obviously available, and whether a free,
  donation-funded service run by an individual is "operated for commercial
  purposes" is **not answered by any source found**. Separately, each
  self-hosted server has its own operator: the person running it.

### US-2. When it applies: "directed to children", or "actual knowledge"

- 15 U.S.C. 6502(a)(1),
  https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap91-sec6502.htm
  (2024 Edition). The duties fall on
  > an operator of a website or online service directed to children, or any operator that has actual knowledge that it is collecting personal information from a child
- **"Directed to children" is a many-factor test, and since 2025 it names
  marketing and representations.** 16 CFR 312.2, "Website or online service
  directed to children", paragraph (1), after the list of factors (subject
  matter, visual content, animated characters, music, models, celebrities,
  advertising):
  > The Commission will also consider competent and reliable empirical evidence regarding audience composition and evidence regarding the intended audience, including marketing or promotional materials or plans, representations to consumers or to third parties,

  The FTC's statement of basis for that change, Federal Register 90 FR 16918,
  22 April 2025,
  https://www.federalregister.gov/documents/2025/04/22/2025-05904/childrens-online-privacy-protection-rule :
  > the Commission is convinced such materials and representations often provide compelling direct evidence regarding an operator's intended audience and audience composition
- **"Mixed audience" (defined in 2025).** 16 CFR 312.2: a service directed to
  children under the factors "but that does not target children as its primary
  audience", and that
  > does not collect personal information from any visitor, other than for the limited purposes set forth in § 312.5(c), prior to collecting age information

  The same definition requires that any age question be asked "in a neutral
  manner that does not default to a set age or encourage visitors to falsify age
  information."
- **General-audience services are covered only on actual knowledge, and need
  not ask ages.** FAQ A.12:
  > The Rule does not require operators to ask the age of visitors.

  FAQ H.6, on a general-audience service with forums where a child posts:
  > you may be considered to have actual knowledge where a child announces her age under certain circumstances, for example, if you monitor user posts, if a responsible member of your organization sees the post, or if someone alerts you to the post

  and
  > Where an operator knows that a particular visitor is a child, the operator must either meet COPPA’s notice and parental consent requirements or delete the child’s information.
- **What this means for us.** A server that never learns anyone's age is
  covered only if it is "directed to children". Knowledge can still arrive by
  other routes: a child saying their age in a public room an admin reads, or a
  parent writing to an admin. The answer then is to comply or delete that
  person's information.

### US-3. Does a no-sign-up app collect "personal information"?

Yes, as the rule defines it, very probably.

- 16 CFR 312.2, "Personal information" includes:
  > Online contact information as defined in this section;

  > A screen or user name where it functions in the same manner as online contact information, as defined in this section;

  > A persistent identifier that can be used to recognize a user over time and across different websites or online services.

  and the same paragraph names "an Internet Protocol (IP) address" as one, plus
  > A photograph, video, or audio file where such file contains a child's image or voice;
- "Online contact information", same section:
  > any other substantially similar identifier that permits direct contact with a person online, including but not limited to, an instant messaging user identifier
- The FTC on screen names when it added them, 78 FR 3972 (17 January 2013),
  https://www.federalregister.gov/documents/2013/01/17/2012-31341/childrens-online-privacy-protection-rule :
  > The description identifies precisely the form of direct, private, user-to-user contact the Commission intends the Rule to cover
- "Collects or collection" includes, 16 CFR 312.2:
  > Enabling a child to make personal information publicly available in identifiable form.
- The only exception for identifiers alone, 16 CFR 312.5(c)(7):
  > Where an operator collects a persistent identifier and no other personal information and such identifier is used for the sole purpose of providing support for the internal operations of the website or online service.

  "Support for the internal operations" includes activities necessary to
  "Authenticate users of, or personalize the content on, the website or online
  service" (312.2).
- **What this means for us.** In HumanityOS a person's public key is what
  others use to send them a direct message, and it is the same key on every
  federated server: that fits "instant messaging user identifier" and
  "persistent identifier" closely. A server also sees network addresses while
  connected, and holds display names, profile text, public posts, uploaded
  pictures and voice. The internal-operations exception covers an identifier
  only when "no other personal information" is collected, which is not the
  case. **So "we collect no personal information" is not a safe sentence in
  COPPA terms**, even though the app asks for no name, email or birth date. No
  FTC source found addresses cryptographic keys specifically.

### US-4. Does a parent-applied protected setup change either test?

No source found answers this directly. What the sources do say:

- **Actual knowledge** attaches to knowing "that a particular visitor is a
  child" (FAQ H.6). The setup sends no "this is a child" flag to any server
  (design section 6.1), so on its own it gives a server operator nothing to
  know. Whether the software's author "knows" anything from a feature that runs
  only on people's devices is not addressed by any source found.
- **Directed to children** now expressly weighs "representations to consumers"
  and "marketing or promotional materials" (US-2). A setup screen and public
  copy that present HumanityOS as something for children are that kind of
  representation. Parental controls on a general-audience product are common
  (the 2025 rule's commenters describe "family plans and parental controls" on
  existing platforms), and nothing found says that offering them makes a
  service child-directed. **The risk is in how it is presented, not that it
  exists.**
- If HumanityOS were found child-directed, or mixed audience, the rule would
  require either age screening before collecting personal information (mixed
  audience) or parental notice and verifiable consent (fully child-directed).
  Both conflict with the no-sign-up, no-age, no-ID design.

### US-5. What COPPA would require if it applied

16 CFR 312.3 lists the duties: notice of practices (312.4), verifiable parental
consent "prior to any collection, use, and/or disclosure of personal
information from children" (312.5), a parent's right to review and delete
(312.6), not conditioning participation on more information than needed
(312.7), security (312.8) and retention limits (312.10). Two were added in
2025:

- 312.8(b): the operator must keep
  > a written information security program that contains safeguards that are appropriate to the sensitivity of the personal information collected from children and the operator's size, complexity, and nature and scope of activities.
- 312.10:
  > Personal information collected online from a child may not be retained indefinitely.

  with "a written data retention policy" published in the online notice.
- **Not a content-safety law.** FAQ A.11, "Will the COPPA Rule keep my child
  from accessing inappropriate materials, such as pornography?":
  > No. COPPA is meant to give parents control over the online collection, use, or disclosure of personal information from children.

### US-6. Dates, penalties and what is pending

- **2025 amendments.** 90 FR 16918 (22 April 2025):
  > Effective date: The amended Rule is effective June 23, 2025.

  > Compliance date: Except with respect to Sec. 312.11(d)(1), (d)(4), and (g), regulated entities have until April 22, 2026 to comply.

  (The exceptions concern the FTC-approved "safe harbor" programs.) The
  compliance date has passed.
- **Penalties.** FAQ B.2:
  > A court can hold operators who violate the Rule liable for civil penalties of up to $53,088 per violation.
- **Age checks are allowed, not required.** FTC press release, 25 February 2026,
  https://www.ftc.gov/news-events/news/press-releases/2026/02/ftc-issues-coppa-policy-statement-incentivize-use-age-verification-technologies-protect-children :
  the Commission "will not bring an enforcement action under the Children’s
  Online Privacy Protection Rule (COPPA Rule) against certain website and online
  service operators that collect, use, and disclose personal information for the
  sole purpose of determining a user’s age via age verification technologies."
  It does not require anyone to verify age.
- **Pending bills (not law).** From the official bill status records on govinfo
  (both records updated 2026-09-08):
  - S. 836, *Children and Teens’ Online Privacy Protection Act* ("COPPA 2.0"),
    https://www.govinfo.gov/bulkdata/BILLSTATUS/119/s/BILLSTATUS-119s836.xml :
    "Passed Senate with amendments by Unanimous Consent." on 2026-03-05;
    latest action 2026-03-16, "Held at the desk." in the House.
  - H.R. 7757, *KIDS Act*,
    https://www.govinfo.gov/bulkdata/BILLSTATUS/119/hr/BILLSTATUS-119hr7757.xml :
    passed the House 267 to 117 on 2026-06-29; latest action 2026-07-13,
    "Received in the Senate and Read twice and referred to the Committee on
    Commerce, Science, and Transportation."

  Neither bill's text was read. Secondary sources (law-firm notes) say both
  would extend protections to teenagers; that is not checked here.

## 2. United States: FTC Act section 5, and words like "safe for kids"

**No FTC source was found that defines or forbids the phrase "safe for kids"
as such.** The rule that governs it is the general one against deceptive
claims, and the FTC has applied it to children's safety and contact-control
claims.

- **The statute.** 15 U.S.C. 45(a)(1),
  https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap2-subchapI-sec45.htm
  (2024 Edition):
  > Unfair methods of competition in or affecting commerce, and unfair or deceptive acts or practices in or affecting commerce, are hereby declared unlawful.

  Section 45(a)(2) applies it to "persons, partnerships, or corporations".
  A "corporation" under 15 U.S.C. 44 is one "organized to carry on business for
  its own profit or that of its members", which is why nonprofits are generally
  outside it (US-1). An individual is a "person".
- **The test.** *FTC Policy Statement on Deception*, 14 October 1983,
  https://www.ftc.gov/legal-library/browse/ftc-policy-statement-deception :
  > the Commission will find deception if there is a representation, omission or practice that is likely to mislead the consumer acting reasonably in the circumstances, to the consumer's detriment.

  > When representations or sales practices are targeted to a specific audience, such as children, the elderly, or the terminally ill, the Commission determines the effect of the practice on a reasonable member of that group.
- **Claims need backing before they are made.** *FTC Policy Statement
  Regarding Advertising Substantiation*, 23 November 1984,
  https://www.ftc.gov/legal-library/browse/ftc-policy-statement-regarding-advertising-substantiation :
  > that advertisers and ad agencies have a reasonable basis for advertising claims before they are disseminated.
- **Parental contact controls with a gap.** FTC press release, 3 May 2023,
  https://www.ftc.gov/news-events/news/press-releases/2023/05/ftc-proposes-blanket-prohibition-preventing-facebook-monetizing-youth-data .
  The FTC alleged that Facebook
  > misrepresented that parents could control whom their children communicated with through its Messenger Kids product.

  because
  > children in certain circumstances were able to communicate with unapproved contacts in group text chats and group video calls.

  These are allegations in a proposed order change; what happened next in that
  proceeding was not researched.
- **A "safe space" claim with moderation that did not work as described.** FTC
  press release, 9 July 2024,
  https://www.ftc.gov/news-events/news/press-releases/2024/07/ftc-order-will-ban-ngl-labs-its-founders-offering-anonymous-messaging-apps-kids-under-18-halt :
  > NGL and its operators marketed the app as a “safe space for teens” and claimed it uses “world class AI content moderation”

  The complaint alleged the moderation claims were false; the case settled.
- **A "complies with COPPA" claim.** FTC press release, 3 September 2025,
  https://www.ftc.gov/news-events/news/press-releases/2025/09/ftc-takes-action-against-robot-toy-maker-allowing-collection-childrens-data-without-parental-consent :
  > despite claims in Apitor’s privacy policies that it complies with Children’s Online Privacy Protection Rule (COPPA Rule), Apitor failed to notify parents and obtain their consent
- **Oversight claims.** COPPA FAQ I.14 (about app stores, but general in its
  words):
  > it could be a deceptive practice to misrepresent the level of oversight you provide for a child-directed app.
- **What this means for us.** Every safety sentence on screen or in public copy
  has to be literally true of the shipped build, judged by how a reasonable
  parent reads it, and the project needs to be able to show why it is true.
  "Only friends can message your child" is exactly the kind of sentence that
  was at issue with Messenger Kids; it is safe only if no route (groups, voice
  rooms, public rooms, admin notices, friends made before the setup) contradicts
  it.

## 3. United States: state laws (brief)

Only the laws named in the question were read. Other states have passed
age-appropriate design codes, social-media age laws and app-store laws; **none
were read for this finding.**

- **California Age-Appropriate Design Code Act** (Civil Code 1798.99.28 and
  following). It applies to a "business" as defined in the California Consumer
  Privacy Act (1798.99.30(a)), and that definition,
  https://leginfo.legislature.ca.gov/faces/codes_displaySection.xhtml?lawCode=CIV&sectionNum=1798.140
  (amended by AB 1170, effective 1 January 2026), requires an entity
  > organized or operated for the profit or financial benefit of its shareholders or other owners,

  plus a size threshold (over $25 million revenue, or personal information of
  100,000 or more consumers or households bought, sold or shared, or half of
  revenue from selling it). HumanityOS meets none of these, so it appears
  outside the Act. Litigation: *NetChoice v. Bonta*, Ninth Circuit No. 25-2366,
  opinion filed 12 March 2026,
  https://cdn.ca9.uscourts.gov/datastore/opinions/2026/03/12/25-2366.pdf ,
  kept the injunction only for sections 1798.99.31(b)(1) to (4) and (b)(7),
  "and VACATE the remainder of the preliminary injunction." Proceedings after
  remand were not checked.
- **California Digital Age Assurance Act (AB 1043)**, Civil Code 1798.500 to
  1798.505,
  https://leginfo.legislature.ca.gov/faces/codes_displaySection.xhtml?lawCode=CIV&sectionNum=1798.500
  ("Effective January 1, 2026. Operative January 1, 2027"). **This one could
  reach HumanityOS.** It has no business or size threshold:
  > “Developer” means a person that owns, maintains, or controls an application.

  and "Application" covers software run "on a computer, a mobile device, or any
  other general purpose computing device". 1798.501(b)(1):
  > A developer shall request a signal with respect to a particular user from an operating system provider or a covered application store when the application is downloaded and launched.

  and 1798.501(b)(2)(A), a developer that receives a signal "shall be deemed to
  have actual knowledge of the age range of the user to whom that signal
  pertains". Penalties are up to $2,500 per affected child for each negligent
  violation and $7,500 for each intentional one, in an action by the Attorney
  General (1798.503). Whether a desktop app downloaded from GitHub Releases or
  the project's own site counts, and how this "actual knowledge" meets COPPA's,
  were not worked out.
- **Utah App Store Accountability Act** (2025 S.B. 142, amended by 2026 H.B.
  498), enrolled texts
  https://le.utah.gov/Session/2025/bills/enrolled/SB0142.pdf and
  https://le.utah.gov/Session/2026/bills/enrolled/HB0498.pdf :
  > "Developer" means a person that owns or controls an app made available through an app store in the state.

  An app store is for downloading apps "onto a mobile device". H.B. 498 moves
  the app stores' duties to "Beginning May 6, 2027". Secondary sources (a
  law-firm note of 4 May 2026) say H.B. 498 also removed the Attorney General's
  enforcement and kept a private right of action, and that the trade group CCIA
  dropped its lawsuit on 21 April 2026; not checked against the court record.
  HumanityOS has no app in a mobile app store, so this does not reach it today.
- **Texas App Store Accountability Act** (S.B. 2420, Business and Commerce Code
  chapter 121), enrolled text
  https://capitol.texas.gov/tlodocs/89R/billtext/html/SB02420F.htm (takes
  effect 1 January 2026), section 121.051:
  > This subchapter applies only to the developer of a software application that the developer makes available to users in this state through an app store.

  and an app store is one that distributes applications "to the user of a
  mobile device". Status: a federal district court enjoined it on 23 December
  2025 and the Fifth Circuit (No. 25-50001) stayed that injunction (both dates
  from secondary sources); the Supreme Court docket 25A1390,
  https://www.supremecourt.gov/search.aspx?filename=/docket/docketfiles/html/public/25a1390.html ,
  shows on 6 July 2026: "Application (25A1390) to vacate stay presented to
  Justice Alito and by him referred to the Court is denied." So it is
  enforceable while the appeal runs. It does not reach a desktop app that is
  not in a mobile app store.

## 4. United Kingdom

### UK-1. The ICO's Age Appropriate Design Code (the Children's code)

- **Dates.** ICO, *About this code*,
  https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/childrens-information/childrens-code-guidance-and-resources/age-appropriate-design-a-code-of-practice-for-online-services/about-this-code/ :
  > It was laid before Parliament on 11 June 2020 and issued on 12 August 2020 under section 125 of the DPA 2018. It comes into force on 2 September 2020.
- **Scope.** ICO, *Services covered by this code*,
  https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/childrens-information/childrens-code-guidance-and-resources/age-appropriate-design-a-code-of-practice-for-online-services/services-covered-by-this-code/
  (no date shown):
  > This code applies to “information society services likely to be accessed by children” in the UK.

  "Child" means under 18. On free and not-for-profit services:
  > This code also covers not-for-profit apps, games and educational sites, as long as those services can be considered as ‘economic activity’ in a more general sense.

  > For example, they are types of services which are typically provided on a commercial basis.

  The same page lists "online messaging or internet based voice telephony
  services" and "online games" among information society services.
- **"Likely to be accessed."**
  > We consider that for a service to be ‘likely’ to be accessed, the possibility of this happening needs to be more probable than not.

  > If the nature, content or presentation of your service makes you think that children will want to use it, then you should conform to the standards in this code.
- **Services outside the UK.** Same page: "the DPA 2018 still applies if you
  offer your service to users in the UK, or monitor the behaviour of users in
  the UK."
- **Parental controls, standard 11**,
  https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/childrens-information/childrens-code-guidance-and-resources/age-appropriate-design-a-code-of-practice-for-online-services/11-parental-controls/ :
  > If you provide parental controls, give the child age appropriate information about this.

  The same standard explains why: parental controls also "impact on the
  child’s right to privacy". The protected setup does not monitor anyone, so the
  standard's second sentence (an obvious sign while monitoring is active) does
  not bite, but the first does.
- **A 2026 change in the law behind it.** Data (Use and Access) Act 2025,
  section 81, https://www.legislation.gov.uk/ukpga/2025/18/section/81 (in force
  5 February 2026, per the page's commencement note), adds to UK GDPR Article 25
  that for information society services likely to be accessed by children "the
  controller must take into account the children’s higher protection matters",
  which include
  > how children can best be protected and supported when using the services, and
- **What this means for us.** A server operator who offers HumanityOS to
  people in the UK is probably running an information society service (chat
  and games are "typically provided on a commercial basis"), and a setup screen
  made for children is the "presentation" the ICO describes. So if UK data
  protection law applies to a given server, the code probably does too. One
  concrete ask follows for the design: the child is told, in plain words, that
  the protected setup is on.

### UK-2. The Online Safety Act 2023 and Ofcom's children's codes

Sections read on legislation.gov.uk were shown "up to date with all changes
known to be in force on or before" 8, 9 or 10 October 2026. The pages also list
changes by a 2026 Act ("2026 c. 20") not yet applied to the text; those were not
researched.

- **A chat service is a "user-to-user service".** Section 3(1),
  https://www.legislation.gov.uk/ukpga/2023/50/section/3 : an internet service
  "by means of which content that is generated directly on the service by a user
  of the service" or shared on it "may be encountered by another user, or other
  users, of the service." The government's explainer,
  https://www.gov.uk/government/publications/online-safety-act-explainer/online-safety-act-explainer
  ("Updated 24 April 2025"), lists among covered services "online instant
  messaging services".
- **The exemptions do not fit.** Schedule 1 Part 1,
  https://www.legislation.gov.uk/ukpga/2023/50/schedule/1/part/1 , exempts a
  service whose only user content is email (para 1), SMS or MMS (para 2), or
  "one-to-one live aural communications" (para 3):
  > A user-to-user service is exempt if one-to-one live aural communications are the only user-generated content (other than identifying content) enabled by the service.

  HumanityOS carries text messages, groups, public rooms and pictures, so none
  of these applies.
- **An individual can be the provider, and it is whoever controls who can use
  the service.** Section 226(2) and (3),
  https://www.legislation.gov.uk/ukpga/2023/50/section/226 :
  > The provider of a user-to-user service is to be treated as being the entity that has control over who can use the user-to-user part of the service (and that entity alone).

  and if no entity has that control, "an individual or individuals" who do. For
  a self-hosted server, that is the person running it, not the project that
  wrote the software (the project's reading of section 226).
- **Which services are regulated.** Section 4(5),
  https://www.legislation.gov.uk/ukpga/2023/50/section/4 : a service "has links
  with the United Kingdom" if
  > (a) the service has a significant number of United Kingdom users, or

  or if "United Kingdom users form one of the target markets for the service";
  section 4(6) adds a service usable in the UK that presents "a material risk of
  significant harm" to people there. "Significant number" is not defined in the
  sections read.
- **The children's access assessment.** Section 35(1),
  https://www.legislation.gov.uk/ukpga/2023/50/section/35 : an assessment "to
  determine whether it is possible for children to access the service or a part
  of the service", and if so whether the "child user condition" is met. Section
  35(2):
  > A provider is only entitled to conclude that it is not possible for children to access a service, or a part of it, if age verification or age estimation is used on the service with the result that children are not normally able to access the service or that part of it.

  Section 35(3), the child user condition:
  > (a) there is a significant number of children who are users of the service or of that part of it, or

  > (b) the service, or that part of it, is of a kind likely to attract a significant number of users who are children.

  Section 35(4)(b): whether (a) is met "is to be based on evidence about who
  actually uses a service, rather than who the intended users of the service
  are."
- **When, and keeping a record.** Section 36,
  https://www.legislation.gov.uk/ukpga/2023/50/section/36 : the first
  assessment at the time set by Schedule 3 (the explainer: "In-scope service
  providers had until 16 April to carry out a children’s access assessment",
  meaning 16 April 2025), then at most a year apart while a service is not
  treated as likely to be accessed by children, and
  > (a) before making any significant change to any aspect of the service’s design or operation to which such an assessment is relevant,

  and 36(7): "A provider must make and keep a written record, in an easily
  understandable form, of every children’s access assessment." Section 37(4),
  https://www.legislation.gov.uk/ukpga/2023/50/section/37 : a provider that
  fails to do the first assessment is treated as likely to be accessed by
  children.
- **What follows if a service is likely to be accessed by children.** A
  children's risk assessment (section 11) and the safety duties of section 12,
  https://www.legislation.gov.uk/ukpga/2023/50/section/12 , including 12(3)(a),
  systems designed to "prevent children of any age from encountering, by means
  of the service, primary priority content that is harmful to children".
  Section 61, https://www.legislation.gov.uk/ukpga/2023/50/section/61 , defines
  that as pornographic content (text alone excepted) and content which
  "encourages, promotes or provides instructions for" suicide, "an act of
  deliberate self-injury" or "an eating disorder". 12(4) requires age
  verification or estimation for that, "highly effective" under 12(6), except,
  12(5), where
  > (a) a term of service indicates (in whatever words) that the presence of that kind of primary priority content that is harmful to children is prohibited on the service, and

  and "(b) that policy applies in relation to all users of the service."
- **In force.** gov.uk news, published 24 July 2025,
  https://www.gov.uk/government/news/whats-changing-for-children-on-social-media-from-25-july-2025 :
  "From 25 July, the way children experience the internet will fundamentally
  change, as new laws come into force". The explainer says the protection of
  children codes were laid on 24 April 2025.
- **Ofcom's guidance was not reached** (see "Sources that could not be
  reached"), so what Ofcom counts as a "significant number", and which code
  measures it expects of a small, low-risk service, are not answered here.
- **What this means for us.** A HumanityOS server with UK links is a regulated
  user-to-user service, and its provider is whoever runs it, volunteer or not.
  With no age checks, children can access it, so it turns on the child user
  condition, judged on who actually uses it. Adding a children's setup is
  arguably a "significant change" calling for a fresh assessment, and it is
  evidence that the service expects children. Nothing in the sections read
  requires breaking end-to-end encryption to do any of this; the powers that
  concern scanning (section 121) were not read.

## 5. European Union

### EU-1. Digital Services Act, Article 28

Regulation (EU) 2022/2065,
https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32022R2065
(Official Journal L 277, 27 October 2022; EUR-Lex shows it in force).

- **The duty.** Article 28(1):
  > Providers of online platforms accessible to minors shall put in place appropriate and proportionate measures to ensure a high level of privacy, safety, and security of minors, on their service.

  Article 28(3):
  > Compliance with the obligations set out in this Article shall not oblige providers of online platforms to process additional personal data in order to assess whether the recipient of the service is a minor.
- **Small services are exempt.** Article 28 sits in Section 3 of Chapter III,
  and Article 19(1):
  > This Section, with the exception of Article 24(3) thereof, shall not apply to providers of online platforms that qualify as micro or small enterprises as defined in Recommendation 2003/361/EC.

  Recommendation 2003/361/EC,
  https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32003H0361 ,
  Article 1: "An enterprise is considered to be any entity engaged in an
  economic activity, irrespective of its legal form", including "self-employed
  persons"; Article 2(3):
  > a microenterprise is defined as an enterprise which employs fewer than 10 persons and whose annual turnover and/or annual balance sheet total does not exceed EUR 2 million.
- **Private messaging is not an "online platform"; open channels might be.**
  Article 3(i): an online platform is a hosting service that "stores and
  disseminates information to the public". Recital 14 says interpersonal
  communication services (as defined in Directive (EU) 2018/1972),
  > such as emails or private messaging services, fall outside the scope of the definition of online platforms

  but platform obligations "may apply to services that allow the making
  available of information to a potentially unlimited number of recipients, not
  determined by the sender of the communication, such as through public groups
  or open channels." The same recital: where access "requires registration or
  admittance to a group", information is disseminated to the public only if
  people are "automatically registered or admitted without a human decision or
  selection of whom to grant access."
- **What this means for us.** HumanityOS direct messages and invite-only groups
  are interpersonal communication, not an online platform. A server's open
  public rooms could be. Either way, a server run by one volunteer on donations
  is at most a micro enterprise, and Article 28 does not apply to it. (Whether a
  free volunteer service is an "information society service" at all, which the
  DSA requires, is a separate open question, also noted in the 2026-10-10
  report-duties finding.)

### EU-2. The Commission's guidelines on Article 28 (2025)

*Guidelines on measures to ensure a high level of privacy, safety and security
for minors online, pursuant to Article 28(4) of Regulation (EU) 2022/2065*,
C/2025/5519, Official Journal C series 10 October 2025,
https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=OJ:C_202505519 ;
announced by the Commission on 14 July 2025,
https://digital-strategy.ec.europa.eu/en/library/commission-publishes-guidelines-protection-minors :
"The guidelines will apply to all online platforms accessible to minors, with
the exception of micro and small enterprises."

- **Scope, paragraph 8:**
  > Pursuant to Article 19 of Regulation (EU) 2022/2065, the obligation laid down in Article 28(1) of Regulation (EU) 2022/2065 does not apply to providers of online platforms that qualify as micro or small enterprises,
- **A terms statement is not enough, paragraph 6:** a provider "cannot solely
  rely on a statement in its terms and conditions prohibiting access to
  minors, to argue that the platform is not accessible to them."
- **Tools for guardians are a complement, paragraph 80:**
  > Compliance with the obligation of providers of online platforms accessible to minors to ensure a high level of privacy, safety and security on their services must never rely exclusively on tools for guardians.

  Paragraph 79 describes such tools as helping guardians "while respecting
  children’s agency and privacy".
- **What this means for us.** Not binding on a micro service, but a useful
  benchmark that matches the design's direction (the Commission's page lists
  accounts private by default, blocking and muting, and not being added to
  groups without consent). It also confirms the framing: the protected setup is
  one layer on top of safe defaults for everyone, never the whole answer.

### EU-3. GDPR Article 8 (a child's consent)

Regulation (EU) 2016/679,
https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32016R0679
(OJ L 119, 4 May 2016; applies from 25 May 2018; EUR-Lex shows a consolidated
version of 4 May 2016).

- Article 8(1):
  > Where point (a) of Article 6(1) applies, in relation to the offer of information society services directly to a child, the processing of the personal data of a child shall be lawful where the child is at least 16 years old.

  Below 16 it needs consent "given or authorised by the holder of parental
  responsibility"; member states may set a lower age, not below 13. Article
  8(2): "The controller shall make reasonable efforts to verify in such cases
  that consent is given or authorised by the holder of parental responsibility
  over the child, taking into consideration available technology."
- Point (a) of Article 6(1) is consent. **Article 8 applies only where consent
  is the legal basis** for the processing. Which legal basis a HumanityOS
  server relies on was not analysed.
- **What counts as personal data.** Article 4(1): an identifiable person is one
  who can be identified "in particular by reference to an identifier such as a
  name, an identification number, location data, an online identifier". A
  public key, together with posts and network addresses, is very likely
  personal data (the project's reading).
- **Reach outside the EU.** Article 3(2)(a): the regulation applies to a
  controller outside the Union offering "goods or services, irrespective of
  whether a payment of the data subject is required, to such data subjects in
  the Union".
- **A family server may be outside it.** Article 2(2)(c): the regulation does
  not apply to processing "by a natural person in the course of a purely
  personal or household activity". Recital 18 adds:
  > However, this Regulation applies to controllers or processors which provide the means for processing personal data for such personal or household activities.

  The UK's version of the GDPR has the same household exemption; it was not
  re-read for this finding.

## 6. What we may truthfully say

Everything below is the project's own reading of the sources above. Each
sentence must stay true of the build that ships; if a sentence stops being
true, it comes off the screen the same day (Deception and Substantiation
statements, section 2).

**Sentences we could use** (on the setup screen, in Settings > Safety and in
public copy):

1. "The protected setup locks these safety settings with a PIN on this device.
   It does not check anyone's age, and no setting can make an app, or the
   people in it, safe."
2. "With it on, only people on this device's friends list can send direct
   messages, and adding a friend, joining a group or joining a voice room needs
   the PIN. Public rooms stay public: anyone on that server can post there, and
   the server's admins can still send notices."
3. "Warnings are simple checks for known patterns, run on this device only.
   They miss things. They never send your messages to anyone and never report
   anything."
4. "Nothing about this setup is sent to any server. The server does not know
   this device is protected."
5. "HumanityOS has not been reviewed or certified under any children's privacy
   or online safety law. Our dated research notes are public."

Sentence 2 matches the design as written (sections 6.2 and 6.5). If, when it
ships, existing friends are reviewed by the parent (see the judgement calls),
"friends you approve" becomes true and can replace "people on this device's
friends list". Calls are left out because the design does not yet settle
whether the protected setup has calls (section 10, question 8).

**Words and claims to avoid:**

- "kid safe", "child safe", "safe for kids", "family safe", "safe space",
  "child-proof", "protects your child". (No source supports them; NGL's "safe
  space for teens" was part of an FTC case.)
- "compliant", "COPPA compliant", "meets the Children's code", "certified",
  "approved", "verified". (Apitor.)
- "only friends can contact your child", "parent-approved contacts", unless
  every route is closed, friends made before the setup included. (Messenger
  Kids.)
- "filters", "blocks predators", "detects grooming", "AI moderation",
  "moderated" for direct messages. Warnings are pattern checks, and nobody reads
  encrypted messages. (NGL.)
- "collects no personal information", "no personal data", "anonymous". Under
  COPPA and the GDPR, the key, network address, display name, photos and voice
  can all count. Say instead what is not asked for: "no email, phone number,
  birth date or real name".
- "parental consent" for the PIN. It is a COPPA term of art for something
  specific (verifiable parental consent); say "the PIN" or "a parent's PIN".
- Marketing HumanityOS itself as being "for kids" or "for children", or
  child-oriented art in public copy. Under the 2025 COPPA rule, marketing and
  representations are evidence of intended audience.
- "monitor", "see what your child is doing". The setup does not monitor, and
  saying it does would be both untrue and, under the ICO code, a reason the
  child must be told.

---

## Where the sources pull in different directions

**"No personal information" in plain speech, and in the law.** The project
truthfully says it asks for no name, email, phone number or birth date. COPPA
and the GDPR define personal information far more widely: online contact
identifiers, persistent identifiers including IP addresses, photos, voice,
"an online identifier". Both statements are true in their own terms; only the
second is the one a regulator uses. Public copy should use the first kind of
sentence and never the bare claim "no personal data".

**An 18+ gate and a children's setup.** The web chat's entry screen says "By
entering, you confirm you are 18 years or older" (see the code notes below).
A setup labelled for children contradicts it. Under COPPA the 18+ line points
towards a general-audience service; a children's setup points the other way
(representations about intended audience). Under the EU guidelines (paragraph
6) a terms statement alone does not make a service inaccessible to minors
anyway. The two should be made consistent before the setup ships.

**Telling the child.** The ICO's standard 11 asks that a child be told when
parental controls are on. Nothing read in COPPA asks for or against that. The
EU guidelines speak of respecting "children’s agency and privacy". These agree
in direction; only the UK says it as a standard.

## What is still unknown, and what would settle it

- **Whether COPPA reaches HumanityOS at all** ("operated for commercial
  purposes", for a free, donation-funded service run by an individual, and
  separately for each self-hosted server). No source found answers it. What
  would settle it: a written question to the FTC staff's COPPA mailbox,
  CoppaHotLine@ftc.gov (the FAQ: "please send an email to our COPPA hotline"),
  which costs an email and gives staff guidance rather than a ruling (the FAQ
  says of itself: "This document represents the views of FTC staff and is not
  binding on the Commission."); or a lawyer's opinion.
- **Whether a public key used to address messages is "online contact
  information"** (an "instant messaging user identifier"). No FTC source on
  cryptographic keys was found. Same route: the COPPA Hotline or a lawyer.
- **Whether a parental-control feature presented for children makes a
  general-audience service "directed to children" or "mixed audience".** Not
  answered by any source found. The FAQ suggests asking an FTC-approved COPPA
  safe harbor program; a lawyer would also know.
- **California AB 1043 from 1 January 2027**: whether a desktop app downloaded
  from GitHub Releases or the project's own site must request an age signal
  (is GitHub a "covered application store"?), what the operating systems will
  provide, how its deemed "actual knowledge" interacts with COPPA, and whether
  it has been challenged in court. What would settle it: a separate dated
  finding reading the whole title (1798.500 to 1798.505) and any Attorney
  General guidance, before the end of 2026.
- **UK thresholds**: what "a significant number of United Kingdom users"
  (section 4) and "a significant number of children" (section 35) mean, and
  which code measures Ofcom expects of a small, low-risk service. What would
  settle it: Ofcom's children's access assessments guidance (published 24 April
  2025, not reached) and its codes, read in a browser that passes Ofcom's bot
  check, or a question to Ofcom.
- **Whether a free, volunteer-run server is an "information society service"**
  ("normally provided for remuneration") in UK and EU law. The ICO says
  not-for-profit services are covered if they are "economic activity" in a
  general sense; EU court decisions on remuneration were not read. A lawyer in
  the UK or an EU member state would know.
- **Whether one person running a server is an "enterprise"** for the DSA's
  micro-enterprise exclusion, and whether open public rooms make a server an
  "online platform". Moot for Article 28 if the exclusion applies, which it very
  probably does.
- **The GDPR legal basis** a HumanityOS server relies on (consent, contract or
  legitimate interests), which decides whether Article 8 applies. Not analysed.
- **US state laws other than the four named**, none read; **the KIDS Act and
  S. 836** after 8 September 2026 and their texts; **the Messenger Kids
  proceeding's outcome**; **the California design code after remand**; **the
  Fifth Circuit's stay order itself** (only the Supreme Court docket was read).
- **Whether server admins can erase a specific person's data** on learning
  they are under 13 (FAQ H.6: meet the notice and consent requirements "or
  delete the child’s information"). A person can erase
  their own data on a server (`account_erased` in `src/relay/relay.rs`); an
  admin tool for someone else was not checked.

## Sources that could not be reached

- https://www.ofcom.org.uk/online-safety/illegal-and-harmful-content/quick-guide-to-childrens-access-assessments :
  a bot check in the browser; no attempt was made to get past it. Wanted:
  Ofcom's plain summary of the children's access assessment.
- https://www.ofcom.org.uk/siteassets/resources/documents/consultations/category-1-10-weeks/statement-protecting-children-from-harms-online/main-document/childrens-access-assessments-guidance.pdf :
  HTTP 403 to direct and tool requests. Wanted: "significant number", whether
  self-declared age counts, measures for small services.
- ftc.gov returned 404 to direct (non-browser) requests for every page tried;
  the pages were read in a browser instead. The Deception Policy Statement PDF
  was fetched by a tool and its text extracted, so its quotations were
  script-checked.
- leginfo.legislature.ca.gov returned 403 to direct requests; read in a
  browser.
- https://le.utah.gov/xcode/Title13/Chapter76/ (the current Utah Code): the
  page builds its text by script, so the enrolled bills were read instead.
- The Fifth Circuit's order in No. 25-50001 and the district court's
  injunction were not looked for; their dates come from secondary sources.

## What it means for the protected setup

### Certain (stated by the sources above)

1. No source read requires HumanityOS to verify anyone's age. COPPA does not
   require general-audience services to ask (FAQ A.12); the DSA says protecting
   minors does not require processing extra data to find out who is one
   (Article 28(3)). The exception is the UK: a regulated service likely to be
   accessed by children must use highly effective age checks for primary
   priority content, unless its terms prohibit that content for all users
   (section 12(4) to (6)).
2. No source supports calling the setup "kid safe", "child safe" or
   "compliant". The FTC judges claims aimed at parents by how a reasonable
   parent reads them and expects them to be backed up before they are made;
   it has alleged deception in exactly this territory (parental contact
   controls with gaps; a "safe space" with moderation that did not work; a
   false "complies with COPPA").
3. Under the 2025 COPPA rule, marketing and "representations to consumers" are
   evidence of a service's intended audience.
4. Sending no "this is a child" flag means a server learns nothing about age
   from the setup. It does not stop a server operator learning a person's age
   another way, and COPPA's answer then is to comply or delete.
5. If UK data protection law and the Children's code apply, a child should be
   told that parental controls are on (standard 11).
6. The EU guidelines treat tools for guardians as a complement to safe
   defaults, never the only measure (paragraph 80).
7. As the laws define it, a HumanityOS server does hold personal information
   (keys that address messages, network addresses, names, photos, voice,
   public posts), however little it asks for.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **Keep the feature and its name, "Protected setup".** Describe it as what it
   is, a PIN lock on safety settings that a parent can apply, and keep the
   button's wording neutral ("Lock these settings with a PIN", or "Set up for
   someone you look after") rather than presenting HumanityOS as a product for
   children. Do not market the app to children. (COPPA 312.2 paragraph (1); the
   Deception Statement.)
2. **Make the age line consistent before shipping; this is the operator's
   decision.** The web chat says 18 or older; a children's setup says
   otherwise. Two coherent positions: a general-audience service for 13 and
   over, with the protected setup aimed at parents of teenagers; or 18 and over
   with the setup described for someone a person looks after, accepting that
   this reads oddly. A service openly for under-13s would need COPPA's notice
   and consent machinery, which conflicts with the no-sign-up design.
3. **Close the Messenger Kids gap.** When the setup is applied, show the parent
   every existing friend, group and voice room and let them remove any, so that
   "friends you approve" becomes true; and say on screen that admin notices
   still arrive and public rooms are public (or turn public rooms off or
   read-only by default in the setup). Until then, use sentence 2 of section 6
   as written.
4. **Tell the child.** A plain, always-visible line on the child's device
   ("A parent has turned on the protected setup: only friends can message you,
   and new friends need their PIN") and an age-appropriate explanation. This
   follows the ICO's standard 11 and the EU guidelines' respect for "children’s
   agency and privacy", and costs nothing.
5. **Keep the no-flag rule and add no age collection.** It lines up with DSA
   Article 28(3), with data minimisation, and with keeping server operators out
   of COPPA's actual-knowledge trigger by design.
6. **On the project's own server, write the content rules the UK Act keys
   on.** Its rules page prohibits "illegal content" but does not name
   pornography or content encouraging suicide, self-harm or eating disorders.
   A rule that prohibits these for all users is the condition in section 12(5)
   under which mandatory age checks would not be required, if the server were
   ever a regulated service likely to be accessed by children.
7. **Give admins a way to erase a specific person's data** on learning they are
   under 13, and say in the self-hosting guide that a server's operator is the
   "operator" (COPPA) and "provider" (Online Safety Act) for their own server,
   linking this finding.
8. **Write the AB 1043 finding before 1 January 2027**, since it is the one law
   found that may reach a desktop app distributed outside any app store.
9. **Ask before claiming more.** A short email to the COPPA Hotline describing
   the no-sign-up design and the protected setup would turn three of the
   unknowns above into staff guidance at the cost of one email.

### Observed in the code on 10 October 2026 (relevant to the judgement calls)

Read, not changed, while writing this; a reviewer should confirm.

- `web/chat/index.html`, the entry screen (line 116): "By entering, you confirm
  you are 18 years or older and agree to be respectful." No matching age line
  was found in the native app's source.
- `web/pages/rules.html` ("Published 6 September 2026"): rule 4 is "No illegal
  content, threats or fraud."; the "limits that are not negotiable" box names
  sexual exploitation of children. No rule names pornography or content
  encouraging suicide, self-harm or eating disorders, and a search of
  `docs/accord/` found none either.
- Design section 6.2 keeps server admins' system notices (`Private` messages)
  available under "friends only", and section 6.5 says public rooms are public.
  Both are routes a parent reading "only friends can message" would not expect,
  which is why sentence 2 of section 6 names them.

---

This document is a reading of public sources on 10 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. Laws, guidance and court
cases in this area change often; check the dates above, and ask a lawyer before
relying on any of it for a decision with legal consequences.
