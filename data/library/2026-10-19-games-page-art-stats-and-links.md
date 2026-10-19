# Games page: artwork, store names, local play records, links out and open metadata

**Researched 19 October 2026** (sources read between about 14:30 and 15:10
Pacific time). Everything quoted below was read that day, or, where a live page
refused the request, read from an Internet Archive copy whose date is given.
Store terms change often. If you are reading this later, check each source's own
date before relying on it.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such.

This finding extends
[`2026-10-13-game-stores-and-a-games-library.md`](/library#2026-10-13-game-stores-and-a-games-library)
(listing installed games and starting them through each store's own link), which
in turn extends
[`2026-09-25-permission-routes-and-launchers.md`](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-09-25-permission-routes-and-launchers.md).
It does not repeat or replace either. Already settled there and not re-argued
here: that no store's terms mention third-party launchers, their launch links or
their install records; Steam's Automation and "unauthorized third-party software"
rules (section 4 of the Steam Subscriber Agreement); Epic's EULA licence
conditions; GOG's rules 11.1(c), (e) and (f) as they bear on starting games;
SteamCMD; and the advice that every store action follows a person's click. The
2026-10-13 finding left "store names and logos" as an open question; section 2
below answers it.

## The question

What may the desktop Games page (`src/gui/pages/games.rs`, the installed-games
list read from Steam's, Epic's and GOG's local install records) add beyond a
list and a Play button, while everything stays on the person's own computer and
nothing signs in to a store? Five parts:

1. **Game artwork**: may it show a game's cover, header, hero image and logo,
   (a) read from files the store's own client already cached on that computer,
   or (b) fetched from a store's servers?
2. **Store names and logos**: may it show "Steam", "Epic Games Store" and "GOG"
   as words, and the stores' logos?
3. **Local play records**: is anything said about reading Steam's playtime,
   achievement, screenshot and Workshop files, Epic's local data, or GOG
   GALAXY's local database?
4. **Links out**: may it open, on a click, a store's page for a game, the Steam
   community hub, PCGamingWiki or SteamDB?
5. **Open metadata**: if it ever fetched descriptions, genres or release dates,
   which sources allow that, on what terms, and what does fetching reveal?

## Short answer

1. **Artwork.** The artwork belongs to each game's publisher, and the three
   stores say so. No store's terms or developer documentation mention another
   program showing it, for or against. Valve documents the library images as
   made for display "in customer's libraries and in the community", but does
   not document the Steam client's image cache at all. US copyright law gives
   the owner the right to display a work *publicly*; showing it to the person on
   their own screen is not public by the statute's definition. **Reading the art
   in place from the store's own cache and showing it only to that person is the
   lowest-risk version found.** Fetching from a store's servers has no grant
   from any store (Valve's only licence for "Steam Data" needs a Web API key
   that must be kept confidential), though Playnite does it.
2. **Names and logos.** Valve's brand guidelines say: "Nothing in these
   guidelines conveys any rights to use any Steam trademarks or logos. To make
   such use, you must receive a separate license from Valve." Epic's only public
   rules on its marks (its Fan Content Policy) forbid altering them and using
   them to identify or promote anything else. No GOG brand guidelines were found.
   US trademark law lets a person name another's product with a plain word when
   there is no other way to identify it, and the leading case contrasts naming
   "Coca-Cola" with using its "distinctive lettering". **Words, yes; logos, no.**
3. **Local play records.** **Not addressed** anywhere for Steam or Epic: no
   agreement or developer page read mentions `localconfig.vdf`, the stats files,
   the screenshots folder or the Workshop files. Valve's developer documentation
   only confirms that the client "keeps a local cache of the stats and
   achievement data" and a "local screenshot library". **GOG GALAXY is
   different:** its licence says GALAXY's "databases, data" belong to GOG and
   "You may not use or exploit any part of GOG GALAXY except as explained in this
   Agreement." Reading GALAXY's database leans against that text.
4. **Links out.** Nothing read forbids opening a store's or a fact site's page on
   a person's click. Valve's community wiki documents `steam://store/<id>`; Epic documents
   store deep links; GOG's own help centre sends readers to PCGamingWiki, which
   offers a redirect by Steam App ID or GOG ID. SteamDB forbids scraping, not
   linking.
5. **Open metadata.** Wikidata's structured data is CC0 (public domain), needs no
   key and maps Steam, GOG, Epic, PCGamingWiki and IGDB IDs. PCGamingWiki is
   CC BY-NC-SA 3.0 and its data API now needs a login. IGDB needs a secret key
   that must be kept secret, so an open-source desktop app cannot carry one.
   Steam's `appdetails` endpoint is not documented by Valve; the community page
   that describes it says it is "not meant for public consumption". **Any fetch
   tells that site which games the person has.**

## How each finding is recorded

Each finding gives the source's address, the source's own date where it shows
one, and a quotation of the words that carry the meaning. Long passages are
trimmed to the words that matter; "(fragment)" marks a quotation that is part of
a longer sentence, and nothing inside a quotation is reworded. Every quotation
was checked by script against the text read on the day.

## 1. Game artwork

### 1.1 What Valve's developer documentation says the library images are for

Source: Steamworks documentation, "Library Assets",
https://partner.steamgames.com/doc/store/assets/libraryassets (no date shown).
Companion pages: "Graphical Assets - Overview",
https://partner.steamgames.com/doc/store/assets (carries a note dated "August,
2024"), and "Graphical Asset Rules",
https://partner.steamgames.com/doc/store/assets/rules (rules effective
"Sept 1, 2022").

- "As part of the release process on Steam, you will be required to deliver
  various graphical assets for your product. These assets are displayed in
  customer's libraries and in the community."
- Library Capsule: "The library capsule is the primary way of presenting your
  game within the Steam Library, including on the library overview and within
  collections." Size: "600px x 900px".
- Library Hero: "Appears at the top of a user’s library details page for this
  product." Size: "3840px x 1240px".
- Library Logo: "This image should contain only your game's logo type"
  (fragment).
- Rules page: "Library Capsule - this image must include your game logo."

These pages are written for developers delivering art to Valve. They say who
makes the images and where Valve shows them. **They say nothing about any other
program showing them, for or against.**

### 1.2 Who owns the artwork, in each store's own words

- **Steam.** Subscriber Agreement, https://store.steampowered.com/subscriber_agreement/
  ("This Agreement was last updated on September 10, 2026"). Section 1.B makes
  "Content and Services" cover "any other software, content, and updates you
  download or access via Steam" (fragment, recorded in the 2026-10-13 finding).
  Section 2.F: "All title, ownership rights and intellectual property rights in
  and to the Content and Services and any and all copies thereof, are owned by
  Valve and/or its or its affiliates’ licensors." Section 2.A grants the person
  "a non-exclusive license and right, to use the Content and Services for your
  personal, non-commercial use" (fragment).
- **Epic.** Epic Games Store EULA, https://legal.epicgames.com/store/eula ("Last
  Updated January 15, 2025"), section 6: "As between you and Epic, Epic and its
  licensors own all title, ownership rights, and intellectual property rights in
  the Software and Services."
- **GOG.** User Agreement,
  https://support.gog.com/hc/en-us/articles/212632089-GOG-User-Agreement ("Last
  update (effective date): 9 March 2026"), 10.1: "GOG content is owned by its
  developers/publishers and licensed by us."

So the copyright in a game's cover, hero and logo art belongs to the game's
publisher or developer (Valve, Epic or GOG only for their own games). That is a
separate question from the stores' trademarks, which section 2 covers.

### 1.3 Copyright law: showing a picture privately, and making copies (United States)

Source: 17 U.S.C. §§ 101 and 106, as published by Cornell's Legal Information
Institute, https://www.law.cornell.edu/uscode/text/17/106 and
https://www.law.cornell.edu/uscode/text/17/101 (current text; the notes quoted
are from House Report No. 94-1476, 1976).

- § 106 gives the owner the exclusive rights "to reproduce the copyrighted work
  in copies or phonorecords" (clause 1) and, for pictorial and graphic works,
  "to display the copyrighted work publicly" (clause 5).
- § 101: "To perform or display a work “publicly” means—" (1) "at a place open
  to the public or at any place where a substantial number of persons outside
  of a normal circle of a family and its social acquaintances is gathered"
  (fragment); or (2) "to transmit or otherwise communicate a performance or
  display of the work to a place specified by clause (1) or to the public"
  (fragment).
- House Report notes on § 106: "The term “a family” in this context would
  include an individual living alone, so that a gathering confined to the
  individual’s social acquaintances would normally be regarded as private."
- Same notes: "the showing of images on a screen or tube would not be a
  violation of clause (1), although it might come within the scope of clause
  (5)." (fragment)

Read together: showing a cover to the person at their own computer is not a
*public* display, and putting a picture on a screen is not, by itself, a
reproduction. Saving a further copy of the image (for example into HumanityOS's
own cache folder) is a reproduction question; sending it anywhere others can see
it is a public-display or distribution question. Whether a program's temporary
in-memory copy counts as a reproduction under case law was **not researched**
(see "What is still unknown"). Other countries' laws were not researched.

### 1.4 The Steam client's own image cache

The Steam client keeps library images on disk (the operator's brief gives
`appcache/librarycache/<appid>/` with `library_600x900.jpg`, `library_hero.jpg`,
`logo.png` and `header.jpg`; the layout was not checked on a machine for this
finding). **Negative finding:** no Valve page read mentions `librarycache` or
`appcache` (searched Valve's Steamworks documentation and Steam's help results;
the only hits were community forum posts). The Subscriber Agreement does not
mention local files at all (2026-10-13 finding, 1.4).

### 1.5 Fetching artwork from a store's servers

- **Steam, through the Web API.** The Steam Web API Terms of Use,
  https://steamcommunity.com/dev/apiterms ("Last updated July 2010"), section 2
  lets a licensee "distribute Steam Data to end users for their personal use via
  your Application" (fragment), and adds: "You agree to keep your Steam Web API
  key confidential, and not to share it with any third party." Section 9: "The
  Steam Web API, Steam Data, and Valve Brand & Links are the property of Valve,
  and subject to the intellectual property rights of Valve and its licensors."
  Valve's Web API page, https://steamcommunity.com/dev (no date shown): "All use
  of the Steam Web API requires the use of an API Key."
- **How Web API users build image addresses.** Valve Developer Community wiki,
  "Steam Web API", https://developer.valvesoftware.com/wiki/Steam_Web_API ("This
  page was last edited on 11 April 2026, at 10:47."; read from the Internet
  Archive copy of 19 October 2026,
  https://web.archive.org/web/20261019092328/https://developer.valvesoftware.com/wiki/Steam_Web_API ):
  "To construct the URL to the image, use this format:
  https://media.steampowered.com/steamcommunity/public/images/apps/{appid}/{hash}.jpg."
  The wiki is hosted by Valve but written by its users.
- **Steam, with no key.** The store's `appdetails` endpoint returns image
  addresses; it is covered in 5.4 (not documented by Valve). No Valve page read
  grants a program that holds no Web API key a right to fetch store or CDN
  images, and none forbids it in terms; the general Automation rule settled in
  the 2026-10-13 finding is the nearest text.
- **GOG.** "How to join the GOG Affiliate Program",
  https://support.gog.com/hc/en-us/articles/4405004689297 (no date shown): "If
  you need a product feed, feel free to use our API (here). Note that there is a
  200 request/hour/IP on the api.gog.com/products/* endpoint." (fragment) The
  article is addressed to affiliates; GOG's own User Agreement 11.1(e) also lists
  "spiders" among software that may not "interact with or affect GOG services"
  (2026-10-13 finding). Both are GOG's words; see "Where the sources pull in
  different directions".
- **Epic.** **Negative finding:** no Epic page was found that documents store art
  or a store catalogue for third parties. Epic's developer documentation for
  store presence was not found by search, and the store's own data addresses
  that Playnite uses (section 6) are not documented on any Epic page read.

### 1.6 Free-licence sources do not carry cover art

Source: "Commons:Licensing", https://commons.wikimedia.org/wiki/Commons:Licensing
("This page was last edited on 29 September 2026, at 09:58."). "Wikimedia
Commons only accepts free content" (fragment); "Wikimedia Commons does not
accept fair use justifications". So Wikidata and Wikimedia Commons, the free
sources in section 5, are not a source of cover art for commercially published
games, whose covers are not freely licensed.

## 2. Store names and logos

### 2.1 Steam

- Steamworks documentation, "Steam Branding Guidelines",
  https://partner.steamgames.com/doc/marketing/branding (no page date; the linked
  Brand Guidelines are marked "Last Updated: December 2024"):
  - "The Steam logo may not be used in any manner that might imply that any
    non-Valve materials are sponsored, endorsed, licensed by, or affiliated with
    Valve."
  - "The Steam trademarks may not be displayed as primary or prominent features
    on any non-Valve materials."
  - "Use only approved Valve artwork for the logo."
- Steam Brand Guidelines for Software, Hardware & Games (PDF),
  https://shared.fastly.steamstatic.com/community_assets/images/steamworks_docs/english/steam_brandGuidelines.pdf
  (its attribution line is dated 2024), page 5: "Nothing in these
  guidelines conveys any rights to use any Steam trademarks or logos. To make
  such use, you must receive a separate license from Valve." And: "When in doubt
  please send artwork for approval to steambranding@valvesoftware.com".
- Steam Web API Terms of Use (1.5 above), section 3: Valve grants a licence "to
  implement the Valve name(s), logo(s), and links to Valve" (fragment), but only
  "on any Web page incorporating the Steam Web API and/or Steam Data" (fragment),
  and section 2 says Steam Data may not be presented "so that it appears (a) that
  your Application is endorsed or affiliated with Valve or Steam" (fragment).
- Steam's legal page, https://store.steampowered.com/legal/ (no date shown),
  lists "Steam, the Steam logo" (fragment) among Valve's trademarks.

### 2.2 Epic

- **Negative finding:** no Epic Games Store brand or logo guidelines for third
  parties were found (searched Epic's legal site and developer documentation).
- Epic's Fan Content Policy, https://legal.epicgames.com/epicgames/fan-art-policy
  (no date shown on the page; its page record carries an update time of 19
  August 2025), is the only public statement found on Epic's marks:
  - "In most cases, using our games, artwork, videos, music, characters, logos,
    and other intellectual property (“Epic IP”) without our permission is illegal
    and a violation of our rights."
  - It covers "(ii) Epic IP-related websites and apps that are freely accessible
    to the public." (fragment)
  - "Marks must be used only in connection with discussion of the Epic products
    or business they represent. Do not use Marks to identify or promote you, your
    Fan Content, or any other products or business."
  - "Do not alter any Marks."
  - "1.6 You may not say nor imply that your Fan Content is endorsed or approved
    by Epic."
- Epic Games Store EULA (1.2 above), section 6: "Epic, Epic Games, Unreal, Unreal
  Engine, and their respective logos, are trademarks or registered trademarks of
  Epic and its affiliates in the United States of America and elsewhere."

### 2.3 GOG

- **Negative finding:** no GOG brand, logo or press-kit guidelines were found
  (https://www.gog.com/press and https://www.gog.com/en/legal answered 404;
  searches found only third-party logo sites).
- GOG's site footer, read on the same pages: "All trademarks and registered
  trademarks are the property of their respective owners."
- User Agreement 11.1(j): "We ask you not to do anything in connection with GOG
  that infringes any copyright, trademark, patent, trade secret, privacy,
  publicity, or other right of others" (fragment).

### 2.4 Trademark law: naming someone else's product (United States)

- 15 U.S.C. § 1125(a)(1)(A), https://www.law.cornell.edu/uscode/text/15/1125 :
  liability for a use in commerce that "is likely to cause confusion, or to cause
  mistake, or to deceive as to the affiliation, connection, or association of
  such person with another person" (fragment).
- *New Kids on the Block v. News America Publishing, Inc.*, 971 F.2d 302 (9th
  Cir. 1992), decided 24 July 1992, read at https://openjurist.org/971/f2d/302 .
  The Ninth Circuit (which covers Washington, where the operator lives) set out
  the test for naming another's product, now called nominative fair use:
  - "First, the product or service in question must be one not readily
    identifiable without use of the trademark;"
  - "second, only so much of the mark or marks may be used as is reasonably
    necessary to identify the product or service;" (fragment)
  - "and third, the user must do nothing that would, in conjunction with the
    mark, suggest sponsorship or endorsement by the trademark holder."
  - Footnote 7, on words against logos: "Thus, a soft drink competitor would be
    entitled to compare its product to Coca-Cola or Coke, but would not be
    entitled to use Coca-Cola's distinctive lettering."

The Ninth Circuit's current model jury instruction on this defence could not be
read (see "Sources that could not be reached"); the case itself was.

## 3. Local play records: playtime, achievements, screenshots, Workshop items

### 3.1 Steam

Files in question: `userdata/<id>/config/localconfig.vdf` (playtime, last
played), `appcache/stats/UserGameStats_*.bin` and `UserGameStatsSchema_*.bin`
(achievements), `userdata/<id>/760/remote/<appid>/screenshots` (the person's own
screenshots), `steamapps/workshop/appworkshop_<appid>.acf` and
`steamapps/workshop/content` (subscribed Workshop items).

- **Negative finding: the Subscriber Agreement does not mention any of these
  files or folders**, nor the words `userdata` or `localconfig`, nor
  screenshots kept on the computer, nor reading the client's data (searched on
  the day). The 2026-10-13 finding found the same for `appmanifest` files and
  `libraryfolders.vdf`.
- **Valve's developer documentation acknowledges the local data, for
  developers only:**
  - "Stats and Achievements", https://partner.steamgames.com/doc/features/achievements
    (no date shown): "Steam keeps a local cache of the stats and achievement data
    so that the APIs can be used as normal in offline mode."
  - "Steam Screenshots", https://partner.steamgames.com/doc/features/screenshots
    (no date shown): the Screenshots API lets a game "take screenshots and add
    them to a users local screenshot library" (fragment, Valve's spelling).
  - "Steam Workshop Implementation Guide",
    https://partner.steamgames.com/doc/features/workshop/implementation (no date
    shown), has a section "Retrieving information about the local copy of the
    Workshop Item", through the game's own API.
  None of these says anything about other programs reading the files. None names
  the paths above; the file formats of the stats files are not documented on any
  Valve page found.
- **The nearest rule is about writing, not reading.** Section 4.C (Automation)
  lists among forbidden uses "Faking gameplay statistics (e.g., inflated wins or
  losses, XP, playtime)," (fragment). Changing the playtime or achievement files
  would come close to those words; reading them is not mentioned.
- **Workshop items are the person's own Subscriptions.** Section 6.B: "Workshop
  Contributions are Subscriptions, and therefore you agree that any Subscriber
  receiving distribution of your Workshop Contribution will have the same rights
  to use your Workshop Contribution" (fragment).
- **Valve's licence to a person's screenshots starts only on upload.** Section 6.A:
  "When you upload your content to Steam to make it available to other users
  and/or to Valve, you grant Valve and its affiliates the worldwide,
  non-exclusive right" (fragment). A screenshot sitting in the local folder has
  not been uploaded.

### 3.2 Epic

**Not addressed.** Neither the Epic Games Store EULA nor Epic's Terms of Service
mentions local files, playtime or achievements stored on the computer (searched
on the day). No Epic page documents a local playtime or achievement file; the
playtime Playnite shows for Epic comes from Epic's account service after a
sign-in (section 6), not from a local file.

### 3.3 GOG GALAXY's local database

GOG GALAXY keeps a local SQLite database; Playnite's source opens it at
`c:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db` (section 6).

- GOG GALAXY Licence Agreement,
  https://support.gog.com/hc/en-us/articles/360023617474-GOG-GALAXY-Licence-Agreement
  ("Last update : 30th January 2026"):
  - "GOG GALAXY including (but not limited to) its graphics, computer code, user
    interface, look and feel, audio, video, text, layout, databases, data and all
    other content" (fragment) are "either owned by us or we license them from
    third parties" (fragment).
  - "You may not use or exploit any part of GOG GALAXY except as explained in
    this Agreement."
  - What GALAXY itself does with other platforms: "you can import your
    achievements, game and friends lists from such an external platform to GOG
    GALAXY" (fragment), when the person links that account.
- GOG User Agreement 11.1(e) (2026-10-13 finding, 3.1) ends with "any
  unauthorized third party programs that collect information about GOG services"
  (fragment). GOG GALAXY is one of the "GOG services" (1.1).
- GOG Privacy Policy, https://support.gog.com/hc/en-us/articles/212632109-Privacy-Policy
  ("Last updated (effective date): 9 March 2026"), 4.3: GALAXY collects
  "information about achievements in games you play and how long you play them"
  (fragment). Whether the local database holds those records was not checked.

**This is the one place in this finding where the words lean against the
idea**: GALAXY's database is, by GALAXY's licence, part of GALAXY and GOG's, and
using it is limited to what that licence explains, which does not include other
programs reading it.

## 4. Links out

### 4.1 Steam store and community

- Valve Developer Community wiki, "Steam browser protocol" ("This page was last
  edited on 14 July 2024, at 06:43."; read from the Internet Archive copy of 17
  October 2026,
  https://web.archive.org/web/20261017231231/https://developer.valvesoftware.com/wiki/Steam_browser_protocol ):
  "steam://store/<id>" "Opens up the store for an app, if no app is specified
  then the default one is opened." Also lists `steam://url/GameHub/<app ID>`
  (the community hub).
- The Steam Web API terms (1.5), section 3, for those who use the Web API: "You
  shall not tag links to Valve hereunder with a "nofollow" attribute or otherwise
  prevent or discourage search engines from following or scoring the link."
  This is a rule for Web API users, but it shows Valve wants links to it.
- https://store.steampowered.com/robots.txt and https://steamcommunity.com/robots.txt
  (read on the day) do not exclude `/app/` pages.
- **Negative finding:** nothing in the Subscriber Agreement or the Web API terms
  restricts linking to a store or community page.

### 4.2 Epic

- Epic Developer documentation, "Protocol Activation",
  https://dev.epicgames.com/docs/epic-games-store/protocol-activation (no date
  shown): "Activation parameters can be used to launch directly to specific
  Product Details Pages (PDPs), a process also known as deep-linking."
  (fragment) and "The path portion of a store URI will match the product slug
  that is available on the web." Its example maps
  `https://www.epicgames.com/store/product/[product slug]` to
  `com.epicgames.launcher://store/product/[product slug]`.
- https://store.epicgames.com/robots.txt (read on the day) excludes account,
  cart and library pages, not product pages.
- **Negative finding:** neither Epic document read restricts linking.

### 4.3 GOG

- GOG's own help centre pages (the User Agreement and the affiliate article,
  among others) carry a banner reading "Check out additional solutions and
  resources on the PCGamingWiki", linking to
  `https://www.pcgamingwiki.com/w/index.php?search=`. GOG itself sends its
  readers to that site.
- User Agreement 15.1 is about GOG's links outward: "You might get links to
  third-party websites or content through GOG services which aren’t connected to
  us." (fragment)
- **Negative finding:** nothing read restricts linking to a GOG store page.

### 4.4 PCGamingWiki

- "PCGamingWiki:API", https://www.pcgamingwiki.com/wiki/PCGamingWiki:API ("This
  page was last edited on 21 September 2026, at 16:03."): "Our custom redirect
  API allows us to redirect incoming users using any registered GOG Product ID or
  Steam App ID to the relevant PCGamingWiki page." Examples given:
  `https://www.pcgamingwiki.com/api/appid.php?appid=674020` and
  `https://www.pcgamingwiki.com/api/gog.php?page=1785384169`.
- https://www.pcgamingwiki.com/robots.txt (read on the day) disallows `/w/` and
  `/api/` for automated agents. robots.txt speaks to crawlers; a person following
  a link is not one.
- General disclaimer, https://www.pcgamingwiki.com/wiki/PCGamingWiki:Disclaimer
  ("This page was last edited on 11 November 2020, at 19:33."): nothing about
  inbound links. **Negative finding:** no PCGamingWiki terms of use page exists
  (`PCGamingWiki:Terms_of_use` answered 404).

### 4.5 SteamDB

- SteamDB's FAQ, https://steamdb.info/faq/ (no date shown): "SteamDB is not
  affiliated with Valve or Steam." On automated access: "We also do not allow
  scraping/crawling on SteamDB." (fragment)
- https://steamdb.info/robots.txt (read on the day) allows `/app/` pages for
  crawlers in general.
- **Negative finding:** nothing read restricts a person following a link to a
  SteamDB page. The rule that exists is against programs fetching its pages.

## 5. Open metadata (descriptions, genres, release dates)

### 5.1 Wikidata

- "Wikidata:Licensing", https://www.wikidata.org/wiki/Wikidata:Licensing ("This
  page was last edited on 24 September 2026, at 03:42."): "All structured data
  (i.e. the main, Property, Lexeme, and EntitySchema namespaces) is released into
  the public domain under Creative Commons Zero."
- Wikidata has properties for the store and site IDs a Games page already holds
  (checked through Wikidata's own API on the day): P1733 "Steam application ID",
  P2725 "GOG application ID", P6278 "Epic Games Store ID", P6337 "PCGamingWiki
  ID", P5794 "Internet Game Database game ID"; and for the facts wanted: P136
  "genre", P577 "publication date", P178 "developer", P123 "publisher". A search
  for `haswbstatement:P1733=620` returned one item (Q279446, Portal 2), so a
  lookup by Steam App ID works.
- "Wikimedia Foundation API Usage Guidelines",
  https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_API_Usage_Guidelines
  ("This page was last edited on 21 February 2026, at 05:54."): an operator
  "must" "Follow the User-Agent policy and otherwise correctly label user
  agents;" (fragment), and must not "Request data at a high rate, far beyond
  common use cases" (fragment). "Operators should use our APIs within the
  guidelines described in this policy" (fragment), and the policy "does not
  require members of the Wikimedia community to get prior permission from the
  Wikimedia Foundation before using the APIs in a manner consistent with this
  policy." (fragment)
- "Wikimedia Foundation User-Agent Policy",
  https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy
  ("This page was last edited on 27 March 2026, at 17:17."): "Scripts should use
  an informative User-Agent string with contact information, or they may be
  blocked without notice."
- Wikidata is not a source of cover art (1.6). Its descriptions are one-line
  labels, not store blurbs: Q279446's English description, read through the API
  on the day, is "2011 first-person puzzle-platform video game".

### 5.2 PCGamingWiki

- Licence: "Content is available under Creative Commons Attribution
  Non-Commercial Share Alike unless otherwise noted." (footer of every page;
  "PCGamingWiki:Copyrights" was "last edited on 5 September 2022"), linking to
  CC BY-NC-SA 3.0. What "NonCommercial" means for a donation-funded project is
  unsettled; see
  [`2026-10-12-media-item-licences.md`](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-10-12-media-item-licences.md),
  section 4.
- "PCGamingWiki:API" (4.4):
  - "As of the August 2026 server migration some API endpoints requires
    authenticating with the API endpoint using a bot password and the login
    method." (fragment, the site's grammar)
  - "Current rate limit: 60 requests/minute."
  - "Use a proper custom user-agent string, preferably one with contact
    information, when interfacing with our APIs." (fragment)
  - "remember that this is a small community-run project, with infrastructure
    not designed to handle modern AI scrapping and bots." (fragment, the site's
    spelling)
- Cover images that PCGamingWiki stores are the publishers' art ("unless
  otherwise noted" in the licence line); the CC licence does not cover them.

### 5.3 IGDB (Twitch)

- IGDB API documentation, https://api-docs.igdb.com/ (no date shown):
  - "The IGDB.com API is free for non-commercial usage under the terms of the
    Twitch Developer Service Agreement."
  - "The Client Type must be set to Confidential to generate Client Secrets"
  - "There is a rate limit of 4 requests per second."
  - "Interested about using the API for a commercial project? No problem, we
    allow commercial usage." (fragment)
- Twitch Developer Services Agreement,
  https://legal.twitch.com/legal/developer-agreement/ ("Last modified on"
  "12/04/2024"):
  - IV.B, Keys: "All activities that occur using your Keys are your
    responsibility. Keep them secret." (fragment)
  - VII.C: "You must ensure that there is a clear path to the source from
    displays of Program Materials in Your Services."
  - "Program Materials" include "data, metadata" (fragment, section II.A,
    definition 3).
  - Schedule 1, which applies "if you use the Twitch APIs" (fragment): "Do not
    store copies of Twitch Content or Program Materials, unless you" (fragment)
    have permission, control the rights, or "cache such information for only a
    twenty-four hour time period without further sharing it with third parties."
    (fragment) And: "Re-syndication and re-distribution of Program Materials or
    data as available from a Twitch API is prohibited." And: "You must provide a
    publicly available and easily accessible privacy policy or notice" (fragment).

### 5.4 Steam's `appdetails` endpoint

- **Negative finding: not documented by Valve.** `store.steampowered.com/api/appdetails`
  appears on no Steamworks documentation page found (Valve's Web API reference
  and a site-restricted search of partner.steamgames.com, on the day).
- The page that documents it is a user page on the community-run Official TF2
  Wiki, "User:RJackson/StorefrontAPI",
  https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI ("This page was
  last edited on 1 September 2020, at 18:23."): "This API is not actively
  documented, under development, and not meant for public consumption. It may
  change at any time without notice."
- What Valve does document for store data is `IStoreService/GetAppList`,
  https://partner.steamgames.com/doc/webapi/IStoreService (no date shown): "These
  service methods provide public information from the Steam Store." Its `key`
  parameter is required and described as "Any web API key." That brings in the
  Web API terms (1.5): a key kept confidential, and data retrieved about a user
  only "as requested by the end user" (fragment).
- https://store.steampowered.com/robots.txt does not exclude `/api/`. That
  speaks to crawlers only; it is not a permission.

### 5.5 What any fetch tells the site

Every request for a game's details names the game. Asking a site about each
installed game sends that site the person's list of games, together with their
network address. The Wikimedia Privacy Policy,
https://foundation.wikimedia.org/wiki/Policy:Privacy_policy ("This page was last
edited on 21 February 2026, at 05:29."), states the general case plainly: "when
you visit any Wikimedia Site, we automatically receive the IP address of the
device (or your proxy server) you are using to access the Internet" (fragment).
The same holds for every other site in this section, and for a store's image
servers. A key-holding relay run by the project (Playnite's pattern for IGDB,
section 6) would put the project itself in that position for every user.

## 6. Prior practice: Playnite (what is common practice, not what is allowed)

Playnite is an open-source game library manager under the MIT licence. Its
documentation, https://api.playnite.link/docs/manual/library/games/metadata.html
(no date shown): "Metadata is generally obtained with metadata providers as the
form of Metadata extensions, which obtain this data from third party services to
Playnite to add to imported games." and "By default Playnite will automatically
obtain and apply to your games when you import them." (the documentation's
wording).

Its store plugins' source code was read on the day at
https://codeberg.org/CrowIsTaken/PlayniteExtensions , branch `main10`, latest
commit `f008798` of 18 October 2026 ("Epic 2.34 release"):

- **Steam covers and data:** `SteamShared/MetadataProvider.cs` fetches art from
  Valve's CDN (`https://steamcdn-a.akamaihd.net/steam/apps/{0}/library_600x900_2x.jpg`,
  `.../header.jpg`, `.../library_hero.jpg`) and details from
  `https://store.steampowered.com/api/appdetails?appids=` (`WebApiClient.cs`). It
  does not read the client's `librarycache` (no reference to it in the source).
- **Steam last played:** `Services/SteamLocalService.cs` reads
  `userdata/<account>/config/localconfig.vdf` read-only for each game's last
  activity; playtime comes from the Web API with the person's own key or token.
- **Steam links:** the store page `https://store.steampowered.com/app/{appId}`,
  the community hub `https://steamcommunity.com/app/{appId}`, discussions,
  guides, news, and PCGamingWiki's redirect
  `https://pcgamingwiki.com/api/appid.php?appid={appId}`.
- **GOG:** `GogLibrary.cs` opens GALAXY's `galaxy-2.0.db` read-only
  (`SQLITE_OPEN_READONLY`) to list installed games from its
  `InstalledBaseProducts` and `LimitedDetails` tables; art and descriptions come
  from `api.gog.com/products/<id>`; it links to PCGamingWiki by title search.
- **Epic:** names from the launcher's local catalogue cache
  (`%LOCALAPPDATA%\com.epicgames.eglv2\cache\prod\catalog-cache.json`); art,
  playtime and catalogue data from Epic's account and store web services (the
  account ones after a sign-in, `Services/EpicAccountClient.cs`,
  `Services/WebStoreClient.cs`); and, because (in the code's own comment)
  "There's not icon available on Epic servers", the icon is read from the
  installed game's own executable.
- **IGDB:** the IGDB metadata plugin's `plugin.cfg` points at Playnite's own
  server, `https://api2.playnite.link/api/`, which holds the Twitch credentials,
  so users never handle a key and every lookup passes through Playnite's server.
- **Store logos:** each library plugin ships a store icon
  (`Resources/steamicon.png`, `epicicon.png`, `gogicon.png`).

This shows what a widely used open-source launcher does in public. It is not
permission, and some of it (the GALAXY database, store logos, undocumented store
endpoints) is exactly what the stores' words above do not cover.

## Where the sources pull in different directions

- **GOG's product API.** GOG's affiliate article says "feel free to use our API"
  for a product feed; GOG's User Agreement 11.1(e) lists "spiders" among software
  that may not interact with GOG services. Our reading (a judgement): the article
  is GOG's permission for its affiliates' product feeds, not a general licence
  for any program to fetch GOG data, and 11.1(e) is aimed at cheats, bots and
  data collection. A non-affiliate fetching art from that endpoint sits between
  the two.
- **Which Steam Web API calls need a key.** Valve's Steamworks reference lists
  some Web API methods without a `key` parameter (for example
  `ISteamRemoteStorage/GetPublishedFileDetails`, which returns Workshop item
  titles); Valve's Web API landing page says "All use of the Steam Web API
  requires the use of an API Key." We take the landing page and the terms as the
  rule, since the terms are the contract.
- **Whether Twitch's Schedule 1 applies to IGDB.** IGDB says its API is used
  "under the terms of the Twitch Developer Service Agreement", but Schedule 1
  defines "Twitch APIs" as materials "specifically found here:
  https://dev.twitch.tv" (fragment), and IGDB's documentation is at
  api-docs.igdb.com. If Schedule 1 applies, IGDB data may be cached for only 24
  hours. The body of the agreement (keep keys secret, show a clear path to the
  source) applies either way.
- **Epic's Fan Content Policy as the rule for store names.** The policy is
  written for fan art and fan sites about Epic's own games; a library that says
  a game came from the Epic Games Store is not obviously "Fan Content". It is
  still the only published statement of how Epic wants its marks used, so we
  follow its rules against altering marks and implying endorsement anyway.

## What is still unknown, and what would settle it

- **Whether any store minds a launcher showing the art its own client cached.**
  Not addressed by any text. Settled by asking each store; Valve's only published
  brand contact is steambranding@valvesoftware.com (for logo artwork), GOG's
  business address is inquiries@gog.com (2026-09-25 finding), and Epic's
  developer pages send developers to its partner teams. Cost: one message each
  and a wait; a reply may never come.
- **Whether a program's in-memory or cached copy of an image is a
  "reproduction"** under US case law, and how other countries treat it. Not
  researched. Settled by a copyright lawyer, or by a finding that reads the case
  law (starting with the Ninth Circuit's). Until then, reading in place and
  never saving a further copy keeps the question small.
- **Whether reading Steam's binary stats files is "reverse engineering"** under
  the Subscriber Agreement's 2.G (which reaches "the Content and Services or any
  software accessed via Steam"; 2026-10-13 finding). Reading a data file is not
  obviously reverse engineering software, but Valve does not document the
  format. Settled by Valve in writing, or a lawyer. Low cost to avoid: leave
  achievements until there is a reason to need them.
- **Whether GOG would let a launcher read GALAXY's database read-only.** Settled
  by writing to inquiries@gog.com; 11.1(c) promises requests are reviewed "in
  good faith" (2026-09-25 finding).
- **Whether Epic has store brand guidelines for third parties.** Not found
  publicly. Settled by asking Epic; no public channel for it was found.
- **Whether HumanityOS counts as "NonCommercial"** for PCGamingWiki's licence, if
  it ever fetched and showed PCGamingWiki text. Settled by asking PCGamingWiki
  (it asks for contact through Discord) or by the reading in the 2026-10-12
  finding. Not needed for links.
- **How many games Wikidata actually covers by Steam, GOG or Epic ID.** Settled by
  a one-off count through Wikidata's query service; costs an hour.
- **The exact layout of Steam's current library image cache.** Not checked on a
  machine for this finding (Steam was not found at its default path on the
  research computer). Settled by looking on a machine with Steam.

## Sources that could not be reached

- https://developer.valvesoftware.com/wiki/Steam_Web_API and
  https://developer.valvesoftware.com/wiki/Steam_browser_protocol : the live pages
  answered with the "Making sure you're not a bot!" proof-of-work check (Anubis),
  which says it is there to stop AI scraping. Not passed. Read from Internet
  Archive copies of 19 October and 17 October 2026, whose "last edited" lines
  (11 April 2026 and 14 July 2024) show they are the current revisions.
- https://www.ce9.uscourts.gov/jury-instructions/civil/chapter-15/15-26-defenses-nominative-fair-use/
  (the Ninth Circuit's model jury instruction on nominative fair use, found by
  search): HTTP 404, and no Internet Archive copy. Hoped for the court's current
  statement of the test; the 1992 case was read instead. Also tried for the case:
  law.justia.com (403), CourtListener (its API needs a login), law.resource.org
  (404).
- https://www.igdb.com/terms and https://www.igdb.com/api : HTTP 403. Hoped for
  IGDB's own terms page; the API documentation and the Twitch agreement it names
  were read instead.
- https://www.gog.com/press and https://www.gog.com/en/legal : HTTP 404. Hoped for
  brand guidelines; none found anywhere.
- https://legal.epicgames.com/store/terms-of-service : HTTP 404 (no such page;
  the store's terms are its EULA).
- https://www.pcgamingwiki.com/wiki/PCGamingWiki:Terms_of_use : HTTP 404 (no such
  page).
- Epic Games Store developer documentation on store art: not found by search; it
  may sit behind Epic's developer portal sign-in.
- A Valve help article on the screenshots folder: not found by search (only
  third-party guides and forum posts).

## What it means for HumanityOS

### Certain (stated by the sources above)

- The artwork belongs to each game's publisher or developer; Steam, Epic and GOG
  each say so in their own terms. The stores' trademarks are a separate matter.
- No store's agreement or developer page read mentions another program showing
  cached art, reading the client's local play records, or linking to store
  pages, for or against. Valve's developer pages acknowledge the local stats
  cache and the "local screenshot library", for developers.
- US copyright law reserves *public* display to the owner, and defines public so
  that a person alone at home is private. Clause (2) of the definition makes
  transmitting a display to the public a public display.
- Valve's brand guidelines convey no right to use Steam's logos and require a
  separate licence; Epic's only public rules on its marks forbid altering them
  and implying endorsement; no GOG brand rules were found.
- The Ninth Circuit allows naming another's product with only as much of the mark
  as is needed and nothing suggesting sponsorship, and distinguishes a plain name
  from a company's "distinctive lettering".
- Steam's Subscriber Agreement forbids "Faking gameplay statistics"; it says
  nothing about reading them.
- GOG GALAXY's licence makes GALAXY's "databases, data" GOG's and limits their
  use to what the licence explains.
- Valve's community wiki documents `steam://store/<id>`; Epic documents store
  deep links; PCGamingWiki offers a redirect by Steam App ID and GOG ID;
  GOG's help centre links to PCGamingWiki; SteamDB forbids scraping.
- Wikidata's structured data is CC0 and reachable under the Wikimedia API rules
  without a key; PCGamingWiki is CC BY-NC-SA 3.0 with a login now needed for its
  data API; IGDB needs a secret key kept secret; Steam's `appdetails` is not
  documented by Valve; Steam's documented store data needs a confidential key.
- Any fetch of details or art tells the site which games the person has, with
  their network address.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **Artwork: show what the store's own client already put on the disk, read in
   place, to this person only.** For Steam, read the images from the client's
   library cache when the page is open and draw them; never copy them into a
   HumanityOS cache, never upload, sync or send them anywhere, and never put them
   on an in-world screen or anything other players on a server can see (that
   would be the transmission "to the public" covered by clause (2) of the
   definition in 1.3). For Epic and GOG, whose clients' image caches this
   finding did not look for, use the icon Windows itself shows for the game's
   own executable (what Playnite does for Epic), or a plain text tile. **Do
   not fetch art from any store's servers in the first version.** If the
   operator later wants missing Steam covers, make it a button the person
   presses, fetching only that person's installed games from Valve's own servers,
   with a line saying the request tells Valve which games were looked up; and
   ask Valve first if the feature grows beyond that.
2. **Store names: keep the plain words, add no logos.** "Steam", "Epic Games
   Store" and "GOG" in the app's own font and colours name where a game came
   from, which is the nominative use the Ninth Circuit allows: there is no other
   way to say it, only the name is used, and nothing suggests sponsorship. No
   store logos, icons or lettering, since Valve conveys no right to its logo and
   neither of the others publishes one. Add one quiet line on the page, something
   like (our draft wording, not a quotation): HumanityOS is not affiliated with
   Valve, Epic Games or GOG. Never use a store name in HumanityOS's own branding or
   marketing.
3. **Local play records: Steam's, read-only, for the signed-in account; not
   GALAXY's.** Reading `localconfig.vdf` for playtime and last played, listing
   the person's own screenshots from their screenshots folder (with a button that
   opens the folder), and listing subscribed Workshop items from the
   `appworkshop` file are not addressed by any Steam text, involve no network and
   no store service, and are the person's own records. Read them, never write
   them: changing playtime or achievement records is what the "Faking gameplay
   statistics" rule is about. Show only the Steam account the person is using
   (Steam keeps `userdata` in one folder per account, which is how Playnite's
   code finds it, so a shared family computer can hold several people's
   records). Leave achievements
   (Valve's undocumented binary stats files) for later. **Do not read GOG
   GALAXY's database** unless GOG says yes; a short note to inquiries@gog.com
   would settle it. Show nothing for Epic beyond what its install records give.
4. **Links out: yes, on a click, opened in the person's browser or the store's
   own client.** Store page (`steam://store/<appid>` or
   `https://store.steampowered.com/app/<appid>`; Epic's product page or store
   deep link when the slug is known, else the store's front page; GOG's game page
   when known), the Steam community hub, PCGamingWiki through its own redirect
   (`api/appid.php?appid=` or `api/gog.php?page=`), and SteamDB. Links only: never
   prefetch, preview or read these pages in the background, which is exactly what
   SteamDB and PCGamingWiki ask programs not to do.
5. **Open metadata: none by default; if ever, Wikidata only, and only when the
   person asks.** Wikidata's CC0 data (genre, release date, developer, publisher,
   looked up by the Steam, GOG or Epic ID the page already has) carries no
   licence duty and needs no key. Fetch it only when the person presses a button,
   say on that button that the lookup tells Wikidata which game they asked about,
   send a User-Agent with the project's contact address, keep the result on the
   person's computer, and never send any game list to the HumanityOS relay. Not
   IGDB (an open-source desktop app cannot keep a secret key, and a project
   relay would see every user's games), not Steam's `appdetails` (undocumented,
   "not meant for public consumption"), and not PCGamingWiki's data API (login
   needed, NonCommercial unsettled, a small community server): link to
   PCGamingWiki instead.

- **Say it on the screen**, per the project's rule on platform-holder limits.
  Draft wording (ours, not a quotation): covers come from the store's own app on
  this computer; HumanityOS shows store names but not store logos because the
  stores have not licensed their logos to us (a platform holder's rule); nothing
  about your games leaves this computer unless you press a button that says so.
  Link this finding from that screen.

- **Decision taken on reading this finding (the session's own call, 2026-10-19, not
  the operator's):** achievements ARE read too, read-only, from Steam's local stats
  cache. Judgement call 3 defers them because the binary files are undocumented, a
  technical reason, not a legal one: reading them is unaddressed in the same way as
  `localconfig.vdf`, which the same call allows, and Valve's own page above confirms
  the cache exists ("Steam keeps a local cache of the stats and achievement data").
  The guard is technical: where a file cannot be read with certainty, the page shows
  nothing for that game rather than a wrong count, and nothing ever writes to the
  files.

---

This document is a reading of public sources on 19 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. The stores' and sites' terms
change often; check their dates before relying on any of it.
