# Running a mail service for others: US federal law, Washington law and the GDPR

**Research date: 14 November 2026 (Pacific).** Every statute below was read that day from an official
source (the US Code's 2024 edition on govinfo.gov, the Federal Trade Commission, the Washington
Attorney General, the European Data Protection Board), never from memory.

**This is not legal advice.** It is an AI's reading of what the texts say, written down with the
words that carry the meaning so that a lawyer can check them. Where the texts do not settle a
question, this file says so and names what a lawyer would need to decide. The operator should have a
lawyer read it before HumanityOS receives or sends anyone's email.

Companion files: the providers' and hosts' rules,
[`2026-11-14-email-sending-rules-and-host-terms.md`](2026-11-14-email-sending-rules-and-host-terms.md),
and the research notes,
[`../research/2026-11-14-microsoft-services/email-bridge.md`](../research/2026-11-14-microsoft-services/email-bridge.md).
It builds on [`2026-10-10-report-duties-child-abuse-material.md`](2026-10-10-report-duties-child-abuse-material.md)
(the CyberTipline duty in depth) and [`2026-11-07-privacy-notice.md`](2026-11-07-privacy-notice.md)
(the GDPR's reach in depth).

Method: `curl -A "Claude-User (research tool)"` with page text extracted by script and quotations
checked against it, except the FTC's guide, which refused curl and was read through WebFetch (marked
**not script-checked**). Robots.txt read first for each host, in a step of its own. Quotations are
exact (curly quotes normalised to straight); a quotation that is part of a sentence is still the
text's exact words.

## The question

What do US federal law, Washington law and the EU GDPR say about a project that receives (and
possibly sends) personal email on behalf of its people?

## Short answer

- **CAN-SPAM** regulates "commercial" email and whoever "initiates" it. Its definition of "initiate"
  leaves out "routine conveyance", the automatic relaying of mail whose recipients someone else chose.
  On its words, a server that only receives and passes on its people's personal mail is not the
  initiator, and personal mail is not commercial. It would bind any commercial mail the project sends
  itself (up to $53,088 per violating email, by the FTC's guide).
- **The Stored Communications Act**: a service that lets people "send or receive" email is an
  "electronic communication service" by the statute's words. If it serves "the public", it may not
  knowingly disclose stored contents except as the law lists; the government needs a **warrant** for
  mail stored 180 days or less; and a provider must **preserve** records for 90 days when the
  government asks. Whether a free, invitation-free, volunteer-run HumanityOS server serves "the
  public" is a question for a lawyer.
- **The Wiretap Act** lets a provider read mail in transit when it is "a necessary incident to the
  rendition of his service" (spam filtering is the obvious case), and forbids a public provider to
  divulge mail in transit to others.
- **18 U.S.C. 2258A**: a "provider" that gains actual knowledge of apparent child sexual abuse
  material must report it to NCMEC's CyberTipline; nothing requires it to monitor or scan, and a
  report counts as a request to preserve the material for one year.
- **47 U.S.C. 230(c)**: the project is not treated as the publisher of mail others write, and is
  protected when it blocks objectionable mail in good faith; Section 230 does not touch federal
  criminal law (including the child-protection chapter) or the privacy statutes.
- **Washington (RCW 19.255)**, read through the Attorney General's pages because the statute's own
  site barred automated reading: it applies to individuals and businesses; a breach of unencrypted
  personal information must be told to residents within 30 days, and to the Attorney General when
  more than 500 residents are affected. **"Personal information" includes private keys for
  electronic signature and a username or email address with a password.**
- **The GDPR** reaches a controller outside the EU only when it offers services to people in the EU
  or monitors their behaviour (Article 3(2)); if it applies, Article 32 asks for security
  "appropriate to the risk". EUR-Lex barred automated reading today, so Article 32 was read only
  through the EDPB's summary.

## The findings

### 1. CAN-SPAM: what it covers and who it binds

Source: 15 U.S.C. 7702, US Code 2024 edition,
https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap103-sec7702.htm
(read 14 November 2026).

- What it covers, 7702(2)(A): "The term "commercial electronic mail message" means any electronic
  mail message the primary purpose of which is the commercial advertisement or promotion of a
  commercial product or service (including content on an Internet website operated for a commercial
  purpose)."
- Who it binds, 7702(9): "The term "initiate", when used with respect to a commercial electronic mail
  message, means to originate or transmit such message or to procure the origination or transmission
  of such message, but shall not include actions that constitute routine conveyance of such message."
- The carve-out, 7702(15): "The term "routine conveyance" means the transmission, routing, relaying,
  handling, or storing, through an automatic technical process, of an electronic mail message for
  which another person has identified the recipients or provided the recipient addresses."

Source: 15 U.S.C. 7704,
https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap103-sec7704.htm

- The false-header rule reaches more than advertising, 7704(a)(1): "It is unlawful for any person to
  initiate the transmission, to a protected computer, of a commercial electronic mail message, or a
  transactional or relationship message, that contains, or is accompanied by, header information that
  is materially false or materially misleading."
- What every commercial message must carry, 7704(a)(5)(A): "(i) clear and conspicuous identification
  that the message is an advertisement or solicitation; (ii) clear and conspicuous notice of the
  opportunity under paragraph (3) to decline to receive further commercial electronic mail messages
  from the sender; and (iii) a valid physical postal address of the sender."
- Opt-outs: sending after an objection is unlawful from "10 business days after the receipt of such
  request".

Source: 15 U.S.C. 7707,
https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap103-sec7707.htm

- A provider may refuse mail, 7707(c): "Nothing in this chapter shall be construed to have any effect
  on the lawfulness or unlawfulness, under any other provision of law, of the adoption,
  implementation, or enforcement by a provider of Internet access service of a policy of declining to
  transmit, route," (the sentence continues with receiving, handling or storing certain mail).
- State laws, 7707(b)(1): "This chapter supersedes any statute, regulation, or rule of a State or
  political subdivision of a State that expressly regulates the use of electronic mail to send
  commercial messages, except to the extent that any such statute, regulation, or rule prohibits
  falsity or deception in any portion of a commercial electronic mail message or information attached
  thereto."

Source: the FTC's CAN-SPAM compliance guide for business,
https://www.ftc.gov/business-guidance/resources/can-spam-act-compliance-guide-business (August 2023,
edited January 2024 for inflation-adjusted penalties, as the page itself says; read through WebFetch,
**not script-checked**):

- "Despite its name, the CAN-SPAM Act doesn't apply just to bulk email."
- "Each separate email in violation of the CAN-SPAM Act is subject to penalties of up to $53,088,"
- "You must honor a recipient's opt-out request within 10 business days."
- On hiring a mail service: "The law makes clear that even if you hire another company to handle your
  email marketing, you can't contract away your legal responsibility".

### 2. The Stored Communications Act: what a provider may disclose, and what it must hand over

Definitions. Source: 18 U.S.C. 2510,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap119-sec2510.htm
and 18 U.S.C. 2711,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap121-sec2711.htm

- 2510(15): ""electronic communication service" means any service which provides to users thereof
  the ability to send or receive wire or electronic communications;"
- 2510(17)(A), part of the meaning of "electronic storage": "any temporary, intermediate storage of a
  wire or electronic communication incidental to the electronic transmission thereof;"
- 2711(2): "the term "remote computing service" means the provision to the public of computer storage
  or processing services by means of an electronic communications system;"

Voluntary disclosure. Source: 18 U.S.C. 2702,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap121-sec2702.htm

- 2702(a)(1): "a person or entity providing an electronic communication service to the public shall
  not knowingly divulge to any person or entity the contents of a communication while in electronic
  storage by that service;"
- 2702(a)(3): "a provider of remote computing service or electronic communication service to the
  public shall not knowingly divulge a record or other information pertaining to a subscriber to or
  customer of such service (not including the contents of communications covered by paragraph (1) or
  (2)) to any governmental entity."
- Among the exceptions, 2702(b)(1) "to an addressee or intended recipient of such communication" and
  2702(b)(5) "as may be necessarily incident to the rendition of the service or to the protection of
  the rights or property of the provider of that service;"

Required disclosure. Source: 18 U.S.C. 2703,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap121-sec2703.htm

- 2703(a): "A governmental entity may require the disclosure by a provider of electronic
  communication service of the contents of a wire or electronic communication, that is in electronic
  storage in an electronic communications system for one hundred and eighty days or less, only
  pursuant to a warrant issued using the procedures described in the Federal Rules of Criminal
  Procedure" (the sentence continues with State and military courts).
- 2703(f)(1): "A provider of wire or electronic communication services or a remote computing service,
  upon the request of a governmental entity, shall take all necessary steps to preserve records and
  other evidence in its possession pending the issuance of a court order or other process."
- 2703(f)(2): "Records referred to in paragraph (1) shall be retained for a period of 90 days, which
  shall be extended for an additional 90-day period upon a renewed request by the governmental
  entity."

### 3. The Wiretap Act: reading mail in transit (spam filtering)

Source: 18 U.S.C. 2511,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap119-sec2511.htm

- 2511(2)(a)(i): "It shall not be unlawful under this chapter for an operator of a switchboard, or an
  officer, employee, or agent of a provider of wire or electronic communication service, whose
  facilities are used in the transmission of a wire or electronic communication, to intercept,
  disclose, or use that communication in the normal course of his employment while engaged in any
  activity which is a necessary incident to the rendition of his service or to the protection of the
  rights or property of the provider of that service".
- 2511(3)(a): "a person or entity providing an electronic communication service to the public shall
  not intentionally divulge the contents of any communication (other than one to such person or
  entity, or an agent thereof) while in transmission on that service to any person or entity other
  than an addressee or intended recipient of such communication".

### 4. 18 U.S.C. 2258A: reporting apparent child sexual abuse material

Source: 18 U.S.C. 2258A,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2258A.htm
(the 2024 edition includes the 2024 amendments, Pub. L. 118-59); and 18 U.S.C. 2258E,
https://www.govinfo.gov/content/pkg/USCODE-2024-title18/html/USCODE-2024-title18-partI-chap110-sec2258E.htm

- Who: 2258E(6), "the term "provider" means an electronic communication service provider or remote
  computing service;"
- The duty, 2258A(a)(1)(A)(i): a provider "shall, as soon as reasonably possible after obtaining
  actual knowledge of any facts or circumstances described in paragraph (2)(A), take the actions
  described in subparagraph (B)" (those actions begin with a report to NCMEC's CyberTipline).
- No duty to look, 2258A(f): "Nothing in this section shall be construed to require a provider to",
  then: "(1) monitor any user, subscriber, or customer of that provider; (2) monitor the content of any
  communication of any person described in paragraph (1); or (3) affirmatively search, screen, or
  scan for facts or circumstances described in sections (a) and (b)."
- Preservation, 2258A(h)(1): "a completed submission by a provider of a report to the CyberTipline
  under subsection (a)(1) shall be treated as a request to preserve the contents provided in the
  report for 1 year after the submission to the CyberTipline."
- The 2026-10-10 finding covers this duty in full, and notes that no official source says whether a
  volunteer running a small server is a "provider".

### 5. 47 U.S.C. 230: liability for what others write

Source: 47 U.S.C. 230,
https://www.govinfo.gov/content/pkg/USCODE-2024-title47/html/USCODE-2024-title47-chap5-subchapII-partI-sec230.htm

- 230(c)(1): "No provider or user of an interactive computer service shall be treated as the publisher
  or speaker of any information provided by another information content provider."
- 230(c)(2)(A): no liability for "any action voluntarily taken in good faith to restrict access to or
  availability of material that the provider or user considers to be obscene, lewd, lascivious,
  filthy, excessively violent, harassing, or otherwise objectionable, whether or not such material is
  constitutionally protected".
- Who: 230(f)(2), "The term "interactive computer service" means any information service, system, or
  access software provider that provides or enables computer access by multiple users to a computer
  server".
- Limits: 230(e)(1), "Nothing in this section shall be construed to impair the enforcement of section
  223 or 231 of this title, chapter 71 (relating to obscenity) or 110 (relating to sexual exploitation
  of children)" of title 18; and 230(e)(4), "Nothing in this section shall be construed to limit the
  application of the Electronic Communications Privacy Act of 1986".
- A notice duty, 230(d): "A provider of interactive computer service shall, at the time of entering an
  agreement with a customer for the provision of interactive computer service and in a manner deemed
  appropriate by the provider, notify such customer that parental control protections (such as
  computer hardware, software, or filtering services) are commercially available".

### 6. Washington: data breach notification (RCW 19.255)

The statute's own site (app.leg.wa.gov) bars automated reading in its robots.txt ("User-agent: *",
"Disallow: /"), so these words are the Attorney General's description of the law, not the statute.

Source: Washington State Attorney General, "Washington's Data Breach Notification Laws",
https://www.atg.wa.gov/washington-s-data-breach-notification-laws (undated page; it describes the law
"As of 2023" and the 2019 changes in force "on March 1, 2020").

- "RCW 19.255 applies to individuals and businesses".
- "In general, notification must be made "in the most expedient time possible" and within 30 days
  after the breach was discovered."
- "If a data breach affects more than 500 Washington residents, notification must also be provided to
  the Attorney General's Office".
- The duty is triggered when personal information "was not secured (i.e. encrypted)".
- What counts, among the elements listed with a person's name: "Private keys for electronic
  signature"; and on its own: "Username and email address in combination with a password or security
  questions and answers that would permit access to an online account."
- "If the breach involves a resident's login credentials for an email account, the breached entity
  may not provide the breach notification to the resident via that email address."
- Encryption alone may not be enough: "22 states (including Washington) require notification when an
  encryption key or security credential is included in the breach along with the encrypted or secured
  information."

Source: same office, "Data Breach Notifications", https://www.atg.wa.gov/data-breach-notifications
(undated page): notices are filed "in accordance with RCW 19.255 and RCW 42.56.590", and the listed
breach categories include "Unique Private Key (e.g. used to authenticate or sign an electronic
record)".

### 7. The EU GDPR: when it would apply, and its security article

- Territorial scope, Article 3(2), as quoted in the 2026-11-07 finding from EUR-Lex (Regulation (EU)
  2016/679, adopted 27 April 2016; read there on 7 November 2026, because EUR-Lex's robots.txt barred
  automated reading on 14 November 2026): "This Regulation applies to the processing of personal data
  of data subjects who are in the Union by a controller or processor not established in the Union,
  where the processing activities are related to: (a) the offering of goods or services, irrespective
  of whether a payment of the data subject is required, to such data subjects in the Union; or (b) the
  monitoring of their behaviour as far as their behaviour takes place within the Union."
- Recital 23 (same source): "the mere accessibility of the controller's, processor's or an
  intermediary's website in the Union, of an email address or of other contact details, or the use of
  a language generally used in the third country where the controller is established, is
  insufficient to ascertain such intention".
- Security, Article 32, summarised by the European Data Protection Board,
  https://www.edpb.europa.eu/sme/be-compliant/secure-personal-data_en (undated page): "The GDPR
  specifies that data controllers and data processors have to implement appropriate technical and
  organisational measures to ensure a level of security of personal data appropriate to the risk."
  The Article's own text was not read today.

## What is still unknown, and what would settle it

Each of these needs a lawyer who practises US internet and privacy law (cost not researched; a
written question list, this file and the 2026-10-10 finding would keep a consultation short).

- **Is a HumanityOS mail server a service "to the public"** under 2702 and 2511(3)? It decides whether
  the disclosure limits bind it. Free and volunteer-run are not exceptions in the words read.
- **Is a volunteer running one server a "provider"** under 2258A? No official source answers it
  (2026-10-10 finding).
- **What legal process can reach mail sealed on arrival** to a key only the person holds. The texts
  read say what a provider must disclose or preserve "in its possession"; none read says what happens
  when the provider holds only sealed copies, or whether a court can order a change of design. The
  server still sees the envelope (sender and recipient addresses, times, connecting addresses) before
  sealing; what of that it keeps is a design choice that decides what it can be asked for.
- **The exact words of RCW 19.255.010 and 19.255.005** (the definitions), including whether a breach
  of sealed mail with no key on the server counts. Settle: read the RCW in a browser (free), then a
  lawyer if needed.
- **Washington's own commercial email law.** CAN-SPAM leaves standing state rules on "falsity or
  deception" (7707(b)). Washington's statute on this was not read (the legislature's site barred
  automated reading); whether it reaches a server that only relays is for a lawyer.
- **Whether 230(d)'s parental-control notice** applies to a free service with no paid "agreement with
  a customer". Settle: a lawyer.
- **Whether the GDPR reaches the project** if it offers addresses to people in the EU. The 2026-11-07
  finding judged it "probably not" for the website as it stands; a mail service people in the EU sign
  up to is closer to "the offering of goods or services". Settle: a lawyer with EU experience, or the EDPB's
  guidelines 3/2018 read against the actual sign-up flow.
- **Laws not researched at all**: Washington's own privacy and interception statutes, CALEA (the
  federal law on intercept capability), the UK and other countries' rules, record-keeping or
  retention rules, and export controls on encryption. Each needs its own finding if the project goes
  ahead.

## Sources you could not reach

- https://app.leg.wa.gov/RCW/default.aspx?cite=19.255.010 (and 19.255.005; and Washington's
  commercial email chapter): barred by robots.txt. Hoped for: the statute's exact words.
- https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32016R0679 : barred by robots.txt
  today. Hoped for: Article 32's exact words, and a fresh read of Article 3.
- https://commission.europa.eu/law/law-topic/data-protection/rules-business-and-organisations/obligations_en :
  HTTP 429. Hoped for: the Commission's own summary of a controller's security duties.
- https://uscode.house.gov/ section pages: 404 while the site runs "in a temporary format" (it says
  the Code there is current through Public Law 119-142, 10 November 2026). The govinfo 2024 edition
  was read instead, so any amendment after that edition was not checked.
- https://www.ftc.gov/business-guidance/resources/can-spam-act-compliance-guide-business : curl was
  answered with a "Page Not Found" page; read through WebFetch instead, so its quotations are not
  script-checked.

## What it means for this project

**Certain (from the words quoted):**
- CAN-SPAM's duties fall on whoever initiates commercial email; "routine conveyance" is not
  initiating. Any commercial mail the project sent itself (a fundraising newsletter, say) would carry
  CAN-SPAM's duties: identification as an advertisement where it is one, an opt-out honoured within
  10 business days, a postal address, no false headers.
- A service that lets people receive email fits the statutory words "electronic communication
  service".
- No statute read requires a provider to scan mail for child sexual abuse material; the reporting
  duty starts at actual knowledge.
- Section 230 protects good-faith spam blocking and does not make the project the publisher of mail
  others write, but it gives no shelter from federal criminal law or the privacy statutes.
- Washington counts private signing keys and email-plus-password as personal information, and wants
  residents told within 30 days of a breach of unencrypted personal information.

**Judgement calls (an AI's reading, for the operator and a lawyer to accept or reject):**
- Option 5 (the app reads the person's existing mailbox; the server never holds their mail) keeps the
  project out of the mail path, and so out of most of the above, provided the mailbox password or
  token stays on the person's device. Kept on a server, it would be exactly the "Username and email
  address in combination with a password" that the Attorney General lists as personal information.
- Option 2 (receive only, sealed to the person on arrival) puts the project inside the Stored
  Communications Act's words, but sealing on arrival keeps the stored contents unreadable to the
  server, which narrows what a breach exposes and what the server can be asked for. The envelope the
  server sees before sealing is the part to minimise and to state in the privacy notice.
- Spam filtering before sealing looks like the "necessary incident to the rendition of his service"
  the Wiretap Act allows; a lawyer should confirm it for automated filtering.
- Before HumanityOS receives anyone's email: a lawyer's reading of the four open questions above, a
  written policy for answering legal process and preservation requests, and terms of service that say
  what the server keeps.

**This is not legal advice.** It records what these texts and official pages said on 14 November
2026. Laws and their official descriptions change; re-read them before relying on this file.
