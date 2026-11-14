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
- [`2026-10-21-mpc-orbit-data.md`](2026-10-21-mpc-orbit-data.md),
  whether the project may ship a file of about 20,000 asteroid orbits derived
  from the Minor Planet Center's MPCORB to draw the belts on the Maps page.
  Yes, on the three conditions the MPCORB header itself sets for software: ship
  the header (a separate file is fine), attribute the MPC, say where updated
  copies are. The MPC's website policy, read the same day, says data files are
  copyrighted and "may be redistributed as long as the source for the data is
  clearly specified"; that page was reworded between 25 September and 21 October
  2026, and a new nested line could be read more broadly (the finding reads it
  as about circulars). JPL's small-body API gives only a citation request, and
  Lowell's astorb was down. Two open points, one message to the MPC settles both.
- [`2026-10-30-quest-chain-facts-review.md`](2026-10-30-quest-chain-facts-review.md),
  the refutation pass over the 61 survival facts in `docs/design/quest-chains.md` section 3 (water,
  food, warmth, air, hygiene, first aid, sleep), each re-read against its primary source (CDC, EPA,
  NASA, FDA, MedlinePlus, NCHFP, extension services, the Red Cross, the NHS). 38 confirmed, 19
  corrected, 1 refuted in part (distillation does not remove chemicals that boil with the water),
  3 unconfirmed (a solar still's yield, WHO's excreta guidelines, the crop gas figures). Sand filters
  take out viruses in the lab, not in typical use; bleach doubles for cloudy water; a burn cools for
  20 minutes; NASA's 2022 life support baseline supersedes the 2018 one.
- [`2026-10-30-wip-warning-and-liability.md`](2026-10-30-wip-warning-and-liability.md),
  what an unfinished-software warning on every page can and cannot do. The licence (CC0 1.0)
  already disclaims warranties "as-is" but has no clause limiting liability; the AGPL and CC BY-SA
  in the licence vote have one, each yielding to the law. A warning makes the risk obvious, and
  every law read weighs warnings (the EU Product Liability Directive's presentation and
  instructions, Washington's product liability act's adequate warnings, its sales law's "as is").
  No warning makes anyone immune: where the UK's Unfair Contract Terms Act 1977 s.2(1) or the
  Consumer Rights Act 2015 s.65 apply, liability for death or personal injury from negligence
  cannot be excluded by any notice, and the EU directive (Article 15) and the UK's 1987 Act (s.7)
  forbid limiting their liability. Several of these may not reach a free, donation-funded,
  open-source project (the directive leaves out free software supplied outside a commercial
  activity; the UK rules bind a business or trader; Washington's warranties need a sale for a
  price), and no text read settles that. Ends with the warning's words. Not legal advice; a lawyer
  should read it before 1.0.
- [`2026-11-01-pipe-and-cable-routing-rules.md`](2026-11-01-pipe-and-cable-routing-rules.md),
  the "to code" numbers for laying pipe, hose and cable: bend radii (the Copper Development
  Association's copper table, the PEX guide's six times the diameter, PVC and CPVC by fittings
  only, hose, gas tubing, NM-B and THHN data sheets, data and fibre, EMT benders, shipboard and
  aircraft cable), support spacing (24 CFR 3280, the DoD's guide specifications, IPC and UPC 2009,
  46 CFR, FAA, NASA), separation (communications 2 in from power under OSHA, nothing foreign over a
  panel, no piping over switchboards, no shared holes or supports, hot pipes, wiring above fluid
  lines), drain slope and through-wall rules. The NEC, the 2024 plumbing codes and the sold
  standards were not read; no general cable-to-water-pipe distance was found. The data basis for
  `docs/design/spline-routing.md`.
- [`2026-11-01-manual-door-releases.md`](2026-11-01-manual-door-releases.md),
  how a powered door, exit or hatch must open by hand when its power fails: passenger rail (49 CFR
  238.112: no tool, both sides, a cover removable without a tool, luminescent marking), aircraft
  (14 CFR 25.809 to 25.813), buses (FMVSS 217), ships (46 CFR 170.250's power and hand gear,
  174.100, 116.435 and 116.500, the UK MCA), NASA's hatches (NASA-STD-3001 Vol. 2 Rev F), and lifts,
  the exception (keys for trained people only, power off first; California bans the device). The
  basis for the emergency release panel in `docs/design/spline-routing.md`, section 13.
- [`2026-11-03-mp4-container.md`](2026-11-03-mp4-container.md),
  whether the desktop app may read the MP4 container itself (ISO/IEC 14496-12 and 14496-14) to
  play the AV1 video inside it (BUG-228). Probably yes: no pool (Via LA, Access Advance, Sisvel)
  lists a file-format programme, the MPEG-4 Systems pool that once reached the first MP4 format
  stopped licensing by 2011, and the two base-format patents tied to it expired in 2018; AOMedia's
  AV1-in-ISOBMFF binding is a Final Deliverable under its royalty-free terms. Open: the 2026 edition
  says patents were declared, and ISO's database refuses automated reading. The Rust readers'
  licences (`mp4-atom` MIT OR Apache-2.0). Extends the 2026-09-19 codec finding.
- [`2026-11-06-social-platform-apis.md`](2026-11-06-social-platform-apis.md),
  whether a free, open-source desktop app, holding each person's sign-in on their own device, may
  post for them to Mastodon, Bluesky, X, Threads, Facebook, Instagram, YouTube, Reddit, LinkedIn,
  Discord, Telegram, Rumble and Patreon, read their replies, mentions and DMs, and draft with AI;
  whether each platform's DMs are end-to-end encrypted and what it hands to legal process; and the
  laws on labelling AI-written posts. Mastodon and Bluesky fit fully (free, no review, a sign-in a
  public app can do alone); X is paid per call ($0.20 a post with a link); LinkedIn bars automated
  posting and closes reading; Discord forbids acting as the person and credentials in open source;
  Telegram forbids public credentials and using its data to deploy AI; Threads, Facebook Pages and
  Instagram need review, business verification and the app secret; no app can post to a personal
  Facebook profile; Reddit closed new access on 31 October 2026; YouTube, Rumble and Patreon have no
  community-post or message API. Only X Chat, Telegram Secret Chats and personal Messenger chats are
  end-to-end encrypted. The EU AI Act's Article 50 has applied since 2 August 2026 (its review
  exception needs a substantive review and a named responsible person; whether the project is a
  "provider" is open); California's bot law and the FTC do not require the label. The basis for
  `docs/design/outbox-and-inbox.md`. Extends the 2026-09-25 permission-routes finding (Discord).
- [`2026-11-06-music-sources.md`](2026-11-06-music-sources.md),
  which routes can give the CC0 app music anyone may redistribute, and what the research says about
  A = 432 Hz, "healing" frequencies, relaxing music and brown noise. Sample libraries: VSCO 2 CE,
  VCSL, Karoryfer's free libraries and FreePats' Upright Piano KW are CC0 (ship and render freely),
  the Fluid soundfont MIT, Salamander CC BY 3.0; no CC0 choir exists; Sonatina (Sampling Plus,
  retired 2011), the Philharmonia samples (never "as samples or as a sampler instrument"), Virtual
  Playing Orchestra and No Budget Orchestra cannot ship in the app. Models: MusicGen weights CC
  BY-NC, Stable Audio Open capped at USD 1 million revenue and weak at music, Magenta RealTime
  Apache 2.0 with CC BY 4.0 weights and no claim on outputs but a GPU cost. Suno Free and Basic are
  personal and non-commercial only; Udio's terms could not be read; the Copyright Office does not
  protect material whose expressive elements an AI determined, which leaves CC0 working and CC BY
  binding nothing. Contributors: CC0. Health: a few small, mixed 432 Hz trials, mostly one group,
  no systematic review; no human trial of solfeggio frequencies against other music; brown noise
  unstudied for attention, continuous noise "very low" evidence for sleep. The basis for
  `docs/design/music.md`.
- [`2026-11-06-space-program-sources.md`](2026-11-06-space-program-sources.md),
  where the mothership's space program gets its area per person for every kind of space. NASA
  SP-413 (1977) sized its 10,000-person settlement exactly this way: 47 m2 of projected area a
  person for the community and 20 m2 for agriculture, from a table of space uses (homes 49 m2 of
  floor on four storeys, open space 10, transportation 12, a quarter of a car city's, storage 5,
  and the rest), all quoted; circulation inside buildings adds 40 to 60% (GSA, 2012); farm area from
  the 2026-10-03 finding. No sourced floor area for hangars, a bridge, reactor halls or heavy
  industry: the program marks those Proposals. The basis for `data/ship/space_program.toml`.
- [`2026-11-06-noise-and-sleep.md`](2026-11-06-noise-and-sleep.md),
  how quiet a home must be and the other crew-health factors a ship's layout decides. WHO: 30 dB
  inside a bedroom at night with events under 45 dB, 35 dB indoors by day (1999, kept valid in
  2018), 40 dB outside at night (2009), lower for low-frequency noise; NASA: NC-40 in sleep areas,
  events at most 10 dB over background; a closed shell takes off about 25 dB, England asks 45 dB
  between flats. Light: 250 lux melanopic EDI by day, 10 in the evening, 1 at night (Brown 2022).
  Green space of 0.5 to 1 ha within 300 m (WHO 2017), the 3-30-300 rule, WHO crowding and
  accessibility recommendations (2018) and air quality levels (2021). The basis for the master
  plan's crew-health rules.
- [`2026-11-07-take-two-discord-subpoena.md`](2026-11-07-take-two-discord-subpoena.md),
  what the Grand Theft Auto VI leak subpoenas of August 2026 asked Discord and Microsoft for, read
  from the court filings (S.D.N.Y. 1:26-mc-422 and 1:26-mc-421) and the companies' own posts. Take-Two
  asked under 17 U.S.C. 512(h), both were approved on 21 August 2026 (Judge Rochon's Microsoft order
  read; the Discord order is a scanned page, its grant in Take-Two's words), and the Discord subpoena
  asked for message logs, IP access logs, emails, phone numbers, linked accounts and device
  identifiers of every member of three communities since 1 June 2026. No filing gives a number (the
  "~100k" came from a commentator; the largest server has 117,129 members by Discord's count on 7
  November 2026), no real names were asked for, and nothing public shows either company handed
  anything over (Discord said on 24 August it had not yet been served). Corrects the opening story
  of `docs/outreach/how_your_privacy_is_built.md`.
- [`2026-11-07-community-size.md`](2026-11-07-community-size.md),
  whether about 500 people is "the upper limit for one massive group to work together", as the
  operator had heard. In Dunbar's layers 500 is the circle of acquaintances, not the group that
  works together (about 150, where Hutterite colonies split at a mean of 166 and Alpine commons
  held 140 to 176 for six centuries); 500 is also where communities in the record start to need a
  formal structure (kibbutzim near 468, campsites near 500, mega-bands near 567). The 2021
  critique (intervals of 4 to 520) and Dunbar's reply, both quoted. Recommends 500 as a sector's
  cap with homes of 5, clusters of 15, blocks of 50 and three neighbourhoods of about 150 inside
  it. The basis for the master plan's 500-person sector.
- [`2026-11-07-drum-limits.md`](2026-11-07-drum-limits.md),
  how big, how long and how steep a 1 g spinning drum can be. Hoop stress is density x g x radius
  (Jensen 2024): steel drums top out near 4.5 km radius, aramid and carbon composites at tens of
  kilometres; a free cylinder spins stably only up to 1.3 radii long (Globus et al. 2007, via
  Jensen), so Kalpana One's 250 m radius was cut to 325 m and Drum 1's 2 km is 8 radii; the main
  belt is about 4 x 10^-4 Earth masses (Pitjeva and Pitjev 2018); the ADA lets a walkway slope 1 in
  20 before it is a ramp. The basis for the master plan's scale study and the lean.
- [`2026-11-07-privacy-notice.md`](2026-11-07-privacy-notice.md),
  what a privacy notice for united-humanity.us must say, and which laws reach a free,
  non-commercial project run by one person in Washington State: the EU GDPR probably not (no
  targeting; the EDPB's own Example 16), though being free is no exit and an EU representative
  would follow if it did; the UK GDPR the same, with the new right to complain to the controller
  (s.164A, 19 June 2026); the CCPA no; CalOPPA unclear on "commercial"; Washington's My Health My
  Data Act no while no health data reaches the server (checked in the code); COPPA only if
  commercial and child-directed or on actual knowledge. No consent banner, age gate or request
  form is required as things stand. The basis for the /privacy page.
- [`2026-11-08-motionbricks.md`](2026-11-08-motionbricks.md),
  whether HumanityOS may use NVIDIA's MotionBricks (SIGGRAPH 2026): its code is Apache-2.0 (port it
  with the notices; no NOTICE file; one MIT file); its weights are under the NVIDIA Open Model
  Agreement (2 April 2026): free to ship, fetch, modify and run on any hardware, outputs unclaimed,
  no use limits, but no patent licence, ended by a copyright or patent suit about the model or its
  output, not OSI-approved, and released without training code or data, so whether it meets the
  open-source-only rule is the operator's call; the 380 MB FP16 model was trained on Bones RP1 under
  a negotiated licence nobody can read; the bundled sample clips are evaluation-only; TensorRT is not
  required (ONNX Runtime's CPU path is documented). The method is free of copyright (17 U.S.C.
  102(b)) but the field has active Electronic Arts patents (US12582906B2 on transformer
  inbetweening, two on motion matching variants). Open motion data: CMU, ACCAD (CC BY 3.0) and
  100STYLE (CC BY 4.0) usable; LAFAN1, AMASS, SFU, Bandai Namco, AI4Animation and BONES-SEED (bars
  generative motion models) not; MakeHuman bodies and Quaternius clips CC0. The basis for
  `docs/design/character-animation.md`.
- [`2026-11-08-nvidia-simready-assets.md`](2026-11-08-nvidia-simready-assets.md),
  whether NVIDIA's SimReady Assets (2,677 real-scale 3D objects with physics data, on Hugging Face)
  may be converted from USD to glTF, changed and shipped in the game. Yes under CC BY 4.0: every
  asset's own packaging file says "CC-BY-4.0" and every one carries the licence text (all checked);
  format conversion alone is a "technical modification", real changes are an adaptation and need a
  "modified" note; no gate, NOTICE or NVIDIA terms beyond the licence; credit NVIDIA with the
  licence and dataset links (no wording requested), on the Credits page and in each asset's folder.
  The MDL shader files carry NVIDIA's BSD-style licence and need not ship; no one's marks are
  licensed, and two assets are named after trademarked products (WD-40, Rubik's Cube), so leave them
  out; textures were not looked at, so each chosen asset's colour map needs a look for printed
  brands. The assets stay CC BY inside the CC0 project.
- [`2026-11-08-paradee-tts.md`](2026-11-08-paradee-tts.md),
  whether Paradee 1.2 (an 8.5 MB English text-to-speech model) may ship in the app and on the
  website for Library read-aloud and a ship-computer voice. Yes under Apache 2.0 for the code and
  weights, with the licence text beside the files; no NOTICE file exists. eSpeak NG (GPL-3.0) is not
  in the model and can be left out cleanly, but it enters through Paradee's Python helper and through
  kokoro-js's browser phonemizer, which carries a compiled eSpeak NG under an Apache label; without a
  fallback, misaki 0.9.4 drops unknown words silently. Unclear: misaki's dictionaries (the silver ones
  are eSpeak output) and the voices' origin (Kokoro trained partly on closed providers' synthetic
  speech; `am_puck` and `bm_fable` share names with Google and OpenAI voices; no consent record), and
  whether StyleTTS 2's "tell listeners it is synthesized" condition reaches it. Recommends read-aloud
  now with a "computer voice" label, the ship-computer voice held for a voice with written consent.
- [`2026-11-08-spark-x2-6-small-model.md`](2026-11-08-spark-x2-6-small-model.md),
  whether Spark-X2.6-1.7B (a 1.77 GB small language model) may be bundled or fetched for local NPC
  dialogue and an on-device helper. Yes: Apache 2.0 on every card, the `LICENSE` file present and
  identical in the post-trained, base and GGUF repositories ("Copyright 2026 XHToken"), no NOTICE,
  no outside base model, no dataset named, no acceptable-use policy and no claim over outputs, so
  nothing binds players. Unclear: the publisher ("SparkLLM", unverified, Singapore on GitHub, its
  site unreadable) is called an iFlytek subsidiary only by third parties, and iFlytek is on the US
  Entity List (since 2019, for its part in surveillance of Uighurs), which governs exports to it, not
  using its open model. Recommends GGUF through llama.cpp only, served from the project's own release
  assets, an "AI" label, and settling who the publisher is before it becomes the default.
- [`2026-11-08-ministral-3-3b.md`](2026-11-08-ministral-3-3b.md),
  whether Ministral 3 3B Instruct 2512 (Mistral AI) may be served as a GGUF for speaking NPCs and the
  helper. Yes: Apache 2.0 on the instruct, base and GGUF cards, the launch post and the paper; every
  ancestor is Mistral's own (Mistral Small 3.1, Apache 2.0; instruction tuning distilled from Mistral
  Medium 3); Mistral's Usage Policy excludes "our open-source AI models"; one italic sentence under
  the licence restates the law. No LICENSE or NOTICE file; training data undisclosed. The vision
  encoder is a separate `mmproj` file and need not ship (text model 2.15 GB); the repository's system
  prompt makes the model say it powers "Le Chat", so the project writes its own.
- [`2026-11-08-granite-4-0-1b.md`](2026-11-08-granite-4-0-1b.md),
  whether IBM's Granite 4.0 1B may be served as the low-memory NPC brain. Yes: Apache 2.0 on every
  card and as the unaltered `LICENSE` in IBM's GitHub repository; the base is IBM's own, trained from
  scratch; IBM's disclosure (30 March 2026) lists 131 pre-training and 115 post-training datasets,
  none marked as user data, but no licence per dataset. A sentence on IBM's Granite page (in its
  watsonx indemnity section) forbids using Granite to infringe copyright, which is not in the licence;
  IBM's Responsible Use Guide link redirected and could not be read. IBM's own Q4_K_M GGUF, 1.02 GB.
- [`2026-11-08-gemma-4-e2b-e4b.md`](2026-11-08-gemma-4-e2b-e4b.md),
  whether Gemma 4 E2B and E4B are plain Apache 2.0 now. Confirmed: the cards link Google's "Gemma 4
  license" page, which is the unaltered Apache text (1 April 2026); the Gemma Terms of Use, revised
  the same day, open "For Gemma 4 terms, see the Gemma 4 license" and list no Gemma 4 model; the 2024
  Prohibited Use Policy attaches only through those Terms, so it does not reach Gemma 4 as written,
  though it still sits beside the licence in the docs menu. Google's own Q4_0 GGUFs keep the image
  and audio encoders in a separate `mmproj` file (E2B 3.35 GB, E4B 5.15 GB without it). Use "Gemma"
  only to say where the model came from.
- [`2026-11-08-pocket-tts.md`](2026-11-08-pocket-tts.md),
  whether Kyutai's Pocket TTS may voice NPCs and the helper. Yes under CC BY 4.0 (weights) and MIT
  (code), with attribution; the "Prohibited use" text is not part of the licence and must not be
  passed to players as one. Found what the scan could not: the January 2026 weights were trained on
  AMI, EARNINGS22, GigaSpeech, SPGISpeech, TED-LIUM, VoxPopuli, LibriHeavy and Emilia (88k hours),
  four of which carry non-commercial or research-only terms for their users; the current default
  English weights and the other languages have no published data list. Consent records quoted per
  voice set: the CC0 volunteer donations (terms of 26 June 2025 that foresee synthetic speech and
  allow withdrawal), Kyutai's own CC0 recordings, Alba MacKenna's CC BY 4.0 RPG characters,
  Voice-Zero, VCTK; the default voices "cosette" and "jean" are non-commercial and two others have
  no source. No licence requires a label, but Kyutai's text and the EU AI Act's deep-fake rule point
  to one. Recommends the ungated weights, documented voices only, a "computer voice" label, and a
  project-trained voice on documented data later.
- [`2026-11-08-audio-cpp.md`](2026-11-08-audio-cpp.md),
  whether audio.cpp (ShugoAI LLC) may be the runtime for Pocket TTS. Yes under Apache 2.0 with its
  vendored dependencies (ggml MIT, SentencePiece Apache 2.0 with BSD and MIT parts, cJSON, httplib,
  libyaml, LZ4's library, utf8proc), each notice shipped. eSpeak NG is compiled in only with
  `AUDIOCPP_STATIC_ESPEAK=ON` (default off), but every build carries a loader for an installed
  eSpeak, used by other models, never by Pocket TTS; a `custom` build of only `pocket_tts` leaves it
  unreachable and leaves out a model folder carrying Liquid AI's LFM licence. A GGUF conversion keeps
  the model's own licence (audio.cpp's table, Apache's "Object" form, CC BY 4.0 section 2(a)(4)).
- [`2026-11-08-food-data-sources.md`](2026-11-08-food-data-sources.md),
  which ingredient, nutrition, allergen, additive and food-safety sources may ship, and what a label
  reader must never claim. USDA FoodData Central and FoodKeeper are CC0, FDA's pages public domain
  (copies dated and linked). Allergen lists are law and differ: nine in the US (sesame since 1 January
  2023), fourteen in the EU, Canada's own; "may contain" is voluntary in the US, risk-assessed in the
  EU, and Codex adopted guidelines on 7 July 2026. Status is not risk: FDA proposed new GRAS oversight
  on 10 August 2026 and revoked Red No. 3 for food from 15 January 2027 while calling risk claims
  unsupported. Open Food Facts is ODbL (contents DbCL, photos CC BY-SA, "must not be used for medical
  purposes", one API call per real scan): runtime and opt-in only, with the same two obligations as
  OpenStreetMap if anything derived is ever published. EFSA, FAO/Codex and WHO/JECFA are facts only;
  the Canadian Nutrient File 2026 (OGL-Canada 2.0) and ANSES Ciqual (Licence Ouverte 2.0) are clear.
  FDA lists gluten-free information apps as not medical devices. OCR: PP-OCRv6 and Tesseract are
  Apache 2.0; Surya's weights are revenue-capped. The prototype's Yellow 5 line is refuted. The basis
  for `docs/design/ingredients-and-labels.md`.
- [`2026-11-08-whole-earth-map-data.md`](2026-11-08-whole-earth-map-data.md),
  which open data can draw the whole Earth in a CC0 game and what each licence obliges. OpenStreetMap:
  the rendered game is a Produced Work (a notice where drawn; for games the credits page or menu, with
  a short line in view), our processed tiles are a Derivative Database (ODbL, the licence inside each
  file, offered free), and tiles fetched on demand are still "Conveyed", so each pack carries its own
  notices; nothing permissive replaces OSM for roads, water and names (Overture's transportation, base
  and buildings themes are ODbL; Microsoft's roads are ODbL; GRIP4 says CC0 but is built partly from
  OSM). Keeping heights, seabed and land cover as independent layers, joined only at run time, keeps
  share-alike off them (the OSMF Horizontal Layers and Collective Database guidelines). Heights:
  Copernicus GLO-30 is free with a notice and a liability sentence that bind later users, and is a
  surface model (trees and roofs); NASADEM is CC0 but stops at 60 N; ETOPO 2022 is CC0 bare earth,
  though built partly from Copernicus and FABDEM; FABDEM is non-commercial; GEDTM30 (CC BY 4.0, 2026)
  is the only global 30 m bare-earth model found that any use may carry; GEBCO 2026 and USGS 3DEP are public
  domain; the AWS Terrain Tiles behind today's `.dem.bin` files carry two "All rights reserved"
  national sources. Microsoft's building footprints moved from ODbL to CDLA Permissive 2.0 on
  11 March 2026; Google's are CC BY 4.0 or ODbL; ESA WorldCover is CC BY 4.0; Natural Earth is public
  domain; Wikidata CC0. Two compliance gaps still open in the code: the ODbL offer is not in the
  release archive, and the in-world credit hides in the construction editor and showroom. One source
  recommended per layer. The basis for `docs/design/whole-earth-maps.md`.
- [`2026-11-09-model-runtime.md`](2026-11-09-model-runtime.md),
  how the app runs AI models when the runtime itself must be a separate, optional download. Compares
  llama.cpp and ggml (MIT; ggml joined Hugging Face on 20 February 2026), audio.cpp (Apache 2.0;
  carries its own modified copy of ggml), ONNX Runtime (MIT; DirectML "in sustained engineering") with
  the `ort` binding (a release candidate that downloads binaries from pyke's CDN by default), candle
  (no Vulkan), burn (widest Rust GPU reach, limited ONNX), tract (pure Rust, CPU and WebAssembly) and
  the engine's own wgpu path: licences quoted, publishers checked against the Entity List and the
  1260H list (none listed), hardware, Windows archive sizes, hosting. Model files are hostile input
  (thirteen llama.cpp advisories, four tract model-load advisories in 2026); a library in the app
  would run beside the person's unlocked keys; GitHub caps each release file under 2 GiB, so Gemma 4
  must be split. Recommends one AI engine program the project builds, downloaded separately, talking
  over pipes with no network: llama.cpp, audio.cpp with only Pocket TTS, and tract (ONNX Runtime as
  the fallback); a learned motion model stays on the wgpu path with its shaders in its pack. The basis
  for `docs/design/embeddable-ai.md`.
- [`2026-11-09-open-3d-sources-index.md`](2026-11-09-open-3d-sources-index.md),
  one table of the day's verdicts on the open model, material and HDRI sources the 2026-11-08 asset
  scan ranked, with their credit lines and conditions, the sources not researched, and six lessons
  that apply to every source (read `robots.txt` first; licences often live outside the files;
  credits chain; licences change; CC covers copyright only; NoAI is the one AI question).
- [`2026-11-09-poly-haven-and-ambientcg.md`](2026-11-09-poly-haven-and-ambientcg.md),
  whether Poly Haven's and ambientCG's models, textures and HDRIs may ship. Both CC0, no credit
  required (both ask for it). Poly Haven's API terms (changed 18 July 2026) bind only live-API use
  inside an app, never downloaded files; its website terms forbid scraping, so scripts use the API
  with a user agent naming our software; its descriptions, renders and logo are not CC0. ambientCG
  has no terms of service or API terms at all. CC0 clears no brand, sign or person shown in an asset.
- [`2026-11-09-gazebo-fuel-models.md`](2026-11-09-gazebo-fuel-models.md),
  how Gazebo Fuel models state their licence (a field in Fuel's API, never a file in the model; seven
  licences possible, CC0 and CC BY 4.0 usable), what CC BY models owe (per-model credit from
  `model.config`, whose Open Robotics `Oven` credits a Sketchfab original too), and that Fuel has no
  terms of service beyond copyright, deletion and data policies. Google's branded scans and two
  authorless CC0 models are left out.
- [`2026-11-09-amazon-berkeley-objects.md`](2026-11-09-amazon-berkeley-objects.md),
  Amazon Berkeley Objects' 3D models: CC BY 4.0 (the bucket's licence file is the legal code word for
  word), changed from CC BY-NC in 2023 as Amazon's registry entry says (its 2022 paper still says
  NC); credit "(c) Amazon.com" plus the dataset builders (six names in the README, ten on the index
  page). Real products with brands: the licence grants no marks and does not clear other makers'
  rights, so generic names only, textures checked; brands per model need the 83 MB listings metadata.
- [`2026-11-09-kenney-assets.md`](2026-11-09-kenney-assets.md),
  Kenney's game assets: CC0 by its support and Furniture Kit pages, attribution optional, logo
  reserved; the licence file inside downloads and the terms of service unread, because kenney.nl's
  `robots.txt` disallows AI agents (two pages were read before it was seen; recorded). The project
  ships 15 Furniture Kit models and Kenney audio without Kenney's own licence file.
- [`2026-11-09-sketchfab-cc-models.md`](2026-11-09-sketchfab-cc-models.md),
  Sketchfab's CC0 and CC BY models under its terms of 27 October 2026 (KitBash SF Operations LLC):
  the licence at download governs, downloads need a logged-in adult's account, Standard and Editorial
  models cannot ship in an open data folder. NoAI is a contract term of the platform, not part of the
  CC licence (Sketchfab says only the CC licence follows a model off the platform), but an AI-run
  conversion is plausibly "inputs to Generative AI Programs": avoid NoAI tags and statements. Per-model
  credit in Sketchfab's own pattern, and a seven-point checklist per pick.
- [`2026-11-09-reference-3d-collections.md`](2026-11-09-reference-3d-collections.md),
  NASA 3D Resources (US public domain by NASA's guidelines of 27 October 2026, credit NASA, insignia
  reserved; the repository's `meta.json` names NOSA 1.3 but includes no agreement, and per-model
  authorship is unrecorded), Khronos glTF Sample Assets (per model: 62 CC0, 45 CC BY 4.0, 9 not
  usable, logos never licensed) and Smithsonian Open Access (CC0 per record; its terms page refused
  automated reading).
- [`2026-11-12-music-genre-copyright.md`](2026-11-12-music-genre-copyright.md),
  what copyright protects in music (the composition, words included, and the sound recording) and
  what it leaves free (ideas and methods, scales, standard chord progressions, a genre's commonplace
  building blocks), so a genre in the live score may compose from a style's general characteristics
  but must never copy a band's recordings, melodies, lyrics, riffs or one song's distinctive
  combination of elements, nor use a band's name to suggest it approves. The basis for the electro
  swing genre in `docs/design/music.md` section 7a.
- [`2026-11-13-active-shielding.md`](2026-11-13-active-shielding.md),
  a negative finding: superconducting magnets (and electric or plasma fields) cannot replace or
  usefully thin the mothership drum's water shield. With their structure modelled they cut the
  cosmic-ray dose by about a fifth to two thirds (SR2S, Vuolo et al. 2016; MAARSS 2012;
  Hesse-Withbroe and Arquilla 2025), never the factor of about 100 the drum needs; they remove the
  solar-storm hazard (Musenich et al. 2018), which 10 m of water already does; a field adds less the
  thicker the passive shield (OLTARIS, Globus and Strout 2017); NASA SP-413 rejected them in 1977.
  ICNIRP's static-field limits (400 mT public, 0.5 mT for implants) quoted. The basis for keeping
  the passive shell in the master plan's 3.16.
- [`2026-11-13-pulleys-and-rigging.md`](2026-11-13-pulleys-and-rigging.md),
  the block and tackle's numbers for docs/design/pulleys.md: friction compounding per sheave
  (Crosby's line-pull ratios, the Army's 10 percent rule), rope strength by the square of the
  diameter (FM 5-125), sheave and block size rules (FM 5-125, the Navy's Steelworker manual), design
  factors (4 for fibre rope and blocks, 5 for running wire rope), how hard people pull, winch and
  chain hoist ratings. The truck winch maker's page and the eCFR refused automated reading.
- [`2026-11-13-pulling-force-by-age.md`](2026-11-13-pulling-force-by-age.md),
  a NEGATIVE finding: no readable primary source gives the force a person keeps up while pulling by
  age (an older adult, a child). The HSE's figures are by sex and the CCOHS's for one population,
  both safe-work limits, not capacities; the DTI's 2000 strength study measured maximum exertions
  and its original was not found. So the Pulleys mode adds no lighter pullers yet.
- [`2026-11-13-free-instrument-libraries.md`](2026-11-13-free-instrument-libraries.md),
  which free multi-sampled instrument libraries may ship in the app or its packs: Karoryfer's whole
  free catalogue is CC0 only from its releases of 2022-11-04 (CC BY 4.0 and one GPL 3 before),
  FreePats' CC0 banks, AKWF, LMMS assets, Virtuosity Drums and jSteelDrum version 2 ship anywhere;
  epic-choir (the first open choir of quality), TinySOL, Greg Sullivan's electric pianos (CC BY 4.0
  since 2026-10-21) and DrumGizmo's kits need credit; no drum-machine sample set may be redistributed;
  No Budget Orchestra is a mix of licences, not CC BY-SA. Freesound, kenney.nl and several sample
  sites refused automated reading.
- [`2026-11-13-slag.md`](2026-11-13-slag.md),
  what slag from asteroid ore is and what a ship can do with it: a magnesium silicate (about 55%
  silica, 35% magnesia, computed from Lodders 2021), not Earth's lime-rich slag (FHWA); 2.3 to 24 t
  per tonne of metal from stony ore, 10 to 15 kg from metallic (Ross 2001, Vernazza et al. 2016,
  Sanchez et al. 2021); Earth's uses (USGS 2017); the cement rule it fails (EN 197-1 via a 2025
  EPD) and the alkali route; cast basalt (NASA 1991); slag wool (NAIMA, NIST); meteoroid flux and
  crater depth (Grün via ESA SPENVIS, NASA 2006); the mass-driver reaction engine (O'Neill 1977,
  SP-428's powder rule); NASA SP-413's slag shield that ran out of slag; balance weights (Kalpana
  One, NASA TN D-6924); heat (Siemens Gamesa, CSIRO); soil and phosphorus filters; molten
  electrolysis at about 104 kWh per kg of oxygen (NASA 2026). The basis for docs/design/slag.md.
  USGS's newest slag chapters, Jarosewich 1990 and mdpi.com refused automated reading.
