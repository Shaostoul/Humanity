# Open Source Ecology: may HumanityOS use its machine blueprints and documentation, and how

**Researched 2026-10-25** (sources read on 25 October 2026, Pacific time). Everything below was
read on that day; each source carries its own date. If you are reading this later, read the
licence page (finding 2) again first: it was edited as recently as 8 July 2026.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** Where it says what the project should do, that is marked as a
judgement call.

## The question

Can HumanityOS use what Open Source Ecology (OSE) publishes, its machine blueprints, bills of
materials, build instructions and housing designs, and on what terms, given that HumanityOS's own
work is CC0 and only public-domain material ships in its release (operator decision, 2026-09-15,
`schemas/source.toml`)?

## Short answer

- **Yes as a source of facts and a place to send people; no as text or files to copy.** OSE puts
  its website, its wiki and its design files under **CC BY-SA 4.0**, hardware files under **CERN
  OHL-S-2.0** and software under **AGPL** (findings 1 to 3). All three are share-alike, so none of
  it may be copied into HumanityOS's CC0 code, data or Library, or shipped in the release.
- **What HumanityOS may do:** link to it (it is already in `data/external/catalog.json`), and cite
  it as the authority for facts restated in our own words, which is `use = "facts"` in
  `data/sources/registry.json` (registered as `opensourceecology` with this finding). Facts are not
  copyrightable (Feist, 1991, as `schemas/source.toml` sets out); its text, tables, drawings and
  CAD are.
- **What it is good for:** real machine numbers for the game's machines and the homestead (for
  example, the CEB press makes compressed earth bricks of 4 by 6 by 12 inches at 6 to 10 a
  minute, finding 4), and a pointer for people who want to build the real thing. How complete
  each machine's documentation is varies a great deal (finding 5).

## Findings

### 1. The website's licence

- Source: https://www.opensourceecology.org/ (front page footer), as served 2026-10-25. The
  newest front-page posts were dated 19 April 2025 to 27 July 2025.
- Quote: "All Open Source Ecology content on this website is licensed under a Creative Commons
  Attribution-ShareAlike 4.0 International License"

### 2. The wiki's licence page: text, design files, hardware files, software

- Source: https://wiki.opensourceecology.org/wiki/OSE_License, "last edited on 8 July 2026, at
  19:02".
- Quotes: "CC-BY-SA-4.0 is what we put all of our content under on the wiki, same for our design
  files." "For actual hardware files, we use CERN OHL-S-2.0" "Software should be AGPL." "The
  license we use must be OSHWA compliant for both hardware and software."
- And: "However, the OSE License requires additional information to be disclosed", which points
  to the OSE License for Distributive Economics (not read today: see Sources not reached).

### 3. The wiki's footer

- Source: every wiki page read today, including the licence page above.
- Quote: "Content is available under CreativeCommons by-sa4.0 unless otherwise noted."

### 4. What one machine's documentation contains (the CEB press)

- Source: https://wiki.opensourceecology.org/wiki/CEB_Press, "last edited on 30 April 2021, at
  01:46".
- The page links its own "Bill of Materials | Manufacturing Instructions | User's Manual" and has
  a section "Specifications of the Standard Model".
- Quotes: "Full size bricks are 4"x6"x12", plain flat-faced bricks", "6-10 bricks per minute,
  depending on power unit", "Version 17.08 is our most current model."
- **It disagrees with finding 2 about its own licence:** "Open source hardware, controller, and
  software ( CC-BY-SA 4.0 , GPLv3 )". The licence page says hardware files are CERN OHL-S-2.0 and
  software AGPL. Both are share-alike, so the difference does not change what HumanityOS may do;
  it does mean a particular file's terms should be read on its own page before anyone copies it.

### 5. What the Global Village Construction Set is

- Source: https://wiki.opensourceecology.org/wiki/Global_Village_Construction_Set, "last edited
  on 4 September 2026, at 01:34".
- Quote: "a modular, DIY, low-cost set of blueprints that enables fabrication of the 50 different
  Industrial Machines that it takes to build a small, sustainable civilization with modern
  comforts."
- The website's machine list on 2026-10-25 showed the MicroHouse at "Prototype Stage" and the
  Microcombine, the electric motor/generator and the 50 kW wind turbine at "Planning Stage"; its
  2025 posts are about housing (the Seed Eco-Home, an 800 square foot starter home course).

## What is still unknown, and what would settle it

- **Whether the OSE License for Distributive Economics asks anything of someone who only cites
  facts or links.** Its wording suggests it applies to those who distribute OSE designs or
  products (disclosure of toolchains, bills of materials and supply chains). Settled by reading
  that page (free, about ten minutes) before HumanityOS ever distributes an OSE design or file.
- **Which machines are documented to a build-ready level.** Per machine: read its page's date,
  stage and bill of materials before relying on any number from it.
- **Where "design files" end and "hardware files" begin** (CC BY-SA 4.0 or CERN OHL-S-2.0). It
  does not matter while HumanityOS only cites facts; it matters if a file is ever redistributed.

## Sources not reached

- http://opensourceecology.org/wiki/OSE_License_for_Distributive_Economics: not fetched today;
  wanted for the extra disclosure terms named in finding 2.

## What it means for this project

**Certain (from the sources):**
- Everything OSE publishes is under a share-alike licence, so none of its text, drawings, tables,
  CAD or software may go into HumanityOS's CC0 work or its release.
- Its numbers (dimensions, outputs, materials) may be cited as facts restated in our own words,
  with the page as the source; registered as `opensourceecology`, `use = "facts"`, `bundle =
  false`.

**Judgement calls (ours, not OSE's):**
- Most useful for the game's machines: realistic outputs, sizes and materials for a brick press,
  a tractor, a power unit or a sawmill, the way USDA figures went into food (v0.1527.0), each
  number cited to its machine's page.
- Useful as a reference for the homestead and cabin designs (OSE's housing work), again as facts.
- For build instructions, link to OSE rather than restate them: a machine build is a hazardous
  procedure, a How-to would need its own sourced, reviewed steps, and OSE's pages vary in date and
  stage.
