# Game stores and a Games library: what Steam, Epic and GOG say about another program listing and starting their games

**Researched 13 October 2026** (sources read between about 16:30 and 16:45
Pacific time). Everything quoted below was read that day, or, where a live page
refused the request, read from an Internet Archive copy whose date is given.
Store terms change often: Steam's and Epic's general terms were both revised
on 10 September 2026. If you are reading this later, check each source's own
date before relying on it.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such.

This finding extends
[`2026-09-25-permission-routes-and-launchers.md`](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-09-25-permission-routes-and-launchers.md),
which covered GOG's contacts and affiliate programme, the Steam Web API terms,
and how Playnite's and Heroic's source code reach Steam and GOG. It does not
replace it. New here: the Steam Subscriber Agreement's full automation and
third-party wording, Valve's own developer documentation of `steam://`,
SteamCMD, everything about Epic, GOG's GALAXY licence, and Playnite's own
documentation.

## The question

May a Games library inside the HumanityOS desktop app list every game installed
on the computer (Steam, Epic Games Store, GOG, and games installed from
anywhere else, such as Dwarf Fortress Classic from Bay 12 Games) and start them,
while each store keeps doing its own downloads, updates and licence checks?

## Short answer

- **No store's terms say yes or no to a third-party library starting games
  through the store's own link.** None of the three agreements read mentions
  `steam://`, `com.epicgames.launcher://`, a store's local library files, or
  third-party launchers at all.
- **Each store documents its own way of starting an installed game**, and these
  are the entry points a library would use:
  - Valve's Steamworks documentation describes a game "launched via Steam URL,
    e.g. steam://run/<appid>//<command line>/". The fuller list, including
    `steam://rungameid/<id>`, is on the Valve Developer Community wiki, which is
    a community wiki and warns that "Some commands are no longer functional".
  - Epic documents `com.epicgames.launcher://apps/...?action=launch&silent=true`
    in its developer documentation, written for game clients.
  - GOG says its games need no launcher at all: "No, you don't need any
    additional apps or launchers."
- **What is not allowed, as written:**
  - Steam: "scripts, bots, macros, or other non-human-controlled systems" may
    not "interact with Content and Services on Steam in any manner", and the
    Steam client itself counts as Content and Services. So anything a library
    does on its own, without a person asking (launching on a timer, triggering
    updates in the background), falls within the words of that rule. A person
    clicking Play is not "non-human-controlled". Also barred: unauthorized
    third-party software "controlling the processes or user interface of Steam
    Content and Services", and emulating Valve's network protocols.
  - Steam, downloading without the client: Valve's wiki says SteamCMD's
    "primary use is to install and update various dedicated servers". The same
    page shows it can fetch purchased games after a login, but no Valve page
    read says another program may use it to install a person's games, and the
    Subscriber Agreement bars distributing "any software accessed via Steam"
    without Valve's written consent. Not allowed until Valve says otherwise.
  - Epic: the Store's licence bars using the software "in a way that is not
    expressly authorized" and circumventing "security technology". Epic's own
    games (Fortnite, Fall Guys, Rocket League) add: "Use bot software or
    services to automate your use of Licensed Products".
  - GOG: rule 11.1(e) bars "automation software, robots, bots, mods, hacks
    [...] or other software that interact with or affect GOG services or GOG
    content in any way". Read word for word, "other software that interact
    with [...] GOG content in any way" could reach any program that starts a
    GOG game; read in its list of cheats and bots, it targets those. GOG's own
    support article, which says installed games launch "just like any other",
    points to the second reading. GOG also says its client protocols are not
    open yet, and it does not document GOG GALAXY's command line.
- **Unknown:** whether Valve, Epic or GOG regard a third-party library calling
  their launch link as "authorized" or "expressly permitted"; whether SteamCMD
  may be used for games by anyone other than Valve's partners. Each store would
  have to be asked (see "What is still unknown").
- **Games from anywhere else**, like Dwarf Fortress Classic, involve no store
  at all: starting them is what a desktop shortcut already does.

## How each finding is recorded

Each finding gives the source's address, the source's own date where it shows
one, and a quotation of the words that carry the meaning. Long passages are
trimmed to the words that matter; "[...]" marks a cut inside a quotation, and
nothing inside a quotation is reworded. Every quotation was checked by script
against the text read on the day.

## 1. Steam

### 1.1 The Steam Subscriber Agreement

Source: https://store.steampowered.com/subscriber_agreement/ . Its own date:
"This Agreement was last updated on September 10, 2026". This is the same
revision the 25 September finding read.

**What counts as "Content and Services".** Section 1.B: "The Steam client
software and any other software, content, and updates you download or access
via Steam, including but not limited to Valve or third-party video games and
in-game content" (fragment) are what the agreement calls "Content and
Services". So every rule below about Content and Services covers both the
games and the Steam client itself.

**The client may be required.** Section 2.A: "To make use of the Content and
Services, you must have a Steam Account and you may be required to be running
the Steam client and maintaining a connection to the Internet."

**Updates are Valve's.** Section 2.A: "Valve may need to automatically update,
pre-load, create new versions of or otherwise enhance the Content and Services"
(fragment), and "You consent to such automatic updating."

**No copying or distributing Valve's software.** Section 2.G: "you may not, in
whole or in part, copy, photocopy, reproduce, publish, distribute, translate,
reverse engineer, derive source code from, modify, disassemble, decompile,
create derivative works based on, or remove any proprietary notices or labels
from the Content and Services or any software accessed via Steam without the
prior consent, in writing, of Valve."

**No emulating Valve's network protocols.** Section 2.G(ii), you may not
"emulate or redirect the communication protocols used by Valve in any network
feature of the Content and Services" (fragment), "through protocol emulation,
tunneling, modifying or adding components to the Content and Services, use of a
utility program or any other techniques now known or hereafter developed"
(fragment).

**Third-party software (section 4.B, "Cheating").**

- "You agree that you will not tamper with the execution of Steam or Content
  and Services unless otherwise authorized by Valve."
- "You may not use Cheats, mods, hacks, or any other unauthorized third-party
  software, to modify any Subscription Marketplace process, the process of
  Steam account creation or otherwise in interacting with or controlling the
  processes or user interface of Steam Content and Services, except to the
  degree expressly permitted."

**Automation (section 4.C).** "You may not use any form of scripts, bots,
macros, or other non-human-controlled systems (“Automation”) to interact with
Content and Services on Steam in any manner, including but not limited to:"
then four examples: "Automating the Steam account creation process", "Faking
gameplay statistics (e.g., inflated wins or losses, XP, playtime)", "Earning
rewards or progress without genuine user input", and participating in peer
review or "overwatch" systems "through automated means" (fragment). Section 4.D:
"Valve may restrict or terminate your Account(s) or a particular Subscription
for any conduct or activity that is illegal, constitutes a Cheat or Automation
or breaches the Steam Online Conduct Rules."

**What it does not say (negative finding).** The agreement's text contains no
mention of `steam://`, "launcher", "library", "appmanifest" or "SteamCMD"
(searched on the day). It neither permits nor forbids a third-party program
asking the Steam client, through Steam's own registered link, to start a game.
Whether that is "interacting with or controlling the processes [...] of Steam
Content and Services" is not answered by the text.

### 1.2 `steam://` in Valve's own developer documentation

Source: Steamworks documentation, "ISteamApps Interface",
https://partner.steamgames.com/doc/api/ISteamApps (no date shown). This is
Valve's official documentation for developers publishing on Steam.

- GetLaunchCommandLine: "Gets the command line if the game was launched via
  Steam URL, e.g. steam://run/<appid>//<command line>/. This method is
  preferable to launching with a command line via the operating system, which
  can be a security risk."
- GetLaunchQueryParam: "Gets the associated launch parameter if the game is run
  via steam://run/<appid>/?param1=value1;param2=value2;param3=value3 etc."

So Valve documents that games are started by a `steam://run/<appid>` URL. The
page is written for game developers; it does not say who may send such a URL.

### 1.3 `steam://rungameid` on the Valve Developer Community wiki

Source: "Steam browser protocol",
https://developer.valvesoftware.com/wiki/Steam_browser_protocol . Its own date:
"This page was last edited on 14 July 2024, at 06:43." The live page answered
with an automated "Making sure you're not a bot!" check, which was not passed;
the text was read from the Internet Archive's copy of 25 August 2026,
https://web.archive.org/web/20260825213745/https://developer.valvesoftware.com/wiki/Steam_browser_protocol .

- "There are numerous system-wide commands available that interact with Steam.
  All of them open up Steam if it is not open." and "you can normally create
  links to them as you would web page links" (fragment).
- "Warning: Some commands are no longer functional, and some commands might be
  missing on this list."
- "steam://run/<id>//<args>/" : "Runs an application. It will be installed if
  necessary."
- "steam://rungameid/<id>" : "Same as run, but with support for mods and
  non-Steam shortcuts."
- "steam://install/<id>", with "Example: steam://install/8230 installs Sam &
  Max: Episode 4."

The Valve Developer Community is hosted by Valve but written by its users. It
is documentation of behaviour, not a statement of what Valve permits.

### 1.4 Reading the local Steam library files

- **appmanifest files are mentioned once**, in Valve's Steamworks
  documentation "Uploading to Steam", https://partner.steamgames.com/doc/sdk/uploading
  (no date shown), under "You can use these Steam client commands and
  client-side files to debug issues:", the entry: "file
  "steamapps\appmanifest_[appid].acf" - Shows the current install state of
  this app (KeyValues)." It is addressed to developers debugging their own
  builds, and says nothing about other programs reading the file.
- **libraryfolders.vdf**: not mentioned on any Valve page read.
- **The Subscriber Agreement** says nothing about either file (1.1 above).

So reading these files is **not addressed** anywhere found, for or against.

### 1.5 SteamCMD: servers, or games too?

Source: "SteamCMD", https://developer.valvesoftware.com/wiki/SteamCMD . Its own
date: "This page was last edited on 25 July 2026, at 08:54." The live page gave
the same automated check as 1.3; read from the Internet Archive's copy of 11
October 2026,
https://web.archive.org/web/20261011125146/https://developer.valvesoftware.com/wiki/SteamCMD .

- "The Steam Console Client or SteamCMD is a command-line version of the Steam
  Client. Its primary use is to install and update various dedicated servers
  available on Steam using a command-line interface."
- "To download most game servers, you can login anonymously using login
  anonymous" and "Some servers require you to login with a Steam Account."
- "If you are uncertain about the appid for the server, tool, or game you want
  to download, you can use steamdb.info to locate it." (The page itself names
  games as something it can download.)
- On the "No subscription" error: "the game/server you are trying to download
  either requires a login or that you have purchased the game. You will
  therefore have to log in with a Steam username and password."
- "A user can only be logged in once at any time (counting both graphical
  client as well as SteamCMD logins)."

Valve's Steamworks "Uploading to Steam" page (1.4) describes `steamcmd.exe` as
"the command line version of Steam" (fragment), used by developers to build and
upload their games.

**Negative finding:** no Valve page read says that a third-party program may
use SteamCMD to download or update a person's games, and none says it may not.
The wiki presents it as a server tool that can also fetch purchased games for
someone logged in. A program that drove it would be handling the person's Steam
password (the 25 September finding records the Steam Web API terms' "you will
not intercept or store the end user's Steam password on log in"), and shipping
SteamCMD inside another app would be distributing Valve software, which 2.G of
the agreement bars without written consent.

## 2. Epic Games Store

### 2.1 The Epic Games Store End User License Agreement

Source: https://store.epicgames.com/en-US/eula , which redirected to
https://legal.epicgames.com/store/eula . Its own date: "Last Updated January
15, 2025".

- Definitions: "“Epic Games Store” means the online store known as Epic Games
  Store and the Epic Games Launcher." and "“Software” means (i) the Epic Games
  Launcher and (ii) video games or other software made available for download
  or use through the Epic Games Store."
- 1.1: Epic grants "a personal, non-exclusive, non-transferable,
  non-sublicensable limited right and license to use the Software and its
  associated Services for your personal, non-commercial use" (fragment).
- 2, "License Conditions": "You may not do any of the following with respect to
  the Software or any of their parts:" including "(c) copy, reproduce, display,
  perform, or otherwise use it in a way that is not expressly authorized in this
  Agreement or its Software Specific Terms" and "(f) remove, disable,
  circumvent, or modify any proprietary notice or label or security technology
  included in it" (fragments).
- 3, "Updates and Patches": "Epic may provide patches, updates, or upgrades to
  the Software that must be installed in order for you to continue to use the
  Software or Services."

**Negative finding:** the EULA's text contains no mention of protocol links,
`com.epicgames.launcher`, or third-party launchers (searched on the day). The
only uses of "launch" are in the name "Epic Games Launcher".

### 2.2 Epic's general Terms of Service

Source: https://legal.epicgames.com/epicgames/tos (the address
https://www.epicgames.com/site/en-US/tos answered 403, "Enable JavaScript and
cookies to continue"). Its own dates: "Last Updated: September 10th, 2026" and
"Effective: September 10th, 2026".

- Scope: the terms cover Epic's games "(including Fall Guys, Fortnite, and
  Rocket League)" (fragment) and "Services we provide, including launchers,
  portals (e.g., Epic Developer Portal), plugins, Epic Games Accounts" (fragment),
  but: "These Terms do not apply to Unreal Engine, the Epic Games Store, or
  Epic’s marketplaces (e.g., Fab)."
- Section 4, "Gameplay Integrity", you agree not to: "Attempt to tamper with,
  modify, disable, disrupt, or circumvent Gameplay Integrity Tools or Licensed
  Products, or encourage others to do so;" and "Use bot software or services to
  automate your use of Licensed Products;".

These two documents overlap oddly (see "Where the sources pull in different
directions"). For games Epic itself publishes, the bot rule applies; for the
store and its launcher, the EULA in 2.1 does.

### 2.3 Epic's launch link

Source: Epic Developer documentation, "Protocol Activation",
https://dev.epicgames.com/docs/epic-games-store/protocol-activation (no date
shown). Its own summary: "Activate game clients to launch the Epic Games Store
or specific product pages with protocol activation (deep linking)."

- "The Epic Games Store (EGS) Launcher can be launched via protocol activation
  by game clients."
- "All of these protocol activations can be accomplished using OS-provided
  functionality."
- "The EGS Launcher is registered via the OS to handle com.epicgames.launcher
  protocol activations."
- Actions: "launch -launches an app", "updatecheck - polls for available
  updates", "installer - opens any installation options screen for the
  application".
- "silent": "used with the action=launch parameter to launch the app in
  question without visibly popping up the EGS Launcher."
- Address form:
  "com.epicgames.launcher://apps/[SandboxID]%3A[CatalogID]%3A[ArtifactId]?action=launch&silent=true",
  or "a url encoded installation path to the app’s install directory on the
  machine (or any location inside of it) may be used" (fragment).
- "The specific strings to use for your app and offers are determined by back
  end configuration. Please work with the Service Delivery team to configure
  these appropriately."

The page is written for developers of games on the store, about their own
games. It does not say whether other programs may send these links.

## 3. GOG

### 3.1 The GOG User Agreement

Source: https://support.gog.com/hc/en-us/articles/212632089-GOG-User-Agreement .
Its own date: "Last update (effective date): 9 March 2026". (GOG's help centre
records the article as last edited on 19 March 2026.)

- 1.1: the agreement applies to "GOG GALAXY application (including GOG GALAXY
  store), any games or videos or other content or services which you purchase or
  access via us" (fragment). 1.3: games and videos together "are “GOG content”"
  (fragment).
- 2.1: "We give you and other GOG users the personal right (known legally as a
  'license') to use GOG services and to download, access and/or stream
  (depending on the content) and use GOG content. This license is for your
  personal use."
- 2.5: "GOG GALAXY, which is our (optional) application" (fragment), and its
  features include "unified game library" (fragment).
- 11.1(c): "at some point in the future we may want to open client protocols to
  make it easier for users to work with GOG data/software without any need for
  reverse engineering or similar techniques." (fragment)
- 11.1(e): "Do not create, use, make available and/or distribute cheats,
  exploits, automation software, robots, bots, mods, hacks, spiders, spyware,
  cheats, scripts, trainers, extraction tools or other software that interact
  with or affect GOG services or GOG content in any way (including, without
  limitation, any unauthorized third party programs that intercept, emulate, or
  redirect any communication between GOG or its partners and GOG services
  and/or any unauthorized third party programs that collect information about
  GOG services)."
- 11.1(f): "Do not access or attempt to access areas of GOG.COM, GOG GALAXY or
  GOG servers that have not been made available to the public." (fragment)
- 17.3, in the plain-language summary beside it: "In the very unlikely situation
  that we have to stop running GOG we'll do our best to give you advance notice,
  so that you can download and safely store all your DRM-free content."

**Negative finding:** the agreement's body contains no mention of offline
installers, launchers, or other programs starting GOG games. "DRM-free" appears
only once, in the 17.3 summary quoted above. The agreement does not grant a
right to keep or back up installers in so many words; that comes from GOG's
support pages and its "DRM-free" label, not from the contract.

### 3.2 GOG: no launcher needed

Source: "Do I need some launcher application to play my GOG games?",
https://support.gog.com/hc/en-us/articles/212554329 (GOG's help centre records
it as created 19 October 2016 and last edited 17 January 2021).

- "No, you don't need any additional apps or launchers. All games downloaded
  from GOG are nicely wrapped in a self-executable (exe) setup file"
  (fragment).
- "After the installation is complete, shortcuts will be added to your Start
  Menu, desktop and Games Explorer, so you can launch your game just like any
  other."

### 3.3 The GOG GALAXY Licence Agreement

Source: https://support.gog.com/hc/en-us/articles/360023617474-GOG-GALAXY-Licence-Agreement .
Its own date: "Last update : 30th January 2026".

- GOG describes GALAXY itself as offering "unified game library, launcher for PC
  games" (fragment), and says that with linked external platforms "You may even
  be able to talk with your friends across different platforms, purchase and
  launch some non-GOG games via GOG GALAXY!" A multi-store library is what GOG's
  own product does.
- On redistributing GALAXY: "You can repackage it and distribute it with other
  software, but you must not do anything that involves it being integrated with
  third party software or hardware" (fragment). This sits in the licence to
  reproduce and distribute GALAXY; it is about shipping GALAXY, not about
  another program starting a game.
- GALAXY "may also allow you to add community-created plugins, applications,
  integrations and other new components" (fragment), "Community Integrations",
  which plug other platforms into GALAXY (the direction recorded in the 25
  September finding, 2.2).

**Negative finding:** GOG GALAXY's command line (`/command=runGame`, used by
Playnite as an option) is **not documented by GOG** on any page found: not in
GOG's help centre and not in the search index of GOG's developer documentation
at https://docs.gog.com/ (searched on the day). The only descriptions found
were on user forums. GOG's developer documentation does describe the "tasks"
that a game's build declares (for example `"isPrimary": true` with a `"path"`
to the game's executable) on https://docs.gog.com/pb-build-game/ , which is
the information a launcher reads to start a GOG game without GALAXY.

## 4. Prior art: Playnite (what is common practice, not what is allowed)

Playnite is an open-source game library manager,
https://github.com/JosefNemec/Playnite , MIT licence, last pushed 29 September
2026 (GitHub API, read on the day). Its own documentation, at
https://api.playnite.link/docs/ (no dates shown), says:

- "Does Playnite replace game clients?" "No. [...] generally Playnite does not
  replace game clients like Steam, Epic Games Store, GOG, or others." "You may
  still require these clients to access platform-specific features like games
  downloading, installing and uninstalling, multiplayer, friends, or community
  features offered by the games respective clients." (manual, Frequently Asked
  Questions,
  https://api.playnite.link/docs/manual/gettingStarted/helpAndTroubleshooting/faq.html)
- On accounts: "When connecting accounts, it primarily relies on the practices
  implemented by these plugins, such as using official login web forms and
  storing web session cookies or tokens" (fragment, same page).
- On Steam: "Some game launchers may offer alternative methods, such as using a
  Link type, for example, Steam by using steam://launch/<appid>." and, for games
  added to Steam by hand, "copy the URL value (e.g.,
  steam://rungameid/11370944766805016576)" (fragments, Game library FAQ,
  https://api.playnite.link/docs/manual/library/games/faq.html).
- For plugin authors: a library plugin's "GetGames" must "Return games available
  in library", each with a "PlayAction", "Game action used to start the game.
  Only if game is reported as installed via State property." (Library Plugins,
  https://api.playnite.link/docs/tutorials/extensions/libraryPlugins.html)

How each store plugin works, from the source code:

- **Epic** (read on the day, Codeberg,
  https://codeberg.org/CrowIsTaken/PlayniteExtensions , branch `main10`,
  `source/Libraries/EpicLibrary/EpicLauncher.cs`): installed games come from
  `%PROGRAMDATA%\Epic\UnrealEngineLauncher\LauncherInstalled.dat` and the
  `*.item` manifests in `%PROGRAMDATA%\Epic\EpicGamesLauncher\Data\Manifests`;
  games start with
  `com.epicgames.launcher://apps/{0}%3A{1}%3A{2}?action=launch&silent=true` and
  install with `com.epicgames.launcher://apps/{0}?action=install`.
- **Steam and GOG**: recorded in the 25 September finding, 2.4 (not re-read
  today). Steam: `appmanifest` files and `libraryfolders.vdf` for installed
  games, `steam://rungameid/<id>` to start. GOG: the uninstall registry and each
  game's `goggame-<id>.info` file, starting the game's own executable, or GOG
  GALAXY's command line if the user chooses.

In short, the common practice is: find installed games from each client's own
files on disk, and start them through the client's own link (Steam, Epic) or
the game's own executable (GOG), leaving downloads and updates to the clients.
Playnite has done this in public for years. That shows the practice is
widespread; it is not permission.

## 5. Games from anywhere else

Source: Bay 12 Games, https://www.bay12games.com/dwarves/ (no page date). Under
"DOWNLOAD" it lists "DWARF FORTRESS CLASSIC 53.16 (August 5, 2026)", and
just above that it says "Now available at Steam and itch.io!" So the same game
can be on a computer twice: once from Bay 12 directly, once from a store.

A game installed from a developer's own download has no store agreement
between the player and the game. Starting it is what the desktop shortcut the
installer made already does. Each such game may have its own licence about
copying or changing it, but listing and starting an installed copy is not
copying it. (No game's own licence was researched for this finding.)

## Where the sources pull in different directions

- **Epic's two documents.** The Terms of Service list "launchers" among the
  services they cover, then say "These Terms do not apply to [...] the Epic
  Games Store", and the EULA defines the Epic Games Store as including the Epic
  Games Launcher. Our reading (a judgement): the EULA governs the store and its
  launcher, and the Terms govern Epic's own games and accounts, so the bot rule
  in the Terms applies to Fortnite and the like, not to the store's other games.
- **GOG 11.1(e) against GOG's own support page.** Word for word, 11.1(e) could
  reach any "other software that interact with [...] GOG content in any way".
  GOG's support article says installed games launch "just like any other",
  with shortcuts on the desktop. Our reading (a judgement): 11.1(e) is a list of
  cheats, bots and data-collecting tools, and a program that starts an
  installed game the way a desktop shortcut does is not what it is aimed at.
  We find the support article the better guide to GOG's intent, but the
  agreement is the binding text, and only GOG can settle it.
- **SteamCMD.** The wiki says its "primary use" is dedicated servers, and the
  same page describes downloading purchased games with it. Neither sentence is
  a permission for other programs to use it.

## What is still unknown, and what would settle it

- **Whether Valve treats a library sending `steam://run` or
  `steam://rungameid` on a person's click as "authorized" software.** Settled by
  asking Valve. No public channel for this kind of question was found;
  Steam Support (https://help.steampowered.com/) is the general route. Cost: one
  message and a wait.
- **Whether SteamCMD may be used by a third-party program for a person's
  games**, and whether SteamCMD itself can be redistributed. Settled by Valve in
  writing. Until then, the project's reading is that it should not be used for
  this.
- **Whether GOG reads 11.1(e) as reaching a library that starts installed
  games.** Settled by writing to GOG at inquiries@gog.com (the business address
  recorded in the 25 September finding); GOG says such requests are reviewed "in
  good faith".
- **Whether Epic minds a non-game program sending its protocol links.** Epic's
  page sends developers to a "Service Delivery team", a partner channel. No
  public route for third-party launchers was found.
- **Showing store names and logos in the library.** Not researched. The Steam
  Web API terms (25 September finding) bar implying "any other relationship with
  Valve" without written approval; each store's trademark rules would need their
  own finding before logos are used.
- **Each game's own licence.** Not researched; matters only if the library ever
  copies, patches or modifies a game, which it should not.

## Sources that could not be reached

- https://developer.valvesoftware.com/wiki/Steam_browser_protocol and
  https://developer.valvesoftware.com/wiki/SteamCMD : both answered with an
  automated "Making sure you're not a bot!" proof-of-work page that says it is
  there to stop AI scraping. It was not passed. Both were read from Internet
  Archive copies (25 August 2026 and 11 October 2026), whose "last edited" lines
  (14 July 2024 and 25 July 2026) show that the copies are of the current
  revisions as of those dates.
- https://www.epicgames.com/site/en-US/tos : HTTP 403, "Enable JavaScript and
  cookies to continue". The same terms were read at
  https://legal.epicgames.com/epicgames/tos .
- GOG GALAXY command-line documentation: not unreachable, but not found; see
  3.3.
- Not re-read today, relied on from the 25 September finding: the Steam Web API
  Terms of Use (https://steamcommunity.com/dev/apiterms , "Last updated July
  2010"), and Playnite's Steam and GOG plugin source.

## What it means for HumanityOS

### Certain (stated by the sources above)

- No Steam, Epic or GOG agreement read on 13 October 2026 mentions third-party
  launchers, the stores' launch links, or their local library files.
- Valve documents `steam://run/<appid>` in its own Steamworks documentation;
  Epic documents `com.epicgames.launcher://apps/...?action=launch` in its own
  developer documentation; GOG says its games need no launcher and install
  ordinary shortcuts.
- Steam forbids "non-human-controlled systems" interacting with its content and
  client "in any manner", unauthorized third-party software controlling Steam's
  processes or interface, emulating Valve's network protocols, and distributing
  software accessed via Steam without written consent.
- Steam's agreement says a person "may be required to be running the Steam
  client"; Epic's EULA says updates "must be installed in order for you to
  continue to use the Software". Both stores keep updates as their own job.
- Valve's wiki describes SteamCMD as a tool whose "primary use" is dedicated
  servers. No Valve page read permits other programs to use it for games.
- GOG's client protocols are not open, by GOG's own statement, and GOG does not
  document GOG GALAXY's command line.
- Playnite, under the MIT licence, lists installed games from the stores'
  local files and starts them through each store's own link or the game's own
  executable, and says plainly that it does not replace the stores' clients.

### Judgement calls (the project's own reading; a lawyer could disagree)

- **The Games library is buildable as the operator described it**, if it stays
  a list and a Play button: read the stores' local files (read-only, never
  written), show the games, and on a person's click hand the start to the
  store's own documented route (`steam://rungameid/<appid>` or
  `steam://run/<appid>`; Epic's `?action=launch` link; a GOG game's own primary
  executable as declared in its `goggame-<id>.info` file; any other game's own
  executable). That is the same thing a desktop shortcut does, and it leaves
  downloads, updates and licence checks with the stores.
- **Nothing on its own.** Every store action should follow a person's click:
  no launching on a timer, no button that updates every game at once, no
  background polling of Epic's `updatecheck`. Steam's Automation rule is broad enough that anything the app
  does by itself against Steam is the riskiest part of the design.
- **Install and Update buttons should open the store's own client** (Steam's
  `steam://install/<id>`, Epic's `installer` action, GOG GALAXY or gog.com),
  never download anything themselves. No SteamCMD, no reimplemented store
  protocols (Legendary, gogdl and the like), no bundling of any store's
  software.
- **No store sign-in in the first version.** Listing only what is installed
  avoids the Steam Web API key and password rules and GOG's 11.1(e) and (f)
  entirely. Showing owned-but-not-installed games is a later, separate decision
  with its own finding.
- **GOG games through their own executables, not GOG GALAXY's command line**,
  since GOG documents the first (its build tasks, and its support article
  saying no launcher is needed) and not the second. A short note to inquiries@gog.com before listing
  GOG games would settle 11.1(e) cheaply; it is optional, not required by
  anything read.
- **Say it on the screen**, per the project's rule on platform-holder limits:
  something like (our draft wording, not a quotation): HumanityOS starts your
  games through each store's own app; downloads and updates stay with the
  store, because the stores' terms keep those for their own software (a
  platform holder's rule). Link this finding from that screen.
- **No logos or store branding** until trademarks are researched; plain text
  names ("Steam", "Epic Games Store", "GOG") describe where a game came from
  without implying a partnership.

---

This document is a reading of public sources on 13 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. The stores' terms change
often; check their dates before relying on any of it.
