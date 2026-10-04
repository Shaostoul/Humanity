# Is a donation to Sponsor-a-Can tax-deductible?

**Research date: 4 October 2026.** Everything below was read on that day. If you
are reading this later, check the date of each source against anything that has
changed since.

This is a reading of public sources by an AI working on the project, not legal or
tax advice. Nobody who wrote it is a lawyer or a tax professional.

## The question

Can the Donate page tell people that a donation to Sponsor-a-Can, the nonprofit
the operator named on 2026-10-04 as the tax-deductible way to give, "is
tax-deductible as the law allows"?

## Short answer

Yes. The IRS's own published list of exempt organizations carries Sponsor-a-Can
(EIN 93-1624890) as a section 501(c)(3) charitable organization with the code that
means "Contributions are deductible", ruling dated April 2026. That matches what
Sponsor-a-Can's own website says. Whether a particular donor can deduct a
particular gift still depends on that donor (for 2025 returns, generally only if
they itemize), which is why the copy says "as the law allows" and the FAQ tells
people to check with a tax advisor.

A gift to the maintainer directly (Patreon, GitHub Sponsors, PayPal, Cash App,
crypto) is a personal gift to a person, not a contribution to a qualified
organization, and the page says it is not tax-deductible. That was not researched
further here: nothing in the sources below treats a gift to an individual as
deductible, and the operator stated it plainly.

## Findings

### 1. Sponsor-a-Can's own website

- Source: https://www.sponsor-a-can.org/ and https://www.sponsor-a-can.org/donate/
  (both returned HTTP 200 on 2026-10-04; the pages carry no publication date).
- Quoted from the footer of both pages, exactly as written:

  > EIN: 93-1624890 | 501(c)(3) — Effective April 16, 2026 | Established May 2023

- And from the home page: "We're a Washington State 501(c)(3) nonprofit (EIN
  93-1624890)".
- The site writes its own name "Sponsor-A-Can" and "SPONSOR-A-CAN". The project
  writes it "Sponsor-a-Can"; both are the same organization at
  sponsor-a-can.org.

### 2. The IRS Exempt Organizations Business Master File extract

- Source: https://www.irs.gov/pub/irs-soi/eo_wa.csv (Washington State file),
  linked from https://www.irs.gov/charities-non-profits/exempt-organizations-business-master-file-extract-eo-bmf
  ("Page Last Reviewed or Updated: 09-Sep-2026"). The CSV's Last-Modified header
  was Mon, 07 Sep 2026 04:13:24 GMT.
- The row for this EIN, with only the fields that matter here (the street address
  is left out on purpose):

  | Field | Value |
  |---|---|
  | EIN | 931624890 |
  | NAME | SPONSOR-A-CAN |
  | CITY, STATE | SILVERDALE, WA |
  | SUBSECTION | 03 |
  | CLASSIFICATION | 1000 |
  | RULING | 202604 |
  | DEDUCTIBILITY | 1 |
  | FOUNDATION | 16 |
  | STATUS | 01 |

### 3. What those codes mean, from the IRS's own data dictionary

- Source: https://www.irs.gov/pub/foia/ig/tege/eo-info.pdf, the "eo-info.pdf"
  linked from the BMF page above (retrieved 2026-10-04; the PDF carries no date
  we could read).
- On the deductibility code, quoted exactly:

  > Deductibility Code signifies whether contributions made to an organization are deductible.

  > 1   Contributions are deductible.

- On subsection 03, classification 1, quoted exactly: "Charitable Organization".
- Foundation code 16 begins: "Organization that normally receives no more than
  one-third of its support from gross investment income and" (the line continues
  past the end of the extract; it is the public-charity category, not a private
  foundation).

### 4. A second reading of the same IRS data

- Source: https://projects.propublica.org/nonprofits/api/v2/organizations/931624890.json
  (ProPublica Nonprofit Explorer, which republishes the BMF). Its record says
  `"data_source":"current_2026_09_16"` and `"updated_at":"2026-09-16T19:54:23.498Z"`.
- It agrees: `"subsection_code":3`, `"ruling_date":"2026-04-01"`,
  `"deductibility_code":1`, `"exempt_organization_status_code":1`. This is a
  secondary source and is here only as a cross-check of finding 2.

### 5. What a donor needs to deduct a gift

- Source: IRS Publication 526, Charitable Contributions,
  https://www.irs.gov/publications/p526, which says it is "For use in preparing
  2025 Returns".
- Quoted exactly:

  > You can deduct your contributions only if you make them to a qualified organization.

  > Generally, to deduct a charitable contribution, you must itemize deductions on Schedule A (Form 1040).

## Still unknown, and what would settle it

- **The rules for 2026 returns.** Publication 526 above is the 2025 edition. If
  the law for later years lets people deduct some gifts without itemizing, the
  copy is still right ("as the law allows") but the FAQ could say more. Settle
  it by reading the 2026 edition of Publication 526 when the IRS publishes it.
- **Publication 78 / Tax Exempt Organization Search.** Not checked: the search at
  https://apps.irs.gov/app/eos/ is a script-driven page that a plain request
  cannot read. The BMF extract is the same IRS determination data, but a look at
  the search in a browser would confirm the listing in the IRS's own words.
- **Whether a donation to Sponsor-a-Can pays for any HumanityOS work.** Not a tax
  question and not researched. The operator's answer was that a gift to him
  directly is the way to give to him "instead of the nonprofit", so the page says
  only that the donation goes to Sponsor-a-Can.

## Sources that could not be reached

- https://www.irs.gov/pub/irs-soi/eo_info.pdf returned 404. The data dictionary
  was found instead at https://www.irs.gov/pub/foia/ig/tege/eo-info.pdf, linked
  from the BMF page.

## What it means for this project

**Certain** (from the IRS's own data, read 2026-10-04): Sponsor-a-Can, EIN
93-1624890, is listed as a 501(c)(3) charitable organization whose contributions
are deductible, with an April 2026 ruling. Its website says the same.

**Judgement calls:**

- The Donate page says a donation to Sponsor-a-Can "is tax-deductible as the law
  allows" and the FAQ adds that how much someone can deduct depends on their own
  situation. That hedge is ours, chosen because deductibility for a given donor
  turns on itemizing and limits the page cannot know.
- The page discloses that the maintainer serves as Sponsor-a-Can's Vice
  President, because a donor deciding between the two routes should know it.

This is a reading of public sources, not legal or tax advice.
