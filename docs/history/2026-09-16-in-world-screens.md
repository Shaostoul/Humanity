# The in-world screens ladder, 2026-09-16

The operator's direction, verbatim: "include the menu pages as in-game
touchscreens for like the inventory page. Not the webpage but, the actual
native app inventory page. That way we could make a command deck that is
populated by all the different displays in one place." And then: "Let's do
all 6 rungs and can we test without using affiliate links? We just want to
get it working in a basic form. We want to make sure people can actually
interact with the web display screen in-game. Then we can figure out the
legality of embedding the various platforms."

Six rungs were fenced: the screen surface, the screen as data, live feeds and
an in-game camera, the console room, a video player of our own, and the
readable web. This page records what shipped, how it was built, and what the
reviewers caught. Design of record: `docs/design/in-world-screens.md`,
`docs/design/readable-web.md`, `docs/design/media-player.md`, and the console
room section of `docs/design/homestead.md`.

## What was already in the tree

Nothing here was research. The engine had every ingredient before the day
started: `src/gui/ui_snapshots.rs` already rendered app pages into an
offscreen wgpu texture under a second `egui::Context` and drove them with
synthetic pointer events (the "shows != works" harness); the temporal cloud
map already bound a render target at a material's albedo slot with no
bind-group-layout change; `src/net/live_viewer.rs` already decoded MJPEG into
a newest-frame slot; the Opus decoder was already compiled in for voice. The
work was wiring, and the token cost was verification.

## Phase 1: four rungs in parallel (v0.1313.0)

Four implementers in isolated worktrees, one per rung, each followed by an
adversarial critic that ran both cargo checks inside the builder's worktree
and read every test the builder named, then a fix pass only when the critic
found blockers or majors. Seventy-seven minutes, ten agents, every branch
mergeable.

**Rungs 1 and 2, the surface and the data.** A `ScreenSurface` is a
persistent second egui context rendered into a `Rgba8UnormSrgb` texture, the
same format the PBR albedo textures use, so the scene material samples it
with no conversion and no copy. A new material type 24 draws it as an
emitter at the def's brightness: a display is a light source, not a
reflector, so the sun and the shadow map never darken it. The three
context-bound `TextureHandle`s on `GuiState` are swapped for the surface's
own for the span of its page draw, through a guard whose `Drop` restores
them on a panic. The page dispatch that lived inline in `lib.rs` became
`gui::dispatch::draw_tool_page`, one table for the main UI and every wall.
A screen is a `screen: Some((...))` field on a machine def; `wall_screen` and
`desk_monitor` entered both home designs, and two wall screens were placed in
the common room. The look ray in mouse-look, or the cursor ray with a free
cursor, hits the nearest screen quad within 3.5 m and becomes `PointerMoved`;
a click, the wheel and typed text route to it; Escape releases focus.

The builder's own negative proof: with the u axis mirrored and v flipped, six
tests failed (the corner mapping, the handedness, the bezel inset, the mesh
winding, the click-through-UV toggle, the keyboard-focus test); restored,
twelve of twelve passed. The critic's real catches: a `consumed_click` flag
written and never read (the mechanism was `route_button`'s return value all
along, and the doc claimed the flag), a compatibility arm in `page_from_id`
for a page name no data file had ever carried, a mesh test that rebuilt the
corners instead of reading the production vertex data, and a key-name test
that copied the prefix strip inline instead of calling the function
`route_key` uses. All four fixed on main in the refactor commit.

**Rung 4, the console room.** Zones gained `room_type`, a room covered by a
zone takes the zone's id instead of `room_N`, and the rooms.ron purpose and
actions join through that name; `console_room` is a zone type, a rooms.ron
entry and a placed zone in the home. Vocabulary settled: the command deck is
the mothership bridge; the console room, nicknamed the battlestation, is the
home's fixed workstation; "battlestation" is also the device role in
`device-mesh.md`, a different layer, and both stay. The critic found that the
main checkout's working copy of `ship_structure.ron` was an uncommitted
editor re-save; the fixer's answer was to re-serialize the branch's copy
through the editor's own `save()` so the merge became a small diff instead
of a whole-file conflict, and in doing so found that the "churn" carried two
real edits from 2026-09-08 (a window width, a strip light), which the branch
now carries.

**Rung 5, the player core.** `src/media` opens a WebM/Matroska file and
decodes AV1 (`rav1d`, the pure-Rust port of dav1d, asm off, no C toolchain)
and Opus (the decoder already in the tree) on a background thread with an
audio-led clock; play, pause, seek-to-start; a generated colour-bar fixture
under `tests/fixtures/media/`. Purpose-built rather than embedding VLC,
because libvlc is LGPL with its own window and render path and we need
frames in our own texture with royalty-free codecs. The critic's catches:
a WebM with no Duration element would play no audio at all, and a playing
clip ignored later master-volume changes; both noted as follow-ups on the
media doc, neither a blocker for a core with no display yet.

**Rung 6, the readable web and the sites database.** A page fetched over
HTTPS on a background thread (10 s, 4 MB, http(s) only, no cookies), parsed
with `html5ever` into a readable document (headings, paragraphs, links,
images, lists, tables, code; nav, footer, aside, script and style dropped)
and drawn in egui with clickable links, history and an open-in-system-browser
escape hatch, behind `readable_web`, which defaults to off. Decision brief 4
already made the non-Chromium call; this is its "readable web".
`data/web/sites.json` replaced `bookmarks.json` and `web.html`'s
`DEFAULT_SITES`, so native and web read one list, and every site carries
`embed.status` (`needs_review` until a human reads the terms and records
basis, terms URL, date and name) with every affiliate field null. The critic
caught a test asserting `http:///nohost` is a bad URL (the WHATWG parser the
`url` crate implements collapses the extra slashes, so it is not), an
unbounded image download reachable from any third-party page through the
chat image cache (now capped), a doc that promised lazy image loading the
code did not do (now it does), and two verification claims written before
they were executed (re-executed, and the click test's negative proof run
for real).

**The merge.** Serial, foundation first, onto a shared checkout where two
other sessions were active. Two of the three uncommitted files the merges
had to touch were pure editor churn (proven by comparing HEAD and the working
copy as normalised line multisets: 3192 lines each way for home.ron, zero
difference); the third was the ship structure above. One conflict set on the
readable-web branch: a module declaration, a doc note both branches added at
the top of the old CEF design, and the native feature list in Cargo.toml, all
resolved as unions. Then one refactor on main so phase 2 could not collide:
`ScreenDef.page` became `ScreenDef.source`, a scheme string (`page:`,
`watch:`, `camera:`, `video:`, `web:`) parsed once into `ScreenSource`, with
a `ScreenProvider` trait and the one registry `provider_for`, so each later
rung adds one arm and one file.

**The runtime proof.** Static checks cannot see a dark or mirrored wall, and
this repo has shipped renderer changes broken for ten releases on green
static checks before. So the release waited for a boot: the verifier built
the release exe, ran the existing world-entry gate (three vantages, zero
panics), then parked in the common room with the showcase camera verb and
drove the wall through `debug/screen_request.json`. Both walls showed their
pages upright and lit; the look ray reported hover at 1.45 m on its own; a
click on "Collapse all" changed 73.7 percent of the wall's pixels between
two 3D captures, and a click on the Status arrow re-opened one section while
the others stayed closed, which is per-screen egui memory working. Found and
left for after phase 2: both placed screens hang over door openings (data;
they move into the console room), the snapshot counter restarts per session,
the IPC click path does not set keyboard focus, and no camera request verb
parks inside the home (the showcase verb does).

Shipped as v0.1313.0 and the v0.1313.1 exe stamp.

## Phase 2: the walls come alive (v0.1314.0, landed 2026-09-17 and 18)

The weekly usage limit fell in the middle of phase 2: two of its three
builders finished, none was reviewed, and the third died with nine files
edited and uncommitted. Two days later the work resumed from disk, which is
what the commit-early discipline is for: the two finished branches got their
critics, the third got a continuation agent that read the uncommitted diff
file by file, kept the design (a typed IPC stage, a three-frame click), closed
one real hole on the way (the rig would have clicked a third-party link when
no same-host link existed; now it refuses), and committed.

**What the critics caught this time.** The live provider's status page drew
over the picture on every frame the stream had no new frame, so a 20 fps
stream on a 60 fps game would have strobed "Connecting" two frames in three;
the decision became a pure function with a test that fails on the old logic.
The video branch shipped its looping audio and its per-frame mix with no
executed test at all, and started every clip at full master volume for 60 ms
before the placed mix landed (a 33 times burst at a 10 percent slider);
both got tests, the burst got the mix baked into the stream at attach. The
rig's inventory leg could not tell a dead click from a live one: the IPC's
own pointer move made egui's floating scrollbar appear, and one changed
pixel was the whole proof. Now the pointer is hovered before the first
snapshot, the click must report the header row's pointing-hand cursor, and
a child row must vanish; a dead-click fixture with exactly that scrollbar
column fails the gate. Grounding that check headlessly found another real
defect: the inventory header's labels were selectable, so a press started
a text-selection drag and a good click would have failed the live rig.

**The merges.** Three branches, each conflicting with the others in the
same five files (the placed screens, the provider registry, the surface
trait, the IPC, the design doc), all three having named their screen
`wall_screen_3`. Resolved as unions by hand, renumbered, and the six screens
laid out on the console room's three doorless walls. A camera-module test
that asserted video was "a later rung" was the one test the union broke.

**The first live run failed, usefully.** The web leg passed at once: the
wall showed our site, the rig clicked its first link, the page navigated to
Download. The inventory leg found no "Home" on the wall: at 1280 by 720 the
Status cards push the places section below the fold. Bringing it into view
meant collapsing Status by clicking its title, and that did nothing, for
the same reason as the header labels: the universal section widget drew
its title as a default egui label, selectable, so the press became a
selection drag and only the triangle toggled (the phase-1 verifier had seen
exactly that and noted it). One line in the widget, on every page that uses
it; the rig gained a reveal step; the headless twin clicks the Status title
on a wall-sized inventory and fails with the fix reverted. The second live
run passed 12 of 12: the click on the inventory wall collapsed Home (its
Garage row gone, 57,625 pixels changed), the web wall navigated, the tasks
wall drew, zero panics.

**Left on the table, on purpose.** A placement gate on the sites database's
`embed.status` (recorded and shown, not enforced); synchronised playback
between players (the relay clock); a VR-controller ray; the interior frame
cost with six live walls (9 fps on rig defaults, the next perf item);
rav1d's debug-only borrow checker aborting about one debug test run in
twenty (device tests run in release).
