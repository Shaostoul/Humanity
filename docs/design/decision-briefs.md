# Decision briefs: the queued taste calls

> Written 2026-07-07 (final Fable session) so each of these can be green-lit,
> amended, or rejected with one line from the operator instead of being
> re-derived from scratch later. Each brief is: context, options, a concrete
> recommendation, and the first increment an implementing session would ship.
> Nothing here is decided until the operator says so; when a call is made,
> move the decision into the relevant design doc and delete the brief.

---

## Brief 1: Vehicle BAY and zones (replaces "assembler machine" thinking)

**Context.** Today a Vehicle Assembler machine auto-builds vehicles onto a pad
in front of itself (v0.679 pad-lane model). The operator's direction (field
session 3): every machine must justify itself physically; a "bay" is a
dedicated standard-vehicle-sized AREA justified by gravity/safety; it should
select the held vehicle; it ties into hangar / mech-dock ZONES; and a 3D
printer is more physically justified than a magic assembler box.

**Options.**
- A. Keep the assembler machine, rename/reskin it as a bay.
- B. Introduce ZONE as a first-class data concept (a floor rectangle with a
  role: vehicle_bay, hangar, mech_dock, landing_pad) declared in
  `data/machines/home*.ron` beside machines; migrate the assembler's
  spawn-pad + selector logic onto the bay zone; fabrication becomes
  3D printer (makes parts) + bay (assembly site where parts become the
  vehicle).
- C. Full physical assembly simulation (parts placed by hand). Rejected as a
  v1: months of work for little teaching value over B.

**Recommendation: B.** Zones are the missing spatial primitive - the hangar,
mech dock, greenhouse, and future medbay/workshop areas all want the same
"this floor area has a role" concept, so it pays for itself beyond vehicles.
It is infinite-of-X clean (zones are data rows), and it keeps the working
v0.679 job engine: the bay inherits the assembler's AutoRefine/selector code,
only the anchor changes from a machine entity to a zone.

**First increment.** `zones:` list in home.ron (id, role, rect, label) +
loader + renderer outline on the floor + the construction editor draws/saves
them. Second increment: vehicle build UI anchors to the bay zone (the
assembler machine catalog entry is retired), vehicle outputs park in-bay,
Summon targets the bay.

---

## Brief 2: Unified map (one map to rule them)

**Context.** Operator direction (2026-07-04): ONE map. The Maps/Cosmos page
should show the player's location (marker beside Earth) and located asteroids;
today map surfaces are split (Maps page, Cosmos view, asteroid list) and none
shows "you are here."

**Options.**
- A. Add a player marker to the existing Cosmos page and call it done.
- B. One Map page with a SCALE LADDER: home floorplan -> orbit -> solar system
  -> near stars. Each rung renders from data that already exists
  (home_structure.ron / solar_system + star catalogs), zoom crosses rungs,
  and markers (player, asteroids, other players, points of interest) are one
  shared overlay list at every rung.
- C. Full 3D seamless zoom from galaxy to floor tile. Beautiful, enormous,
  premature.

**Recommendation: B.** The scale ladder matches how the engine already thinks
(floating origin, LOD icospheres) without demanding seamlessness. The marker
overlay as ONE data-driven list (id, kind, position, scale-rung visibility) is
the infinite-of-X move: quests, drones, friends, and mining claims all become
marker rows later with zero new map code.

**First increment.** Marker overlay struct + player marker + located-asteroid
markers on the existing system view, plus a rung switcher (System / Orbit /
Home) that swaps the underlying render. Merge the Maps page into it; retire
duplicates.

---

## Brief 3: Studio: chat layers now, streaming pipeline later (feed OBS, do not become OBS)

**Context.** The Studio page is UI-only; the real gap to streaming is
capture -> encode -> RTMP, which is a codec/performance rabbit hole. Operator
wants merged chat layers (HOS + YouTube/Twitch/Rumble) and eventually a real
pipeline for relay + multistream.

**Options.**
- A. Build native capture/encode/RTMP in the app (ffmpeg or gstreamer
  integration). Months, heavy deps, duplicated OBS.
- B. Make HumanityOS the best OBS COMPANION: (1) native Studio page gets the
  HOS channel view (reuse the chat widgets); (2) the relay serves a
  browser-source overlay URL (chat + alerts as a transparent web page) that
  OBS captures - this is how every commercial chat overlay works, and our web
  mirror already renders chat; (3) external-platform chat layers come in as
  read-only merges later (their APIs/IRC), rendered in the same overlay.
- C. Defer Studio entirely.

**Recommendation: B.** It ships value in days (streamers can use HOS chat on
stream, which is also free marketing for the platform), keeps the exe lean,
and loses nothing: if a native pipeline ever matters, the overlay work is
still the front-end for it. The "one cohesive app" rule is satisfied because
the native Studio page remains the control surface; OBS is just the encoder
appliance, like nginx is for the website.

**First increment.** Relay route `/overlay/chat?channel=...` serving a
transparent, auto-scrolling chat page (web mirror CSS, no nav), plus the
native Studio page embedding the HOS channel view and showing the overlay URL
with a copy button.

---

## Brief 4: In-game browser R&D (the non-Chromium call)

**Context.** Long-term: real websites on in-game monitors without embedding
Chromium/CEF. Seeds exist (web.html bookmarks, native Browser page). This is
genuine R&D; candidates are Servo/Verso, Blitz (HTML/CSS renderer, no JS),
an OS webview, or a custom limited renderer.

**Options.**
- A. Servo/Verso embed: closest to "real web," but embedding maturity is low,
  the binary is huge, and it drags a JS engine (the bloat line we drew).
- B. Blitz-class HTML/CSS renderer (Rust, wgpu-friendly, NO JavaScript):
  renders modern HTML/CSS well; cooperating sites and ALL of our own pages
  work; general JS-heavy sites do not.
- C. OS webview (WebView2/WebKitGTK): free rendering, but per-OS divergence,
  no in-world texture compositing on our terms, and the WebKit caveat.
- D. Custom limited renderer for our own content only.

**Recommendation: B, framed honestly as "the readable web."** The mission
case (kiosks, docs, our own pages, shopping/affiliate pages we author) needs
faithful HTML/CSS, not arbitrary JS apps. No-JS is a feature: no tracking, no
popups, fast, safe to composite onto in-game monitors. Ship it as the ONE
browsing surface (consolidating web.html + the native Browser page), with a
"open in system browser" escape hatch for everything else. Re-evaluate
Servo/Verso yearly; if it matures, it slots behind the same monitor surface.
Prerequisite increment either way: the in-world MONITOR surface (render any
egui/text content onto a world quad) - that is engine work with value even if
the web renderer choice changes.

---

## Brief 5: Crew alignment (relay ship vs client homestead)

**Context.** BUG class from field testing: the relay simulates its multi-deck
ship for crew chores while the client renders the flat homestead, so crew
"work" at places that do not exist locally (crew grounded client-side,
v0.681 note). This blocks NPCs feeling real.

**Options.**
- A. Relay learns the client's home layout (client uploads home.ron; relay
  simulates chores against it).
- B. Client-authoritative crew for the HOME (single-player-ish scope), relay
  keeps only shared-world actors.
- C. Leave crew visual-only until multiplayer zones land.

**Recommendation: B for now, A's schema later.** The home is the player's
local world; simulating your own crew locally against the layout you actually
render kills the mismatch class outright and works offline. When shared
stations/colonies arrive, THAT world is relay-authoritative and A's
upload-layout schema applies there. Keeping one chore-site resolver that both
sides use (fed by whichever layout is authoritative in context) prevents a
second drift.

**First increment.** Move chore-site selection into a function over
`MachineHome` + `home_structure` (the layouts the client renders), tick crew
locally in the systems runner, delete the relay chore path for the home.

---

## Brief 6: How long is a day? (one clock for the garden, the body, the air and the sky)

**ANSWERED 2026-09-27 by the operator**, verbatim: "The default day length
should be 24 hours. Though we want it to be configurable. Like, maybe prefer
a 20 hour day or 36 hour day. However that shouldn't change how long an hour
is unless they change the setting that makes stuff happen faster/slower." So
the day is 24 hours by default and its number of hours is a setting; an hour
is always an hour; only the separate time-speed setting makes everything run
faster or slower. That is none of the lettered options below exactly: it is
real-length hours with a configurable day, and one speed control over all of
it. The brief as written follows, for the record.

**How it was built (2026-09-27, `src/systems/time.rs`).** One game clock.
An hour is 3,600 game seconds; a day is `hours_per_day` of them (Settings,
default 24, 12 to 48); a year is `days_per_year` days (Settings, default
365, 28 to 1,000; the operator did not choose the year, 365 is the agent's
choice to match real hours). The time speed (Settings, default 1 =
realistic, 1 to 1,000) is the only speed-up: the crop growth multiplier of
2026-09-20 and the 1,200 s day are deleted. Settings > Gameplay > Time holds
all three (`gui/pages/settings_time.rs`); the F11 panel's speed slider is the
same setting, and its "Hold the clock still" is a hold over it, as are sleep
and the probe rig's freeze (`time::request_speed_hold`).

What reads the one clock: crops (their `growth_days` are 24-hour days, so a
crop takes the same hours whatever the day length), their water and health
rates (now per game second, keeping the game hours they had: dry in 8 h, die
2 h later), the tanks (plumbing on game minutes), batteries (already), the
body's daily needs (hunger, thirst, tiredness, waste, urine, spoilage), the
weather (rolls every 6 to 18 game hours, events of real length, a front in
half an hour), passive income, the gameplay sun and solar power, the drawn
Sun and the planet's spin (one turn per game day of any length, through
`GameTime::solar_hour`: the sun is up the middle half of the day, so noon of
a 36-hour day is 18:00), the station's orbit (`orbit::sim_seconds`), the HUD
clock and Day N, seasons and environment Layer 1's year. What stays on real
seconds, deliberately: breath, body heat, a burn's g-load and timed status
effects, because the player moves and acts in real seconds (at 72x a held
breath would last half a real second); a summoned vehicle's drive to the
player (2026-10-04: it had run on the game clock, so at 72x a rover crossed
the field in a blur; the mining drone's trip, an errand off the map, stays
on the game clock); and the planets' orbital positions
(`renderer/celestial.rs` still reads the wall clock, months-scale; the next
step if the sky should also run at the time speed). Sleep runs the clock at
7,200x, so a night of 8 hours passes in 4 real seconds at any setting, and
the clock-jump rule (BUG-100) keeps its slack at twice one frame's move.
Offline progression counts the time away at the time speed.

**The simplified mode, proposed:** time speed 72, a day in 20 real minutes
(the pace the game had), offered as the "Simplified" preset. At 1x a lettuce
(45 days) takes 45 real days, a 12-hour night 12 real hours, and one 4 kWh
battery bank carries a 500 W night load 8 real hours; at 72x the lettuce
takes 15 hours of play, the night 10 minutes, the bank 6.7 minutes. The
default ships at 1 because the operator's words fix an hour at an hour
"unless they change the setting"; making 72 the default for general play
(the dual-mode house rule's softened default) is his call.

**The shared world's default, ANSWERED 2026-10-04 by the operator** (it had
shipped at 72x earlier that day), verbatim: "For normal mode, especially for my
MMO server, let's have everything be real time, not the 72x. That way anyone
joining isn't dealing with accelerated death. We'll wait until we have
everything actually working before we accelerate everything for fast mode."
So a server's shared world runs at 1x on a new server, the same as a new
solo game (`relay::storage::default_world_time_scale`, the schema DEFAULT
of `server_settings.world_time_scale`); its admin can still pick another
speed in Server Settings > ADMIN > Shared world clock, and the Simplified
72x preset stays in the player's own Time setting.

Written 2026-09-27. Asked before as PRIORITIES "Blocked on the operator" #3;
this is the same question laid out so it can be answered with one letter.
Three pieces of garden work wait on it: the ship's sun reaching the crops
(BUG-090), room air and tank water balancing (BUG-092 item 7), and the
body's share of the garden's nitrogen.

**What runs on what today** (read from the code on 2026-09-27):

| Thing | Clock | One day takes |
|---|---|---|
| World clock, the gameplay sun, room air | game days (`time.rs`, `SECONDS_PER_DAY` 1,200) | 20 real minutes |
| Crop growth | game days x the growth speed you chose (10x shipped) | 2 real minutes of growth per crop day |
| Crop water and nutrients from the tanks | real days (`farming/mod.rs`, litres per 1,440 real minutes) | 24 real hours |
| The body (hunger, thirst, urine) | real seconds | 24 real hours |
| The Sun drawn in the sky | the wall clock (the real ephemeris) | 24 real hours |

So a lettuce grows from seed to harvest in about 1.5 real hours while its
water is billed as if it took 45 days, the sun the crops feel rises every 20
minutes while the Sun you see rises once a day, and one person's urine is a
fraction of a percent of what a garden growing that fast needs. None of those
balance, and each is two clocks meeting.

**Your earlier call stands** (2026-09-20): crops have a growth speed
separate from the world clock, 1x / 10x / 100x, shipping at 10x. The question
now is what else follows it.

**Options.**
- **A. Game days for everything.** The drawn sky follows the 20-minute day
  (as most games do), and the body, the tanks and the air all run on game
  days too. Crops at 10x drink and breathe at their own growth speed, so the
  tanks and air handling are sized for it. Everything balances. Cost: you
  eat and drink about three times per 20 real minutes, and the sky no longer
  shows the real Sun, Moon and Earth at this moment.
- **B. Real days for everything.** The world clock becomes 24 real hours, the
  sky stays real, the body is already there. Crops still grow at the speed
  you chose, and what they drink and breathe speeds up with them, so a 10x
  garden needs a 10x water system. Everything balances. Cost: a day and a
  night take a real day, so a quick session sees little of either.
- **C. Both, as the two modes the house rule asks for.** Full realism is B:
  real days, the real sky, every flow balanced. The simplified mode is A: 20
  minute days, the sky on the game clock, gentle upkeep. Each mode is one
  clock; the two are never mixed inside one mode.

**Recommendation: C, with full realism as B.** It is the only answer that
keeps a single clock inside each mode, which is what makes mass and light
balance, and it follows the rule already set for every deep system (full
realism plus a simplified mode). It also settles BUG-090 without a
compromise: in each mode the crops, the panels and the drawn Sun read the
same clock. The one sub-choice left is which mode new players start in.

**First increment.** Make the growth speed also scale the water and air a
crop draws, as it already scales its nutrients (`soil::uptake_fraction`), so
a crop at 10x drinks and breathes at 10x. That is correct under all three
options, because a crop needs the same water per kilogram it grows however
fast it grows, and it closes BUG-092 item 7 on its own. Then the mode switch
that picks the day length and puts the drawn sky on the same clock.
