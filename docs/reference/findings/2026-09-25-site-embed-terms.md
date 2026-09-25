# May HumanityOS's readable web show these 34 sites' pages?

**Research date: 2026-09-25.** Every page quoted below was read on that date.
If a site's terms changed after it, this document is stale for that site, not
wrong: compare the site's own "last updated" date against the date above.

**This is a reading of public sources, not legal advice.** Nobody who wrote it
is a lawyer. It exists so that a person deciding each site's
`embed.status` in `data/web/sites.json` can decide from quotations instead of
from memory, and so that the next person to ask can see what was read and when.

## The question

For each of the 34 sites in `data/web/sites.json` still marked `needs_review`:
do the site's own terms of use, and its content licence, allow what the
readable web does with a page?

What it does, for reference: when a person clicks a link or types an address,
it fetches that ONE page over HTTPS with an honest user agent naming the app,
keeps the text, headings, links, images and tables, runs no JavaScript, stores
nothing beyond the session, and draws the result in its own window (and, when
a wall screen is set to it, on a screen in the 3D world) with an "Open in
browser" button. It does not crawl, re-host or strip attribution. Three
sub-questions were asked of every site: (a) access by software other than a
browser, (b) displaying or reformatting inside another application, (c)
reproducing content.

## Short answer

None of the 34 is a clean yes except the Rust book. Most are **conditional**,
almost always on attribution and a licence notice, and the terms of most
sites say nothing about (a) or (b) at all, so their content licence decides.
**Seven forbid it as their terms are written.** One site no longer exists.

| site | reading | the deciding clause or condition, in brief |
| --- | --- | --- |
| rust-book | allowed | dual Apache 2.0 / MIT licence |
| wikipedia | conditional | CC BY-SA 4.0 attribution and licence notice; user agent must carry contact information (fixed in the same commit as this document) |
| internet-archive | conditional | terms (2014) say access is for "scholarship and research purposes only"; whether ordinary reading counts is unclear |
| openstax | conditional | new and updated books CC BY-NC-SA since 22 April 2026: is a donation-funded app non-commercial? |
| ifixit | conditional | terms allow display only through iFixit's own embed means; content licence CC BY-NC-SA 3.0. The two pages disagree |
| appropedia | conditional | CC BY-SA 4.0 attribution |
| codeberg | conditional | each repository's own free licence decides; reproduction broadly allowed with its notice kept |
| forgejo-project | conditional | website under CC BY-SA 4.0 |
| gnu-project | conditional | CC BY-ND 4.0: format changes allowed, no adaptation |
| free-software-foundation | conditional | CC BY-ND 3.0: keep notices, do not alter the substance |
| iss-live-tracker | conditional | NASA credit, no implied endorsement. **The listed URL is dead (404)** and the live tracker needs JavaScript |
| mit-opencourseware | conditional | CC BY-NC-SA 4.0; MIT judges non-commercial by "the use, not the user" |
| github | conditional | open whether one person-requested fetch counts as "scraping" under its terms |
| mdn-web-docs | conditional | CC BY-SA 2.5 attribution |
| stack-overflow | conditional | human use is exempt from its scraping ban, but framing that copies "the appearance or function" of the site is barred |
| element | conditional | acknowledge authorship, non-commercial |
| nasa | conditional | credit NASA, no implied endorsement, respect marked third-party material |
| arxiv | conditional | only bulk automated downloading and re-hosting are forbidden |
| openstreetmap | conditional | credit "OpenStreetMap and its contributors", honest user agent, no overload |
| libreoffice | conditional | CC BY-SA 3.0 |
| steam | conditional (restrictive) | one personal copy, not modified or passed to "any other Web site or network"; a shared in-world screen may be exactly that |
| pubmed | conditional | public domain with acknowledgment; publisher-owned abstracts; rate limit |
| **project-gutenberg** | **forbidden as written** | the website "is intended for human users only" and blocks automated tools (the books themselves are free to reuse) |
| **khan-academy** | **forbidden as written** | bans software used "to scrape the Services or otherwise copy materials"; a plain fetch gets a bot challenge |
| **instructables** | **forbidden as written** | Autodesk terms (2013) bar framing or mirroring and adapting |
| **coursera** | **forbidden without written consent** | |
| **discord** | **forbidden without written consent** | |
| **gog** | **forbidden as written** | clause 11.1(e) bars other software that interacts with its services; GOG will consider permission requests |
| **examine** | **forbidden as written** | bans scraping and access without written permission; pages sit behind a JavaScript browser check |
| stellarium-web | not addressed | no terms published; the site shows nothing without JavaScript |
| spaceweather-com | not addressed | no terms published |
| signal | not addressed | its only automation rule is about creating accounts |
| itch-io | not addressed | closest rule forbids "misrepresenting" its interface |
| openfarm | unreachable | **shut down April 2025**; the domain redirects to GitHub; its data was CC0 |

## What this means for the project

**Certain** (a reader can check these against the quotations):

- Seven sites' own terms, as written, do not permit this kind of access or
  display: Project Gutenberg, Khan Academy, Instructables, Coursera, Discord,
  GOG and Examine. Under the placement gate (v0.1339.0) a site becomes
  unshowable only when a person records `forbidden` for it; until then these
  seven are shown with a "review pending" note.
- OpenFarm no longer exists, and the ISS tracker URL is dead. Both catalog
  entries point at nothing.
- Several sites cannot be shown by a reader that runs no JavaScript no matter
  what their terms say (Stellarium Web, the live ISS tracker, the Internet
  Archive and OpenStax front pages, Khan Academy and Examine behind their bot
  checks).
- Wikimedia's User-Agent policy asks for contact information. The reader's
  user agent now carries the project URL.

**Judgement calls** (for the person deciding; a reader may disagree):

- Most Creative Commons conditions attach to distributing or publicly
  displaying a work. One person reading a page in the app is close to ordinary
  viewing; the same page on a wall screen other players can see is closer to
  public display. The two cases may deserve different answers.
- Whether a donation-funded, no-ads app counts as "non-commercial" (OpenStax,
  MIT OpenCourseWare, iFixit, Element) is not settled by any page read here.
- Whether keeping an article's text while dropping its navigation and sidebars
  is a format change or an adaptation (it matters for CC BY-ND, the FSF) is not
  settled by the licence text.

## Still unknown, and what would settle it

- The non-commercial question: a lawyer's reading, or asking each licensor
  directly (OpenStax and iFixit publish contact addresses for licensing).
- GitHub's "scraping" wording against a single person-requested fetch: asking
  GitHub support.
- GOG and Examine both say they consider permission requests; asking is the
  only route to "allowed" for them.

## The findings, site by site

The three parts below were researched in parallel on 2026-09-25 and are
reproduced as written, each with its own method note, per-site terms URL and
date, quotations of at most 15 words, and a list of pages that could not be
reached.


## Part 1: sites 1 to 12

Research date: 2026-09-25. A reading of public sources, not legal advice.

The question asked of each site: do its terms of use (and content licence) address
(a) accessing pages with software other than a web browser / automated access,
(b) displaying, framing, embedding or reformatting its pages inside another application,
(c) reproducing its content? Readings are: allowed / forbidden / conditional / not addressed.

Method: every quotation below was copied from the page as read on 2026-09-25, by direct
HTTPS fetch, or (where the page is built by JavaScript and the plain fetch returned an empty
shell) by loading it in a rendering browser and reading the text. No form was submitted and
no cookie banner was accepted. Quotations are at most 15 words; anything longer is marked as
a paraphrase. "Observed" lines record what a plain, honestly labelled non-browser fetch got
back on 2026-09-25; they are not terms, but they matter for whether the reader can work at all.

---

### wikipedia

Pages checked:
- Terms of Use: https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use . States "These Terms of Use went into effect on June 7, 2023." Page footer: last edited 21 February 2026.
- User-Agent Policy: https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy . Last edited 27 March 2026.
- Robot policy (the "acceptable usage policy" the Terms refer to): https://wikitech.wikimedia.org/wiki/Robot_policy . Last edited 16 March 2026.
- Page footer of https://en.wikipedia.org/wiki/Main_Page (licence line).

Key clauses:
- Terms, section 4 (prohibited): "automated uses of the Project Websites that are abusive or disruptive of the services" ... or that "violate acceptable usage policies where available, or have not been approved by the Wikimedia community".
- Terms, section 4: "Disrupting the services by placing an undue burden on an API, Project Website" (paraphrase of the rest: or on connected networks/servers).
- Terms, section 7 (Re-use): "Reuse of content that we host is welcome". "Any reuse must comply with the underlying license(s)."
- Terms, section 7, attribution option: "Through hyperlink (where possible) or URL to the page or pages that you are reusing".
- Terms, section 7, for each copy distributed: "you agree to include a licensing notice stating which license the work is released under" (paraphrase of the rest: plus a link to, or copy of, the licence).
- Wikipedia footer: "Text is available under the Creative Commons Attribution-ShareAlike 4.0 License".
- Robot policy scope: "As an Operator of a program automatically consuming the content of the wikis".
- Robot policy: "do not emulate a browser - do not store cookies or execute javascript" (paraphrase of the rest of that sentence: the rule is written for crawlers and relaxed for low-volume clients); "Honor every directive in our robots.txt file."; paraphrase: keep concurrency under 10 and average under 20 requests per second when reading /wiki/ pages.
- User-Agent policy: "Scripts should use an informative User-Agent string with contact information"; "Do not copy a browser's user agent for your bot".

Reading:
- (a) Conditional. Automated access is forbidden only when abusive, disruptive, or against the robot/User-Agent policies; the app's pattern (one page per human request, honest user agent, no JavaScript, no cookies) matches what those policies ask for, provided the user agent carries contact information (a URL or email).
- (b) Not addressed. The Terms say nothing about framing or reformatting in another application.
- (c) Conditional. Reuse is welcomed under CC BY-SA 4.0 with attribution by link to the page and a licence notice.
- Overall: conditional (attribution link to the page + CC BY-SA licence notice; descriptive user agent with contact; stay within robot-policy rates).

### internet-archive

Pages checked:
- Terms of Use: https://archive.org/about/terms.php (redirects to https://archive.org/about/terms). Dated "31 Dec 2014" on the page. The plain HTTPS fetch returned only a JavaScript shell, so the text was read in a rendering browser.
- Access Policy: https://help.archive.org/help/internet-archive-access-policy/ (last updated August 29, 2023). It defers to the Terms of Use and says nothing about automated access.

Key clauses:
- Scope: access to the Collections "is granted for scholarship and research purposes only".
- "not to interfere with the work of other users or Archive personnel, servers, or resources".
- Use of Collections "will be limited to noninfringing or fair use under copyright law".
- Where a Creative Commons or other licence is declared: "you may use the content according to the terms and conditions of the applicable license" (the page adds the declarer is "rarely the Internet Archive").
- Paraphrase: a sentence says no other access or use is authorized, but it sits inside the paragraph about passwords issued for restricted Collections, not general browsing.
- Paraphrase: the Archive asks that research publications list it in the bibliography.

Reading:
- (a) Not addressed. No clause on software, bots or scraping; only the general duty not to interfere with servers or resources.
- (b) Not addressed. Nothing on framing, embedding or reformatting.
- (c) Conditional. Reproduction is limited to noninfringing or fair use, or to whatever licence an item declares. The "scholarship and research purposes only" scope line is dated 2014 and is written broadly; whether it limits ordinary reading is unclear and worth the decider's attention.
- Overall: conditional (noninfringing/fair use or the item's own licence; unclear "scholarship and research" scope).
- Observed: https://archive.org pages returned a JavaScript-only shell to a non-browser fetch ("Javascript is required for this site." in the noscript text), so a reader that runs no JavaScript may get no content from the main site.

### project-gutenberg

Pages checked (no date shown on any of them; the server's Last-Modified header for the Terms page said 18 Aug 2026, which may only reflect a site rebuild):
- Terms of Use: https://www.gutenberg.org/policy/terms_of_use.html
- Robot access: https://www.gutenberg.org/policy/robot_access.html
- Licence: https://www.gutenberg.org/policy/license.html
- Permission How-To: https://www.gutenberg.org/policy/permission.html

Key clauses:
- Terms, "Audience": "This website is intended for human users only." Also: "Any perceived use of automated tools to access this website" (paraphrase of the rest: results in a temporary or permanent IP block). The robot page repeats both.
- Terms, "Deep Linking": "Do not link to specific files or anchors" (link to the book's landing page instead; referrer checks enforce it).
- Terms, "Embedding or Wrapping our Site or Contents" (marked "Applies mainly to website owners."): "We do not allow large-scale deep-linking to eBook files hosted on our servers." and "We will not tolerate mock Gutenberg front-ends that pocket advertising revenues".
- Terms, "OPDS Feed" (for applications): "Use a proper user-agent" with a contact address, and make "no more requests to our servers than a user with a browser typically would make".
- Licence: "you may do practically ANYTHING in the United States" with eBooks not restricted by US copyright; trademark rules apply if the name "Project Gutenberg" is used.
- Permission How-To: "No permission is needed for non-commercial use."

Reading:
- (a) Forbidden as literally worded. The site is declared for human users only and "automated tools" are blocked. A counter-reading exists: the app fetches one page only when a person asks, like a browser, and the OPDS section shows applications acting for users are tolerated when they identify themselves and keep to browser-like request volume. That section covers the OPDS feed, not the website, so this is an open question, not a permission.
- (b) Partly addressed. Large-scale deep-linking to files and ad-funded mock front-ends are refused; a one-page, no-ads reader view is not named. Links should go to the book landing page, not to file URLs.
- (c) Allowed for US-public-domain books (the large majority); some books are still in copyright and need the author's permission; outside the US, local law applies.
- Overall: forbidden as literally worded for (a); content reuse allowed. The safest pattern is to open Gutenberg links in the system browser, or ask Project Gutenberg directly.

### openstax

Pages checked:
- Terms of Use: https://openstax.org/tos . No date shown. JavaScript-rendered; read in a rendering browser.
- https://openstax.org/license redirects to the blog post "An update on OpenStax licensing", https://openstax.org/blog/openstax-licensing , dated Apr 22, 2026.
- Site footer licence line (same on every page read).

Key clauses:
- Each work is licensed on the terms "set forth in the Creative Commons license associated with that individual work".
- You may "use the works in the Service in accordance with a license to do so" (or as law otherwise permits).
- Everything else (logos, taglines, book covers, text copy, web site design; paraphrase of the list) "is Copyright Rice University All Rights Reserved".
- Footer: "licensed under a Creative Commons Attribution-NonCommerical-ShareAlike 4.0 International License" (spelling as on the site).
- Blog, Apr 22, 2026: the library moved from a mix of CC BY and CC BY-NC-SA "to CC BY-NC-SA licensing across the library" for new and updated titles (paraphrase), with limited exceptions.
- CC BY-NC-SA 4.0 legal code defines "NonCommercial means not primarily intended for or directed towards commercial advantage or monetary compensation."

Reading:
- (a) Not addressed. Nothing on software access, bots or scraping.
- (b) Not addressed. Nothing on framing or reformatting.
- (c) Conditional. Textbook content is under each book's CC licence, now mostly CC BY-NC-SA 4.0 (attribution, non-commercial, share-alike); older editions may be CC BY. The site's own design, covers and copy are all rights reserved, so the reader should show the book text with attribution, not reproduce the site's branding.
- Overall: conditional (attribution + non-commercial + share-alike for NC-SA titles; the decider should judge whether a donation-funded app counts as non-commercial).
- Observed: a non-browser fetch of https://openstax.org/subjects returned a JavaScript shell whose noscript text reads "You must enable JavaScript in order to use this site." The reader may get no content.

### khan-academy

Pages checked:
- Terms of Service: https://www.khanacademy.org/about/tos (redirects to https://www.khanacademy.org/about/docs/khan-academy-terms-of-service). Page states "Last Updated: January 30, 2026". Read in a rendering browser, because the plain fetch was answered with a "Client Challenge" page.

Key clauses:
- Section 9, prohibited: "use any software, technology or other means or processes to scrape the Services" ... "or otherwise copy materials or other data from the Services".
- Section 8.3: access and use is granted "solely for your personal, non-commercial purposes"; unless an item is marked with another licence, "you may not download, distribute, sell, lease, modify, or otherwise provide access" to third parties (paraphrase of the tail).
- Section 8.3.1 / 8.3.2 (paraphrase): where an item is expressly marked with an alternate licence, that licence applies; an unqualified "Creative Commons" reference means CC BY-NC-SA 4.0.
- Section 8.6: anyone who displays or distributes the content must show "All Khan Academy content is available for free at www.khanacademy.org" and follow the brand guidelines (paraphrase).
- Section 8.1 (paraphrase): the visual interfaces, design, videos, exercises and all other elements are owned by or licensed to Khan Academy, all rights reserved.
- No clause found on framing, embedding or mirroring (searched the full text for frame, embed, mirror, robot, spider, crawl, automat).

Reading:
- (a) Forbidden. Using software to copy material or data from the Services is prohibited in plain words, and extracting a page's text in the app is that.
- (b) Forbidden in effect. Not addressed by name, but showing the content to anyone other than the account holder counts as providing access to a third party unless an item carries an alternate licence.
- (c) Forbidden by default; conditional only for items expressly marked CC BY-NC-SA (attribution notice above, non-commercial).
- Overall: forbidden. Link out to the browser instead.
- Observed: a plain fetch with an honest user agent got HTTP 200 with a page titled "Client Challenge" (a JavaScript bot check), so the reader would be blocked technically as well.

### ifixit

Pages checked:
- Terms of Use: https://www.ifixit.com/Info/Terms_of_Use . Page states "Last updated 08-April-2026."
- Content Licensing Policy: https://www.ifixit.com/Info/Licensing (no date shown).

Key clauses:
- Terms, content-rights section just before "Guide Content and Photographs" (paraphrase): except as authorized, you must not copy, reproduce, publicly display, adapt or create derivative works from the Content; then iFixit grants a licence to display it "using the means provided for such by iFixit (e.g., through the use of “embed” codes)" and "provided that you retain all copyright and other proprietary notices contained therein".
- Terms: reproducing, copying or distributing Content for any other purpose (paraphrase) "is strictly prohibited without the express prior written permission of iFixit", and the next sentence points to the Content Licensing Policy.
- Terms (paraphrase): you must not place an unreasonable or disproportionately large strain on the infrastructure, or bypass measures used to restrict access.
- Licensing Policy: "All iFixit content is licensed under the Creative Commons BY-NC-SA 3.0 license." Attribution terms include: "Please also link to iFixit.com in your attribution." and "You may not use the material for commercial purposes."
- Licensing Policy FAQ: "May I embed your guides on my website?" Answer: "Yes."
- Licensing Policy (paraphrase): using iFixit data to train an AI model violates the terms.
- CC BY-NC-SA 3.0 legal code: rights may be exercised in all media and formats, and "include the right to make such modifications as are technically necessary" (paraphrase of the tail: to do so in other media and formats).
- The only automation clause found concerns the FixBot chatbot (no automated or bulk queries except through an approved API), not the website.

Reading:
- (a) Not addressed for the website, beyond not straining the infrastructure or bypassing access controls.
- (b) Conditional, with a tension. The Terms' own display licence is limited to iFixit's means (embed codes), but the Licensing Policy puts all content under CC BY-NC-SA 3.0, which permits sharing in any format with attribution; the two documents read differently and the Terms point to the Policy.
- (c) Conditional: attribution with a link to the licence and to iFixit.com, non-commercial, share-alike, keep copyright notices.
- Overall: conditional (CC BY-NC-SA 3.0 terms; the decider should weigh the Terms' "means provided by iFixit" wording).
- Observed: a plain fetch returned the full server-rendered page, so the reader can work technically.

### instructables

Pages checked:
- The site footer's "Terms of service" link goes to Autodesk: https://www.autodesk.com/company/legal-notices-trademarks/terms-of-service-autodesk360-web-services/instructables-terms-of-service-june-5-2013 . Page states "Last Updated: June 05, 2013". Read in a rendering browser.
- https://www.instructables.com/terms/ , /about/terms.jsp and /terms-of-service/ all return 404.
- One project page (https://www.instructables.com/How-to-make-an-Instructable-Full-Guide/), to see how licences are shown.

Key clauses (section 13, things you agree not to do):
- "use any robot, spider, or other system, device or mechanism to access the Service" ... "likely to disrupt or disable or destroy the Service or any Content".
- "frame or mirror any part of the Service".
- "modify, translate, adapt, arrange, or create derivative works of the Service" (except as the Terms permit).
- "remove or alter, any copyright, trademark, confidentiality or other proprietary notices".
- "create a database by downloading and storing any Content".
- Elsewhere in the same Terms: "All Content, including Your Content, is the property of its copyright owner(s)" (paraphrase of the next sentence: using the Service grants no ownership rights).
- The project page checked carries a licence link to CC BY-NC-SA 4.0 (creativecommons.org/licenses/by-nc-sa/4.0/); licences appear to be chosen per project by authors.

Reading:
- (a) Allowed in principle. Automated access is forbidden only where it is likely to disrupt, disable or destroy the Service; a one-page, on-request fetch is not that.
- (b) Forbidden as worded. "Frame or mirror any part of the Service" and "adapt, arrange" are both prohibited, and a reformatted in-app view of a page is close to both; whether a reader view counts as framing or mirroring has not been tested.
- (c) Conditional on each project's own licence (the one checked: CC BY-NC-SA 4.0); site-wide content remains the owners' property.
- Overall: forbidden as worded for (b). The safer choice is to open Instructables links in the browser.
- Observed: a plain fetch returned a small JavaScript-built shell (about 14 KB for the home page, 22 KB for a project page), so the reader may get little content.

### appropedia

Pages checked:
- https://www.appropedia.org/Appropedia:Terms_of_use : the page does not exist (the wiki offers "Create this page").
- https://www.appropedia.org/Appropedia:Copyrights (renders Appropedia:Intellectual property): last edit December 11, 2025.
- https://www.appropedia.org/Appropedia:Policies and https://www.appropedia.org/Appropedia:General_disclaimer (no date shown).
- Site footer licence line.

Key clauses:
- Footer: "Content is available under CC-BY-SA-4.0 unless otherwise noted."
- Intellectual property: "Content on Appropedia defaults to a Creative Commons Attribution-ShareAlike 4.0 license".
- Among listed example uses: "Making digital copies of Appropedia pages (including full copies )".
- Policies: "only a couple of strict rules are necessary" (paraphrase: on how people treat others and use others' work).
- No text found on bots, automated access, framing or mirroring (searched Policies and General disclaimer).

Reading:
- (a) Not addressed. No terms of use exist.
- (b) Not addressed.
- (c) Conditional: CC BY-SA 4.0 (attribution, share-alike), and the site explicitly lists full digital copies as a normal use.
- Overall: conditional (attribution + CC BY-SA notice).
- Observed: a plain fetch returned the full server-rendered page.

### openfarm

Pages checked:
- https://openfarm.cc : answers HTTP 301 to https://github.com/openfarmcc/OpenFarm . The site no longer exists.
- The repository README (https://raw.githubusercontent.com/openfarmcc/OpenFarm/mainline/README.md), no date shown beyond the notice's own text.
- Because the link now lands on GitHub: GitHub Acceptable Use Policies source text (https://raw.githubusercontent.com/github/docs/main/content/site-policy/acceptable-use-policies/github-acceptable-use-policies.md), section "Information Usage Restrictions".

Key clauses:
- README, "Shutdown Notice": "The OpenFarm servers were shutdown in April of 2025" (paraphrase: the repository was archived publicly).
- README, "Data License": "All data within the OpenFarm.cc database is in the Public Domain (CC0)."
- GitHub AUP: "Scraping refers to extracting information from our Service via an automated process" (paraphrase: the section limits what scraped information may be used for, forbids spam uses, and requires compliance with GitHub's Privacy Statement).

Reading:
- The OpenFarm site and whatever terms it had are gone; the catalog entry now opens a GitHub repository page, so GitHub's terms, not OpenFarm's, govern what the reader shows.
- OpenFarm's data itself is CC0 (no conditions), available from the archived repository.
- Overall: unreachable (site shut down April 2025). The catalog entry should be retired or repointed.

### codeberg

Pages checked:
- Terms of Use: https://codeberg.org/codeberg/org/src/branch/main/TermsOfUse.md (read as raw text). No date shown in the text; the repository's commit history shows the file last changed 2026-07-21 (a merge of 2026-07-02 edits).

Key clauses:
- Section 2(1)1: public repository content must be under a licence giving everyone rights including "the right to make unlimited copies of the content and redistribute those copies to others" (paraphrase of the list: use for any purpose, examine, modify).
- Exception (paraphrase): personal-opinion or personal-creative works may use a licence "such as a Creative Commons licence with NC or ND restrictions".
- Section 3(2): "Contributors own all rights and ownership for content and contributions they provided."
- Section 2(5): "Actions intended to damage the association, its reputation, service availability or performance" may lead to suspension; "In the case of excessive traffic/resource usage" a warning is normally issued first (paraphrase).
- No clause on bots, scraping, framing or embedding.

Reading:
- (a) Not addressed, beyond excessive traffic or actions intended to harm availability.
- (b) Not addressed.
- (c) Conditional on each repository's own free licence; the Terms require public content to carry a licence that permits copying and redistribution, so reproduction is broadly allowed with that licence's notice kept.
- Overall: conditional (keep each repository's licence and attribution notices).
- Observed: a plain fetch with an honest user agent returned the full page (no challenge).

### forgejo-project

Pages checked:
- https://forgejo.org/terms/ , /legal/ , /license/ and /privacy/ return 404; there is no terms of use page.
- Imprint ("Legal"): https://forgejo.org/imprint/ (no date shown).
- Privacy Policy: https://forgejo.org/privacy-policy/ (no date shown).
- Site footer licence line.

Key clauses:
- Footer: "Content available under CC BY-SA 4.0 , unless stated otherwise." (spacing as on the site).
- Imprint: "The Forgejo website is published under the CC BY-SA 4.0 license." (paraphrase: the software is GPL v3 or later.)
- Privacy Policy: "All site data is subjected to the Codeberg Terms of Use." (the site is hosted on Codeberg; see the codeberg section).
- Privacy Policy: "This website does not track information about its visitors."

Reading:
- (a) Not addressed on the site; the Codeberg Terms apply to the hosting and address only excessive traffic.
- (b) Not addressed.
- (c) Conditional: CC BY-SA 4.0 (attribution, share-alike).
- Overall: conditional (attribution + CC BY-SA notice).

### gnu-project

Pages checked:
- No terms of use page was found on www.gnu.org (checked the home page footer and searched the web for gnu.org terms of use or service).
- Footers of https://www.gnu.org/ (Updated: $Date: 2026/05/24 17:24:06 $), https://www.gnu.org/philosophy/free-sw.html (2026/06/09) and https://www.gnu.org/licenses/gpl-faq.html (2026/01/13): all carry the same licence line.
- https://www.gnu.org/robots.txt ("Crawl-delay: 4" for all user agents; aimed at crawlers).
- CC BY-ND 4.0 legal code: https://creativecommons.org/licenses/by-nd/4.0/legalcode.en

Key clauses:
- Footer: "This page is licensed under a Creative Commons Attribution-NoDerivatives 4.0 International License."
- CC BY-ND 4.0, section 2(a)(4): the licensor authorizes use in all media and formats and "to make technical modifications necessary to do so"; "simply making modifications authorized by this Section 2(a)(4) never produces Adapted Material".
- CC BY-ND 4.0, section 3(a)(2) (paraphrase): attribution may be given in any reasonable manner for the medium, for example by a link to a page carrying the required information.
- CC 4.0 defines "Share means to provide material to the public" (paraphrase: by reproduction, display, distribution and similar); one person viewing a page they requested may not be "sharing" at all, which is a point for the decider.

Reading:
- (a) Not addressed (robots.txt speaks to crawlers, which the app is not).
- (b) Not addressed by any terms; under the licence, changing fonts, colours and layout for a different medium is arguably a permitted technical modification, while editing the text would be a derivative, which ND forbids.
- (c) Conditional: CC BY-ND 4.0 (attribution, licence notice, no altered text).
- Overall: conditional (keep attribution and the licence notice; show the text unaltered).
- Observed: a plain fetch returned the full page.

---

### Could not reach (part 1)

- https://archive.org/about/terms.php : a plain HTTPS fetch returned only a JavaScript shell with no terms text. Reached instead by rendering in a browser; the text above is from that rendering.
- https://openstax.org/tos : same (JavaScript shell); read in a rendering browser.
- https://openstax.org/license : hoped for a standalone licensing page; it redirects to the blog post of Apr 22, 2026, which was used instead.
- https://www.khanacademy.org/about/tos : a plain fetch got a "Client Challenge" bot check instead of the terms; read in a rendering browser (it redirected to /about/docs/khan-academy-terms-of-service).
- https://www.instructables.com/terms/ , /about/terms.jsp , /terms-of-service/ : 404. The real terms are on autodesk.com (link above).
- https://openfarm.cc : site shut down in April 2025; the domain redirects to GitHub. OpenFarm's own former terms page (if it had one) was not looked for in web archives, because it no longer governs anything the reader would show.
- https://www.appropedia.org/Appropedia:Terms_of_use : does not exist.
- https://forgejo.org/terms/ , /legal/ , /license/ , /privacy/ : 404 (the real pages are /imprint/ and /privacy-policy/).
- gnu.org terms of use: none found; the per-page licence was used instead.
- No date was shown on the Project Gutenberg, OpenStax, Codeberg, Forgejo or Appropedia policy pages (dates given above come from page footers, commit history or HTTP headers, and are labelled as such).

## Part 2: sites 13 to 23

Research date: 2026-09-25. A reading of public sources, not legal advice.

Method: every page named below was downloaded directly on 2026-09-25 and each quotation was copied from that downloaded page text (line breaks and repeated spaces collapsed; HTML entities such as `&copy;` shown as the character they render). Each quotation is at most 15 words. Anything longer is marked as a paraphrase.

The question applied to each site: (a) accessing pages with software other than a web browser, or automated access; (b) displaying, framing, embedding or reformatting pages inside another application; (c) reproducing content.

One note that applies to several licences below: Creative Commons conditions (keep the notices, credit the author, link the licence) attach to distributing, sharing or publicly performing a work. One person reading a page in the app is close to ordinary viewing. A page drawn on an in-world screen that other players can see may be closer to public display. The decider may want to judge those two cases separately.

### free-software-foundation

Site: https://www.fsf.org

- Terms of use: none found. Checked the homepage footer (its legal links are Privacy Policy, JavaScript Licenses and Copyright Infringement Notification, with no terms link), https://www.fsf.org/about/terms (404), a web search for an fsf.org terms of use, the FSF privacy policy (https://www.fsf.org/about/free-software-foundation-privacy-policy, nothing on access or reuse), and https://www.fsf.org/robots.txt.
- Content licence: the footer of https://www.fsf.org/ (no date shown; the footer reads "Copyright (c) 2004-2026"), which links to CC BY-ND 3.0, legal code https://creativecommons.org/licenses/by-nd/3.0/legalcode (version 3.0, no date shown). The footer's "Why this license?" link goes to https://www.gnu.org/licenses/license-list.html#OpinionLicenses (page stamp "$Date: 2026/04/22").

Key clauses:
- fsf.org footer: "licensed under a Creative Commons Attribution-No Derivative Works 3.0 license (or later version)"
- CC BY-ND 3.0, section 3 (the grant): "to Reproduce the Work, to incorporate the Work into one or more Collections"
- CC BY-ND 3.0, section 3: "modifications as are technically necessary to exercise the rights in other media and formats"
- CC BY-ND 3.0, section 4(a), when you distribute or publicly perform: "You must keep intact all notices that refer to this License"
- CC BY-ND 3.0, section 4(b), paraphrase: when distributing or publicly performing, keep the copyright notices and credit the author, the title and the licensor's chosen URI, in a way reasonable to the medium.
- GNU licence list, on why opinion pieces use this kind of licence: "just the permission to copy and distribute the work verbatim"
- robots.txt: "Crawl-delay: 10" for all agents (a crawler setting; the app does not crawl).

Reading: **conditional.** (a) not addressed. (b) not addressed by any terms; the licence allows the format-level changes needed to show the work in another medium but grants no right to adapt it. (c) allowed verbatim. The condition is keeping the copyright notice, the licence notice and attribution, and not altering the substance of the text. Open point for the decider: whether leaving out page furniture (navigation, sidebars) while keeping the article intact is a format change or an adaptation; the licence text does not settle it.

### iss-live-tracker

Listed URL: https://spotthestation.nasa.gov/tracking_map.cfm returned HTTP 404 ("404 page not found") over both https and http on 2026-09-25. The site root https://spotthestation.nasa.gov/ now redirects to https://www.nasa.gov/spot-the-station/ (Page Last Updated: May 06, 2026). That page has a "Track the Space Station" section, but the tracker is drawn by JavaScript: the static page carries only "Unable to render the provided source" where the map would be.

NASA has no single terms of use. The pages that govern this are:
- NASA Web Privacy Policy & Important Notices, https://www.nasa.gov/nasa-web-privacy-policy-and-important-notices/ (redirects to https://www.nasa.gov/privacy/), Page Last Updated: Sep 11, 2026.
- NASA Images and Media Usage Guidelines, https://www.nasa.gov/nasa-brand-center/images-and-media/, Page Last Updated: Aug 13, 2026.
- NASA Brand Center, section "Linking to NASA Websites", https://www.nasa.gov/nasa-brand-center/, Page Last Updated: Sep 28, 2024.

Key clauses:
- Website Security Notice: "Unauthorized attempts to upload or change information on NASA servers are strictly prohibited"
- Media guidelines, on NASA images, audio, video and model files: "generally are not subject to copyright in the United States"
- Media guidelines: "You may use this material for educational or informational purposes"
- Media guidelines: "NASA should be acknowledged as the source of the material."
- Media guidelines: "NASA content used in a factual manner that does not imply endorsement" (paraphrase of the rest: may be used without explicit permission).
- Media guidelines, paraphrase: third-party copyrighted material on NASA sites is marked with the holder's name, and NASA's use gives nobody else a right to it.
- Media guidelines: "The NASA Insignia, Logotype, identifiers, and imagery are not in the public domain."
- Brand Center: "may be linked to from other websites, including individuals’ personal websites, without explicit permission"

Reading: **conditional.** (a) not addressed; the only access rule is against uploading or changing information, which a GET does not do. (b) not addressed; linking is expressly allowed, framing and embedding are not mentioned. (c) allowed for educational or informational use. The conditions are crediting NASA as the source, not implying NASA endorsement, and leaving marked third-party images to their owners. Practical note: the listed URL is dead, and the live tracker is a JavaScript map the reader cannot show.

### stellarium-web

Site: https://stellarium-web.org

- Terms of use: none found. Checked https://stellarium-web.org/ (the HTML is a JavaScript application shell and contains no text content), https://stellarium-web.org/robots.txt (404), and the site's own JavaScript bundle (frontend-1.16.0), which holds the in-app About, Privacy and Data Credits dialogs. None of them is a terms of use. The Privacy dialog shows no date.

Key clauses:
- Page without JavaScript: "We're sorry but stellarium-web doesn't work properly without JavaScript enabled."
- Privacy dialog: "By using this website, you agree with our" (followed by a "Privacy Policy" link). The policy covers only the personal data the site collects.
- About dialog: "based on the open source" Stellarium Web Engine project (paraphrase: it links to github.com/Stellarium/stellarium-web-engine, whose repository holds a file named LICENSE-AGPL-3.0.txt; that is a licence for the code, not for page content).
- Data Credits dialog, on planet images: "All images from NASA & JPL under public domain license"

Reading: **not addressed.** (a), (b) and (c) are all not addressed anywhere the site publishes. Practical note: the page shows no readable content without JavaScript, so the reader would display only the notice quoted above.

### spaceweather-com

Site: https://www.spaceweather.com

- Terms of use: none found. Checked the homepage https://www.spaceweather.com/ (its footer has no terms, legal, privacy or copyright link), https://www.spaceweather.com/robots.txt (404), the site's gallery submissions page https://spaceweathergallery2.com/submissions/ (footer notice only), and a web search for a spaceweather.com terms or copyright page (results only for other sites, such as SpaceWeatherLive).
- Date: no date shown on any terms, because none exist.

Key clauses:
- Homepage footer: "©2021 Spaceweather.com. All rights reserved."
- Gallery submissions page footer: "©2019 Spaceweather.com. All rights reserved."

Reading: **not addressed.** (a) and (b) are not addressed. (c) no licence is granted; the "All rights reserved" notice restates ordinary copyright, so anything beyond ordinary viewing rests on copyright law alone and not on any site term.

### mit-opencourseware

Site: https://ocw.mit.edu

- Terms: "Privacy and Terms of Use", https://ocw.mit.edu/pages/privacy-and-terms-of-use/ (the footer links it as "Terms and Conditions"), "Last Updated: 08/11/2026".
- Content licence: CC BY-NC-SA 4.0, legal code https://creativecommons.org/licenses/by-nc-sa/4.0/legalcode.en.

Key clauses:
- Licence named: "Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)"
- Share: "copy and redistribute the material in any medium or format"
- Attribution: "give appropriate credit, provide a link to the license, and indicate if changes were made"
- Noncommercial: "You may not use the material for commercial purposes."
- MIT's reading of noncommercial: "Determination of commercial vs. non-commercial purpose is based on the use, not the user."
- MIT's reading of noncommercial: "Users may not directly sell or profit from OCW materials"
- MIT name: "may not use MIT’s names or logos, or any variations thereof, without prior written consent" (except the attribution the licence requires).
- CC BY-NC-SA 4.0 legal code, section 2(a)(4): "exercise the Licensed Rights in all media and formats whether now known or hereafter created"
- Same section: "simply making modifications authorized by this Section 2(a)(4) never produces Adapted Material."
- OCW's exit notice for outside links: "external sites may have terms and conditions, including license rights, that differ from ours"
- robots.txt (https://ocw.mit.edu/robots.txt): "Allow: /" for all agents.

Reading: **conditional.** (a) not addressed. (b) not addressed; the licence expressly treats format-only changes as not creating an adaptation. (c) allowed under CC BY-NC-SA 4.0. The conditions are attribution with a link to the licence, noncommercial use (judged by the use, not by who the user is; whether a free, donation-funded reader counts as noncommercial is the decider's call, though nothing is sold), share-alike only if the material is adapted, and no use of MIT's name that suggests endorsement.

### coursera

Site: https://www.coursera.org

- Terms: Terms of Use, https://www.coursera.org/about/terms, "Effective: January 1, 2026." The Acceptable Use Policy is a section of the same page.

Key clauses:
- Acceptable Use Policy, section 3, heading of the list: "Without prior written consent from us, you also aren't allowed to:"
- Section 3: "Visit or use our Services for any form of content, data, or text scraping"
- Section 3: "(including but not limited to screen scraping, web harvesting, or web data extracting)"
- Section 3: "manual, mechanical, or automated means including by the use of bots or other similar software"
- Section 2: "Reproduce, transfer, sell, resell, or otherwise misuse any content from our Services" (followed by "unless specifically authorized to do so").
- Section 2, you may not use the Services: "for anything other than for completing online courses or for pedagogical purposes"
- "Our License to You": "only for your personal, non-commercial use, unless you obtain our written permission otherwise"
- "Commercial Use": "Any use of our Services for commercial purposes is strictly prohibited."
- Framing or embedding: no clause found.

Reading: **forbidden** (without prior written consent). (a) forbidden: the reader extracts text from a page by software, and the policy names screen scraping and web data extracting by software, done by any means, as needing Coursera's prior written consent. (b) not addressed as such. (c) forbidden unless authorized.

### github

Site: https://github.com

- Terms: GitHub Terms of Service, https://docs.github.com/en/site-policy/github-terms/github-terms-of-service, "Effective date: April 27, 2026".
- GitHub Acceptable Use Policies, https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies, no date shown.

Key clauses:
- Terms, section D.8 "Public Repositories and Lawful Access": "do not restrict lawful access to or use of the contents of public repositories"
- Terms, section D.5, paraphrase: making a repository public lets other users view and fork it through the service. Further rights: "You may grant additional rights by adopting a license"
- Terms, section G.1: "reuse any portion of the HTML/CSS, JavaScript, or visual design elements or concepts" (is not allowed "without express written permission from GitHub").
- Acceptable Use, section 7, definition of scraping: "extracting information from our Service via an automated process, such as a bot or webcrawler"
- Section 7, the permitted uses: "You may use information from our Service for the following reasons"
- The two reasons listed: "Researchers may use public, non-personal information from the Service for research purposes" (only if the publications are open access), and "Archivists may use public information from the Service for archival purposes."
- Section 6: "You will not reproduce, duplicate, copy, sell, resell or exploit any portion of the Service" (paraphrase of the rest: or use of, or access to, the Service, without express written permission).
- Section 4: "using our servers for any form of excessive automated bulk activity"
- Terms, section D.9 "Access Reciprocity", paraphrase: concerns automated access for training commercial AI systems; it does not bear on a page reader.

Reading: **conditional, and partly unclear.** (a) partly addressed: scraping is defined as automated extraction; the only permitted purposes listed for scraped information are research and archiving; excessive automated bulk activity is banned. (b) not addressed, except that GitHub's own HTML, CSS and visual design may not be reused (the reader uses its own styling, so it does not reuse them). (c) public repository content is expressly not restricted by the Terms, and reproduction is governed by each repository's own licence; GitHub's own site content may not be reproduced without permission. The unclear part: whether a single, person-requested page fetch counts as scraping under section 7, or as reproducing part of the Service under section 6. The text does not say.

### mdn-web-docs

Site: https://developer.mozilla.org

- Terms: Mozilla "Websites & Communications Terms of Use", https://www.mozilla.org/en-US/about/legal/terms/mozilla/ (linked from the MDN footer; section 1 lists MDN among Mozilla's websites), dated "September 16, 2026".
- Mozilla Acceptable Use Policy, https://www.mozilla.org/en-US/about/legal/acceptable-use/, no date shown.
- MDN content licence: "Attribution and copyright licensing", https://developer.mozilla.org/en-US/docs/MDN/Writing_guidelines/Attrib_copyright_license, "This page was last modified on Sep 10, 2026". CC BY-SA 2.5 legal code: https://creativecommons.org/licenses/by-sa/2.5/legalcode.

Key clauses:
- Mozilla Terms, section 3: "generally made available for public sharing and reuse through open licenses"
- Mozilla Terms, section 3: "Where possible, the Content or Website footer will display a notice with the applicable license." and "You agree to abide by such notices."
- Mozilla Acceptable Use Policy: "Engage in any activity that interferes with or disrupts Mozilla’s services or products"
- MDN licence page: "available under the terms of the Creative Commons Attribution-ShareAlike license (CC-BY-SA), v2.5" (or any later version).
- MDN licence page: "attribution is given to the material as well as to "Mozilla Contributors""
- MDN licence page: "Good attribution is the title of the document, with a hyperlink" (to the specific page, with any changes briefly described).
- MDN licence page: "Code samples added on or after August 20, 2010 are in the public domain CC0"
- MDN licence page: "the look and feel of this website, are not licensed under the Creative Commons license"
- CC BY-SA 2.5, section 3: "modifications as are technically necessary to exercise the rights in other media and formats"

Reading: **conditional.** (a) not addressed; the only access rule is not interfering with or disrupting Mozilla's services. (b) not addressed; the site's look and feel is not licensed, which does not bite because the reader draws the page in its own style. (c) allowed under CC BY-SA 2.5 or later. The condition is attribution: the page title, a link to the page and credit to "Mozilla Contributors", plus the licence notice. Code samples are public domain.

### stack-overflow

Site: https://stackoverflow.com

- Terms: Public Network Terms, https://stackoverflow.com/legal/terms-of-service/public, "Last updated: November 13, 2025".
- Acceptable Use Policy, https://stackoverflow.com/legal/acceptable-use-policy, incorporated into the Terms; no last-updated date shown (the text says it has also applied to stackoverflow.ai "As of 6/18/25").
- Content licensing: https://stackoverflow.com/help/licensing, no date shown.

Key clauses:
- Acceptable Use, "Content Scraping/Bot Policy", the exemption: "what is necessary for human interaction with the Network (such as local browser caching)"
- Same policy, tools covered: "spider, bot/robot, cheat utility, scraper, unauthorized script, offline reader, data miners"
- Same policy, paraphrase: the ban applies only when the purpose is (1) "Building a similar or competitive website, product, or service; or" (2) developing, training, indexing or improving AI or machine-learning tools, or (3) any purpose "at a volume or frequency that negatively impacts Network bandwidth" or other users' access. Prior written consent lifts the ban, with accessibility named as an example.
- Acceptable Use, "Framing and Mirroring": "You may not manipulate or otherwise simulate the appearance or function of the Network"
- Same clause: "by using framing, mirroring, or similar other methods"
- Same clause: "This restriction does not apply for accessibility-related usages such as screen readers."
- Terms, section 6, Stack Overflow's own content: "for download or personal use provided that you maintain all copyright and other notices"
- Terms, section 6: "for other than personal, noncommercial use is expressly prohibited without prior written permission" (this covers Stack Overflow's own content, not user posts).
- Licensing page: "all publicly accessible user contributions are licensed under Creative Commons Attribution-ShareAlike license" (CC BY-SA 2.5, 3.0 or 4.0 depending on the date of the post; posts from 2018-05-02 on are "distributed under the terms of CC BY-SA 4.0").
- Terms, section 6: "all such Public Content must have appropriate attribution."

Reading: **conditional.** (a) automated tools are barred only for the listed purposes, and access needed for human interaction is exempt; a single page a person asked to see appears to fall within the exemption and outside every listed purpose. (b) framing or mirroring that simulates Stack Overflow's appearance or function is forbidden (accessibility use is exempt). The reader restyles the page rather than imitating it, but the words "or function" are the point for the decider to weigh. (c) user posts are CC BY-SA and need attribution (author, link, licence); Stack Overflow's own content is for personal, noncommercial use with its notices kept.

### rust-book

Site: https://doc.rust-lang.org/book

- Terms of use: none found for doc.rust-lang.org. Checked the book's title page https://doc.rust-lang.org/book/ (no terms link), https://www.rust-lang.org/policies (it lists Code of Conduct, Licenses, Logo Policy and Media Guide, Security Disclosures and Privacy Notice, with no terms of use; no date shown), https://rustfoundation.org/policies-resources/ (no website terms of use listed), https://foundation.rust-lang.org/policies/terms-of-use/ (redirects to rustfoundation.org and returns 404), and https://doc.rust-lang.org/robots.txt (it disallows only old book editions).
- Content licence: https://www.rust-lang.org/policies/licenses (no date shown), and the book repository's own files https://github.com/rust-lang/book/blob/main/COPYRIGHT (last changed 2023-03-30 per its GitHub commit history) and LICENSE-MIT.

Key clauses:
- rust-lang.org Licenses: "The Rust Programming Language and all other official projects, including this website, are generally dual-licensed"
- Book COPYRIGHT file: "This repository is licensed under the Apache License, Version 2.0" (or the MIT licence, "at your option.")
- LICENSE-MIT: "The above copyright notice and this permission notice shall be included in all copies"
- Licenses page, paraphrase: third-party logos on the Rust website are not under the same licence.

Reading: **allowed.** (a) and (b) are not addressed. (c) allowed under MIT or Apache-2.0, both of which permit copying, modifying and redistributing. Viewing needs nothing; any saved or redistributed copy would need the copyright and licence notice kept.

### discord

Site: https://discord.com

- Terms: Terms of Service, https://discord.com/terms, "Effective: September 29, 2025", "Last Updated: August 29, 2025".

Key clauses:
- Under "Don't use the services to do harm to Discord": "scraping our services without our written consent, including by using any robot, spider, crawler, scraper"
- Same sentence continues: "or other automatic device, process, or software"
- Same sentence: "using any unauthorized software designed to modify the services"
- Same sentence: "copying, dismantling, or reverse engineering any of our services"
- Software licence section: "You may not copy, modify, create derivative works based upon, distribute" (paraphrase of the rest: sell, lease or sublicense any of Discord's software or services).
- Discord's content: "the design of our apps and websites, our art and images" and "We retain all intellectual property rights in our content."
- Other users' content: "You may not use this content without that person’s consent, or as allowed by law."

Reading: **forbidden.** (a) scraping by any automatic process or software needs Discord's written consent. The word scraping is not defined, so whether one person-requested page counts is arguable, but the clause is written broadly. (b) not addressed as framing, though "unauthorized software designed to modify the services" and the ban on copying the services point the same way. (c) forbidden without permission. Practical note: most of discord.com (the chat itself) needs sign-in and JavaScript, so only the public marketing and help pages would show in the reader anyway.

### Could not reach (part 2)

- https://spotthestation.nasa.gov/tracking_map.cfm: HTTP 404 ("404 page not found") over https and http. I hoped to find the tracker page itself and any page-specific notice. The site root now redirects to https://www.nasa.gov/spot-the-station/, whose tracker is drawn by JavaScript and so has no static content to read.
- https://spotthestation.nasa.gov/robots.txt: 404.
- https://www.fsf.org/about/terms: 404 (a guessed URL for a website terms of use; no such page appears to exist).
- https://stellarium-web.org/robots.txt: 404. The site's pages render only with JavaScript, so its dialogs were read from the site's JavaScript bundle instead.
- https://www.spaceweather.com/robots.txt: 404.
- https://foundation.rust-lang.org/policies/terms-of-use/: redirects to rustfoundation.org and returns 404 (a guessed URL for a Rust Foundation website terms of use).

## Part 3: sites 24 to 34

Research date: 2026-09-25. A reading of public sources, not legal advice.

Method: every page below was downloaded this session (curl with a normal browser user agent, then reduced to text) or, where noted, read from an Internet Archive copy. Quotations are copied from that text, at most 15 words each. Anything longer is marked as a paraphrase. "(a)", "(b)", "(c)" refer to the three questions: (a) access by software other than a browser or automated access, (b) displaying, framing, embedding or reformatting inside another application, (c) reproducing content.

Readings are one of: allowed / forbidden / conditional (condition stated) / not addressed.

### 24 element

- Terms URL: https://element.io/terms-of-use (redirects to the PDF https://static.element.io/legal/terms-of-use.pdf). Also checked: https://element.io/legal/acceptable-use-policy-terms (near-identical text) and the index https://element.io/legal.
- Date stated: Terms of Use: "These Terms of Use were most recently updated on 10 November 2023." Its own Document History at the end adds a later entry: "2025, October 24: UK entity name change update." Acceptable Use Policy: "most recently updated on 28 December 2022."
- Scope: applies to "the rules for using Element.io software and services (collectively, our platform)". Whether the marketing website element.io counts as "platform" is not spelled out, but the linking rules (below) only make sense for a website.
- Key clauses (Terms of Use, sections 9, 17 and 23):
  - (c) authorship: "as the authors of content on our platform must always be acknowledged"
  - (c) commercial use: "must not use any part of the content on our platform for commercial purposes" (without a licence; open-source parts excepted)
  - (c) "Not to reproduce, duplicate, copy or re-sell any part of our platform in contravention" (of these Terms)
  - (a) "Not to access without authority, interfere with, damage or disrupt" (any part of the platform)
  - (b) linking: "must not establish a link to our platform in any site or platform" (that is not owned by you)
  - (b) catch-all: "make any use of content on our platform other than that set out above" means contact support@element.io
- Reading: conditional. (a) is not addressed beyond "without authority"; (b) is not addressed directly, but section 23 says any use of content beyond the listed linking rules should be cleared with support@element.io; (c) is allowed only with authorship acknowledged and no commercial use. A reader view that keeps Element's attribution, shows the page to the person who asked for it, and is non-commercial fits the stated conditions, but the catch-all in section 23 makes asking Element the cautious route.
- Could not reach: nothing.

### 25 signal

- Terms URL: https://signal.org/legal/ (Terms of Service and Privacy Policy on one page).
- Date stated: "Effective as of May 25, 2018" and "Updated May 25, 2018".
- Scope: the Terms apply "by installing or using our apps, services, or website (together, “Services”)", so the website is covered.
- Key clauses:
  - (a) the only automation clause is about accounts: "create accounts for our Services through unauthorized or automated means"
  - (a) general: "access, use, modify, distribute, transfer, or exploit our Services in unauthorized manners" (forbidden)
  - (c) "You may not use our copyrights, trademarks, domains, logos" ... "unless you have our written permission" (two fragments of one sentence)
  - licence: "Signal grants you a limited, revocable, non-exclusive, and non-transferable license to use our Services"
- Reading: not addressed. Nothing speaks to reading the website with another program, to framing, or to reformatting; the automation clause covers account creation only. The broad intellectual property sentence (written permission needed to "use our copyrights") is a general reservation, not a rule about viewing pages, so it is flagged for the decider rather than treated as a prohibition.
- Could not reach: nothing. No separate website content licence was found on signal.org.

### 26 nasa

- Terms URLs: https://www.nasa.gov/nasa-brand-center/images-and-media/ (Images and Media Usage Guidelines), https://www.nasa.gov/nasa-brand-center/ (Brand Center hub, includes "Linking to NASA Websites"), https://www.nasa.gov/privacy/ (Web Privacy Policy and Important Notices, includes the site security notice). NASA has no separate "terms of use" page; these are its web policies.
- Dates stated: Images and Media page "Page Last Updated: Aug 13, 2026"; Brand Center hub "Page Last Updated: Sep 28, 2024"; Privacy and notices page "Page Last Updated: Sep 11, 2026".
- Key clauses:
  - (c) NASA content "generally are not subject to copyright in the United States"
  - (c) "You may use this material for educational or informational purposes" (the list includes "Internet Web pages")
  - (c) condition: "NASA should be acknowledged as the source of the material."
  - (c) condition: "NASA content used in a factual manner that does not imply endorsement" (may be used without explicit permission)
  - (c) third-party items: "NASA's use does not convey any rights to others to use the same material."
  - (c) marks: "The NASA Insignia, Logotype, identifiers, and imagery are not in the public domain." (paraphrase of the rest: imagery is made available under the Media Usage Guidelines; the insignia and logotype are protected by law)
  - (b) linking: "NASA websites may be linked to from other websites" (without explicit permission; no implied commercial endorsement)
  - (a) security notice: "Unauthorized attempts to upload or change information on NASA servers are strictly prohibited"
- Reading: conditional (strongly permissive). Reproduction and display of NASA-created material is allowed for informational purposes provided NASA is acknowledged as the source, no endorsement is implied, and items marked as third-party copyright are respected. (a) automated or non-browser reading is not addressed (only uploading or changing information is prohibited); (b) framing or embedding is not addressed.
- Could not reach: https://www.nasa.gov/web-policies/ returned 404 (was looking for a consolidated web policy page; the three pages above cover the same ground).

### 27 arxiv

- Terms URLs: arXiv has no general website "terms of use" page (https://info.arxiv.org/help/policies/terms_of_use.html returns 404, and the policies index says the list "is incomplete"). The relevant pages are https://info.arxiv.org/help/robots.html ("Robots Beware"), https://info.arxiv.org/help/license/reuse.html (Permissions and Reuse), https://info.arxiv.org/help/license/index.html (Licenses), https://info.arxiv.org/help/api/tou.html (Terms of Use for arXiv APIs, applies to the APIs, not to page reading) and https://arxiv.org/robots.txt.
- Date stated: no date shown on any of these pages.
- Key clauses:
  - (a) "Indiscriminate automated downloads from this site are not permitted" (Robots Beware, and repeated in robots.txt)
  - (a) "our first priority is to support interactive use by human users"
  - (a) "arXiv monitors activity and will deny access to sites that violate these guidelines."
  - (a) robots.txt for all agents: "Crawl-delay: 15", with /abs, /pdf and /html allowed.
  - (c) "All e-prints submitted to arXiv are subject to copyright protections."
  - (c) "arXiv is not the copyright holder on any of the e-prints in our corpus."
  - (c) API terms, as a "must not": "Store and serve arXiv e-prints (PDFs, source files, or other content) from your servers" (unless the licence or copyright holder permits)
  - (c) API terms, as encouraged: "Direct users to arXiv.org to retrieve e-print content"
  - (c) metadata is CC0: "free to use descriptive metadata", which "includes fields such as title, abstract, authors"
  - endorsement, API terms "must not": "Represent your project as endorsed or supported by arXiv.org without our permission."
- Reading: conditional. What arXiv forbids is indiscriminate automated downloading and re-hosting e-prints; what it prioritises is human interactive use. Fetching one page because a person clicked it, from arXiv's own servers, without storing or re-serving it, and without implying endorsement, fits these conditions. (b) framing or reformatting is not addressed. Each paper's own licence (shown on its abstract page) governs reuse of the paper itself; titles and abstracts are CC0.
- Could not reach: https://info.arxiv.org/help/policies/terms_of_use.html (404; hoped for a general site terms page, none appears to exist).

### 28 openstreetmap

- Terms URLs: https://osmfoundation.org/wiki/Terms_of_Use (covers "the openstreetmap.org website and associated services and APIs"), https://www.openstreetmap.org/copyright (Copyright and License), https://operations.osmfoundation.org/policies/tiles/ (Tile Usage Policy), https://operations.osmfoundation.org/policies/api/ (API Usage Policy). The Terms incorporate the usage policies by reference.
- Date stated: Terms of Use: "This page was last edited on 31 October 2018, at 12:39." Copyright page, Tile Usage Policy, API Usage Policy: no date shown.
- Key clauses:
  - (a) "any manner that could damage or overburden the Services" (forbidden)
  - (a) "You agree to abide by OSMF’s Usage Policies" (incorporated into the Terms)
  - (a)(b) "Recreate or proxy any part of the Services in order to evade these Terms" (forbidden)
  - (c) data: "You are free to copy, distribute, transmit and adapt our data"
  - (c) condition: "as long as you credit OpenStreetMap and its contributors" (and share-alike for altered data, under the ODbL)
  - (c) "Our documentation is licensed under the Creative Commons Attribution-ShareAlike 2.0 license (CC BY-SA 2.0)."
  - (a) Tile policy, must: "Send a valid HTTP User-Agent that clearly identifies your application"
  - (a) Tile policy, must not: "Bulk download (“scrape”) tiles or offer prefetch features."
  - (a) Tile policy, permitted example: "Normal interactive viewing by a human" (only the tiles for the current view)
  - (b) Tile policy on attribution: "Do not hide attribution beneath UI, behind toggles, or off-screen."
  - API policy: "Valid User-Agent identifying application and version." and "Do not submit website forms in an automated manner or on behalf of users."
- Reading: conditional. Open data and documentation may be copied and shown with credit to "OpenStreetMap and its contributors" (and the licence named); access must use an honest user agent, must not overburden the servers, and tile images (if any are shown) must be for what the person is viewing, with attribution visible. The app's honest user agent and one-page-per-click behaviour match what the policies ask for. (b) framing or reformatting the website itself is not addressed.
- Could not reach: https://osmfoundation.org/wiki/Acceptable_Use_Policy (404; guessed URL, no such page appears to exist).

### 29 libreoffice

- Terms URL: www.libreoffice.org has no terms of use page. The site footer carries its content licence; also checked https://www.libreoffice.org/imprint/ (Impressum, only a liability note about external links) and the Document Foundation trademark policy https://wiki.documentfoundation.org/TradeMark_Policy.
- Date stated: footer and imprint: no date shown. Trademark policy: "This page was last edited 09:07, 14 May 2020".
- Key clauses:
  - (c) footer: "Unless otherwise specified, all text and images on this website are licensed" (under the Creative Commons Attribution-Share Alike 3.0 License, per the same sentence)
  - (c) footer: "This does not include the source code of LibreOffice" (which is MPL 2.0)
  - (c) footer: "Their respective logos and icons are also subject to international copyright laws." (use explained in the trademark policy)
  - (b) CC BY-SA 3.0 legal code (https://creativecommons.org/licenses/by-sa/3.0/legalcode) grants "the right to make such modifications as are technically necessary" (to exercise the rights in other media and formats)
  - (b) trademark policy: "a website may not copy the look and feel of TDF websites" (aimed at look-alike websites; a reader view that uses its own styling is the opposite case)
- Reading: conditional (attribution and share-alike). Reproducing and displaying the site's text and images is allowed under CC BY-SA 3.0 with attribution and the licence named; reformatting for another medium is covered by the "technically necessary" modifications grant. (a) non-browser access is not addressed. Logos stay under the trademark policy.
- Could not reach: nothing on www.libreoffice.org. Practical note: wiki.documentfoundation.org answered a browser-like user agent with an Anubis anti-scraper challenge page (it requires JavaScript); a plain, non-browser user agent got the page normally.

### 30 steam

- Terms URLs: https://store.steampowered.com/subscriber_agreement/ (Steam Subscriber Agreement, binds account holders); https://store.steampowered.com/legal/ (Legal Info, copyright notice and infringement contact only); the store footer's "Legal" link goes to http://www.valvesoftware.com/legal.htm, which serves "Site Terms of Use - Valve Corporation" at https://www.valvesoftware.com/en/legal. Also read for context: https://steamcommunity.com/dev/apiterms (Steam Web API terms, which apply only to API use).
- Dates stated: Subscriber Agreement: "This Agreement was last updated on September 10, 2026". Valve Site Terms of Use: no date shown. Legal Info page: no date shown.
- Scope caveat: the Subscriber Agreement takes effect "by completing the registration of a Steam user account"; the Valve Site Terms speak of "THIS WEB SITE (THE "SITE")" and live on valvesoftware.com, but are the page the store links to as "Legal". Which one governs an anonymous visitor to a store page is not stated.
- Key clauses:
  - (a) Subscriber Agreement: "You may not use any form of scripts, bots, macros, or other non-human-controlled systems" (to interact with Content and Services; examples given are account creation, faked statistics, unearned rewards, automated reporting)
  - (c) Subscriber Agreement: "you may not, in whole or in part, copy, photocopy, reproduce, publish, distribute, translate" ... "without the prior consent, in writing, of Valve" (two fragments of one sentence)
  - (c) Valve Site Terms, the licence granted: "solely for your personal use, one (1) copy of any Materials" (paraphrase of the rest: for the duration of your next session, downloaded to one computer)
  - (b) Valve Site Terms, may not: "use or transmit any Materials on or to any other Web site or network"
  - (b) Valve Site Terms, may not: "modify, translate, reverse engineer, decompile, disassemble or create derivative works based on any Materials"
  - (c) Valve Site Terms, may not: "remove, obscure or alter any notice of copyright or other proprietary notices"
- Reading: conditional (narrow). A person viewing one store page for their own use fits the single-copy personal-use grant quoted above, and a person-initiated fetch is not "non-human-controlled". Two parts of the app sit badly with the Valve Site Terms as written: showing the page on an in-world screen that other people can see (if that screen is shared with others over the network, that is the "use or transmit any Materials on or to any other Web site or network" bar), and restyling the page, if "modify" is read to include presentation. Copyright notices on the page must be kept.
- Could not reach: nothing.

### 31 gog

- Terms URL: https://support.gog.com/hc/en-us/articles/212632089-GOG-User-Agreement (GOG User Agreement). Guessed URLs on www.gog.com (/en/user_agreement, /en/terms-and-conditions, /en/legal) all returned 404.
- Date stated: "Last update (effective date): 9 March 2026".
- Scope: it "applies to www.GOG.COM, your GOG user account, GOG GALAXY application" and more ("GOG services").
- Key clauses:
  - (a) 11.1(e): "Do not create, use, make available and/or distribute cheats, exploits, automation software, robots, bots"
  - (a) same list continues: "extraction tools or other software that interact with or affect GOG services" ("in any way")
  - (a) same clause: "unauthorized third party programs that collect information about GOG services"
  - (b) 10.1 GOG owns the services' "graphics, computer code, user interface, look and feel, audio, video, text, layout"
  - (b)(c) 11.1(c): "please don’t modify, merge, distribute, translate, reverse engineer, decompile, disassemble, or create derivative works" (of GOG services, without prior GOG permission)
  - (b)(c) 11.1(c) also: "you are free to contact us for permission to do these things"
  - 11.1(a): "Only use GOG services or GOG content for your personal enjoyment"
  - 2.1: "This license is for your personal use."
- Reading: forbidden as written. Clause 11.1(e) bars "other software that interact with or affect GOG services or GOG content in any way", naming spiders, scripts and extraction tools, which on its plain words covers a program that fetches a GOG page and extracts its text for display. It is aimed at cheats and bots, and a person-initiated reader is a sympathetic case, but the text makes no such exception. GOG invites permission requests for the related 11.1(c) activities.
- Could not reach: the www.gog.com guessed legal URLs above (404); the agreement was read on support.gog.com instead.

### 32 itch-io

- Terms URL: https://itch.io/docs/legal/terms (itch.io Terms of Service).
- Date stated: "Updated April 15 2023" (the change note concerns payment terms).
- Scope: "itch.io is a website, desktop application, and digital software and media distribution platform"; agreement is by "registering an account and using the Service".
- Key clauses:
  - (b) closest clause, section 3 prohibited actions: "Hacking, maliciously manipulating, or misrepresenting itch.io’s interface in any way;"
  - (a) section 3 prohibited actions: "Soliciting, harvesting or collecting information about others;" (about personal information)
  - (c) section 4, publishers grant users a licence to use content "as permitted through the functionality of the Service"
- Reading: not addressed. Nothing covers non-browser access, framing, embedding or reformatting of site pages. The closest clause forbids "misrepresenting" the interface, which reads as a bar on deception (a reader view that says what it is and offers "Open in browser" is not presenting itself as itch.io); the decider may want to weigh it.
- Could not reach: nothing.

### 33 pubmed

- Terms URLs: https://www.ncbi.nlm.nih.gov/home/about/policies/ (NCBI Website and Data Usage Policies and Disclaimers, which PubMed points to), https://www.nlm.nih.gov/web_policies.html (NLM Web Policies, Copyright section), https://pubmed.ncbi.nlm.nih.gov/help/ (PubMed User Guide), https://pubmed.ncbi.nlm.nih.gov/disclaimer/ (literature database disclaimer). Also seen: https://www.nlm.nih.gov/databases/download/terms_and_conditions.html, which applies to FTP data downloads, not page viewing.
- Dates stated: NCBI policies page: no date shown. NLM Web Policies: "Last Reviewed: December 2, 2024". PubMed User Guide and disclaimer: no date shown. (FTP terms: "Last Reviewed: May 21, 2019".)
- Key clauses:
  - (c) NCBI: "created by or for the US government on this site is within the public domain"
  - (c) NCBI: "NLM be given appropriate acknowledgment" (requested, for subsequent use)
  - (c) NLM Web Policies: "Please acknowledge NLM as the source of the information" (suggested wording "Source: National Library of Medicine.")
  - (c) NCBI: "NLM does not claim the copyright on the abstracts in PubMed" ... "however, journal publishers or authors may."
  - (c) NCBI: "Transmission or reproduction of protected items beyond that allowed by fair use" ... "requires the written permission of the copyright owners"
  - (a) NCBI scripting guidelines: "Do not overload NCBI's systems." and "Make no more than 3 requests every 1 second."
  - (a) PubMed User Guide: "Users intending to send frequent queries or retrieve large numbers of records" (should use E-utilities)
  - (b)(c) NCBI scripting guidelines: "NCBI's Disclaimer and Copyright notice must be evident to users of your service."
- Reading: conditional. Government-created page content is public domain with acknowledgment requested; abstracts may belong to publishers, so their notices must travel with them; automated or high-volume access must stay within the stated rate and use E-utilities, which one person-requested page at a time is far below. (b) framing or reformatting is not addressed directly; the requirement that NCBI's disclaimer and copyright notice be evident to users of a service is the nearest condition.
- Could not reach: nothing.

### 34 examine

- Terms URL: https://examine.com/terms/ (Terms of Service). The live page could not be read (see below); it was read from the Internet Archive copy captured 1 May 2026: https://web.archive.org/web/20260501040432/https://examine.com/terms/
- Date stated (in that copy): "This document was last updated on January 8, 2026."
- Scope: the Terms apply "By visiting or accessing Examine.com".
- Key clauses (archived copy):
  - (c) Section 3: "Use of Examine.com is restricted to the User’s personal use."
  - (c) Section 3: "not to reproduce, duplicate, copy, distribute, sell, resell, or exploit any portion of Examine.com" ... also "use of Examine.com, or access to Examine.com" ... "without express written permission by Examine" (fragments of one sentence)
  - (c) Section 9, the licence: "authorizes the use or download of a single copy" ... "solely for personal, non-commercial use"
  - (c) Section 9, condition: "All users must include the following copyright notice, or a substantially similar notice" (the notice is "© 2011–2026 Examine.com", at the end of the content)
  - (a) Section 18, prohibited: "to spam, phish, pharm, pretext, spider, crawl, or scrape;"
  - (a) Section 18, prohibited: "to interfere with or circumvent the security features of Examine.com"
- Reading: forbidden as written. The single-copy personal-use grant would fit one person viewing one page, but the Terms also forbid scraping and exploiting "access to Examine.com" without written permission, and the site currently puts a JavaScript browser check in front of its pages, which a no-JavaScript reader cannot pass and which Section 18 forbids circumventing. In practice the app would receive the checkpoint page, not the content.
- Could not reach: the live https://examine.com/terms/ (and /terms-of-service/) returned HTTP 429 with a page titled "Vercel Security Checkpoint" reading "We're verifying your browser", both to a direct download and to the web-fetch tool. No attempt was made to get past it. The archived copy is four months older than the research date; the live text may have changed since 1 May 2026.

### Could not reach (part 3)

- https://examine.com/terms/ (live): HTTP 429, Vercel Security Checkpoint (JavaScript browser check). Hoped for the current Terms of Service; read the Internet Archive copy of 2026-05-01 instead.
- https://info.arxiv.org/help/policies/terms_of_use.html: 404. Hoped for a general arXiv website terms of use; none appears to exist, so the Robots Beware, Reuse, Licenses and API terms pages were used.
- https://www.nasa.gov/web-policies/: 404. Hoped for a consolidated web policy; the Images and Media, Brand Center and Privacy/Notices pages were used.
- https://osmfoundation.org/wiki/Acceptable_Use_Policy: 404 (guessed URL). The Terms of Use plus Tile and API usage policies cover the ground.
- https://www.gog.com/en/user_agreement, /en/terms-and-conditions, /en/legal: 404 (guessed URLs). The User Agreement was read on support.gog.com.

---

**Not legal advice.** A reading of public sources on 2026-09-25 by people who are not lawyers. The decision for each site belongs to the person who records it in `data/web/sites.json`.
