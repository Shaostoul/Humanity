# Selling a paid server: the licence, the Android store rule, the payment processors and the federal tax basics (2026-11-22)

**The question.** If HumanityOS ran a paid server for subscribers beside the free one, what do the AGPL licence, Google Play's rules, the payment processors and US federal rules say about it, for a US individual who is the project's one volunteer maintainer?

**Researched:** 2026-11-22. Every source below was read on that date; each carries its own date where the source gave one. The comparison of funding routes this feeds is [`../research/2026-11-22-project-funding/README.md`](../research/2026-11-22-project-funding/README.md).

**I am not a lawyer, an accountant or a tax adviser, and this is not legal or tax advice.** It records what the primary sources say so the next session need not search again. State-level rules (a business licence, sales tax on subscriptions, state income tax) differ by state and are NOT in this file.

## The short answer

- **The licence allows it.** AGPL-3.0 says "You may charge any price or no price for each copy that you convey, and you may offer support or warranty protection for a fee." Running a paid server is permitted. The condition that matters is section 13: if the project's own server code is modified and users interact with it over a network, those users must be offered the source. The project publishes it all already.
- **Google Play: a website-only subscription is allowed, an in-app one is not unless it uses Google's billing.** Google requires its billing system for in-app purchases of subscription services and cloud services sold inside an app distributed on Google Play. Outside the app the project may tell people about the subscription. Inside the app, outside the US, it may not link to or push a web payment. In the US, right now, an injunction lets apps link out and offer other billing; the terms were still moving as of the page's last text.
- **A payment processor that collects sales tax for you exists, but one was closed to new sign-ups.** Lemon Squeezy "is no longer accepting new sign-ups and will shut down on March 31, 2027". Paddle (5% + 50 cents) and Stripe (2.9% + 30 cents, plus extras) are live. Whether Paddle accepts a one-person hobby project is not published on the page I read.
- **Federal level:** a business licence from the federal government is not generally the question; the licences that matter come from the state, county and city. Net self-employment earnings of $400 or more mean self-employment tax. Donations to a person are not tax-deductible for the donor; only gifts to a qualified organisation are.

## Findings

### 1. AGPL-3.0 allows selling hosting

| | |
|---|---|
| Source | GNU Affero General Public License, version 3, https://www.gnu.org/licenses/agpl-3.0.txt |
| Its date | "Version 3, 19 November 2007" |
| Read | 2026-11-22 |

Section 4 (the sentence follows the grant to convey verbatim copies):

> "You may charge any price or no price for each copy that you convey, and you may offer support or warranty protection for a fee."

Section 2 (running without conveying is unconditional):

> "You may make, run and propagate covered works that you do not convey, without conditions so long as your license otherwise remains in force."

Section 13, the condition on a modified network service:

> "if you modify the Program, your modified version must prominently offer all users interacting with it remotely through a computer network (if your version supports such interaction) an opportunity to receive the Corresponding Source of your version by providing access to the Corresponding Source from a network server at no charge, through some standard or customary means of facilitating copying of software."

The Free Software Foundation's own FAQ (https://www.gnu.org/licenses/gpl-faq.html, read 2026-11-22; the page shows no date of its own) says, about the GPL family:

> "Yes, the GPL allows everyone to do this.  The right to sell copies is part of the definition of free software."

**What this does NOT allow:** a paid tier cannot be a closed fork. Paid-only server code that runs for subscribers and is modified from the AGPL code would have to be offered as source to those subscribers at no charge (section 13). A paid server therefore buys capacity, support and recognition, never secret features. This also matches the house position that money does not buy access to the free server.

**Relicensing is not open to the maintainer alone.** `LICENSING.md` says there is no copyright assignment and no contributor licence agreement, so code others contributed stays under the licence they gave. Selling hosting needs no relicensing; a proprietary paid edition would.

### 2. Google Play: billing for digital goods, and what a website-only subscription means

| | |
|---|---|
| Source A | "Understanding Google Play's Payments policy", https://support.google.com/googleplay/android-developer/answer/10281818 |
| Its date | the page shows no update date; the footer reads "©2026" |
| Source B | "An update regarding Google Play's policies for developers serving users in the US", https://support.google.com/googleplay/android-developer/answer/15582165 |
| Its date | latest dated entry on the page: 22 July 2026 (reporting of fees starting 1 October 2026; a 21 October 2026 entry gives external-link developers until 1 February 2027) |
| Read | 2026-11-22 |

Source A, the rule:

> "Google Play's billing system is required for developers offering in-app purchases of digital goods and services distributed on Google Play."

What counts, from the same page:

> "Subscription services (such as fitness, game, dating, education, music, video, or other content subscription services)"

> "Cloud software and services (such as data storage services, business productivity software, or financial management software)"

Selling on the website is allowed, and telling people is allowed outside the app:

> "Outside of your app, you are free to communicate with your users about alternative purchase options. You can use email marketing and other channels outside of the app to provide subscription offers and even special pricing. Within an app, developers may not lead users to a payment method other than Google Play's billing system unless Section 3, 8, or 9 of Payments policy applies. This includes directly linking to a webpage that could lead to an alternate payment method or using language that encourages a user to purchase the digital item outside of the app."

Not using Google at all is allowed:

> "Android gives developers the freedom and flexibility to distribute their apps on other Android app stores, directly from a website, or through device preloads, all without using Google Play's billing system."

Source B, the United States, as of its text:

> "Google will not require the use of Google Play Billing in apps distributed on the Google Play Store, or prohibit the use of in-app payment methods other than Google Play Billing."

> "On March 4, 2026, we entered a new settlement agreement with Epic and the parties have asked the US District Court to enter a revised Modified Injunction. More details will be provided in the coming months. Our current policies and programs remain in effect."

> "developers enrolled in the external content links and alternative billing programs in the US will need to report transactions and successful downloads and pay the relevant service fees starting on October 1, 2026."

**What this means for the Android app.** Three routes, in order of simplicity: (a) sell the subscription only on the website and show nothing about it inside the Android app (no link, no "subscribe on our site" wording), and announce it through the website and email, which Google says is allowed; (b) in the US, join Google's external-links or alternative-billing programme, which carries service fees from 1 October 2026 and may change when the revised injunction is entered; (c) use Google Play Billing inside the app, at whatever service fee Google applies. A user who already has a subscription bought on the web and simply signs in is a different case than selling inside the app; the page's own FAQ was not read for that case (see "What is still unknown"). Apple's App Store rules for an iPhone app were not read at all.

**What is NOT allowed:** inside a Google Play app outside the US, a button, link or wording that leads a user to pay on the web, unless one of the programmes in Section 3, 8 or 9 of the Payments policy applies.

### 3. Payment processors that collect sales tax (merchant of record) and what they charge

| Source | Read 2026-11-22 | Date |
|---|---|---|
| https://www.paddle.com/pricing | yes | none shown; copyright "2012-2026" |
| https://www.lemonsqueezy.com/pricing | yes | none shown |
| https://stripe.com/pricing | yes | none shown |
| https://polar.sh/resources/pricing | yes | none shown; names "May 27, 2026" as the early-member cut-off |
| https://stripe.com/resources/more/what-sales-are-subject-to-tax | yes | "Last updated May 5, 2026" |

Paddle: "5% + 50¢" per Checkout transaction on the pay-as-you-go plan; the page calls Paddle your "Merchant of Record" and its FAQ says Paddle collects the right tax and files sales tax returns on your behalf. It also says "If you're selling product with under 10$ value you can contact us for bespoke pricing."

Lemon Squeezy (negative finding):

> "Lemon Squeezy is no longer accepting new sign-ups and will shut down on March 31, 2027."

Stripe: "2.9% + 30¢" per successful domestic card transaction; Stripe Billing 0.7% of billing volume on pay-as-you-go; Stripe Tax 0.5% per transaction where registered (no-code), or $0.50 per transaction by API. Stripe is not the merchant of record on ordinary Payments, so the seller must register, collect and file sales tax in each state where it is owed; Stripe's "Managed Payments" is its merchant-of-record product at "3.5% per transaction" on top of Payments fees.

Polar: Starter plan free monthly, "5% + 50¢" per transaction; Pro $20 a month 3.8% + 40¢; extra fees include +1.5% on international cards, 0.25% to 1% currency conversion, and a $15 fee per dispute; payouts incur Stripe's charges ("$2 per month for each month with active payouts, plus 0.25% + $0.25 per payout"). Polar's page names itself a merchant-of-record product; whether it is also the legal seller for each US state's tax was not on the page.

Sales tax on digital services as a class, Stripe's own guide:

> "SaaS is one of the categories that particularly differs state by state."

> "Some states treat it as a taxable software sale, while others treat it as a nontaxable service."

On a $5 monthly subscription the percent-plus-fixed formulas above cost about 9% (Stripe cards alone), about 15% (Paddle or Polar Starter), before Stripe's extras. Small prices are punished by the fixed 30 to 50 cents.

### 4. Federal basics for a US individual

| Source | Its date | Read |
|---|---|---|
| IRS Topic 554, https://www.irs.gov/taxtopics/tc554 | "Page Last Reviewed or Updated: 09-Nov-2026" | 2026-11-22 |
| IRS, Understanding your Form 1099-K, https://www.irs.gov/businesses/understanding-your-form-1099-k | "Page Last Reviewed or Updated: 17-Nov-2026" | 2026-11-22 |
| SBA, https://www.sba.gov/business-guide/launch-your-business/apply-licenses-permits | no date shown | 2026-11-22 |
| IRS, charitable contribution deductions, https://www.irs.gov/charities-non-profits/charitable-organizations/charitable-contribution-deductions | "Page Last Reviewed or Updated: 28-Jun-2026" | 2026-11-22 |
| IRS, exemption requirements for 501(c)(3) organisations | "Last Reviewed or Updated: 28-Jun-2026" | 2026-11-22 |

> "You usually must pay self-employment tax if you had net earnings from self-employment of $400 or more." (Topic 554; the rate on the same page: "This rate consists of 12.4% for Social Security and 2.9% for Medicare taxes.")

> "Most small businesses need a combination of licenses and permits from both federal and state agencies." and "The licenses and permits you need from the state, county, or city will depend on your business activities and business location." (SBA)

> "You may deduct charitable contributions of money or property made to qualified organizations" (IRS). Individuals are not on that page's list of qualified recipients, so a gift straight to a person is not deductible.

> "none of its earnings may inure to any private shareholder or individual" (IRS, 501(c)(3) summary). This is why a maintainer cannot simply call their own account a charity: a tax-deductible gift has to go to an organisation (or a fiscal sponsor's project) whose earnings do not flow to a person.

Form 1099-K (payment apps and marketplaces): the page's reporting section says reporting applies when payments "exceeds $20,000 in more than 200 transactions". The page does not tie the figure to 2026; confirm.

## What is still unknown, and what would settle it

| Open question | What settles it |
|---|---|
| Does Paddle (or Polar, or Stripe Managed Payments) accept an individual with no company? | Their application or seller-verification pages (not read), or their sales contact. Costs nothing to ask. |
| Does a one-person paid hosting service need an entity (LLC) and a state business licence, and in which state does sales tax apply? | The state's revenue department, and a tax professional or free tax-help site (state-specific, so it is answered in the operator's own private notes, not here). |
| Is a web subscription a problem for the Android app if users only sign in? | Google's Payments policy FAQ on "consumption-only" or sign-in cases (not read in full), or Google Play support. |
| Where the US injunction lands after the revised settlement | The same Google page, checked again on the date the operator decides. |
| The Apple App Store rule for an iPhone app | Apple's App Review Guidelines section on in-app purchase (not read). |
| Whether donations received by a person count as taxable income to that person | A tax professional; Topic 554 covers self-employment, not gifts. |

## Sources I could not reach or did not read

- Ko-fi (https://ko-fi.com): its robots.txt bars ClaudeBot, so none of its pages were read.
- Sovereign Tech Agency, Mozilla Foundation, Open Technology Fund: robots.txt bars ClaudeBot; not read (the funding scan says what that costs).
- State legal codes: one state legislature's site disallows every automated reader in robots.txt; state rules are left to the maintainer's own notes.
- Buy Me a Coffee's pricing page: returned 404; thanks.dev's front page held only a title.
- The IRS user-fee page for exemption applications: 404.
- Paddle's and Stripe's seller-eligibility terms, Apple's guidelines: not opened.

## What it means for this project

**Certain** (each is a quotation above):
1. The licence permits charging for a hosted server and for support; it requires the source of a modified network service to be offered to its users.
2. Google Play requires its billing system for in-app subscriptions to digital services, except where a programme or the US injunction changes that; selling on a website and announcing it outside the app is allowed.
3. Lemon Squeezy is closed to new sellers and ends on 2027-03-31.
4. Self-employment net earnings of $400 or more attract self-employment tax; a gift to a person is not a deductible charitable contribution.

**Judgement calls** (mine, labelled so a reader can reject them):
1. Start any subscription on the website only and keep the Android app free of any link to it, because that is the cheapest rule to comply with and costs nothing in fees to Google.
2. Prefer a merchant-of-record processor over raw Stripe for a one-person project, because state sales tax on digital services is the part a volunteer is least able to track, but only if the processor accepts an individual (unknown).
3. Do not start a subscription until donations visibly fall short of server costs; a donation route carries less tax and licence burden than a sale.

*I am not a lawyer. Re-check the dated pages before acting; a platform's terms can change in a week.*
