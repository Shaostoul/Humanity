# Keeping the project alive and free: legal homes, the DMCA address, and what survives a death

**Researched 2026-10-12** (sources read between about 18:00 and 18:35 Pacific
time on 12 October 2026). Everything below was read on that day. IRS fees and
processing times, benefits rules, fiscal sponsors' terms and state fees change
often; if you are reading this later, check each source's own date against
anything that has changed since.

A fuller version with notes specific to the project's founder is kept privately.

**This is a reading of public sources by an AI working on the project. It is
not legal, tax, benefits or financial advice, and nobody who wrote it is a
lawyer, an accountant or a benefits counsellor.** Where it says what the project
should do, that is the project's own reading, marked as such, and a
professional could disagree with any of it.

This continues
[`2026-10-12-media-item-licences.md`](2026-10-12-media-item-licences.md), which
found that "HumanityOS is not a legal entity" and left open who receives a
permission given "to HumanityOS" and how it passes to later maintainers. It also
relies on [`2026-10-04-sponsor-a-can-tax-status.md`](2026-10-04-sponsor-a-can-tax-status.md)
for the fact that gifts to the maintainer are gifts to a person.

## The question

What legal arrangement lets HumanityOS (its domains, servers, donations and the
permissions people grant it) survive its founder's death and stay free forever,
and how can its DMCA designated agent registration avoid publishing a home
address?

The situation, as the operator gave it on 2026-10-12: a project run by one person
in the United States from a home address, code released under CC0, donations
accepted, and two community servers run from home. Where a state's law matters,
Washington's was read (and New York's once, for contrast).

## Short answer (a reading, not advice)

**Recommended path, in order: a will and account successors now, a fiscal
sponsor next, the project's own nonprofit corporation later.**

1. **Now, at little or no cost.** A will plus a written succession letter naming
   who takes over; a GitHub account successor (GitHub offers one); the
   repository moved from the personal account into a GitHub organization with
   at least two owners; a second trusted person who can reach the domain
   registrar. The code is safe already: CC0's waiver is made "to the detriment
   of Affirmer's heirs and successors", so nobody can take back what has been
   released.
2. **Then a fiscal sponsor that holds the money, the domains and any trademark
   in a 501(c)(3)'s name and has a written rule for replacing the project's
   lead.** Software in the Public Interest (SPI) fits best on its published
   terms: "There is no cost to become or remain an associated project", "5% of
   the donated amount is deducted for SPI overhead", the project names a
   liaison and how "Successors will be appointed", and if the liaison is
   missing two years the assets go to a similar charity. Open Source Collective
   is the quicker fallback (10%, a 501(c)(6), so gifts are very likely not
   tax-deductible; it needs an organization repository). Software Freedom
   Conservancy requires an OSI-approved licence, and CC0 is not one. HCB takes
   only teen-led groups. Open Collective Foundation dissolved in 2024, which is
   the risk any sponsor carries.
3. **Later, a Washington nonprofit corporation with 501(c)(3) status** is the
   strongest long-term home: it outlives everyone, and its assets "must be
   permanently dedicated to an exempt purpose". But a Washington corporation
   that seeks public-charity status must have three or more directors, and one
   fiscal sponsor reports that the IRS "generally does not consider open source
   software a public benefit solely because it's free and open source
   software" (the IRS's own refusal letters could not be read); the charitable
   purpose (education, poverty relief) would need careful drafting with a
   lawyer.
4. **Not an LLC** (it dissolves 90 days after its last member's death unless
   heirs admit a member, and the IRS recognizes a 501(c)(3) LLC only if every
   member is itself a 501(c)(3) or a government) and **not an unincorporated
   association** (the IRS needs at least two members who signed a written
   document, and Washington has no statute for one in Title 24).

**Money and a person's benefits.** Pay from the project, donations given to a
person directly and project money held in a person's own name can count as that
person's income for means-tested benefits; donations held by a fiscal sponsor
generally do not. Anyone receiving benefits should ask their benefits agency in
writing first (section 2).

**The DMCA address.** The service provider must publish its "full legal name and
physical street address"; a post office box only with a written waiver from the
Register of Copyrights, given "in exceptional circumstances (e.g., where there is
a demonstrable threat to an individual's personal safety or security". The
designated agent, by contrast, may use a post office box or a third-party
takedown service. **The sources are silent** on commercial registered-agent
addresses, virtual offices and coworking addresses. The clean route is to make
an entity (the sponsor, or the project's own corporation) the service provider,
so that its address is the one published. The same goes for the domain: .us
domains do not allow privacy services, so a person who registers one has their
address on the public record, and an entity registrant takes it off (3.5).

**What survives a death.** Public licences (CC0, CC BY) need no recipient and
survive anyone. Anything in the founder's own name passes by will or by the
state's intestacy rules, which means probate. A permission granted to him
personally may not pass to anyone at all: the Ninth Circuit (which includes
Washington) found "authority to support the proposition that such licenses are
not transferable as a matter of law". Only an entity carries domains, money and
permissions past a death without interruption.

## How each finding is recorded

For each finding: the source, its own date where it shows one, and a quotation
of the words that carry the meaning. Long passages are trimmed to the words that
matter; nothing inside a quotation is reworded, and spelling errors in the
sources are kept. Every block quotation below was checked by script against the
text read on the day.

- **IRS** pages, the Form 1023 and 1023-EZ instructions, Publications 525, 526
  and 557, Revenue Procedure 2026-5 and Notice 2021-56 were fetched from
  irs.gov. Each page's "Page Last Reviewed or Updated" date is given.
- **Copyright Office**: the DMCA directory FAQ, 37 CFR 201.38 on the eCFR (title
  37 "up to date as of 2026-10-08"; the section last amended 2020-02-27), and
  the Federal Register notices behind the rule.
- **Washington**: the Revised Code (RCW) and Administrative Code (WAC) on
  app.leg.wa.gov. One other state (New York) was read to show how states
  differ.
- **Fiscal sponsors**: each sponsor's own published pages. These are **their
  terms, not law**, and can change.
- **Domain records**: the .us registry's own policy pages, and registry lookups
  made on 2026-10-12. No registrant's details are recorded here.

---

## 1. The options

### 1.1 A nonprofit corporation with 501(c)(3) recognition

**What 501(c)(3) requires.** IRS, *Exemption requirements: 501(c)(3)
organizations*,
https://www.irs.gov/charities-non-profits/charitable-organizations/exemption-requirements-501c3-organizations
("Page Last Reviewed or Updated: 28-Jun-2026"):
> To be tax-exempt under section 501(c)(3) of the Internal Revenue Code, an organization must be organized and operated exclusively for exempt purposes set forth in section 501(c)(3), and none of its earnings may inure to any private shareholder or individual.

IRS, *Inurement/private benefit: Charitable organizations*,
https://www.irs.gov/charities-non-profits/charitable-organizations/inurement-private-benefit-charitable-organizations
("Page Last Reviewed or Updated: 09-Oct-2026"), names the founder in so many
words:
> A section 501(c)(3) organization must not be organized or operated for the benefit of private interests, such as the creator or the creator's family, shareholders of the organization, other designated individuals, or persons controlled directly or indirectly by such private interests.

A founder can still be paid for work. Publication 557 (https://www.irs.gov/publications/p557,
"Revised: January 2025", page reviewed 30-Apr-2026) gives sample articles that
say the corporation is "authorized and empowered to pay reasonable compensation
for services rendered", and defines it:
> Reasonable compensation is the value that would ordinarily be paid for like services by like enterprises under like circumstances.

**Why this is the "free forever" structure.** Publication 557:
> Assets of an organization must be permanently dedicated to an exempt purpose. This means that should an organization dissolve, its assets must be distributed for an exempt purpose described in this chapter, or to the Federal Government or to a state or local government for a public purpose. If the assets could be distributed to members or private individuals or for any other purpose, the organizational test isn't met.

**Form 1023-EZ: who may use it.** *Instructions for Form 1023-EZ*,
https://www.irs.gov/instructions/i1023ez ("Revised: 01/2025"). The eligibility
worksheet asks, among 30-odd questions:
> Do you project that your annual gross receipts will exceed $50,000 in any of the next 3 years?

> Have your annual gross receipts exceeded $50,000 in any of the past 3 years?

> Do you have total assets the fair market value of which is in excess of $250,000?

> If you answer “Yes” to any of the worksheet questions, you are not eligible to apply for exemption under section 501(c)(3) using Form 1023-EZ. You must apply on Form 1023.

The same instructions add two negative findings:
> Only certain corporations, unincorporated associations, and trusts are eligible for tax-exempt status under section 501(c)(3). Sole proprietorships, partnerships, and loosely affiliated groups of individuals are not eligible.

> Even though limited liability companies (LLCs) are eligible to receive exemption under section 501(c)(3), they are not eligible to apply for exemption using this form.

And a small-organization note: the types that "may be considered tax exempt
under section 501(c)(3) even if they do not file" include "Any organization
that has gross receipts in each taxable year of normally not more than $5,000."
(A corporation that relies on this has no IRS letter to show donors.)

**Fees.** Revenue Procedure 2026-5, Appendix A, in Internal Revenue Bulletin
2026-01, https://www.irs.gov/irb/2026-01_IRB (linked from the IRS user-fee page,
"Page Last Reviewed or Updated: 28-Jun-2026"):

| Issue (Appendix A, as written) | User fee |
|---|---|
| "(1) Application for recognition of exemption under § 501(c)(3) submitted on Form 1023-EZ" | "$275" |
| "(2) All other applications for recognition of exemption under § 501 ..." | "$600" |

**Timeline.** IRS, *Where's my application for tax-exempt status?*,
https://www.irs.gov/charities-non-profits/charitable-organizations/wheres-my-application-for-tax-exempt-status
("Page Last Reviewed or Updated: 28-Jun-2026"):
> We are currently reviewing Form 1023-EZ applications submitted before February 24, 2026. We issue 80% of Form 1023-EZ application determinations within 22 days.

> If your application requires more information or further review, we'll contact you by phone or by mail. We issue 80% of application determinations requiring further review within 120 days.

> Your application is being reviewed. We issue 80% of Form 1023 application determinations within 191 days.

and "Form 1023-EZ can’t be expedited."

**Keeping it.** Small charities file a yearly e-Postcard (IRS 990-N page,
"Page Last Reviewed or Updated: 09-Oct-2026": "Small tax-exempt organizations
generally are eligible to file Form 990-N to satisfy their annual reporting
requirement if their annual gross receipts are normally $50,000 or less."). The
1023-EZ instructions: "Failure to file a required return or notice for 3
consecutive years will result in auto-revocation of your tax-exempt status."

**A warning about open-source projects and 501(c)(3).** Open Source Collective,
*Who and What is OSC?*, https://docs.oscollective.org/about-osc/what-is-osc
(GitBook page; read 2026-10-12). This is a fiscal sponsor's reading, not the
IRS's:
> The IRS generally does not consider open source software a public benefit solely because it's free and open source software (FOSS). This means many open source projects may not qualify for 501(c)(3) status, which is reserved for organizations with charitable, educational, or similar public benefit.

The IRS's own denial letters were found but could not be read (they are scanned
images with no text; see "Sources that could not be reached"). Search results
summarizing several of them (2021 to 2026) describe the IRS treating software
licensed for "any person for any purpose" as a non-exempt purpose; that summary
is secondary and is not relied on here.

**Washington law (RCW 24.03A, the Washington Nonprofit Corporation Act),**
read at https://app.leg.wa.gov/RCW/default.aspx?cite=24.03A&full=true:

- Board size, RCW 24.03A.505 (2021 c 176 s 2404):
  > (1) A board of directors shall consist of one or more directors, with the number specified in or fixed in accordance with the articles or bylaws.

  > (2) The board of directors of any corporation shall consist of three or more directors if:

  > (a) The internal revenue service has determined the corporation to be a public charity described in section 509(a)(1) through (4) of the internal revenue code;

  (b) and (c) extend this to a corporation that "has applied" for that
  determination or classification. Subsection (4) allows a temporary shortfall
  after "the death, incapacity, resignation, or removal of a director",
  "provided that the entity, body, or person with the power to elect or appoint
  directors makes reasonable and prompt efforts to elect or appoint additional
  directors."
- What the public filing contains, RCW 24.03A.100(1): among others "(b) The name
  and address of the corporation's initial registered agent", "(e) The number of
  directors constituting the initial board of directors, and the names and
  mailing addresses of the persons who are to serve as the initial directors"
  and "(g) The distribution of assets upon dissolution".
- A registered agent is required, RCW 24.03A.110: "(1) Each nonprofit
  corporation shall designate and maintain a registered agent in this state."
  RCW 23.95.415(1) lets that be "The name of the entity's commercial registered
  agent", or a noncommercial agent with a name and address.
- The yearly report, RCW 23.95.255(2) (last amended 2026 c 80 s 4), must state
  "(b) The name and street and mailing addresses of the entity's registered
  agent in this state;", "(c) The street and mailing addresses of the entity's
  principal office;" and "(e) The names of the entity's governors;". **So a
  Washington entity files a street address for its principal office.**
- Fees, WAC 434-112-085(8) (filed 8/31/23, effective 10/1/23), for nonprofits:
  "Public organic record" (the articles) "Thirty dollars, plus the Charitable
  Asset Protection Account fee"; "Annual report" "Ten dollars, plus the
  Charitable Asset Protection Account fee". RCW 24.03A.960(2)(b):
  > The charitable asset protection fee is fifty dollars per year, reduced to ten dollars if the corporation certifies that its total gross revenue in the most recent fiscal year was less than five hundred thousand dollars.

  That makes $40 to form and $20 a year for a small corporation.
- Fundraising registration, RCW 19.09.081 (2011 c 199 s 3): the charity
  registration requirements do not apply to
  > (1) Any charitable organization raising less than fifty thousand dollars in any accounting year when all the activities of the organization, including all fund-raising activities, are carried on by persons who are unpaid for their services and no part of the charitable organization's assets or income inures to the benefit of or is paid to any officer, director, member, or trustee of the organization, other than as part of a charitable class benefited by the charitable organization.

  Paying the founder would end that exemption.

**Other states differ.** New York's Not-for-Profit Corporation Law § 702,
https://www.nysenate.gov/legislation/laws/NPC/702 (undated page; read in a
browser 2026-10-12):
> (a) The number of directors constituting the entire board shall be not less than three.

Washington allows one director (three once public-charity status is sought);
New York always needs three. Every state sets its own fees, board rules and
address filings; only Washington and New York were read.

### 1.2 An unincorporated nonprofit association

- **The IRS needs two people.** Instructions for Form 1023-EZ:
  > An “unincorporated association” formed under state law must have at least two members who have signed a written document for a specifically defined purpose.
- **Washington has no statute for one in its nonprofit title.** The chapter list
  of RCW Title 24 (https://app.leg.wa.gov/RCW/default.aspx?cite=24, read
  2026-10-12) shows chapters for nonprofit corporations, miscellaneous and
  mutual corporations, corporations sole, fraternal societies and others, and
  none for unincorporated nonprofit associations. Whether another title has one
  was not searched. Some states have adopted a uniform act that makes such an
  association a legal entity able to hold property; which states, today, was
  not confirmed from a primary source.
- **Negative finding:** a one-person project cannot be an unincorporated
  association for IRS purposes.

### 1.3 Fiscal sponsorship by an existing nonprofit

A fiscal sponsor is an existing charity that takes the project in, receives its
donations, holds its assets and pays its bills. Software Freedom Conservancy's
own definition (https://sfconservancy.org/projects/apply/, undated page, read
2026-10-12):
> In this context, a fiscal sponsor is a non-profit organization that, rather than fund a project directly, provides the required infrastructure and facilitates the project's ability to raise its own funds.

**Software Freedom Conservancy (SFC)**, a 501(c)(3). Same page:
> Being a FOSS project under an OSI-approved and DFSG-free license is mandatory, but not the only criteria.

> New Conservancy members are required to pay 10% of their revenue that Conservancy processes to Conservancy's general fund, which primarily is used to pay staff.

> We do not move rapidly to accept new projects, as we have found that consideration of joining or forming a non-profit organization for your project is best done with careful consideration over a period of many months rather than rapidly.

On assets:
> Conservancy holds assets on behalf of its member projects and manages and disburses those assets in accordance with the wishes of the project's leadership, as long as those wishes are consistent with non-profit rules, requirements, and Conservancy's mission.

On leaving: projects may leave "with a few months' notice", but assets go only
to another nonprofit, "meaning the assets cannot be transferred to an
individual or a for-profit entity." It also points small projects to Software
in the Public Interest, "which offers fewer services than Conservancy, but only
requires 5% of gross revenue."

**Software in the Public Interest (SPI)**, a 501(c)(3).
https://www.spi-inc.org/projects/associated-project-howto/ (no date on the page;
server date 2026-10-12):
> Free and open source projects that are deemed to be genuine, substantial and significant may be eligible for associated project status.

> There is no cost to become or remain an associated project.

> 5% of the donated amount is deducted for SPI overhead. The remaining money is held on behalf of the project.

The board's template resolution names the person it listens to and how that
person is replaced:
> 4. [Liaison] is recognised by SPI as the authoritative decision maker and SPI liaison for [project]. Successors will be appointed [describe method of selection or election].

> 5. If the last known SPI liaison for [project] is missing in action, and no new SPI liaison is appointed for two consecutive years the associated project is deemed defunct.

Point 6 then moves a defunct project's assets to "one or more other SPI
associated projects or 501(c)(3) charities which can accept the project's
assets and use them for a similar purpose". And:
> For most projects, it is simply one of the project founders who volunteers and appoints their own successor.

https://www.spi-inc.org/projects/services/ (no date on the page):
> Associated projects may transfer intangible assets such as domain names, trademarks and licenses to SPI for protection.

> Once held by SPI, intangible assets may only be transferred to another 501(c)3 US non-profit with a compatible purpose.

and "Substantial equipment, software or other assets valued at over $300 which
an associated project purchases with SPI funds are owned by SPI."

**Open Source Collective (OSC)**, a 501(c)(6). Eligibility,
https://docs.oscollective.org/getting-started/acceptance-criteria ("Last
updated 3 months ago" on 2026-10-12): the licence must meet one of several lists
("Open Source Initiative Approved License List", "Free Software Foundation
License List", "Debian Free Software Guidelines", "Fedora Software License
List" and the Open Source Hardware Association's principles), and:
> The project must be hosted under an organizational repository, not a personal account.

> When possible, the project should always include two or more administrators on the Open Collective page.

> A clear decision-making process for how the project operates and for its future, especially in the event of leadership changes.

Fees, https://docs.oscollective.org/welcome-and-introduction-to-osc/fees ("Last
updated 3 months ago"):
> Open Source Collective (OSC) charges a fixed 10% host fee on incoming funds.

Status, *Who and What is OSC?* (above): "We are structured as a 501(c)(6)
organization — a nonprofit designation for entities that promote the shared
interests of a specific field or industry." IRS Publication 526
(https://www.irs.gov/publications/p526, "For use in preparing 2025 Returns")
lists among organizations you cannot deduct gifts to: "Chambers of commerce and
other business leagues or organizations." So, on the project's reading, gifts
through OSC are not tax-deductible (OSC's pages read did not say so either way).

Domains, https://docs.oscollective.org/for-hosted-member-projects/domain-transfers-and-registration
("Last updated 2 months ago"):
> Please note that we will generally host domains for projects with an established relationship with OSC. In most cases, this means the project has been active on the Open Collective platform for at least six months and maintains a sufficient balance before we agree to host its domain.

OSC then will "Maintain full domain ownership". Its intellectual-property page
says the opposite default for everything else: "Per our Fiscal Sponsorship
Agreement, you retain ownership of all your project's intellectual property."

**NumFOCUS**, https://numfocus.org/projects-overview (undated): a fit only for
scientific tools. "Because NumFOCUS occupies itself with scientific tools,
general-purpose projects (IDEs, utilities, programming languages, build
systems, etc.) are typically not a good fit." It also lists "An OSI approved
license". **Not a fit for HumanityOS.**

**HCB (Hack Club)**, a 501(c)(3) charging "7% of revenue"
(https://hackclub.com/fiscal-sponsorship/). Its help article *Who can apply for
fiscal sponsorship?*,
https://help.hcb.hackclub.com/en/articles/15409923-who-can-apply-for-fiscal-sponsorship
(dated "June 7, 2026"):
> HCB is exclusively focused on teen-led organizations, and only accepts new groups that are run by teenagers, to stay aligned with its mission of supporting youth-led initiatives.

**Negative finding: HCB will not take an adult-led project.**

**Code for Science & Society** runs a fiscal sponsorship programme for
public-interest technology, but its programme page returned 404
(https://www.codeforsociety.org/fiscal-sponsorship) and whether it is taking new
projects, and on what terms, was not confirmed from its own pages.

**Open Collective Foundation closed.** Open Collective's statement, *Open
Collective Official Statement - OCF Dissolution*,
https://blog.opencollective.com/open-collective-official-statement-ocf-dissolution/
("28 Feb 2024"):
> It is with sad hearts that we have heard the news about Open Collective Foundation decision to dissolve, effective December 31.

with "Last day to accept funds/receive donations: March 15, 2024", less than
three weeks after the announcement. **A fiscal sponsor is itself mortal**; a
project under one should keep the means to move (its own books, its own list of
donors, its own domain control arrangements) and should prefer a sponsor whose
agreement lets it leave to another charity, which SFC and SPI both say they
allow.

**The licence problem.** HumanityOS's `LICENSE` is "CC0 1.0 Universal". The Open
Source Initiative's FAQ (https://opensource.org/faq, undated, read 2026-10-12):
> At this time, we do not recommend releasing software using the the CC0 public domain dedication.

> CC0 was not explicitly rejected, but the License Review Committee was unable to reach consensus that it should be approved, and Creative Commons eventually withdrew the application.

The FSF's licence list (https://www.gnu.org/licenses/license-list.html,
"$Date: 2026/04/22") files CC0 under "Licenses for Works of Practical Use
besides Software and Documentation" and says:
> If you want to release your non-software work to the public domain, we recommend you use CC0. For works of software it is not recommended, as CC0 has a term expressly stating it does not grant you any patent licenses.

**Negative finding: CC0 code does not meet SFC's stated mandatory licence
criterion as written**, and NumFOCUS's. OSC's lists are broader, and SPI's
criterion is "a genuine Free Software community project", without naming a
list; neither was asked whether CC0 code qualifies (see "Unknown"). Two
public-domain-like licences are OSI-approved: Zero-Clause BSD ("Approved:
October 14, 2015", https://opensource.org/license/0bsd) and MIT-0 ("Approved:
August 14, 2020", https://opensource.org/license/mit-0).

**The repository is in a personal account.** GitHub's API, queried 2026-10-12
(`gh api users/Shaostoul`), reports the account type as `User`, not
`Organization`. OSC's "organizational repository" requirement is not met today;
moving the repository into a free GitHub organization would meet it.

### 1.4 A limited liability company (LLC)

- **Taxed as the owner.** IRS, *Single member limited liability companies*,
  https://www.irs.gov/businesses/small-businesses-self-employed/single-member-limited-liability-companies
  ("Page Last Reviewed or Updated: 27-Jul-2026"):
  > For income tax purposes, an LLC with only one member is treated as an entity disregarded as separate from its owner, unless it files Form 8832 and affirmatively elects to be treated as a corporation.
- **Not a 501(c)(3) with a person as member.** IRS Notice 2021-56,
  https://www.irs.gov/pub/irs-drop/n-21-56.pdf: the IRS will recognize an LLC
  under 501(c)(3) only if its documents include "(1) Provisions requiring that
  each member of the LLC be either (i) an organization described in section
  501(c)(3) and exempt from taxation under section 501(a) or (ii) a governmental
  unit". It cannot use Form 1023-EZ at all (1.1).
- **Washington fees**, WAC 434-112-085(7): "Public organic record" "One hundred
  eighty dollars"; "Annual report" "Seventy dollars".
- **On the member's death**, RCW 25.15.131(1): a person is dissociated as a
  member upon "(a) The member dies"; RCW 25.15.265 (2015 c 188 s 50): an LLC is
  dissolved on
  > (4) Ninety days following an event of dissociation of the last remaining member, unless those having the rights of transferees in the limited liability company under RCW 25.15.131(1) have, by the ninetieth day, voted to admit one or more members, voting as though they were members, and in the manner set forth in RCW 25.15.121(1);
- **Negative finding:** an LLC owned by the founder is his property. It passes
  through his estate, it ends within 90 days of his death unless his heirs act,
  donations to it are not deductible, and it cannot become a charity while he is
  its member. It adds cost without solving the survival problem.

### 1.5 No entity: a will and a written succession plan

- **Copyright passes like other property.** 17 U.S.C. 201(d)(1), US Code 2024
  edition (govinfo.gov):
  > (1) The ownership of a copyright may be transferred in whole or in part by any means of conveyance or by operation of law, and may be bequeathed by will or pass as personal property by the applicable laws of intestate succession.
- **Washington: an online tool beats the will.** RCW 11.120.040(1) (2016 c 140
  s 4, the state's version of the uniform digital-assets act):
  > (1) A user may use an online tool to direct the custodian to disclose to a designated recipient or not to disclose some or all of the user's digital assets, including the content of electronic communications. If the online tool allows the user to modify or delete a direction at all times, a direction regarding disclosure using an online tool overrides a contrary direction by the user in a will, trust, power of attorney, or other record.
- **GitHub has such a tool.** *Maintaining ownership continuity of your personal
  account's repositories*,
  https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/repository-access-and-collaboration/maintaining-ownership-continuity-of-your-personal-accounts-repositories
  (undated): "You can invite someone to manage your user owned repositories if
  you are not able to." Its *Deceased User Policy* (undated) accepts requests
  from "next of kin, a pre-designated successor, or other authorized individual
  (which could include a collaborator or business partner)" and may ask for "a
  copy of the death certificate".
- **The domain registrar wants probate.** The registrar for united-humanity.us is
  NameCheap, Inc. (registry lookup, 2026-10-12). No Namecheap policy page on
  deceased customers was found; its blog, *What Happens to Your Websites When
  You Die?* (Daniel Agnew, "March 9, 2020", read in a browser), says:
  > For your nominee to gain access to your accounts, they will require a death certificate, a grant of probate, and their executor’s ID.

  A blog post is not a policy, and probate takes months, during which a domain
  can lapse.
- **Gifts to a person are not deductible.** Publication 526: "You can’t deduct
  contributions to specific individuals". For his income tax, Publication 525
  (https://www.irs.gov/publications/p525, "For use in preparing 2025 Returns"):
  "In most cases, property you receive as a gift, bequest, or inheritance isn’t
  included in your income." **Tax rules and benefits rules can count gifts
  differently** (section 2).

### 1.6 The options side by side (project's summary of the findings above)

| | Nonprofit corporation + 501(c)(3) | Unincorporated association | Fiscal sponsor | LLC | No entity + will |
|---|---|---|---|---|---|
| **Cost to start (WA)** | $40 state; $275 (1023-EZ) or $600 IRS | Nothing filed | Nothing (SPI, OSC); application effort | $180 state | A will (cost not researched) |
| **Yearly** | $20 state; 990-N; register as a charity if over $50k or anyone is paid | Unclear | 5% (SPI), 7% (HCB, not eligible), 10% (SFC, OSC) of donations | $70 state | None |
| **Effort** | High: bylaws, board, books, filings | Low, but legally vague | Low to medium; months for SFC | Medium | Low |
| **Who controls** | The board (three or more directors in WA once public-charity status is sought) | The members | The sponsor legally; the project's named lead in practice | The owner | The founder, then his estate |
| **On the founder's death** | Continues; remaining directors fill the seat; if none remain, the Attorney General may appoint | Depends on state law and documents | Continues; the sponsor holds assets and follows the agreed succession rule | Dissolves after 90 days unless heirs admit a member | Probate; heirs decide |
| **Holds domains, servers, money, trademarks** | Yes, in its own name | Uncertain without a statute | Yes, in the sponsor's name (SPI, SFC; OSC for domains after 6 months, IP stays with the project) | Yes, but it is his property | Only as his personal property |
| **Holds permissions people grant** | Yes, as grantee | Uncertain | Yes, if granted to the sponsor | Yes, but it dies with him | Possibly not transferable (4.2) |
| **Tax-deductible gifts** | Yes, once recognized | Only if recognized | Yes (SFC, SPI); very likely no (OSC) | No | No |
| **Stays free** | Assets locked to charitable use by law | No lock | Assets only to another charity (SFC, SPI) | No lock | No lock |

---

## 2. Money from the project and a person's own benefits

Rules differ by programme and by state, so this is the project's general
reading, not a finding about any programme or any person. Money the project pays
a person (a salary or a stipend), donations given to a person directly, and
project money held in a person's own name can each count as that person's income
for means-tested benefits; donations held by a fiscal sponsor that the person
neither owns nor controls generally do not. Anyone who receives benefits should
ask their benefits agency, in writing, before money from the project changes
hands.

---

## 3. The DMCA designated agent: whose address, and which kind

### 3.1 What the rule requires of the service provider

37 CFR 201.38(b)(1)(i), eCFR (section last amended 2020-02-27; eCFR "up to
date as of 2026-10-08"):
> (i) The full legal name and physical street address of the service provider. Related or affiliated service providers that are separate legal entities (e.g., corporate parents and subsidiaries) are considered separate service providers, and each must have its own separate designation.

The rule also requires it on the service provider's own website: 201.38(a)(1),
"by making available through its service, including on its website in a
location accessible to the public, and by providing to the Copyright Office".

The Copyright Office's *DMCA Designated Agent Directory Frequently Asked
Questions*, https://www.copyright.gov/dmca-directory/faq.html (undated; server
Last-Modified 2 July 2026):
> A service provider is required to supply its full legal name, physical street address (not a post office box), any alternate names used by the service provider, and the name, organization, physical mail address (street address or post office box), telephone number, and email address of its designated agent.

A one-person service may use one set of details for every role: "if the service
provider is run by a single person who will be managing the service provider’s
account and acting as its designated agent, then it would be okay for the
contact information for the primary contact, service provider, and designated
agent to all be the same."

### 3.2 The agent's address is a different, looser rule

37 CFR 201.38(b)(3) and (4):
> The designated agent may be an individual (e.g., “Jane Doe”), a specific position or title held by an individual (e.g., “Copyright Manager”), a specific department within the service provider's organization or within a third-party entity (e.g., “Copyright Compliance Department”), or a third-party entity generally (e.g., “ACME Takedown Service”).

> (4) The physical mail address (street address or post office box), telephone number, and email address of the agent designated to receive notifications of claimed infringement.

So the **agent** can be a post office box or a paid takedown service; the
**service provider** cannot.

### 3.3 The exact rule for the post office box exception

37 CFR 201.38(b)(1)(ii):
> (ii) A post office box may not be substituted for the street address for the service provider, except in exceptional circumstances (e.g., where there is a demonstrable threat to an individual's personal safety or security, such that it may be dangerous to publicly publish a street address where such individual can be located) and, upon written request by the service provider, the Register of Copyrights determines that the circumstances warrant a waiver of this requirement.

The request goes by email to poboxwaiver@copyright.gov or by signed letter to the
Office of the General Counsel, and must contain "The name of the service
provider; the post office box address that the service provider wishes to use; a
detailed statement providing the reasons supporting the request, with
explanation of the specific threat(s) to an individual's personal safety or
security; and an email address for any responsive correspondence from the
Office. There is no fee associated with making this request." The FAQ adds:
> Please note that this only applies to a service provider’s address. A waiver is not necessary to provide a post office box for a designated agent’s address.

**Negative finding:** a wish for privacy is not enough. The rule's own example is
"a demonstrable threat", and the Federal Register notice (3.4) calls these
"rare situations".

### 3.4 Why the Office wants a street address

The final rule, 81 FR 75695 (Federal Register, 1 November 2016),
https://www.federalregister.gov/documents/2016/11/01/2016-26257/designation-of-agent-to-receive-notification-of-claimed-infringement :
> The rationale for this requirement is that there are circumstances where it is important for a copyright owners to be able to physically locate the service provider (e.g., for accurate identification of the service provider or to serve a legal notice).

Its footnotes:
> Although the Office is requiring a street address for the service provider, the Office declines to adopt RIAA's suggestion to require proof of this address.

> The Office notes that, in rare situations, the requirement to provide a street address could raise safety or security concerns for an individual who is operating the service.

The home-address case was raised and answered against privacy. The record
industry argued, and the notice reports, that where an agent "is an individual
with only a home address, the individual is either the sole owner of the
service provider (in which case he or she must supply his or her physical
address anyway as part of the service provider contact information)". The 2011
proposal, 76 FR 59953 (28 September 2011), gave the same reason: "as it can be
important that copyright owners are able to physically locate the service
provider, e.g., for service of process."

### 3.5 Registered-agent, virtual-office and coworking addresses, and the domain record

**The sources are silent.** Neither 37 CFR 201.38, the FAQ, nor the 2011, 2016 or
2017 Federal Register notices mention a registered agent's address, a virtual
office, a coworking space or a commercial mail receiving agency. What the sources
do say, for weighing:

- **What a "virtual office" usually is, in the Postal Service's terms.** USPS
  Domestic Mail Manual 508, section 1.8.1, https://pe.usps.com/text/dmm300/508.htm
  (undated page, read 2026-10-12):
  > A Commercial Mail Receiving Agency (CMRA) is defined as a business that, in whole or in part, accepts the delivery of U.S. Mail on behalf of another person or entity as a business service.

  > A CMRA must represent its delivery address designation for the intended addressees by the use of “PMB” (private mailbox) or the alternative “#” sign.
- **A registered agent's address is where a corporation is served.** RCW
  24.03A.020(1): "Service upon a nonprofit corporation of any process, notice, or
  demand required or permitted by law may be made by serving the nonprofit
  corporation's registered agent." Serving legal notice is one of the two
  purposes the Copyright Office gave (3.4). An individual has no registered
  agent; only an entity does.
- **An entity is its own service provider.** 201.38(b)(1)(i): separate legal
  entities "each must have its own separate designation", giving the "full legal
  name and physical street address of the service provider". If the service
  provider is a corporation or a sponsor, it is that entity's name and address
  that go in.

**What this means** (project's reading, not the Office's):
- A **virtual office or mail drop** is, in the Postal Service's own words, a
  private mailbox. Listing one as the service provider's "physical street
  address" fits the rule's words poorly and its purpose (to "physically locate"
  the provider) worse. It is the riskiest of the three.
- A **coworking address** where the person actually holds a membership and can
  be found fits the purpose better, but it is still not where the service is
  operated from. Untested.
- A **commercial registered agent's address** fits the purpose (service of legal
  notice) for an **entity** service provider, and is the address a state itself
  uses to reach that entity. It does not help while the service provider is a
  person. Untested.
- The **clean route** is to make an entity the service provider: the sponsor or
  the project's own corporation, with its own address. Whether a fiscal sponsor
  would accept being the service provider for servers run from home is a
  question for the sponsor (see "Unknown").
- One more cost of the personal route: the service provider's "full legal name"
  is published too.

**The domain record raises the same question.** A domain's registrant is listed
in a public record as well, and for a .us domain no privacy service may hide it:

- **The .us registry forbids privacy services.** *usTLD Privacy Services*,
  https://about.us/policies/us-privacy-services-policy ("Updated: 29 August
  2019"):
  > Neither Registrar nor any of its resellers, affiliates, partners and/or contractors shall be permitted to offer anonymous or proxy domain name registration services which prevent the Registry from having and/or displaying the true and accurate data elements contained in Section 3.3 of the usTLD Registrar Accreditation and Registry-Registrar Agreement, for any Registered Name.
- **The registry's 2025 data-request terms point the other way**, *usTLD
  Registration Data Request Terms and Conditions* (© 2025,
  https://about.us/documents/policies/Registration_Data_Policy.pdf):
  > You must have a legitimate interest in the information being requested that is not overridden by the interests, fundamental rights, and privacy of any individual to whom this information relates.

  Whether this means some registrant data is now withheld from public WHOIS was
  not established.
- **An entity can be the registrant.** *usTLD Nexus Requirements Policy* (©
  2021): "Nexus Category 2" is "A United States entity or organization that is
  (i) incorporated within one of the fifty (50) U.S. states", including
  "non-commercial organizations based in the United States".
- **public.guide** (a .guide domain): its registry's RDAP record
  (rdap.identitydigital.services) showed no registrant contact on 2026-10-12.
  The registrar's own record was not checked.
- **What this means** (project's reading): hiding a person's address from the
  DMCA directory alone would not hide it while that person is the registrant of
  a .us domain. Making an entity (a sponsor holding the domain, or the project's
  corporation) the registrant takes a person's address off the domain record.

### 3.6 Washington's Address Confidentiality Program: a negative finding

RCW 40.24.030 (last amended 2024 c 292 s 2) lets the Secretary of State give a
substitute address to a person who swears "that the applicant, or the minor or
incapacitated person on whose behalf the application is made, is a victim of
domestic violence, sexual assault, trafficking, or stalking and that the
applicant fears for the applicant's safety", or to threatened election
officials, criminal justice participants and protected health care providers.
**It is not available to someone who only wants privacy.** Other states run
similar programmes on their own terms; none was read.

---

## 4. Survival

### 4.1 Public licences need no one, and HumanityOS's code already has one

- The repository's `LICENSE` file is "CC0 1.0 Universal" (read 2026-10-12).
- CC0 1.0 legal code, section 2,
  https://creativecommons.org/publicdomain/zero/1.0/legalcode.en (undated; see
  the 2026-10-12 media finding for its dating):
  > Affirmer makes the Waiver for the benefit of each member of the public at large and to the detriment of Affirmer's heirs and successors, fully intending that such Waiver shall not be subject to revocation, rescission, cancellation, termination, or any other legal or equitable action to disrupt the quiet enjoyment of the Work by the public as contemplated by Affirmer's express Statement of Purpose.
- CC BY 4.0 deed, https://creativecommons.org/licenses/by/4.0/deed.en (undated):
  > The licensor cannot revoke these freedoms as long as you follow the license terms.
- **What this means** (project's reading): every release already made stays free
  whatever happens to the founder, his heirs or any entity. What CC0 does
  **not** do is bind future work: a later controller could stop publishing, or
  publish new versions under another licence. Only an entity whose purpose and
  asset lock require free publication (1.1), or a sponsor that requires free
  licences (1.3), keeps future work free. The 2026-10-12 media finding records
  one open question about 17 U.S.C. 203 termination rights that applies here
  too.

### 4.2 Permissions given to him personally may not pass on

*Harris v. Emus Records Corp.*, 734 F.2d 1329 (9th Cir., 29 May 1984), text from
the Caselaw Access Project (static.case.law, read 2026-10-12):
> The transferability of a copyright license appears to be a question of first impression in this circuit. There is, however, authority to support the proposition that such licenses are not transferable as a matter of law.

and, borrowing from patent law, a licence described as "a mere personal one, and
not transferable, and was extinguished with the dissolution of the corporation."
The Ninth Circuit covers Washington. The opinion relied on authority from the
1909 Act; later cases were not read. The 2026-10-12 media finding adds that a
free non-exclusive licence is "revocable absent consideration" (*Lulirama*,
5th Cir. 1997).

**What this means** (project's reading): a permission a member gives "to the
maintainer" may die with him or be impossible to hand to a successor. A
permission given to an **entity** (the sponsor, or the project's corporation),
expressly "and its successors and assigns", outlives any person; so does a
public licence chosen by the member (CC BY, CC0), which needs no grantee at all.

### 4.3 What each arrangement needs in place beforehand

From the findings above:

- **Nonprofit corporation.** At least one other director, and three once
  public-charity status is sought (1.1). Bylaws saying how directors are
  replaced: RCW 24.03A.535(1) lets "a majority of the directors remaining in
  office" fill a vacancy, and (4):
  > (4) If no directors remain in office, and there are no members with the right to elect directors, then the attorney general has the power to appoint one or more directors selected for their interest and ability to carry out the purposes of the corporation, unless the articles or bylaws provide a different method for electing, appointing, or designating at least one director.

  Domains, accounts and bank accounts in the corporation's name, with more than
  one officer able to reach them.
- **Fiscal sponsor.** A named lead and a written successor method (SPI's
  resolution, point 4; SFC's agreement "Representation" section; OSC's "two or
  more administrators"). Domains transferred to the sponsor (SPI, SFC; OSC after
  six months). Money only ever in the sponsor's accounts (section 2).
- **LLC.** An operating agreement naming who is admitted on his death, acted on
  within 90 days (1.4). Not recommended.
- **No entity.** A will naming who inherits each asset and account; GitHub's
  account successor set; registrar access arranged beyond probate (1.5);
  passwords and the relay's secret files reachable by the successor (see
  [`docs/BUS-FACTOR.md`](../../BUS-FACTOR.md), whose "If I disappeared
  tomorrow" checklist is still unfilled).
- **Servers run from home.** Whatever the legal owner, a machine in a private
  home stops when the home's power, internet or occupant changes. A hosted
  server paid for by the entity survives better. (Project's reading; not a
  legal finding.)

---

## Where the sources pull in different directions

**Public WHOIS for .us.** The 2019 privacy-services policy forbids proxies so the
registry can hold and display "true and accurate" data; the 2025 data-request
terms speak of requesters needing a "legitimate interest" that respects "privacy
of any individual". Which of the two describes what a public lookup shows today
was not established from the texts; the project assumes the first until shown
otherwise.

**Licence lists.** SFC requires OSI approval; the OSI declined to approve CC0;
the FSF calls CC0 free but not recommended for software; OSC accepts any of
several lists. Whether a given sponsor will accept CC0 code is therefore that
sponsor's decision, not a rule of law.

## What is still unknown, and what would settle it

- **Whether SPI (or OSC) would accept a CC0-licensed project.** Settles it: an
  email to SPI's board and to OSC's application team; free, days to weeks. If
  not, a lawyer-free option is to add an OSI-approved public-domain-like licence
  (0BSD or MIT-0) alongside CC0, which the project could decide itself; whether
  that satisfies DFSG-based checks was not researched.
- **Whether the IRS would recognize a HumanityOS corporation under 501(c)(3)**,
  given its record with open-source applicants. Settles it: a nonprofit lawyer
  drafting the purposes around education and poverty relief; the IRS's actual
  denial letters (scans) read by a person; then the application ($275 or $600).
- **Whether a registered-agent, coworking or virtual-office address satisfies
  "physical street address"** for a DMCA service provider. Settles it: a written
  question to the Copyright Office's Office of the General Counsel (free; the
  Office takes written requests at the addresses in 37 CFR 201.1), or a lawyer.
- **Who is the "service provider" when a sponsor owns the project but its
  servers are run from home.** Settles it: the sponsor's answer, written into
  the sponsorship agreement; a lawyer if it matters.
- **Namecheap's actual policy on a deceased customer.** Settles it: asking
  Namecheap support. Avoided by moving the domain to an entity.
- **Code for Science & Society's current intake and fees**; **NumFOCUS fees**
  (not needed: not a fit). Settles it: their pages or an email.
- **Commercial registered-agent, virtual office and coworking prices in
  Washington.** Not researched. Settles it: a few providers' price pages.
- **Which states have a statute for unincorporated nonprofit associations.**
  Settles it: the Uniform Law Commission's enactment page (script-driven, not
  read).
- **The cost of a will in Washington** and whether free legal aid offers one.
  Not researched.

## Sources that could not be reached

- **IRS written determinations** https://www.irs.gov/pub/irs-wd/202152021.pdf ,
  https://www.irs.gov/pub/irs-wd/202221009.pdf and
  https://www.irs.gov/pub/irs-wd/202607023.pdf downloaded but are scanned images
  with no text layer and could not be read; opening one in the desktop app's
  browser pane raised a file-save dialog instead of displaying it. Wanted: the
  IRS's own words when it refused open-source applicants.
- **Namecheap's blog** and **nysenate.gov** refused scripted requests (HTTP 403);
  both were read in a browser.
- **https://www.codeforsociety.org/fiscal-sponsorship** returned 404.
- **The Washington Secretary of State's fee pages** returned 404; the fees were
  read from the rule itself, WAC 434-112-085.
- **California's Corporations Code** (leginfo.legislature.ca.gov) refused the
  request and was not read.
- **The Uniform Law Commission's enactment status page** was not read.

## What it means for HumanityOS

### Certain (stated by the sources above)

1. Everything already released under CC0 stays free whatever happens to the
   founder; the waiver is made "to the detriment of Affirmer's heirs and
   successors".
2. Anything in his own name (accounts, domains, money, copyright in non-CC0
   work, a single-member LLC) passes by will or intestacy, which in practice
   means probate.
3. A 501(c)(3) must not be run for "the creator or the creator's family"; it may
   pay "reasonable compensation"; its assets "must be permanently dedicated to an
   exempt purpose".
4. In Washington a nonprofit corporation may have one director, but three or more
   once it seeks or holds public-charity status, and if no directors remain the
   Attorney General may appoint them unless the bylaws say otherwise.
5. Form 1023-EZ costs $275 and fits organizations under $50,000 a year and
   $250,000 in assets; Form 1023 costs $600. 80% of 1023-EZ decisions take 22
   days; 80% of Form 1023 decisions take 191 days.
6. An LLC owned by a person cannot be recognized as a 501(c)(3), and in
   Washington it dissolves 90 days after its last member's death unless heirs act.
7. SFC requires an OSI-approved licence, and CC0 is not one. HCB accepts only
   teen-led groups. Open Collective Foundation dissolved in 2024. SPI charges 5%
   and nothing to join; OSC charges 10% and requires an organization repository,
   which HumanityOS does not have today.
8. A DMCA service provider must publish its "full legal name and physical street
   address"; a post office box needs the Register's waiver for "a demonstrable
   threat"; the designated agent may use a post office box or a third-party
   service.
9. The .us registry does not allow privacy services, and an entity can be a .us
   registrant.

### Judgement calls (the project's own reading; a professional could disagree)

1. **Do the free things now:** a will and a short succession letter; GitHub's
   account successor; move the repository into a GitHub organization with at
   least two owners; give a trusted second person a way into the registrar
   account; fill in the "If I disappeared tomorrow" checklist in
   [`docs/BUS-FACTOR.md`](../../BUS-FACTOR.md).
2. **Apply to SPI first**, after asking whether CC0 code is acceptable, and name
   a successor method in the resolution. Use OSC if SPI declines or is slow,
   accepting that gifts through it are very likely not deductible. Keep the
   Sponsor-a-Can route for deductible gifts in the meantime (the 2026-10-04
   finding); whether Sponsor-a-Can itself could sponsor the project, given that
   the founder is one of its officers, was not researched.
3. **Never hold project money in a person's own name.** Have the sponsor pay
   hosting, domains and equipment directly, and reimburse only real project
   expenses against receipts. Anyone who receives means-tested benefits should
   ask their benefits agency in writing before the project pays them or
   donations reach them (section 2).
4. **Make the sponsor, or later the project's corporation, the DMCA service
   provider and the .us registrant**, so that its address, not a person's home
   address, is the one published. Until then, if a takedown process is needed
   before an entity exists, the agent's address (not the provider's) can be a
   post office box.
5. **Write every permission the project asks of members to an entity "and its
   successors and assigns"**, or ask members to choose a public licence, so no
   permission depends on one person being alive. This answers the open
   "grantee" question in the 2026-10-12 media finding.
6. **Form the project's own Washington nonprofit corporation and seek 501(c)(3)
   status later**, when three committed directors exist and a lawyer has framed
   the purposes; move from the sponsor to it, which both SFC and SPI say their
   agreements allow.
7. **Prefer hosted servers paid by the entity** over machines run from home for
   anything that must outlive one person.
8. **Re-check this finding** before signing any sponsorship agreement, before any
   payment to the founder, and when the DMCA registration is made.

---

This document is a reading of public sources on 12 October 2026. It is not
legal, tax, benefits or financial advice, and nobody who wrote it is a lawyer,
an accountant or a benefits counsellor. Ask a nonprofit lawyer before forming an
entity or signing a sponsorship agreement, and, for anyone who receives
means-tested benefits, their benefits agency before money from the project
reaches them.
