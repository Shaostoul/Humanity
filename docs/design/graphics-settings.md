# Graphics settings: every one says what it changes and what it costs

The operator, 2026-11-10: "I want to make sure when people change graphics settings they know what
they're actually changing." CLAUDE.md made it a standing rule the same day ("Every graphics setting
says what it actually changes"): each choice states in words what you will see differently and
what it costs (video memory, frame time, download size) on this computer; a bare "Low / Medium /
High" with no explanation is a defect; quality levels are data, and a level whose source art does
not exist yet is left out or says so, never upscaled and labelled as more.

This document is where the rule lives for every future setting: how the page meets it, the checklist
a new setting passes, and the audit of 2026-11-10 that brought every control on Settings > Graphics
up to it.

## How the page meets it

- **One row per control** in `data/gui/graphics_settings.ron` (compiled in): the control's name and
  hint by key (`graphics.<id>`, `graphics.<id>_hint` in `data/i18n/en.json`, or the section's own
  keys) and how its cost is known:
  - `Gpu([ids])`: it turns these passes on or off, or scales them. **Measured live**: the graphics
    card's time each frame from its own timers (`humanity_core::frame_costs`, the Performance page's
    numbers).
  - `Shares([ids])`: it changes work inside passes that do other work too (the world surface pass
    draws ground, planets, sky and water together). **Measured live**: the passes' whole time, and
    the line says this setting is one part of it.
  - `Cpu([ids])`, `Memory([ids])`: processor stages, video memory allocations. **Measured live.**
  - `Frame`: it caps or paces frames. **Measured live**: the frame time and rate.
  - `Computed`: its own lines work the cost out from the choice and show it with it (the bounced
    light levels' memory, the tree models' triangles, the grass tufts, the glow picture's memory,
    the garment texture pools, the star catalogues' memory).
  - `Download`: its sizes are on its buttons, read from the release record; nothing downloads until
    one is pressed.
  - `Free(key)`: nothing measurable, and the words say why (a number the shading already uses, how
    the window sits on the screen).
- **Under each control, `graphics_hint`** (`src/gui/pages/settings/graphics.rs`) draws the hint with
  its glossary marks and the cost line: "Cost on this computer: 1.25 ms of the graphics card's time
  each frame now, in the sun's shadow pass (measured)." It says when it cannot know: before the
  world has drawn a frame ("measured while you are in the world"), a pass this card cannot time (an
  adapter without timestamp queries, rather than a "0.00 ms" that reads as free), a pass not running
  now ("takes no time"). The person's hint mode applies (inline, behind a (?), or off).
- **The ratchet**: `every_graphics_control_states_its_cost` (`settings/graphics_tests.rs`) fails when
  a control on the page has no `graphics_hint` after it, a row is drawn by nothing, a row names no
  cost, a measured id is one the engine never records (it would read 0 and look free), or a row's
  words are missing; `the_cost_line_says_how_it_knows` holds the line's honesty.

### Checklist for a new graphics setting

1. Its name and hint by key (`node scripts/i18n-add.js`): the hint says what you will SEE
   differently, in plain words, jargon marked for the glossary.
2. A row in `data/gui/graphics_settings.ron` saying how its cost is known. Prefer a measurement: if
   the work it drives has no timer, add one (`frame_costs::stage`, a pass timer) rather than
   writing `Shares` of a large pass. A level list (a choice of sizes) is data, with each level's
   memory or download worked out and shown beside it.
3. `graphics_hint(ui, theme, hint, "<id>")` straight after the control.
4. A choice that needs hardware the card may not have says so in words (the hardware-notice rule):
   what it needs, what the card has, what works instead.
5. `just test-lib graphics_tests` green.

## The audit (2026-11-10)

Before: the page's words were written into the code (140 of `settings.rs`'s 420 literal words), most
hints said what you see but costs were words without figures ("more GPU", "small GPU cost", "a little
more FPS"), several said nothing of cost, two controls had no description at all, and one size was
wrong. "Sees" is whether the control said what you will see differently; "Cost" whether it said what
it costs; "Now" is how its cost is known since this audit.

| Control | Sees | Cost (before) | Now |
|---|---|---|---|
| Window mode | yes | none | Free: only how the window sits |
| Presets | names and hover notes (the presets file's own words) | Extreme "4x the reference cost" only | Free: what the controls it sets cost, each under its control |
| VSync | yes | "frame rate capped" | Frame, measured |
| Foreground frame-rate cap (switch and slider) | yes | "saves power and heat" | Frame, measured |
| Background frame-rate cap (switch and slider) | yes | "frees your GPU and CPU" | Free, with why |
| Field of view | yes | none | Shares: scene and world surface passes, measured |
| Render distance | yes | vague ("more GPU work") | Shares: scene pass, measured |
| Procedural surfaces | yes | vague ("small GPU cost") | Shares: world surface pass, measured |
| Planet sharpening threshold (was "LOD pixel threshold") | yes, jargon | vague | Shares, measured; "LOD" marked for the glossary |
| Most planet detail levels | partly | vague ("big meshes") | Shares and Memory (meshes), measured |
| Ground detail around you (was "Chunked surface detail") | yes | none | Shares and Memory (ground pieces), measured |
| Smooth detail transitions | yes | vague ("almost nothing") | Shares, measured |
| Terrain sharpness | yes | vague | Shares and Memory, measured |
| Ground pieces kept at once (was "Terrain patch budget") | yes | vague ("more memory + GPU") | Memory and Shares, measured |
| Fine ground detail distance | yes | vague | Shares, measured |
| Terrain stream speed | yes | "a few ms per frame" | Cpu (building and sending ground pieces), measured |
| Trees: 3D models within | yes | triangles a model, quadratic growth | Shares and Cpu (choosing the trees), measured |
| Trees: 3D models drawn at once | yes | worked-out triangles | Computed (unchanged) and Shares, measured |
| Trees: silhouettes out to | yes, and the effective reach measured | vague ("fill rate") | Shares, measured |
| Trees: forest density | yes | "0 is the cheapest" | Shares, measured |
| Grass: draw distance | yes | worked-out tufts and gathering cost | Computed (unchanged), Cpu and Memory, measured |
| Grass: tuft cap (was "instance cap") | yes | vague ("straight-line") | Memory and Cpu, measured |
| Grass: ground cover | yes | **wrong**: "about how the world LOOKS, not how fast it runs" (more tufts are drawn and kept) | Shares and Memory, measured; the claim removed |
| Grass: blade detail | yes | "for frames" | Shares, measured |
| Water: wave detail | yes | "open ocean costs the same" | Shares (water pass), measured |
| Sun shadows | yes | vague ("a little more FPS") | Gpu (shadow pass), measured; since 2026-11-10 also the near sun view's pass |
| Room light shadows (new 2026-11-10, docs/design/local-light-shadows.md) | yes: what casts, which lamps, Off loses the near sun shadows too | (new) | Levels as data (data/gui/shadow_levels.json: Off, Simplified, Full), each level's lamps, face size, near-sun texel and memory computed beside it; Gpu (the two atlas passes), Cpu (choosing and drawing the casters) and Memory (the atlas), measured; the card's memory against the level, warned past an eighth; how many lamps cast shadows this frame. NOT yet in the line (2026-11-11): the scene pass's share, the sampling and the lookup compiled into it, which costs even at Off (local-light-shadows.md section 10; rung 1b makes Off free) |
| Many lights at once | yes | vague ("slightly cheaper") | Cpu (light tiles) and Shares, measured |
| Bounced light detail | yes | memory per level, worked out | Computed (unchanged) and Gpu (bounced light pass), measured |
| Shadow strength | yes | none | Free: a number the shading uses |
| Aerial haze strength | yes | "almost free" | Free: a number the shading uses |
| Light shafts (was "God ray intensity") | partly | none | Gpu (light shafts pass), measured; "god rays" marked for the glossary |
| Sun glare: camera opening | yes | "costs nothing" | Free |
| Ambient occlusion | yes | none | Gpu (contact shading pass), measured; marked |
| Scattering atmosphere | yes | none | Shares (world surface, sky table), measured |
| Cloud layer | yes | vague ("very old GPUs") | Gpu (cloud passes), measured |
| Live weather | yes | "needs internet once" | Free, with the download worked out from the code: 1440 by 720 pixels a session, capped at 16 MB |
| Cloud quality | yes, each level | none per level | Gpu (cloud passes), measured at the chosen level |
| Surface detail | yes | vague ("very old GPUs") | Shares (world surface, water), measured |
| Underwater clarity | yes | none | Free |
| Orbit rings | **no description** | none | Gpu (sky lines pass), measured; description added |
| Constellation figures | **no description** | none | Gpu (sky lines pass), measured; description added |
| Milky Way glow (switch and brightness) | yes | none | Shares (star pass), measured; its picture's memory computed (128 MB, or 512 MB at Ultra) |
| Glow picture (Standard, Ultra download) | yes | "99 MB", "about 512 MB of GPU memory" | Computed and Download (the size read from the release record: 103,902,016 bytes) |
| Star halos | yes | none | Shares (star pass), measured |
| Star catalogue to load | yes | none | Shares (star pass), measured; memory computed |
| Star catalogue downloads | yes | **wrong**: Ultra "~350 MB" for a 241 MB file | Download, sizes from the release record (37,412,217 and 252,662,356 bytes); memory computed (12 bytes a star: about 29 and 286 MB) |
| Ocean wave spectrum (was "FFT ocean") | yes, jargon | none | Shares (water pass), measured |
| Rain and snow on the graphics card | yes | the processor's way measured | Gpu, Cpu and Memory, measured |
| Far tree sheet | yes | none | Shares, measured |
| Furniture detail | yes | memory per level, measured in the home | Computed (unchanged); its description through `graphics_hint` |
| Furniture models | yes | download size | Download (unchanged) |
| Garment textures | Full and Low memory, each with its memory | memory per level | Levels as data (512, 1K, 2K; 4K not offered), memory per level, the download, the card notices: section "Garment textures" below |
| Machine label distances (three sliders) | yes, but the description sat above the sliders | none | Shares (labels pass), measured; description under them |
| Show roof, Show hull | yes | none | Shares (scene pass), measured |

Elsewhere: the Dev page's graphics controls are developer tools and exempt (the brief); Settings >
Appearance and Animations shape the interface, not the 3D world. Every control on Settings > Graphics
now takes its words from `data/i18n/en.json` (`settings.rs` 420 to 280 literal words; the new
`settings/graphics.rs` none), and five more controls have their description straight after them.

### What the lines read on the operator's RTX 4070

The engine's own numbers at the crowd vantage (`anim-crowd-near`, the Commons' mess hall indoors,
fifty people, rig default graphics, 2560x1387, 2026-11-10; `HUMANITY_FRAME_COSTS=1`), which is what
the cost lines show there: the scene pass 7.57 ms of the graphics card's time each frame, the cloud
passes 1.43 (Cloud quality, Cloud layer), the see-through pass 0.65, the bounced light pass 0.36
(Bounced light detail), the star and Milky Way pass 0.088, the world surface pass 0.059 (indoors:
outdoors it is most of the frame), the contact shading pass 0.044 (Ambient occlusion), the sun's
shadow pass 0.016 (indoors, the sun mostly off-screen); 13.46 ms of GPU stages in all. Video memory
for textures, all of them: 325.3 MB with the garments at 1K, 517.3 MB at 2K. Each figure is a reading of
that place and those settings: the line on the page is the reading where the person stands.

### What a measured cost can and cannot say

- It is measured at the **current** choice on this computer: to compare Low and High cloud quality,
  pick each and read the line (it settles within a second; the numbers are smoothed so they do not
  strobe).
- A `Shares` cost is the whole pass: the line says so. Splitting the world surface pass (ground,
  planets, sky and water share one megashader pass) is what would make those exact: the Performance
  page's registry notes the same limit.
- On a card without GPU timers the passes with a processor-side twin read that twin and the rest say
  they cannot be timed.

## Garment textures

Section 16 of `docs/design/character-animation.md` holds the levels' build. In short: the converter
measures each garment's source maps; a garment is drawn at most at its colour map's size; a level is
offered only where some garment's art reaches it (512, 1K and 2K today: 24 garments were made at
2048, 6 shoes at 1024, the 16 brows and lashes at 512, and no colour map is 4096, though seven
outfits have 4096 normal maps); 512 and 1K ship; 2K is an optional pack built by the converter,
downloaded only when picked, resumable, checked against the compiled-in SHA-256, removable; each
level shows its video memory on this card and its download before anything downloads, and a level
whose pools would pass an eighth of the card's own memory says so in words. Measured at the crowd on
the RTX 4070: 90.7 MB of pools at 1K, 282.7 MB at 2K (256 MB to download), each 2K texture into its
slot in about 0.9 ms, the GPU's work unchanged.

## Gameplay settings follow the same rule (2026-11-11)

The operator, 2026-11-11: "I feel like we need to provide more than two options for our gameplay
settings. Including just turning settings off completely. Like body heat, illness, etc." The rule
above (say what a choice changes before it is chosen; levels are data) applies to Settings >
Gameplay as it does to graphics. A gameplay level costs no frame time worth a line (each is a
branch in a system that runs anyway), so its words say what changes in play instead.

- **One row per setting** in `data/gameplay/levels.json` (compiled in as the fallback): its name,
  an optional longer note, its default (the softened level, CLAUDE.md's dual-modes rule) and its
  levels, gentlest first. Each level has a name and a sentence saying what it changes, both by key
  in `data/i18n/en.json`. Settings shows the sentence of the chosen level under the row and every
  level's sentence as its radio's hint (`src/gui/pages/settings_gameplay.rs`).
- **Off wherever a system can be off**, and then at least two more levels (the file refuses Off
  and only one other). A system that cannot be off (walking, the bodies' animation) has no Off,
  and its levels say why they differ.
- **Off means off, held by a test per system**: body heat (the neutral core always, no harm), illness and
  injury (none happens, any present heals at once), farm animals (they need nothing), carrying
  weight (no limit), death (the player cannot die), the mining drone (free trips), hostile
  wildlife (no predators). Each test was seen red first (docs/FEATURES.md lists them).
- **Every level is understood by its system**: `gameplay_levels::understood` asks each system's
  `Mode::from_level`, and a test fails on a level in the file that its system does not know.
- **Saved** as config.json's `gameplay` (setting id to level id). A level the file no longer
  lists, and a config from before the levels, read as the default.
- **A shared world**: these levels are the person's own; the relay enforces none of them (it keeps
  no body heat, illness, carrying or death rules of its own today), so Off on one player's machine
  changes only what that player's own systems do. When a server comes to enforce one (a server's
  survival rules), its Settings row must say so in words beside the choice, and which levels the
  server allows.

### Checklist for a new gameplay setting

1. A row in `data/gameplay/levels.json`, its words added with `node scripts/i18n-add.js`.
2. The system's `Mode::from_level` (or the number it reads), and a line in
   `gameplay_levels::understood`.
3. Off, if the system can be off, with a test that Off means off, seen red first.
4. The level reaches the system through `SettingsState::level` (engine/survival_env.rs publishes
   the survival ones each frame).
