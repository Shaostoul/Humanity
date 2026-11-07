# What a privacy notice for united-humanity.us must say, and which laws reach it

**Researched 2026-11-07** (sources read between about 13:50 and 14:11 Pacific
time, 21:50 to 22:11 UTC, on 7 November 2026). Everything below was read that
day. If you are reading this later, compare each source's own date with any
change since.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such, and a
lawyer could disagree with any of it.

## The question

What must a privacy notice on https://united-humanity.us say, and does each of
the EU GDPR, the UK GDPR, California's CCPA/CPRA and CalOPPA, Washington's My
Health My Data Act and the US COPPA reach a free, non-commercial, open-source
(CC0) project run by one person in Washington State, with no ads, no sale of
data, no analytics and no sign-up, open to anyone worldwide?

## Short answer

- **No law read clearly reaches the site as it stands, but two might, and a
  posted notice costs nothing**, so the project posts one written to satisfy
  the strictest list (GDPR Article 13) and CalOPPA's contents list together.
- **EU GDPR: probably not, not certain.** Being free does not help (Article
  3(2)(a) applies "irrespective of whether a payment ... is required"), and the
  household exemption does not cover a public website (Lindqvist, para 47). The
  site escapes only if it does not *target* people in the EU: an English-only
  site on a .us domain, with no euro prices, no EU marketing and no mention of
  EU users, matches the EDPB's own example of a service held not to target
  (Guidelines 3/2018, Example 16). Three days of access logs kept for rate
  limiting are probably not "monitoring" (EDPB: not "any online collection").
  **If the GDPR did apply**, an EU representative would probably be required
  (Article 27's exemption needs processing that is "occasional", and the
  EDPB reads that as outside the regular course of activity).
- **UK GDPR: the same tests with "the United Kingdom".** Since 19 June 2026 a
  UK notice must also tell people they may complain to the controller itself
  (Data Protection Act 2018, s.164A), and the regulator is now "the
  Commission". The UK age for a child's own consent is 13.
- **CCPA/CPRA: no.** A "business" is one "operated for the profit or financial
  benefit of its shareholders or other owners" and meeting a threshold
  ($26,625,000 revenue since 1 January 2025, or buying, selling or sharing the
  data of 100,000 people, or half its revenue from selling or sharing). None
  is met; the agency's FAQ says the law "does not generally apply to nonprofit
  organizations".
- **CalOPPA: unclear.** It binds an "operator of a commercial Web site or
  online service"; "commercial" is not defined in any text read. Its contents
  list is short (categories collected, who it is shared with, how to review
  and change it, how changes are announced, the effective date, the Do Not
  Track answer, whether other parties track across sites), so the notice meets
  it anyway.
- **Washington My Health My Data Act: no, as long as no health data reaches
  the server.** The code was checked (section 7): the desktop app's body
  measurements live only in memory, the website keeps its medical fields in the
  browser, and neither is synced or exported to the server. If that ever
  changes, the Act asks for a *separate* consumer health data policy linked on
  the homepage, which may "not contain additional information".
- **COPPA: only if the site is "operated for commercial purposes" AND either
  directed to children or has actual knowledge of a child.** It is a general
  audience site and need not ask ages. The notice says it is not meant for
  children under 13 and how a parent gets a child's data erased.
- **Nothing read requires a cookie banner, a consent step, an age gate or a
  data request form for this site as it stands.** The site sets no cookies.

## How each finding is recorded

Each finding gives the source, its own date where it shows one, and a quotation
of the words that carry the meaning, trimmed to those words; nothing inside a
quotation is reworded. Quotations from pages downloaded directly (EUR-Lex,
edpb.europa.eu, legislation.gov.uk, cppa.ca.gov, oag.ca.gov's PDF,
app.leg.wa.gov, atg.wa.gov, govinfo.gov, the eCFR API, the Colorado and Oregon
legislatures) were checked by script against the downloaded text, tags removed
and spaces collapsed; all matched. The FTC's COPPA FAQ refused direct requests
and could be read only through a summarising fetch tool, so its quotations are
marked **not script-checked**. On legislation.gov.uk the site's amendment
markers ("[F1 ...]") are left out; the words are verbatim.

## 1. European Union: the GDPR

Source for EU-1 to EU-6: Regulation (EU) 2016/679, EUR-Lex,
<https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32016R0679>
(adopted 27 April 2016, OJ L 119, 4.5.2016; every Article quotation also
matched the consolidated text CELEX 02016R0679-20160504, which includes the
corrigendum of 23 May 2018).

### EU-1. Who it reaches outside the EU (Article 3(2), Recitals 23 and 24)

> "2. This Regulation applies to the processing of personal data of data
> subjects who are in the Union by a controller or processor not established in
> the Union, where the processing activities are related to: (a) the offering of
> goods or services, irrespective of whether a payment of the data subject is
> required, to such data subjects in the Union; or (b) the monitoring of their
> behaviour as far as their behaviour takes place within the Union."

Recital 23:

> "Whereas the mere accessibility of the controller's, processor's or an
> intermediary's website in the Union, of an email address or of other contact
> details, or the use of a language generally used in the third country where
> the controller is established, is insufficient to ascertain such intention,
> factors such as the use of a language or a currency generally used in one or
> more Member States with the possibility of ordering goods and services in that
> other language, or the mentioning of customers or users who are in the Union,
> may make it apparent that the controller envisages offering goods or services
> to data subjects in the Union."

Recital 24:

> "it should be ascertained whether natural persons are tracked on the internet
> including potential subsequent use of personal data processing techniques
> which consist of profiling a natural person"

### EU-2. IP addresses and keys are personal data (Article 4(1), Recital 30)

> "an identifiable natural person is one who can be identified, directly or
> indirectly, in particular by reference to an identifier such as a name, an
> identification number, location data, an online identifier"

Recital 30:

> "Natural persons may be associated with online identifiers provided by their
> devices, applications, tools and protocols, such as internet protocol
> addresses, cookie identifiers or other identifiers such as radio frequency
> identification tags."

The Court of Justice, *Breyer* (C-582/14, judgment of 19 October 2016,
ECLI:EU:C:2016:779),
<https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:62014CJ0582>,
operative part 1:

> "a dynamic IP address registered by an online media services provider when a
> person accesses a website that the provider makes accessible to the public
> constitutes personal data within the meaning of that provision, in relation to
> that provider, where the latter has the legal means which enable it to
> identify the data subject with additional data which the internet service
> provider has about that person."

And para 60, on keeping a site working as a legitimate interest:

> "The German Federal institutions, which provide online media services, may
> also have a legitimate interest in ensuring, in addition to the specific use
> of their publicly accessible websites, the continued functioning of those
> websites."

### EU-3. The household exemption does not cover a public website (Article 2(2)(c), Recital 18)

> "(c) by a natural person in the course of a purely personal or household
> activity;"

Recital 18 ends:

> "However, this Regulation applies to controllers or processors which provide
> the means for processing personal data for such personal or household
> activities."

*Lindqvist* (C-101/01, judgment of 6 November 2003, on the identically worded
exemption in Directive 95/46),
<https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:62001CJ0101>,
para 47:

> "That exception must therefore be interpreted as relating only to activities
> which are carried out in the course of private or family life of individuals,
> which is clearly not the case with the processing of personal data consisting
> in publication on the internet so that those data are made accessible to an
> indefinite number of people."

### EU-4. What the notice must say, and how (Articles 12(1), 13(1), 13(2), 14)

Article 12(1):

> "in a concise, transparent, intelligible and easily accessible form, using
> clear and plain language, in particular for any information addressed
> specifically to a child."

Article 13(1), when data is collected from the person:

> "(a) the identity and the contact details of the controller and, where
> applicable, of the controller's representative; (b) the contact details of the
> data protection officer, where applicable; (c) the purposes of the processing
> for which the personal data are intended as well as the legal basis for the
> processing; (d) where the processing is based on point (f) of Article 6(1),
> the legitimate interests pursued by the controller or by a third party; (e)
> the recipients or categories of recipients of the personal data, if any; (f)
> where applicable, the fact that the controller intends to transfer personal
> data to a third country"

Article 13(2):

> "(a) the period for which the personal data will be stored, or if that is not
> possible, the criteria used to determine that period; (b) the existence of the
> right to request from the controller access to and rectification or erasure of
> personal data or restriction of processing concerning the data subject or to
> object to processing as well as the right to data portability; (c) where the
> processing is based on point (a) of Article 6(1) or point (a) of Article 9(2),
> the existence of the right to withdraw consent at any time"

> "(d) the right to lodge a complaint with a supervisory authority; (e) whether
> the provision of personal data is a statutory or contractual requirement, or a
> requirement necessary to enter into a contract, as well as whether the data
> subject is obliged to provide the personal data and of the possible
> consequences of failure to provide such data; (f) the existence of automated
> decision-making, including profiling"

Article 14 (data not obtained from the person, such as a signed profile copied
in from a linked server) adds "(d) the categories of personal data concerned;"
and "(f) from which source the personal data originate".

Recital 49, on security as a legitimate interest:

> "The processing of personal data to the extent strictly necessary and
> proportionate for the purposes of ensuring network and information security,"

### EU-5. The EU representative (Article 27)

> "1. Where Article 3(2) applies, the controller or the processor shall
> designate in writing a representative in the Union. 2. The obligation laid
> down in paragraph 1 of this Article shall not apply to: (a) processing which is
> occasional, does not include, on a large scale, processing of special
> categories of data as referred to in Article 9(1) or processing of personal
> data relating to criminal convictions and offences referred to in Article 10,
> and is unlikely to result in a risk to the rights and freedoms of natural
> persons"

### EU-6. A child's consent (Article 8(1))

> "the processing of the personal data of a child shall be lawful where the
> child is at least 16 years old. Where the child is below the age of 16 years,
> such processing shall be lawful only if and to the extent that consent is
> given or authorised by the holder of parental responsibility over the child.
> Member States may provide by law for a lower age for those purposes provided
> that such lower age is not below 13 years."

(This applies only where consent is the legal basis, "Where point (a) of
Article 6(1) applies"; the project's processing rests on running the service
the person asked for and on keeping it safe, not on consent.)

### EU-7. The regulators' reading of targeting (EDPB Guidelines 3/2018)

Source: European Data Protection Board, Guidelines 3/2018 on the territorial
scope of the GDPR (Article 3), "Version 2.0 12 November 2019 Adoption of the
Guidelines after public consultation", "Version 2.1 07 January 2020 Formatting
change",
<https://www.edpb.europa.eu/system/files/documents/files/file1/edpb_guidelines_3_2018_territorial_scope_after_public_consultation_en_1.pdf>.
Spelling slips are the source's own.

Page 15:

> "the provision is aimed at activities that intentionally, rather than
> inadvertently or incidentally, target individuals in the EU."

Page 16, on free services:

> "Whether the activity of a controller or processor not established in the
> Union is to be considered as an offer of a good or a service is not therefore
> dependent whether payment is made in exchange for the goods or services
> provided"

Pages 17 and 18 list the factors, including:

> "The use of a top-level domain name other than that of the third country in
> which the controller or processor is established, for example “.de”, or the
> use of neutral top-level domain names such as “.eu”;"

> "The use of a language or a currency other than that generally used in the
> trader’s country, especially a language or currency of one or more EU Member
> states;"

Page 18:

> "when goods or services are inadvertently or incidentally provided to a person
> on the territory of the Union, the related processing of personal data would
> not fall within the territorial scope of the GDPR."

Page 19, Example 16 (a Swiss university open to anyone with enough German and
English, not advertising to EU students, paid in Swiss francs):

> "Without other factors to indicate the specific targeting of students in EU
> member states, it therefore cannot be established that the processing in
> question relates to the offer of an education service to data subject in the
> Union, and such processing will therefore not be subject to the GDPR
> provisions."

Page 20, on monitoring:

> "The EDPB does not consider that any online collection or analysis of personal
> data of individuals in the EU would automatically count as “monitoring”. It
> will be necessary to consider the controller’s purpose for processing the data
> and, in particular, any subsequent behavioural analysis or profiling
> techniques involving that data."

Page 25, on Article 27's exemption:

> "a processing activity can only be considered as “occasional” if it is not
> carried out regularly, and occurs outside the regular course of business or
> activity of the controller or processor"

A search of the guidelines for "non-commercial", "free of charge",
"non-profit", "charitable" and "household" found no passage about
non-commercial controllers (a negative finding: being non-commercial is not
itself an exit).

## 2. United Kingdom: the UK GDPR and the Data Protection Act 2018

All legislation.gov.uk pages read showed "There are currently no known
outstanding effects".

### UK-1. Scope (UK GDPR Article 3(2))

<https://www.legislation.gov.uk/eur/2016/679/article/3> (versions 27/04/2016,
31/01/2020, 31/12/2020):

> "2. This Regulation applies to the relevant processing of personal data of
> data subjects who are in the United Kingdom by a controller or processor not
> established in the United Kingdom, where the processing activities are related
> to: (a) the offering of goods or services, irrespective of whether a payment of
> the data subject is required, to such data subjects in the United Kingdom; or
> (b) the monitoring of their behaviour as far as their behaviour takes place
> within the United Kingdom."

### UK-2. What a UK notice adds (UK GDPR Article 13, DPA 2018 s.164A)

<https://www.legislation.gov.uk/eur/2016/679/article/13> (amendments dated
05/02/2026, 19/06/2026 and 30/09/2026, from the Data (Use and Access) Act
2025):

> "(ca) the right to make a complaint to the controller under section 164A of
> the 2018 Act; (d) the right to make a complaint to the Commission under section
> 165 of the 2018 Act;"

Annotation on the page: "F4 Art. 13(2)(ca) inserted (19.6.2026) by Data (Use and
Access) Act 2025 (c. 18), s. 142(1), Sch. 10 para. 3(2)".

<https://www.legislation.gov.uk/ukpga/2018/12/section/164A> ("up to date with all
changes known to be in force on or before 07 November 2026"):

> "(2) A controller must facilitate the making of complaints under this section
> by taking steps such as providing a complaint form which can be completed
> electronically and by other means. (3) If a controller receives a complaint
> under this section, the controller must acknowledge receipt of the complaint
> within the period of 30 days beginning when the complaint is received."

### UK-3. The UK representative and the UK consent age (Articles 27 and 8)

<https://www.legislation.gov.uk/eur/2016/679/article/27>:

> "1. Where Article 3(2) applies, the controller or the processor shall
> designate in writing a representative in the United Kingdom."

Its paragraph 2 exemption is worded as the EU's (EU-5).

<https://www.legislation.gov.uk/eur/2016/679/article/8>:

> "shall be lawful where the child is at least 13 years old."

### UK-4. The ICO's fee (unresolved)

The Data Protection (Charges and Information) Regulations 2018,
<https://www.legislation.gov.uk/uksi/2018/480/body>, reg. 2(1):

> "A data controller must comply with the requirements of this regulation unless
> all of the processing of personal data they undertake is exempt processing."

Reg. 2(5)(b) speaks of "the person's principal place of business in the UK",
which reads as though a UK presence is assumed. Whether the fee reaches a
controller caught only by UK Article 3(2) was not settled: the ICO's own fee
page refused to load (see the unreachable list).

## 3. California

### CA-1. CCPA/CPRA: who is a "business" (Civil Code 1798.140(d))

Source: the California Privacy Protection Agency's compiled statute,
<https://cppa.ca.gov/pdf/20260101_ccpa_statute.pdf>, headed "effective
01/01/2026" and "posted to cppa.ca.gov December 2025" (the legislature's site
refused; see the unreachable list).

> "(1) A sole proprietorship, partnership, limited liability company,
> corporation, association, or other legal entity that is organized or operated
> for the profit or financial benefit of its shareholders or other owners, that
> collects consumers' personal information, or on the behalf of which such
> information is collected and that alone, or jointly with others, determines
> the purposes and means of the processing of consumers' personal information,
> that does business in the State of California, and that satisfies one or more
> of the following thresholds:"

> "(B) Alone or in combination, annually buys, sells, or shares the personal
> information of 100,000 or more consumers or households."

> "(C) Derives 50 percent or more of its annual revenues from selling or sharing
> consumers' personal information."

"Share" (1798.140(ah)) means passing data on "for cross-context behavioral
advertising, whether or not for monetary or other valuable consideration".

The current revenue threshold, <https://cppa.ca.gov/regulations/cpi_adjustment.html>
("Updated Monetary Thresholds in CCPA Effective 1/1/2025; posted 12/17/2024"):

> "Civil Code § 1798.140(d)(1)(A): Annual gross revenue amount within the
> definition of “business” $26,625,000 $25,000,000"

(the new amount, then the previous one). The agency's FAQ,
<https://cppa.ca.gov/faq.html> (no page date):

> "The CCPA does not generally apply to nonprofit organizations or government
> agencies."

### CA-2. CalOPPA (Business and Professions Code 22575 to 22577)

Source: the California Attorney General, "Making Your Privacy Practices
Public" (May 2014), whose appendix reproduces 22575 to 22579 as they stood in
2014,
<https://oag.ca.gov/sites/all/files/agweb/pdfs/cybersecurity/making_your_privacy_practices_public.pdf>.

22575(a):

> "An operator of a commercial Web site or online service that collects
> personally identifiable information through the Internet about individual
> consumers residing in California who use or visit its commercial Web site or
> online service shall conspicuously post its privacy policy on its Web site"

> "An operator shall be in violation of this subdivision only if the operator
> fails to post its policy within 30 days after being notified of
> noncompliance."

22575(b)(1) to (6), the contents:

> "Identify the categories of personally identifiable information that the
> operator collects through the Web site or online service about individual
> consumers who use or visit its commercial Web site or online service and the
> categories of third-party persons or entities with whom the operator may share
> that personally identifiable information."

> "(2) If the operator maintains a process for an individual consumer who uses or
> visits its commercial Web site or online service to review and request changes
> to any of his or her personally identifiable information that is collected
> through the Web site or online service, provide a description of that
> process."

> "(3) Describe the process by which the operator notifies consumers who use or
> visit its commercial Web site or online service of material changes to the
> operator's privacy policy for that Web site or online service."

> "(4) Identify its effective date."

> "(5) Disclose how the operator responds to Web browser "do not track" signals
> or other mechanisms that provide consumers the ability to exercise choice
> regarding the collection of personally identifiable information about an
> individual consumer's online activities over time and across third-party Web
> sites or online services, if the operator engages in that collection."

> "(6) Disclose whether other parties may collect personally identifiable
> information about an individual consumer's online activities over time and
> across different Web sites when a consumer uses the operator's Web site or
> service."

The Attorney General's guidance on placement:

> "Use a conspicuous link on your homepage containing the word "privacy.""

The definitions section, 22577 ("operator", "consumer", "conspicuously post"),
appears in the PDF only as images and could not be read; the Attorney General's
summary of its "personally identifiable information" lists name, physical
address, e-mail, telephone, Social Security number and "Any other identifier
that permits the physical or online contacting of a specific individual."

## 4. Washington: the My Health My Data Act (RCW 19.373)

Source: <https://app.leg.wa.gov/RCW/default.aspx?cite=19.373&full=true> (no
"current through" date shown; every section is credited "2023 c 191", so no
later amendment is shown), and the Attorney General's FAQ,
<https://www.atg.wa.gov/protecting-washingtonians-personal-health-data-and-privacy>
(no page date).

19.373.010:

> "(5) "Collect" means to buy, rent, access, retain, receive, acquire, infer,
> derive, or otherwise process consumer health data in any manner."

> "(8)(a) "Consumer health data" means personal information that is linked or
> reasonably linkable to a consumer and that identifies the consumer's past,
> present, or future physical or mental health status."

> "(23) "Regulated entity" means any legal entity that: (a) Conducts business in
> Washington, or produces or provides products or services that are targeted to
> consumers in Washington; and (b) alone or jointly with others, determines the
> purpose and means of collecting, processing, sharing, or selling of consumer
> health data."

19.373.020(1)(a) and (b), the policy duty:

> "a regulated entity and a small business shall maintain a consumer health data
> privacy policy that clearly and conspicuously discloses: (i) The categories of
> consumer health data collected and the purpose for which the data is
> collected, including how the data will be used;"

> "(b) A regulated entity and a small business shall prominently publish a link
> to its consumer health data privacy policy on its homepage."

The Attorney General's FAQ:

> "The Consumer Health Privacy Policy must be a separate and distinct link on the
> regulated entity’s homepage and may not contain additional information not
> required under the My Health My Data Act."

> "For example, while information about the purchase of toilet paper or
> deodorant is not consumer health data, an app that tracks someone’s digestion
> or perspiration is collecting consumer health data."

The exemptions section, 19.373.100, was read in full: it has no nonprofit
exemption.

## 5. United States: COPPA

Sources: 15 U.S.C. 6501 and 6502, govinfo, "United States Code, 2024 Edition",
<https://www.govinfo.gov/content/pkg/USCODE-2024-title15/html/USCODE-2024-title15-chap91-sec6501.htm>;
the COPPA Rule, 16 CFR Part 312, from the eCFR's official API (title 16 "up to
date as of" 2026-11-05; Part 312's latest amendment 2025-06-23),
<https://www.ecfr.gov/api/versioner/v1/full/2026-11-05/title-16.xml?part=312>.

16 CFR 312.2, "Operator":

> "where such website or online service is operated for commercial purposes
> involving commerce among the several States or with one or more foreign
> nations"

> "This definition does not include any nonprofit entity that would otherwise be
> exempt from coverage under Section 5 of the Federal Trade Commission Act (15
> U.S.C. 45)."

15 U.S.C. 6502(a)(1):

> "It is unlawful for an operator of a website or online service directed to
> children, or any operator that has actual knowledge that it is collecting
> personal information from a child, to collect personal information from a
> child in a manner that violates the regulations prescribed under subsection
> (b)."

16 CFR 312.2, personal information (7):

> "A persistent identifier that can be used to recognize a user over time and
> across different websites or online services. Such persistent identifier
> includes, but is not limited to, a customer number held in a cookie, an
> Internet Protocol (IP) address"

The FTC's COPPA FAQ,
<https://www.ftc.gov/business-guidance/resources/complying-coppa-frequently-asked-questions>
(no date visible; **not script-checked**, read through a summarising tool):
"The Rule does not require operators to ask the age of visitors."

The project's earlier finding,
[`2026-10-10-childrens-online-safety-rules.md`](2026-10-10-childrens-online-safety-rules.md),
reached the same place on "commercial purposes" and is not repeated here.

## 6. Other US states (brief)

Washington has no general consumer privacy statute beyond the My Health My Data
Act: the RCW Title 19 chapter list (<https://app.leg.wa.gov/RCW/default.aspx?Cite=19>)
shows "19.255 Personal information — Notice of security breaches." and "19.373
Washington my health my data act." among its privacy chapters, and the Attorney
General's Data Privacy Hub (<https://www.atg.wa.gov/data-privacy>) lists only the
two.

The comprehensive state laws read start at 100,000 residents of that state a
year. Oregon, ORS 646A.572(1)(a), 2025 Edition
(<https://www.oregonlegislature.gov/bills_laws/ors/ors646A.html>):

> "apply to any person that conducts business in this state, or that provides
> products or services to residents of this state, and that during a calendar
> year, controls or processes: (A) The personal data of 100,000 or more
> consumers"

Colorado's act as signed (SB21-190, effective 1 July 2023,
<https://content.leg.colorado.gov/sites/default/files/2021a_190_signed.pdf>;
later amendments not checked) uses "ONE HUNDRED THOUSAND CONSUMERS OR MORE
DURING A CALENDAR YEAR".

## 7. What the server actually keeps (from the code, 2026-11-07)

The legal questions above turn on facts, so the facts were read from the code
and the live server's configuration (read only), not from design documents.
The full inventory, with file and line for each claim, is
[`docs/design/privacy-notice.md`](../../design/privacy-notice.md); the points
the findings depend on:

- **Access logs:** nginx's default "combined" format (IP address, time, the
  address asked for, status, size, referring page, browser name) in
  `/var/log/nginx/`, rotated daily with two old files kept
  (`/etc/logrotate.d/nginx`: `daily`, `rotate 2`), so about three days.
  fail2ban's own log keeps the addresses it acted on for about five weeks.
- **No IP address in the relay's database.** The relay's log names an address
  only when a limit is hit.
- **No cookies, no analytics, no advertising, nothing sold.**
- **No health data reaches the server.** The desktop's "Body & Measurements"
  fields live only in memory (`src/gui/pages/profile.rs`); the website's
  private profile (date of birth, allergies, medications) stays in the browser
  (`humanity_profile_full_v1`) and is on neither the server sync list nor the
  device-to-device list. Vitals and food in the game are the game character's.
- **Other parties the browser contacts:** every page asks GitHub for the newest
  version number three seconds after loading; the chat loads emoji pictures
  from cdn.jsdelivr.net; the wallet page asks a Solana server and CoinGecko.
  None of them tracks across sites on the project's behalf, but each sees the
  visitor's IP address (relevant to CalOPPA (b)(6) and Article 13(1)(e)).

## Where the sources pull in different directions

- **GDPR reach.** Article 3(2)(a)'s words ("irrespective of whether a payment")
  and the EDPB's refusal to make payment decisive pull toward coverage; Recital
  23 and Example 16 pull away for a site like this one. The project weighs the
  EDPB's worked example most, because it is the regulators' own reading of
  facts closest to these. *Judgement.*
- **My Health My Data and on-device data.** The Attorney General's "an app that
  tracks someone’s digestion or perspiration is collecting consumer health
  data" does not distinguish data kept only on the person's device, while the
  Act's "collect" ("buy, rent, access, retain, receive") reads as the entity
  getting it. The project reads the Act as not reached while nothing leaves the
  device. *Judgement; a lawyer could disagree.*

## What is still unknown, and what would settle it

1. **Whether the GDPR or UK GDPR reaches the site.** Settled only by a
   regulator or a court; a short opinion from an EU or UK data protection
   lawyer would give a professional reading (a few hundred to a few thousand
   dollars; possibly free through a law school clinic or a digital-rights
   group). The facts that would tip it toward coverage, so the project knows
   when to ask: translations into EU languages (the website is already prepared
   for translation), donations in euros or pounds, outreach aimed at EU or UK
   audiences, or naming EU or UK users.
2. **What "commercial" means in CalOPPA, and the 22577 definitions.** Reading
   22577 on leginfo.legislature.ca.gov in a browser would settle the
   definitions; the meaning of "commercial" for a donation-funded project would
   need a California lawyer. The notice meets CalOPPA's list either way.
3. **Whether the UK fee applies to a controller outside the UK.** The ICO's fee
   page in a browser, or an email to the ICO, would settle it (free).
4. **Whether a server-to-server copy counts as a "transfer" under GDPR Chapter
   V** (linked servers in other countries). The EDPB's Guidelines 05/2021 were
   not read.
5. **Whether an individual is a "legal entity" under RCW 19.373.010(23).**
   Matters only if health data ever reaches the server.

## Sources that could not be reached

- <https://leginfo.legislature.ca.gov/faces/codes_displaySection.xhtml?lawCode=CIV&sectionNum=1798.140>:
  HTTP 403, a Cloudflare challenge. Wanted the official CCPA text (the agency's
  compiled copy was used).
- <https://leginfo.legislature.ca.gov/faces/codes_displayText.xhtml?lawCode=BPC&division=8.&title=&part=&chapter=22.&article=>:
  403 to a script and to the fetch tool. Wanted the current 22575 to 22577,
  above all 22577's definitions, and any amendment after 2014.
- The 22576 subdivisions and 22577 in the Attorney General's PDF: images only.
- <https://oag.ca.gov/privacy/privacy-laws>: "Access Denied" (403). Wanted the
  Attorney General's CalOPPA summary.
- <https://www.ftc.gov/business-guidance/resources/complying-coppa-frequently-asked-questions>:
  403 to a script ("resembles an abusive automated request"); read only
  through the summarising tool, so not script-checked and undated.
- <https://www.ecfr.gov/current/title-16/chapter-I/subchapter-C/part-312>: a
  CAPTCHA; the eCFR API the block page points to was used instead.
- <https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/individual-rights/individual-rights/right-to-be-informed/>,
  <https://ico.org.uk/> and <https://ico.org.uk/for-organisations/data-protection-fee/>:
  403 with a Cloudflare page to a script and to the fetch tool. Wanted the
  ICO's privacy notice checklist, anything on controllers outside the UK, and
  the fee's reach.
- <https://curia.europa.eu/juris/document/document.jsf?docid=184668&doclang=EN>:
  a JavaScript-only page; EUR-Lex's copy of *Breyer* was used.
- <https://coag.gov/resources/colorado-privacy-act/>: an empty response. Wanted
  the Colorado Attorney General's statement on nonprofits and the current
  statute.

## What it means for this project

### Certain (the sources say so)

- Being free and donation-funded is not, by itself, a way out of the GDPR
  (Article 3(2)(a); EDPB p. 16), and a public website is not "household"
  processing (Recital 18; *Lindqvist* para 47).
- IP addresses and similar identifiers are personal data under the GDPR
  (Recital 30; *Breyer*) and under the COPPA Rule (312.2 (7)).
- The CCPA's "business" needs a profit purpose and a threshold; the site meets
  neither.
- If CalOPPA applies, the policy must give the six items of 22575(b) and be
  linked conspicuously with the word "privacy".
- If the GDPR applies, Article 13(1) and (2) list what the notice must say, and
  Article 12(1) says it must be in "clear and plain language".
- The My Health My Data Act's policy duty falls on whoever collects consumer
  health data, and its policy must be a separate link with nothing else in it.
- COPPA's duties attach only to a service directed to children or on actual
  knowledge; a general audience site need not ask ages.

### Judgement calls (the project's own reading; a lawyer could disagree)

- Post one plain notice at `/privacy` that answers every item of GDPR Article
  13(1) and (2), UK Article 13(2)(ca), and CalOPPA 22575(b), whether or not each
  law applies. It costs nothing, it is the honest thing to tell people, and it
  stays true if the site later falls under one of them.
- Rest the processing on running the service people ask for and on keeping it
  safe (GDPR Article 6(1)(b) and (f); Recital 49; *Breyer* para 60), not on
  consent, so no consent step or banner is needed; there are no cookies to ask
  about.
- Say plainly that the site is not meant for children under 13, and how a
  parent can have a child's data erased (COPPA's actual-knowledge rule, the
  EU and UK consent ages).
- Do not appoint an EU or UK representative now. Revisit if any of the tipping
  facts in "What is still unknown" item 1 happens.
- Keep health data off the server. If a feature ever syncs body, vitals,
  fitness or diet data through the server, a separate consumer health data
  policy goes on the homepage first.

**Again: this is not legal advice, and nobody who wrote it is a lawyer.**
