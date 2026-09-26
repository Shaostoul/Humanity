# Where to ask for permission, and HumanityOS as a game launcher

**Research date: 2026-09-25.** Every page quoted was read on that date.

**A reading of public sources, not legal advice.** Nobody who wrote it is a
lawyer.

## The question

Two questions from the operator on 2026-09-25, after the site decisions
recorded from [`2026-09-25-site-embed-terms.md`](2026-09-25-site-embed-terms.md):
(1) for each site that forbids, or only permits with written consent, what the
app does with its pages, where is the official place to ask; and (2) could
HumanityOS act as a launcher for other games, starting with GOG.

## Short answer

- **GOG:** business partners write to **inquiries@gog.com**; its User Agreement
  11.1(c) says requests are reviewed in good faith. GOG publishes no API for
  other launchers to read a library (its agreement says it "may want to open client protocols" some day),
  but launching an installed GOG game needs nothing from GOG: GOG says its games
  need no launcher. Affiliate: **affiliate@gog.com**, then AdTraction.
- **Project Gutenberg** names a sanctioned route for applications: its OPDS
  catalog feed, used with a user agent that carries a contact address. The
  books can come in that way without asking.
- **Examine** (support@examine.com), **iFixit** (legal@ifixit.com), **Element**
  (support@element.io), **Discord** (privacy@discord.com), **Khan Academy**
  (info@khanacademy.org, though its help centre says the main platform "is not
  available for licensing"), **Instructables** (Autodesk's Copyright Permission
  Request Form). **Coursera** publishes no permissions address, only its general
  contact page.
- Examine's live terms are dated **June 16, 2026**, newer than the archived
  copy the first findings used; its "forbidden" decision stands on either.
- Steam: launching through `steam://rungameid/` and reading local library
  files is what Playnite (MIT) has done publicly for years; showing a player's
  Steam library has an official route (their own Web API key), under 2010 API
  terms that allow data only "as requested by the end user".

The research notes follow as written.

Method: every page named here was read on 2026-09-25, by a plain HTTPS fetch reduced
to text, or (where the plain fetch got a bot check, a 403 or a JavaScript shell) by
loading it in a rendering browser window and reading the text. No form was submitted,
no account was created, no cookie banner was accepted. Every quotation is at most 15
words, copied from the page as read that day; "(fragment)" marks a piece of a longer
sentence. Source code was read from the projects' own public repositories (downloaded
as archives) and is cited by file path. Where something could not be confirmed on a
page this session it says "not confirmed". Companion to
`docs/reference/findings/2026-09-25-site-embed-terms.md` in the repo.

## Task 1: the official route to ask each site

| site | where to ask | the page that says so | quote (as read 2026-09-25) |
| --- | --- | --- | --- |
| gog | No address is named for permission requests as such. The User Agreement's own contact for users is **support@gog.com**; business partners are pointed to **inquiries@gog.com** | GOG User Agreement, clause 11.1(c) and section 23.1, https://support.gog.com/hc/en-us/articles/212632089-GOG-User-Agreement ("Last update (effective date): 9 March 2026"); "Contact information for affiliates/press/publishers/future employees", https://support.gog.com/hc/en-us/articles/212184689 (no date shown) | 11.1(c): "you are free to contact us for permission to do these things". Contacts article: "A potential business partner or a journalist - please write to inquiries@gog.com" |
| examine | **support@examine.com**, or the form at https://examine.com/contact/ (its Subject menu offers "Business Development") | Terms of Service, section "Contact information", https://examine.com/terms/ ("This document was last updated on June 16, 2026.") | "Questions about the Terms of Service should be sent to Examine at support@examine.com." |
| coursera | **General only.** No permissions or licensing address exists on the pages read. The Terms send all questions to the Contact page, https://www.coursera.org/about/contact (Learner Help Center, business/campus/government sales forms, press@coursera.org, privacy@coursera.org) | Terms of Use, opening paragraph, https://www.coursera.org/about/terms ("Effective: January 1, 2026.") | "contact us if you have any questions, requests for information, or complaints" |
| discord | **privacy@discord.com** (the Terms' named contact for anything about the Terms); otherwise the general request form https://support.discord.com/hc/en-us/requests/new | Terms of Service, "Contacting each other", https://discord.com/terms ("Effective: September 29, 2025", "Last Updated: August 29, 2025") | "wish to contact us in connection with, these terms, please contact us at privacy@discord.com" |
| project-gutenberg | Email "the leadership": the Contact page lists **eric (at) pglaf.org** (the Executive Director of the Project Gutenberg Literary Archive Foundation) and **help2026 (at) pglaf.org** (general help). Addresses are printed in "(at)" form and carry a year | Permission How-To, https://www.gutenberg.org/policy/permission.html ; Contact Us, https://www.gutenberg.org/about/contact_information.html (no dates shown) | "If your inquiry is not addressed, please email the leadership with your permission requests." |
| khan-academy | **info@khanacademy.org**. But see the "not available for licensing" line in the notes | Help Center, "What is Khan Academy's Trademark and Brand Usage Policy?", https://support.khanacademy.org/hc/en-us/articles/202263034 ("Updated April 21, 2025") | "If you have any questions regarding these policies, email info@khanacademy.org." |
| instructables (Autodesk) | Autodesk's **Copyright Permission Request Form** (an online workflow): https://autodesk.tap.thinksmart.com/prod/Portal/ShowWorkFlow/AnonymousEmbed/cf050d55-075d-4da3-9423-db7e85ced572 . Instructables' own help form has no permissions topic | Autodesk "Permission To Use/Display Autodesk Copyrighted Materials", https://www.autodesk.com/company/legal-notices-trademarks/intellectual-property/copyright (no date shown) | "for uses that are not pre-approved, please submit your request through our automated workflow" |
| element | **support@element.io** (Terms of Use). The Acceptable Use Policy web page gives **contact@element.io** in the same sentence | Terms of Use section 23, "Rules about linking to our platform", https://static.element.io/legal/terms-of-use.pdf ("most recently updated on 10 November 2023"; Document History adds "2025, October 24: UK entity name change update."); Acceptable Use Policy, https://element.io/legal/acceptable-use-policy-terms ("most recently updated on 28 December 2022") | "make any use of content on our platform other than that set out above" (fragment; the sentence ends by naming support@element.io) |
| ifixit | **legal@ifixit.com** | Content Licensing Policy, "Commercial" section, https://www.ifixit.com/Info/Licensing (no date shown); the Terms' "Contact" section gives the same address, https://www.ifixit.com/Info/Terms_of_Use ("Last updated 08-April-2026.") | "contact us with any inquiries by emailing legal@ifixit.com" (fragment; the sentence opens by saying iFixit grants usage licences to certain commercial entities) |

### Notes on Task 1

**GOG.** The permission sentence in 11.1(c) covers modifying, merging, distributing,
translating, reverse engineering or making derivative works of GOG services. The clause
the earlier findings rely on, 11.1(e) (other software that interacts with GOG services),
has no permission sentence of its own, but its bar is on "unauthorized third party
programs" (fragment), which implies an authorized one is possible. Section 23.1 lists
two contacts: "Single point of contact for recipients of GOG services" is the GOG
Support Team, support@gog.com, and the "Single point of contact for authorities" is the
GOG Legal Team, legal@gog.com. For an integration or partnership question (see Task 2),
inquiries@gog.com is the address GOG itself gives business partners.

**Examine.** The live Terms now say "last updated on June 16, 2026". The earlier
findings document read an Internet Archive copy dated January 8, 2026, because the live
page was behind a browser check that day for a plain fetch; a rendering browser loaded
it normally this time. The June text keeps the ban ("to spam, phish, pharm, pretext,
spider, crawl, or scrape;") and the "without express written permission by Examine"
wording, and adds a paid-tier (ExPro) permission to reproduce portions, on conditions
that include "provided privately and on a limited basis" (fragment) and a ban on social
media. The earlier document's Examine entry should be re-read against the June text.

**Coursera.** The only named addresses are for privacy, press, security, copyright
claims and arbitration notices. None is for permissions. The "Industry Partnership
Inquiries" form is for companies "interested in creating Professional Certificates on
our platform" (fragment), which is not this request. So the route is the general
Contact page.

**Discord.** The Brand Guidelines (https://discord.com/branding) say "You must have
permission from Discord before using any of the Discord Marks" (fragment) but name no
channel. No licensing or partnerships address was found. The Terms' own contact line is
the only named address.

**Project Gutenberg.** The Permission How-To is mostly about the books, and it says
Project Gutenberg "does not fill out permission forms or otherwise grant permission for
public domain items" (fragment, about the books, not the website). For the website, the
Terms of Use (https://www.gutenberg.org/policy/terms_of_use.html) end their OPDS
section with "If you have special needs, contact us, don't try to `hack around´." The
same section is a sanctioned route for applications: an app using the OPDS feed must
"Use a proper user-agent" (fragment) that includes "a contact address like a web page
or email" (fragment), and make "no more requests to our servers than a user with a
browser typically would make" (fragment). The robot-access page
(https://www.gutenberg.org/policy/robot_access.html) repeats that the website "is
intended for human users only" and lists mirrors, a harvest endpoint and offline catalog
files as "The only exceptions to this rule" (fragment). The contact addresses carry the
year ("help2026") and the page says "please type them manually".

**Khan Academy.** The Terms of Service themselves
(https://www.khanacademy.org/about/docs/khan-academy-terms-of-service, "Last Updated:
January 30, 2026", read in a rendering browser) contain no email address. The only
published address for usage questions is info@khanacademy.org, on the brand policy
article. Two statements bear directly on the request:
- Help Center "Can I use Khan Academy's videos/name/materials/links in my project?",
  https://support.khanacademy.org/hc/en-us/articles/202262954 ("Updated July 15, 2026"):
  "For inquiries regarding the Khan Academy platform, the main platform is not available
  for licensing." This is the closest any of the nine comes to saying no in advance.
- The brand policy article, "Prohibited Use of Materials": you may not link or embed in
  a way that leaves the materials "“framed,” surrounded, or obfuscated by any
  third-party content, materials, or branding" (fragment). It also says apps should be
  named like "Viewer for Khan Academy" (fragment) and clearly marked unofficial.

**Instructables / Autodesk.** Three findings:
- The Instructables Terms (Autodesk, "Last Updated: June 05, 2013", read in a rendering
  browser because a plain fetch got HTTP 403) list "frame or mirror any part of the
  Service" (fragment) in section 13 with no "without permission" exception on that line. The Terms point to "Special Service
  Terms" for contact details, and that link returns 404.
- Autodesk's permission page defines "Autodesk Website Content" as "content displayed
  on Autodesk.com and other Autodesk-owned websites" (fragment), which on its words
  includes instructables.com, and sends uses that are not pre-approved to the request
  form above (confirmed to load: its title is "Copyright Permission Request Form"; it
  carries a reCAPTCHA; nothing was entered). The same page warns that "permission to use
  Autodesk Content does not include permission to use third-party materials" (fragment).
  Most Instructables projects are written by members under their own licences, so an
  Autodesk grant would cover the site, not each author's work.
- Autodesk's "Link Consent" page
  (https://www.autodesk.com/company/legal-notices-trademarks/intellectual-property/link-consent,
  no date shown) says "You must not create a border environment or browser around
  content" (fragment) contained in Autodesk's website. Whether that page governs
  instructables.com, or only autodesk.com, is not stated.
- Instructables' own Help page (https://www.instructables.com/help/) offers these form
  topics only: Publishing, Guest posting, Trademark & copyright claims, Account deletion,
  Contest or prize support, Report a bug.

**Element.** The two Element documents give different addresses for the same sentence:
support@element.io in the Terms of Use PDF, contact@element.io on the older Acceptable
Use Policy web page (the addresses on that page are hidden by Cloudflare's email
protection and were decoded from the page source). The Terms of Use is the newer and
the one the earlier findings cite, so support@element.io is the better first address.

**iFixit.** legal@ifixit.com is the address both the Terms and the Licensing Policy give,
so one question there can also settle the disagreement between the two documents noted
in the earlier findings. The Licensing Policy's FAQ also says embedding guides is allowed
("May I embed your guides on my website?" "Yes.") through iFixit's own embed code.

## Task 2: HumanityOS as a game launcher (GOG and Steam)

### 2.1 GOG's business, partnership and developer contacts

- **Business partners: inquiries@gog.com.** Support article "Contact information for
  affiliates/press/publishers/future employees",
  https://support.gog.com/hc/en-us/articles/212184689 (no date shown): "A potential
  business partner or a journalist - please write to inquiries@gog.com". The same article
  lists creators@gog.com for "A YouTuber, streamer or blogger that wants to be our
  affiliate" and https://www.gog.com/indie for game developers wanting to release on GOG.
  The GOG Support Center home page footer also carries a "Business and press inquiries"
  link to mailto:inquiries@gog.com (read in a rendering browser).
- **Developers: GOG DevPortal**, https://devportal.gog.com/welcome (sign-in only). It
  says "If you don't have a developer account, please contact us." and that link goes to
  https://www.gog.com/support/contact/business, which redirected to the Support Center
  home on 2026-09-25. The DevPortal is for developers publishing games on GOG, not for
  third-party launchers; no launcher-integration programme was found there.
- **What the User Agreement says about third-party software** (9 March 2026):
  - 11.1(c), after the permission sentence quoted in Task 1: "at some point in the future
    we may want to open client protocols" (fragment; the sentence goes on to say this
    would let users work with GOG data and software without reverse engineering). So as
    of 2026-09-25 GOG says openly that it has NOT opened its client protocols.
  - 11.1(e) bars "unauthorized third party programs that intercept, emulate, or redirect
    any communication" (fragment) between GOG and GOG services.
  - 11.1(f) bars interference "through protocol emulation" (fragment), and in its next
    sentence bars access to areas of GOG.COM, GOG GALAXY or "GOG servers that have not
    been made available to the public" (fragment).
  - 11.1(a): "Only use GOG services or GOG content for your personal enjoyment" (fragment).
- **Launching GOG games needs no GOG software.** Support article "Do I need some launcher
  application to play my GOG games?", https://support.gog.com/hc/en-us/articles/212554329
  (no date shown): "No, you don't need any additional apps or launchers." After
  installing from GOG's offline installer, "shortcuts will be added to your Start Menu,
  desktop and Games Explorer" (fragment). A launcher that finds installed games on disk
  and starts their executables never talks to GOG's services at all; reading a player's
  online GOG library is the part that touches 11.1(e) and (f).

### 2.2 Does GOG publish an integration API for third-party launchers?

- **Not confirmed that any exists.** No GOG-published API for another launcher to read a
  player's GOG library was found, and 11.1(c) (above) says the client protocols are not
  open yet.
- **The GOG GALAXY Integrations API** is the one GOG publishes, and it goes the other
  way: it lets developers plug OTHER platforms into GOG GALAXY.
  - URL: https://github.com/gogcom/galaxy-integrations-python-api
  - Licence: MIT, "Copyright (c) 2019 GOG sp. z o.o." (LICENSE file). Not archived; last
    commit 2026-01-28 ("Update README: GOG GALAXY 2.1, Python 3.13"), per the GitHub API.
  - README: "allows developers to easily build community integrations for various gaming
    platforms with GOG GALAXY" (fragment); "Each integration in GOG GALAXY 2.1 comes as a
    separate Python script" (fragment). Features include "importing owned and detecting
    installed games" and "installing and launching games", all inside GOG GALAXY.
  - README "Legal Notice", paraphrase: whoever integrates something into GOG GALAXY
    represents that they hold the rights to it and that it complies with third-party
    licences and law.
  - Relevance to HumanityOS: it would let GOG GALAXY show HumanityOS content, not let
    HumanityOS show a GOG library. It is still a published, permissively licensed
    reference for how GOG models "owned", "installed" and "launch".
- **A catalog (store) API for affiliates.** The affiliate article (2.3 below) says "If you
  need a product feed, feel free to use our API" and "Note that there is a 200
  request/hour/IP on the api.gog.com/products/* endpoint" (fragment). Its documentation is
  a Google Drive folder, which was not opened. This is store catalog data, not a player's
  library.

### 2.3 GOG affiliate programme

- Page: "How to join the GOG Affiliate Program",
  https://support.gog.com/hc/en-us/articles/4405004689297 (no date shown).
  - "The GOG Affiliate Program is open to publishers, influencers and our business
    partners."
  - "Send an email at affiliate@gog.com" (fragment), then sign up on AdTraction, which
    "handles all affiliation partnerships for GOG" (fragment).
  - "we currently cannot accept everyone in the GOG Affiliate Program." (fragment)
  - Conditions as stated: "you get 6% of Net Sales. (Gross - VAT)." and "Attribution
    lasts for 7 days after a click."; payouts "70 days after they happen" (fragment).
  - GOG-branded affiliate links take the form https://af.gog.com/...?as=<channel id>.
- AdTraction's public listing for the programme: https://adtraction.com/advertisers/1578845455
  ("Welcome to the affiliate / partner program of gog.com"). A standalone public terms
  page was **not confirmed**: the formal programme terms appear to sit inside AdTraction
  after sign-up.
- https://affiliate.gog.com/ could not be reached (certificate error, see the last list).
- Prior art: Heroic's donate page (https://heroicgameslauncher.com/donate) funds the
  project partly this way: "Use our link when making purchases on GOG and support the
  project." (link: https://af.gog.com?as=1838482841). A donation-funded open-source
  launcher earning GOG affiliate commission is therefore an existing, public pattern.

### 2.4 Prior art: Playnite

- Repository: https://github.com/JosefNemec/Playnite . Licence: **MIT** (LICENSE.md,
  "Copyright (c) 2020 Josef Nemec"). Active: last push 2026-09-22 (GitHub API).
- README: "An open source video game library manager and launcher with support for 3rd
  party libraries" (fragment). On accounts: connection is "usually done via official login
  web forms" (fragment), keeping only the session cookies or tokens (paraphrase).
- The library plugins lived in https://github.com/JosefNemec/PlayniteExtensions, now
  archived, whose README says "Repository was moved to Codeberg": 
  https://codeberg.org/CrowIsTaken/PlayniteExtensions (LICENSE.md: MIT, "Copyright (c)
  2020 Josef Nemec"; latest commit 2026-09-24, "Steam plugin releases").
- **How the GOG plugin works** (read in `source/Libraries/GogLibrary/`):
  - Installed games: Windows uninstall registry entries whose publisher is "GOG.com",
    plus each game's `goggame-<id>.info` file in its install folder (`GogLibrary.cs`).
  - Owned library: GOG's website account endpoints called with the user's own logged-in
    session, e.g. `www.gog.com/account/getFilteredProducts` and
    `menu.gog.com/v1/account/basic` (`Services/GogAccountClient.cs`). No page found where
    GOG documents these for third parties, so "official API" is **not confirmed**.
  - Store metadata: `api.gog.com/products/<id>` (`Services/GogApiClient.cs`), the same
    endpoint the affiliate article names.
  - Launching: either the game's own primary play task read from `goggame-<id>.info`
    (no GOG software involved), or, if the user chooses, GOG GALAXY's command line
    `/command=runGame` (`GogLibrary.cs`). Installing: `goggalaxy://openGameView/<id>` or
    GOG GALAXY's `/command=installGame` (`GogGameController.cs`), i.e. GOG's own client.
- **How the Steam plugin works** (`source/Libraries/SteamLibrary/`):
  - Installed games: Steam's local `appmanifest` files and `libraryfolders.vdf`
    (`Services/SteamLocalService.cs`).
  - Owned games: the official Steam Web API, `IPlayerService/GetOwnedGames`, called with
    the user's OWN API key or the user's web access token (`Services/PlayerService.cs`).
  - Launching: `steam://rungameid/<id>` passed to the Steam client, and
    `steam://install/<id>` / `steam://uninstall/<id>` (`SteamGameController.cs`).
- In short: Playnite uses an official, documented API for Steam; undocumented GOG website
  endpoints with the user's own session for GOG; local client files for installed games;
  and each platform's own launch mechanism (steam:// URLs, GOG GALAXY's command line, or
  the game's own executable).

### 2.5 Prior art: Heroic Games Launcher

- Repository: https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher . Licence:
  **GPL-3.0** (COPYING: "GNU GENERAL PUBLIC LICENSE Version 3"). Active: last push
  2026-09-25.
- README: GOG support is "GOG Games using our custom implementation with gogdl"
  (fragment); Epic uses Legendary and Amazon uses Nile.
- gogdl: https://github.com/Heroic-Games-Launcher/heroic-gogdl , **GPL-3.0**, last push
  2026-09-08. Its README: "it's meant to be used by some other application wanting to
  download game files" (fragment). Its source talks directly to GOG's own servers
  (auth.gog.com, api.gog.com, content-system.gog.com, embed.gog.com,
  cloudstorage.gog.com) using a client identifier hard-coded in `gogdl/auth.py`.
- **Does it use official GOG APIs?** It uses GOG's own production servers, but no page
  was found where GOG publishes or licenses those endpoints for third-party launchers,
  and 11.1(c) says the client protocols are not yet open. So: official API **not
  confirmed**; the honest description is "GOG's servers, reached the way GOG's own
  client reaches them".
- Heroic FAQ (https://heroicgameslauncher.com/faq, no date shown): "You will login on the
  official Epic Games Store or GOG website" (fragment); "It is unlikely that you will get
  banned for using Heroic" (fragment).
- An official GOG partnership with Heroic: **not confirmed** (none found on any page read;
  the visible link is the affiliate link in 2.3).

### 2.6 Steam

- **Launching through `steam://`.** No Valve rule was found that addresses it, for or
  against. The protocol is documented on the Valve Developer Community wiki, "Steam
  browser protocol", https://developer.valvesoftware.com/wiki/Steam_browser_protocol
  ("last edited on 14 July 2024"; read in a rendering browser after a plain fetch got a
  bot check): "There are numerous system-wide commands available that interact with
  Steam." The page says they can be typed into the Windows Run box or a browser's address
  bar, and "you can normally create links to them as you would web page links"
  (fragment). Entry:
  "steam://rungameid/<id> Same as run, but with support for mods and non-Steam
  shortcuts." That page is documentation on a community wiki, not a policy.
- **Reading the local Steam library files** (appmanifest, libraryfolders.vdf): no Valve
  page was found that addresses it (**not confirmed** either way).
- **Nearest Steam Subscriber Agreement clauses**, https://store.steampowered.com/subscriber_agreement/
  ("This Agreement was last updated on September 10, 2026"), section 4:
  - 4.B: "you will not tamper with the execution of Steam or Content and Services"
    (fragment; the sentence ends with an exception for what Valve authorizes).
  - 4.B, paraphrase of the start of the sentence: no cheats, mods, hacks or other
    unauthorized third-party software used in "interacting with or controlling the
    processes or user interface of Steam Content and Services" (fragment).
    Asking the Steam client to run a game through its own registered `steam://` command
    is not obviously "tampering" or "controlling its processes", but the SSA does not say
    so either; this is a reading, not a settled answer.
  - 4.C Automation lists account creation, faked statistics, unearned rewards and
    automated reporting as examples; a person clicking "Play" is not listed.
- **Steam Web API Terms of Use**: https://steamcommunity.com/dev/apiterms , "Last updated
  July 2010".
  - "only retrieve Steam Data about a Steam end user as requested by the end user"
    (fragment)
  - "You agree to keep your Steam Web API key confidential" (fragment); the licence "is
    personal to you and specific to your Application" (fragment). A distributed desktop
    app therefore cannot ship one shared key inside it; Playnite has each user supply
    their own key or sign in.
  - "you will not intercept or store the end user's Steam password on log in" (fragment)
  - "limited to one hundred thousand (100,000) calls to the Steam Web API per day"
    (fragment)
  - Section 5: no public statements that "assert or imply any other relationship with
    Valve" (fragment) without Valve's written approval.
  - Breach reports go to webapi@valvesoftware.com (a reporting address, not a
    permissions channel).
- Steam Web API documentation page, https://steamcommunity.com/dev (no date shown): "All
  use of the Steam Web API requires the use of an API Key." and "Steam can act as an
  OpenID provider." (sign-in with Steam that returns the user's SteamID without the app
  ever seeing the password).

### 2.7 What this adds up to (judgement, for the decider)

- The two lowest-risk pieces of a launcher are local: finding installed games on disk
  and starting them the way the platform itself does (the game's own executable for GOG,
  which GOG says needs no launcher; `steam://rungameid/` for Steam). Playnite has done
  both for years in public under MIT.
- Showing a player's Steam library has an official, documented route (Steam OpenID
  sign-in plus the Web API with the user's own key or token), under 2010 terms that
  require data only "as requested by the end user".
- Showing a player's GOG library has no published route: GOG's own agreement says the
  client protocols are not open yet, and 11.1(e) and (f) bar unauthorized programs that
  emulate GOG's client. Playnite and Heroic both do it anyway through GOG's servers; that
  is prior art, not permission. The official way to get permission is to write to
  inquiries@gog.com, and 11.1(c) promises such requests will be reviewed "in good faith"
  (fragment).
- A GOG affiliate link is an existing, public way for a donation-funded open-source
  launcher to earn from GOG sales (Heroic does it), subject to GOG accepting the channel.

## Could not reach

- https://affiliate.gog.com/ : the rendering browser refused it with
  "ERR_CERT_COMMON_NAME_INVALID" and a plain fetch got no response (status 000). Hoped
  for the programme's own landing page and terms; used the GOG support article and the
  AdTraction listing instead.
- GOG affiliate product-feed API documentation: a Google Drive folder linked from the
  affiliate article. Not opened.
- https://devportal.gog.com/support/contact : redirects to the sign-in page
  (/welcome); the contact link there, https://www.gog.com/support/contact/business,
  redirected to the Support Center home.
- The Instructables Terms on autodesk.com: HTTP 403 "Access Denied" to a plain fetch;
  read in a rendering browser instead. Its "Special Service Terms" link
  (https://www.autodesk.com/adsk/servlet/item?siteID=123112&id=21959739) returns 404.
- Khan Academy Terms of Service: a plain fetch got a "Client Challenge" page; read in a
  rendering browser instead.
- https://examine.com/contact/ and /terms/ : a plain fetch got HTTP 429 (Vercel Security
  Checkpoint); both loaded normally in a rendering browser.
- https://developer.valvesoftware.com/wiki/Steam_browser_protocol : a plain fetch was
  redirected to a "Making sure you're not a bot!" page; loaded normally in a rendering
  browser.
- The iFixit API documentation (https://www.ifixit.com/api-docs) is built by JavaScript
  and was not read; not needed for the question.
- Dates: no date was shown on the GOG support articles, Examine's contact page, the
  Gutenberg pages, iFixit's Licensing Policy, Autodesk's permission and link-consent
  pages, or Heroic's FAQ.

Not legal advice: a reading of public sources on 2026-09-25 by someone who is not a
lawyer.

---

**Not legal advice.** A reading of public sources on 2026-09-25 by people who are not lawyers.
