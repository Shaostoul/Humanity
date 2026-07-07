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
