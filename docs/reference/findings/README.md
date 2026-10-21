# Findings

Dated research notes on questions with a legal, regulatory or licensing edge.
One file per question, written the day the research was done, never quietly
updated afterwards.

## Why this folder exists

The operator's reason, in his own words:

> If there's something legal or similar then we should research it and create a
> document of what we find, whether it's ok or not. That way when we inevitably
> have to discuss the subject again we can see where we found the information and
> what we found. We should also date our findings as to clearly identify the last
> time we looked. That way if something changes and we're approached we can see
> the date we researched and compare it to the date that the change occurred.

The point is not to be right forever. It is to make it possible, later, to tell
**which kind of wrong we were**. When a question comes back around, a reader
with one of these files in hand can separate two very different situations:

- **The world changed.** The patent expired, the pool restructured its rates, a
  platform changed its terms. Our finding was correct when written and is now
  stale. The date proves it, and the fix is to research again.
- **We were wrong.** The source said something we misread, or we never found the
  source that mattered. The quotations prove it, because a later reader can go
  and read the same words we read.

Without the date, those two look identical, and every old conclusion has to be
re-derived from nothing. With it, most of them can just be re-checked.

There is a practical edge to this too. If someone ever writes to the project
about a codec, a format or a protection scheme, the reply can say exactly what
we looked at, exactly when, and what we concluded. That is a much better
position than remembering.

## File naming

```
docs/reference/findings/YYYY-MM-DD-short-topic.md
```

The date is the date the **research** was done, not the date of any later edit.
It goes first so the folder sorts chronologically.

Do not overwrite an old finding when the answer changes. Write a new file with
the new date and, in it, link back to the one it supersedes and say plainly what
moved. The old file stays: it is the record of what we believed and why, and it
is the thing that lets a later reader date a change.

## Required parts, in this order

1. **The question**, in one sentence. If it takes a paragraph, it is more than
   one finding.
2. **The research date, stated prominently at the top**, before anything else a
   reader might act on.
3. **The short answer**, up front. A reader in a hurry should be able to stop
   after this section and not be misled.
4. **Each finding, separately**, carrying three things every time: the source
   link, the date that source was published or last updated, and a quotation of
   the words that carry the meaning.
5. **What is still unknown, and what would settle it.** Name the specific thing
   that would answer each open question, and what it would cost. "Ask a lawyer"
   is a legitimate answer; "unclear" on its own is not.
6. **Sources you could not reach.** Every 404, paywall and block, by URL, with
   what you were hoping to get from it. This is not an apology, it is a
   work-list for whoever researches next.
7. **What it means for this project**, split into what is **certain** and what
   is a **judgement call**. Keep these apart. A later reader must be able to
   accept the facts and reject the opinion.

## The rules

**Quote, do not paraphrase, and never from memory.** Open the source, read the
words, copy them exactly. A summary is your reading of a source; a quotation is
the source. When a finding turns out to be wrong, the quotation is what lets a
reader see whether the source was wrong or we were. Copyright limits apply, so
quote the passage that carries the meaning and no more, and reproduce quoted
text exactly as written, punctuation included.

**Date every source, not just the document.** "Retrieved 2026-09-19" is not the
same as "published 2013" and the gap between them is often the whole story. A
licence written in 2014 and a patent table edited three days ago carry very
different weight.

**Where sources disagree, show both.** Do not resolve a conflict silently in
favour of the one you prefer. Quote each side, say which you find more credible
and why, and label that as a judgement. A reader who disagrees with your
weighting can then reach their own conclusion from the same material.

**Say you are not a lawyer, plainly, and more than once.** These documents get
read later, out of context, by people who did not commission them. The
disclaimer belongs at the top and at the bottom, not buried.

**Record the answer even when it is "no".** A researched "no, and here is
exactly why" is worth as much as a yes, because it stops the question being
re-opened from scratch every year.

**Do not edit a finding to make it look better later.** Correct an outright
factual error, with a dated note saying what was corrected. Everything else goes
in a new file.

## What belongs here, and what does not

Findings answer *may we?* and *what is actually true out there?*. They are
research about the world outside the repository.

They are not design documents, which answer *how shall we build it?* and live in
`docs/design/`. They are not public stances, which answer *what do we tell
people?* and live in `docs/reference/` proper, such as
[`media-stance.md`](../media-stance.md). A finding usually comes first and feeds
the other two: research the ground, then decide what to build, then write down
what we will say about it.

## Index

- [`2026-09-19-video-codec-licensing.md`](2026-09-19-video-codec-licensing.md),
  whether a free, open-source, donation-funded project can lawfully play H.264
  video and AAC audio. Covers the Cisco OpenH264 grant, remaining patent expiry
  dates, calling the operating system's own decoders, Via LA's published rates,
  and what other free projects did.
- [`2026-09-25-site-embed-terms.md`](2026-09-25-site-embed-terms.md),
  whether the 34 sites awaiting review in `data/web/sites.json` allow the
  readable web to fetch, reformat and show their pages. One allowed, 21
  conditional (mostly attribution), seven forbidden as written, four silent,
  one gone. Evidence for the person who records each site's `embed.status`.
- [`2026-09-25-permission-routes-and-launchers.md`](2026-09-25-permission-routes-and-launchers.md),
  where to ask each site that forbids the reader for permission (GOG,
  Examine, Coursera, Discord, Project Gutenberg, Khan Academy, Instructables,
  Element, iFixit), and whether HumanityOS could act as a game launcher
  (GOG and Steam: contacts, APIs, affiliate programme, Playnite and Heroic).
- [`2026-09-25-container-materials-and-reuse.md`](2026-09-25-container-materials-and-reuse.md),
  which container materials and linings can hold which contents, and which can
  go back to food or drinking water after holding something else (FDA Food
  Code, the Pasteurized Milk Ordinance, Codex previous-cargo lists, USDA dairy
  equipment rules). The data basis for realistic containers in the game.
- [`2026-09-26-crop-daily-light-integrals.md`](2026-09-26-crop-daily-light-integrals.md),
  how much light each of the home's 63 crops needs a day (minimum, target and
  saturation daily light integral, mol/m2/day) and which flower by day length,
  from Purdue, Cornell, Michigan State, NASA and peer-reviewed horticulture;
  27 sourced, 35 not found (listed), fungi not applicable. The data basis for
  each crop's own light need in the game.
- [`2026-09-27-wifi-and-plants.md`](2026-09-27-wifi-and-plants.md),
  whether a household Wi-Fi router harms plants: the peer-reviewed studies
  (positive and null), their exposures against a real router's in the same
  units, the quality grading of the field, and the 2013 Danish cress project.
  The evidence base for deciding the game's v0.620 Wi-Fi crop harm; the
  operator chose removal on 2026-09-27 and the harm was taken out that day.
- [`2026-09-27-electroculture-copper-coils.md`](2026-09-27-electroculture-copper-coils.md),
  whether copper coils and antennas, applied electricity or lightning make crops
  grow better: the 2025 controlled test of passive coils (no effect), the USDA
  1926 bulletin and the UK Electro-Culture Committee (1918 to 1936, closed as
  unreliable), modern seedling-stage stimulation results, and how much nitrogen
  lightning actually fixes. Cut short by the session limit; unread sources
  listed. Options for the game set out, not chosen.
- [`2026-10-03-food-land-and-transport-speeds.md`](2026-10-03-food-land-and-transport-speeds.md),
  how much growing area one person needs for a year's food (biointensive
  beds, conventional farming, lamp-lit chambers from NASA, Lunar Palace and
  MELiSSA) and how fast goods and people really move (lifts, mine hoists,
  conveyors, pneumatic tubes, mobile robots, rail, maglev), with handling
  times. The project's 700 to 1,000 m2 per person has no source; about 232
  to 372 m2 for sun-lit beds and 50 to 65 m2 under lamps do. Checks every
  rail and logistics speed in `data/transportation.ron` (the posted road
  limits were not researched): the space elevator climb and the
  slurry pipe are above the sources; the AGV and tube rows are not.
- [`2026-10-04-pipe-marking-standards.md`](2026-10-04-pipe-marking-standards.md),
  whether real standards exist for marking pipes (colours, bands, labels,
  arrows) or the game must invent its own: ASME A13.1, ISO 14726 (ships),
  ISO 20560-1, BS 1710, DIN 2403, MIL-STD-101C and MIL-STD-1247D, and NASA
  and ISS practice. They exist; all of them mark the content, none the pipe's
  material; the same colour means different things in different schemes, so
  most of them also ask for the content's name in words (BS 1710 allows
  colour alone; ISO 14726's layout clause was not read). Recommends ISO 14726
  colours aboard the ship with ISO 20560-1's labels, and real material shown
  between the markers.
- [`2026-10-04-sponsor-a-can-tax-status.md`](2026-10-04-sponsor-a-can-tax-status.md),
  whether the Donate page can say a donation to Sponsor-a-Can is tax-deductible:
  the IRS exempt-organization extract lists EIN 93-1624890 as a 501(c)(3) with
  "Contributions are deductible" (ruling April 2026), matching its own site;
  deducting still depends on the donor (Publication 526). A gift to the
  maintainer directly is not deductible.
- [`2026-10-09-outside-help-lines.md`](2026-10-09-outside-help-lines.md),
  where the report dialog sends someone outside the app when a child, or
  anyone, is in danger: the emergency number and the official body for
  reporting online child sexual exploitation in 18 countries, each quoted from
  the government's or the body's own page, with INHOPE's member directory as
  the fallback. The data is `data/safety/outside_help.json`. Canada is not in
  INHOPE's directory; Mexico, the Philippines and Nigeria have INHOPE-member
  hotlines but no confirmed government line.
- [`2026-10-10-report-duties-child-abuse-material.md`](2026-10-10-report-duties-child-abuse-material.md),
  what a server operator must do, and must not do, when a report says child
  sexual abuse material may be involved: the US provider duty to report to
  NCMEC's CyberTipline and preserve for one year (18 U.S.C. 2258A, as amended
  by the 2024 REPORT Act), the limited liability in 2258B, the offences that
  make viewing or passing on material a crime, and in brief the UK's NCA
  reporting duty (in force 7 April 2026) and the EU Digital Services Act.
  Whether a volunteer running a small server counts as a "provider" is not
  answered by any official source. Ends with what this means for the report
  flow, including three gaps seen in the code.
- [`2026-10-10-childrens-online-safety-rules.md`](2026-10-10-childrens-online-safety-rules.md),
  whether the protected setup (a parent's PIN lock on safety settings) is
  allowed and what it may be called, under US COPPA (with the 2025 rule) and
  the FTC Act, US state laws in brief, the UK Children's code and Online Safety
  Act, and the EU Digital Services Act and GDPR. Nothing read forbids the
  setup; nothing supports "kid safe", "child safe" or "compliant". Gives five
  sentences the project may use, the words to avoid and why, and finds the web
  chat's "18 years or older" line at odds with a setup for children (the
  operator's decision). Whether COPPA reaches a free, donation-funded app at
  all is not answered by any source.
- [`2026-10-10-california-ab-1043-age-signals.md`](2026-10-10-california-ab-1043-age-signals.md),
  whether California's Digital Age Assurance Act (AB 1043, rewritten by AB 1856
  in September 2026; both in force 1 January 2027) makes the desktop app ask
  the operating system for an age signal. Only a "developer" with an app in a
  "covered application store" must ask, and anyone not required to ask must
  not (1798.501(e)). The project's website is not such a store; whether GitHub
  Releases is one, no source answers. A listing on Flathub, Snap, WinGet,
  Homebrew or F-Droid would very likely bring the app inside. Supersedes the
  AB 1043 bullet of the children's online-safety finding of the same day.
- [`2026-10-12-media-item-licences.md`](2026-10-12-media-item-licences.md),
  which licence a Media item should carry by default, and what a server needs
  from its uploaders. "All rights reserved" is what the law gives anyway and
  can be loosened later; a Creative Commons licence or CC0 cannot be taken back
  for copies already taken. Each server needs its own limited hosting licence
  from each uploader, separate from the item's public licence, and no
  HumanityOS server has such terms yet. NonCommercial is unclear for a
  donation-funded project, and NC or all-rights-reserved items cannot go into
  the CC0 game as they stand. DMCA safe harbour needs a registered agent.
- [`2026-10-12-project-entity-and-succession.md`](2026-10-12-project-entity-and-succession.md),
  what legal home lets the project (domains, servers, donations, permissions)
  outlive its founder and stay free, and how its DMCA registration can avoid a
  home address. Compares a nonprofit corporation with 501(c)(3) status
  (Washington needs three directors once public-charity status is sought), an
  unincorporated association, fiscal sponsors (SPI 5%, OSC 10%, SFC requires an
  OSI licence and CC0 is not one, HCB is teen-only, OCF closed in 2024), an LLC
  and a will. Notes in general terms that pay, personal donations and project
  money in a person's own name can count against means-tested benefits, while
  money a fiscal sponsor holds generally does not. A DMCA service provider
  must publish a street address; a post office box needs a waiver for a
  demonstrable threat; no source addresses registered-agent or virtual-office
  addresses. Recommends a will and account successors now, a fiscal sponsor
  next.
- [`2026-10-13-game-stores-and-a-games-library.md`](2026-10-13-game-stores-and-a-games-library.md),
  whether a Games library in the desktop app may list every installed game
  (Steam, Epic, GOG, and games from anywhere else such as Dwarf Fortress
  Classic) and start them while the stores keep downloads and updates. No
  store's terms mention third-party launchers, their launch links or their
  local library files; Valve documents `steam://run/<appid>`, Epic documents
  `com.epicgames.launcher://apps/...?action=launch`, and GOG says its games
  need no launcher. Steam bars "non-human-controlled" automation of its client
  and games, so every action should follow a person's click; SteamCMD is
  documented for dedicated servers and no Valve page lets another program use
  it for games. GOG's rule 11.1(e) could be read to reach any program that
  touches a GOG game. Playnite's documentation as prior art. Extends the
  2026-09-25 launcher finding.
- [`2026-10-17-windows-media-foundation-decoders.md`](2026-10-17-windows-media-foundation-decoders.md),
  whether the desktop app can play an ordinary MP4 (H.264 video, AAC sound) by
  asking Windows. Windows 7 and later carry both decoders in Media Foundation
  for any desktop application, H.264 falling back to software without suitable
  graphics hardware, so the September finding's missing sound was the browser
  engine's limit, not Windows'. Not covered: Windows N editions (need the Media
  Feature Pack), H.265 (a Store extension), macOS and Linux. Extends the
  2026-09-19 codec finding; the legal reading is unchanged.
- [`2026-10-17-platform-video-decoders.md`](2026-10-17-platform-video-decoders.md),
  what macOS, iOS and iPadOS, Android and Linux give an app, with nothing
  installed, to play an ordinary MP4 (H.264 and AAC) and H.265. Apple systems
  carry all three (H.265 since iOS 11 and macOS 10.13), through AVFoundation;
  the Android 17 CDD makes H.264, H.265 and AAC decoding a MUST on every
  compatible phone and tablet, reached through MediaCodec. Linux promises
  nothing: Fedora takes H.264 from Cisco's OpenH264 after its first update and
  switched H.264 and H.265 off in its graphics drivers in 2022; Ubuntu leaves
  them on in Mesa but puts the software decoders behind an installer tick-box;
  Flatpak's 25.08 runtime brings them in an extension. Cisco's README and
  release notes disagree on whether OpenH264 decodes Main and High Profile.
  Apple's macOS licence carries the pool's consumer notice. Extends the Windows
  finding of the same day; the legal reading is unchanged.
- [`2026-10-19-games-page-art-stats-and-links.md`](2026-10-19-games-page-art-stats-and-links.md),
  what the desktop Games page may show beyond starting games. Artwork: show the
  covers Steam's own client already cached on the computer, read in place, to
  that person only, never copied, uploaded or shown to other players; fetch none
  from store servers. Store names in plain words, no logos (Valve conveys no right
  to its logo, Epic forbids altering its marks, GOG publishes no rules). Steam's
  local playtime, screenshots and Workshop lists are unaddressed by its terms and
  read-only is the reading (achievements too, by the session's own call); not GOG
  GALAXY's database. Links out on a click only; no metadata fetched by default,
  Wikidata (CC0) if ever. Extends the 2026-10-13 finding.
- [`2026-10-19-tools-page-install-launch-and-icons.md`](2026-10-19-tools-page-install-launch-and-icons.md),
  whether the desktop Tools page may find installed programs, install them through
  winget on a click, start them, and show their icons. The Uninstall and App Paths
  keys are documented for installers and read by Microsoft's own scripts, and
  nothing read forbids another program reading them (per-user, 32-bit and Store
  installs each need their own look). winget and its repository are MIT; Microsoft welcomes other package
  managers; the accept switches skip a prompt meant for the person, the COM
  interface accepts by default, and most packages carry no agreement while
  installers run silently, so the page must show each licence and get the
  person's own "I agree" (Berman, 9th Cir. 2022). DaVinci Resolve, Aseprite and
  Ardour are not in winget; Resolve needs Blackmagic's registration form; the
  catalog mislabels Paint.NET, Aseprite and VS Code. An Install button may make
  the page a California "covered application store" from 2027 (open). Show
  each installed program's own icon in place and ship no logos (OBS forbids
  its logo without authorization, Blackmagic publishes no rules). Projects
  and Microsoft say: get software from its maker, never from an advert.
- [`2026-10-20-stove-combustion-air.md`](2026-10-20-stove-combustion-air.md),
  where a wood stove in a tight home should get the air its fire burns. The US
  manufactured-home standard (24 CFR 3280.709(g)) admits a solid-fuel stove only
  with a combustion air inlet leading outside air straight into the fire
  chamber; the Drolet Escape 1800's manuals (2019 and 2024) require its fresh
  air kit in a mobile home (kit AC01211 for the model on legs, AC01316 in the
  older manual) and prefer room air in an ordinary house; EPA's archived Burn
  Wise says nothing on it; NFPA 211 not read. The basis for the game's stove
  taking its fire's air from outside the home through its kit.
- [`2026-10-20-stairs-guards-and-headroom.md`](2026-10-20-stairs-guards-and-headroom.md),
  what a private home's inside stair, its guards and its ceilings must meet: the
  2021 IRC (read through Washington's republished amendments, the Stairway
  Manufacturers' Association's licensed reproduction and city handouts, because
  the ICC's reader serves scripts a captcha), England's Approved Document K 2013,
  Approved Document M volume 1 and the space standard, and OSHA for work
  platforms. Riser at most 196 mm (IRC) or 220 mm (AD K), tread at least 254 mm,
  headroom 2032 mm, a guard over a 600 mm drop (AD K; IRC 762 mm), guards 914 mm
  tall, a 100 mm sphere, ceilings 7 ft; England's 2.3 m ceiling is a planning
  standard, not a building regulation. The game's staircase broke the IRC riser
  limit and every rail rule; a 3.0 m storey needs 16 risers. The 2024 IRC
  renumbered the stair sections to R318 (its text not read).
