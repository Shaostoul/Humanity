# How HumanityOS can pay for its servers and its maintainer as it grows

**Research date: 2026-11-22.** Every page below was read on that date unless a line says "could not read". Each quotation carries its own date where the source gave one; compare those dates with today's before acting. Fees and deadlines change, and this document will not be edited when they do: a later scan gets a new dated folder.

This is a comparison scan. The legal and platform edge (the licence, Google Play, payment processors, federal tax basics) is in [`../../findings/2026-11-22-paid-server-and-platform-rules.md`](../../findings/2026-11-22-paid-server-and-platform-rules.md). **I am not a lawyer, an accountant or a tax adviser; nothing here is legal or tax advice.** The project is described here as it is described everywhere in the repository: an open-source project run by one volunteer maintainer in the United States. Nothing in this folder is about the maintainer personally.

It builds on what exists: the donation routes in [`data/donate/routes.json`](../../../../data/donate/routes.json), the nonprofit tax-status check in [`../../findings/2026-10-04-sponsor-a-can-tax-status.md`](../../findings/2026-10-04-sponsor-a-can-tax-status.md), the funders guide [`../../../outreach/for_funders_and_sponsors.md`](../../../outreach/for_funders_and_sponsors.md), the sent FUTO application [`../../../outreach/applications/futo-microgrant.md`](../../../outreach/applications/futo-microgrant.md) and the deferred NLnet preparation [`../../../outreach/applications/nlnet-open-internet-stack.md`](../../../outreach/applications/nlnet-open-internet-stack.md).

## The short answer

In order, smallest risk first (what each needs from the maintainer is in section 4):

1. **Make donations visibly pay for capacity, with the platforms already in use.** GitHub Sponsors charges personal accounts nothing; Patreon takes 10% plus processing. Publish a plain monthly ledger (server and domain cost against what came in). Cost: nothing but a page and a habit.
2. **Join a fiscal sponsor so donations can be tax-deductible and the books are public.** Software in the Public Interest holds funds for open-source projects and says donations are "typically deductible"; its fee was not on its own pages (a rival's page says 5%). Open Source Collective is easier but its gifts are explicitly not deductible, and it charges 10%. Hack Club's HCB takes only teen-led groups (negative finding).
3. **Apply for grants on their dates.** NLnet's next deadline is 3 January 2027 (5,000 to 50,000 euro, a European dimension is needed, AI use must be disclosed); FUTO takes email applications at any time (microgrants of $1,000 to $5,000; one is already sent).
4. **A paid server for subscribers, last.** The AGPL licence allows it and two comparable projects (Plausible, Ghost) fund themselves that way. It adds tax, a payment processor, a possible entity and an Android store rule. Do it when donations visibly cannot cover costs, and as a separate offering that sells capacity and thanks, never access or hidden code.

## 1. Donation platforms an individual US maintainer can use today

Quotations are from each platform's own pages, read 2026-11-22; none of the pages read showed an update date unless stated.

| Platform | Fee (quoted) | Who holds the money | Tax forms | Tax-deductible for donors? |
|---|---|---|---|---|
| **GitHub Sponsors** | "GitHub Sponsors does not charge any fees for sponsorships from personal accounts"; "a fee of up to 6% for sponsorships from organization accounts" | GitHub pays out to the maintainer's bank account (or a fiscal host); payout schedule not on the page read | the setup page asks for "a W-9 if you're a US resident"; 2FA required | No: the recipient is a person. Through a fiscal host the host's rules apply (the page offers "a bank account or a fiscal host"). |
| **Patreon** | "10%" platform fee "of the income you earn on Patreon", "Plus payment processing, currency conversion, and payout fees, and applicable taxes" | Patreon, paid out to the maintainer | not on the page read | No (a gift to a person; the project's own routes file already says so) |
| **Liberapay** | "Liberapay does not take a cut of payments", "there are payment processing fees"; recent averages "3% for the payments processed by Stripe and 5.1% for the payments processed by PayPal" | "Money sent by a donor immediately goes to your Stripe or PayPal account" | "it's up to you to figure out your country's tax rules" | "Probably not, but it depends on the tax rules of your country" |
| **Open Collective, hosted by Open Source Collective** | "Fees: 10% of funds raised" (OSC's apply page); a 2021 post: "5% for Open Collective" plus "5% for Open Source Collective" | the fiscal host (Open Source Collective) holds and spends it; transparent public ledger | the host issues them; not on the pages read | **No.** OSC "is a 501(c)(6) nonprofit fiscal host" and its donations are "not deductible as charitable contributions for U.S. federal income tax purposes" |
| **Polar** | Starter "5% + 50¢" per transaction; Pro $20 a month "3.8% + 40¢"; extras for international cards and disputes | Polar acts as merchant of record, pays out through Stripe ("$2 per month ... plus 0.25% + $0.25 per payout") | not on the page | No (sales, not gifts) |
| **thanks.dev** | not found | not found | not found | Poor fit anyway: it shares money by dependency, and few projects depend on HumanityOS yet. Its front page held only a title. |
| **Ko-fi** | could not read: its robots.txt bars ClaudeBot | | | |
| **Buy Me a Coffee** | could not read: the pricing page returned 404 | | | |
| **PayPal, Cash App** | already linked on the Donate page; fees not researched here | | | |

A note on the rule in the project's own routes file: a gift straight to the maintainer is not deductible. That is why section 2's fiscal sponsors matter for anyone who wants a deduction.

### Fiscal sponsors that take open-source projects (the route to deductible gifts)

A fiscal sponsor is an existing nonprofit that holds the project's money under its own tax exemption. The IRS rule behind it: "You may deduct charitable contributions of money or property made to qualified organizations" (IRS, updated 28-Jun-2026); a person or an unregistered project is not one.

| Sponsor | Fee and terms (quoted) | Deductible? | Fit |
|---|---|---|---|
| **Software in the Public Interest (SPI)** | Its services page (last edited 2026-09-07): "SPI will accept donations and hold funds on behalf of associated projects." No fee on its own pages; a rival's page says SPI "requires 5% of gross revenue but offers fewer services". Eligibility sits in an "Associated Project HOWTO" that I did not open. | "Donations to associated projects are typically deductible as a charitable donation for US taxpayers." | Best match on cost and deductibility; the HOWTO and the fee need confirming with SPI. |
| **Software Freedom Conservancy** | "New Conservancy members are required to pay 10% of their revenue that Conservancy processes to Conservancy's general fund." Licence rule: "Being a FOSS project under an OSI-approved and DFSG-free license is mandatory, but not the only criteria." Documentation must use CC BY-SA, CC BY or CC0 or a listed licence. (Its page carries no date; footer 22 December 2020.) | It states it is "a 501(c)(3) non-profit charity" | AGPL code and CC BY-SA docs fit the rule. Selective ("not the only criteria"). |
| **Open Source Collective** | 10% (see table above). | No | Fast, public ledger, but its gifts are not deductible. |
| **Hack Club's HCB** | A help article dated 2026-06-07: "only accepts new groups that are run by teenagers." | | **Not available.** Negative finding. |
| **Open Collective Foundation (OCF)** | Closed. Its announcement, "Published on February 28, 2024": "has made the difficult decision to dissolve the Open Collective Foundation (OCF)", "effective December 31, 2024". Do not apply. | | Gone. |
| **Sponsor-a-Can** | The nonprofit already named on the Donate page; its tax status is checked in the 2026-10-04 finding. Whether it can hold money for a software project is a conversation with its board, not a platform rule. | per that finding | Existing relationship; ask. |
| **Start our own 501(c)(3)** | IRS: "organized and operated exclusively for exempt purposes set forth in section 501(c)(3)" and "none of its earnings may inure to any private shareholder or individual" (28-Jun-2026). The IRS fee page returned 404, so the application fee is not recorded here. | yes once recognised | Heaviest route: legal filings, a board, annual returns. A reason to prefer a sponsor for now. |

## 2. Grants an individual open-source maintainer in the US can apply to now

| Funder | What I read (quoted, dated) | Fit for HumanityOS |
|---|---|---|
| **NLnet, Open Internet Stack** (https://nlnet.nl/propose/, read 2026-11-22) | "Between € 5 000 and € 50 000 for a first grant." "Next deadline: January 3rd 2027 12:00 CET (noon)". "usually this means applicants should (mostly) be from Horizon Europe-eligible countries" and others "must clarify how your project might satisfy the 'European dimension' requirement". "Software, hardware and content must be released under open licences". "Proposals and projects should be made by humans, not AI." Applicants must disclose generative AI use and, if used, "paste your prompts/interactions and the AI output". The older Commons Fund page says it "closed on June 1st 2026". | Strong on substance (post-quantum signed identity, end-to-end encrypted messages, federation, self-hostable relay, AGPL). Two real obstacles: a US applicant needs a believable European dimension, and the project is openly AI-co-developed, so the disclosure rules need reading first. The earlier decision to aim at the 3 January 2027 deadline stands. |
| **FUTO** (https://futo.tech/grants/, read 2026-11-22) | "We offer one-time microgrants of $1,000-$5,000 for small or early-stage projects." "Applications are now open." Contact grantapps@futo.org. Larger "legendary grants" are "generally reserved for established entities we trust". No deadline listed; the most recent microgrant listed is dated August 2025. | A microgrant application was already sent (2026-08-02, per the repository). Nothing further to do except ask for a status, and the larger tier is for established projects. |
| **Alpha-Omega** (https://alpha-omega.dev/, page updated 2026-11-10) | Its mission is "catalyzing sustainable security improvements across open source." The page lists no application process. | Weak fit: security work only, and no way in for an individual. |
| **Sovereign Tech Agency** | could not read: its robots.txt bars ClaudeBot | A German public-money fund for critical open-source infrastructure; a later reader should read it by hand. |
| **Open Technology Fund** | could not read: its robots.txt bars ClaudeBot | Funds internet-freedom tools; privacy-first messaging might fit. Read by hand. |
| **Mozilla Foundation** | could not read: its robots.txt bars ClaudeBot. A search summary (second-hand, not a primary page) said the Mozilla Open Source Support awards "is on indefinite hiatus" and that a fellowship round closes 2026-12-17. | Unverified. |
| **GitHub's open-source funds** (the Secure Open Source Fund and similar) | not read | Likely security-scoped like Alpha-Omega; a later reader should check. |
| **Digital public good recognition** | See [`../../findings/2026-11-18-digital-public-goods.md`](../../findings/2026-11-18-digital-public-goods.md). | Recognition is not a grant, but it is the credential public funders look for. |

Honest reading: the grant routes pay one-off sums for defined work, not a salary or recurring server bills. They fit the engineering that already exists (the identity and encryption stack), and they should be written as scoped R&D, not as "fund the project".

## 3. A paid server beside the free one

**What other open-source projects do.** Two read on 2026-11-22:

- **Plausible Analytics** (AGPL-3.0, https://plausible.io/open-source-website-analytics): "Plausible is open source under the GNU Affero General Public License Version 3 (AGPLv3) or any later version." "Open source describes the freedom to inspect, modify and run the code" but "does not mean someone else must operate the service for free." "Cloud subscriptions pay for servers, security, backups, product development and support." Footer: "Funded entirely by our subscribers." The same licence as HumanityOS, a paid hosted service funding the free self-hostable one.
- **Ghost** (https://ghost.org/about/): "funded by a premium managed-service to run it on", "The Ghost(Pro) platform generates revenue for the non-profit foundation", "we don't rely on any external donations or grant funding."

Matrix's support page was read and did not state any hosted-plan economics (it describes membership and donations), and Mastodon's was not read, so neither is claimed here.

**The licence.** AGPL-3.0 section 4: "You may charge any price or no price for each copy that you convey, and you may offer support or warranty protection for a fee." The catch is section 13: users of a modified network service must be offered its source. A paid server therefore cannot hide features; it sells capacity, reliability, support and recognition. That is the house position anyway. Details and quotations: the finding.

**Platform rules (Android).** Google Play's own page: "Google Play's billing system is required for developers offering in-app purchases of digital goods and services distributed on Google Play", and in-app links to a web payment are barred, while "Outside of your app, you are free to communicate with your users about alternative purchase options." A website-only subscription is therefore workable if the Android app shows no link or "buy on the web" wording. For US users, an injunction currently lets apps link out; developers using that programme report and pay service fees from 1 October 2026, and the terms were still moving on the page's last entry (22 July 2026).

**Payment processors.** Costs on a $5 monthly plan (formula only, see the finding for the quotations): Stripe about 9% of the payment (2.9% + 30 cents) and the seller must handle sales tax itself or add Stripe Tax (0.5% per transaction); Paddle and Polar Starter 5% + 50 cents (about 15% at $5) and they act as merchant of record; Lemon Squeezy is **closed to new sign-ups and ends 2027-03-31**. Sales tax on subscriptions varies: "SaaS is one of the categories that particularly differs state by state" (Stripe, 5 May 2026). Whether Paddle or Polar accept an individual without a company is unknown.

**Federal basics, in one place.** Self-employment tax applies at "net earnings from self-employment of $400 or more" (12.4% plus 2.9%, IRS, 9 Nov 2026). Licences and permits come from "the state, county, or city" (SBA). Whether an entity or a state business licence is needed, and whether sales tax applies, is state-specific and for a tax professional.

**Design points that follow from the house position, as judgement.**
- Money never buys access to the free server: the paid server is a second server, not a gate on the first.
- "Verified" stays about trust. A subscription must not appear on the verification path.
- Perks are thanks: supporter recognition, a badge on the paid server, a place on a public supporters page.
- Donations that "visibly pay for capacity": a live cost ledger (server, domain, bandwidth) with the donation total beside it.
- Hardware honesty: say on the server page what the paid tier provides (player slots, region) and what it costs per month to run, so a subscriber sees where the money goes.

## 4. Recommendation, in order

| # | Route | Cost | Why this order | What the maintainer must do himself |
|---|---|---|---|---|
| 1 | Cost ledger page plus the existing donation links; confirm whether GitHub Sponsors is live (it is the cheapest: 0% from personal accounts) | nothing | already built; the ledger turns "donate" into "pays for capacity" | Decide what to publish; create or confirm the GitHub Sponsors profile (identity, W-9, bank account: his alone). |
| 2 | Apply to **SPI** first (lowest stated fee, deductible), **Software Freedom Conservancy** second, **Open Source Collective** as the fast fallback | SPI about 5% (to be confirmed), SFC 10%, OSC 10% | gives deductible gifts and a public ledger; money held by the sponsor and spent on the servers is not a gift to him | The application, any identity checks and agreements are his. Ask SPI for the fee and what spending reimbursements look like. |
| 3 | **NLnet** for 3 January 2027, as a scoped R&D proposal | free to apply | the one grant route that fits the engineering and has a date | Write and submit in his own name; read their generative AI policy first; he decides how to answer the "European dimension" question. |
| 3b | **FUTO** follow-up; Sovereign Tech Agency, Open Technology Fund and the GitHub funds read by hand (they bar automated readers) | free | cheap to check | His identity on any form. |
| 4 | **Paid server**, once costs exceed donations: website-only sale, merchant of record if one accepts him, Android app without a link | processor fee 9% to 15% at $5, plus tax filing | a sale brings tax, a processor and store rules that gifts do not | An account with a processor and any tax or business registration are his and need a tax professional first. |

**What would change this order.** If donations cover the servers for a year, skip route 4. If a funder or city wants to give a large sum, route 2 (a sponsor) becomes first, because an organisation can pay an organisation and a person cannot take a deductible gift. If SPI refuses, Open Source Collective can start in days.

## What I could not reach or confirm

- Ko-fi, Sovereign Tech Agency, Open Technology Fund, Mozilla Foundation: their robots.txt bars ClaudeBot, so nothing was read.
- Buy Me a Coffee's pricing page (404), thanks.dev's detail pages (front page only), GitHub's list of supported regions and payout timing, SPI's Associated Project HOWTO and fee page, Open Source Collective's acceptance criteria and current fee page (the quoted 10% is from its apply page; a 2021 post shows 5% + 5%), Patreon's tax-form wording, the IRS exemption-application fee (404).
- Whether any processor accepts an individual with no company; Apple's rules for an iPhone app.

I am not a lawyer; check the dated pages again before acting.
