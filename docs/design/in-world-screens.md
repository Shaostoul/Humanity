# In-world screens: native pages on displays in the 3D world

Rungs 1 and 2 of the screens ladder (2026-09-16). The operator's ask, verbatim:
"include the menu pages as in-game touchscreens for like the inventory page.
Not the webpage but, the actual native app inventory page." This document is
the architecture of that: how a wall screen in the homestead shows the real
egui inventory page, why it shares the player's real inventory, and how the
player looks at it, clicks it, scrolls it and types into it.

Code map:

| What | Where |
|---|---|
| The one page dispatch table | `src/gui/dispatch.rs` |
| A page rendered into a texture | `src/gui/screen_surface.rs` (`ScreenCore`, `ScreenSurface`) |
| World quads, look-ray hits, input routing, per-frame drawing | `src/engine/screens.rs` |
| The screen material (type 24) | `src/renderer/materials.rs` (`MATERIAL_TYPE_SCREEN`, `add_material_with_albedo_view`), `assets/shaders/pbr/90-fragment-main.wgsl` |
| Data shape | `crates/humanity-ship/src/machines.rs` (`ScreenDef`, `MachineInstance.screen_source`), `data/machines/home.ron`, `home_solo.ron` |
| Sources and providers | `src/gui/screen_surface.rs` (`ScreenSource`, `ScreenProvider`, `WorldRender`, `LoadState`, `ScreenWorld`), `src/engine/screens.rs` (`provider_for`, the registry), `src/engine/screens/live.rs` (the live stream), `src/engine/screens/camera.rs` (the in-game camera) |
| One view of the world from any pose | `src/engine/ipc.rs` (`render_view_onto`, `ViewPasses`), shared by the hi-res screenshot and the camera screens |
| The web provider (rung 6) | `src/engine/screens/web.rs` (`WebProvider`) |
| Dev IPC | `src/engine/ipc.rs` (`poll_screen_request` and its two companions; `poll_camera_request`'s `station` + `screen` pose) |
| The screens rig | `scripts/verify-screens.js` (`just verify-screens`), `scripts/lib/png.js`, fixtures under `tests/fixtures/screens/` |
| Hooks in the main loop | `src/lib.rs` (search "In-world screens") |
| Aim assist (the nearest widget in a cone, the line, the highlight) | `src/engine/screens/aim.rs`, `data/gui/screens.json` (`aim`), read by `src/gui/screen_tuning.rs` (in the GUI layer so Settings and the config may read it) |
| Any size, the build-mode handles, the cog's panel | `src/engine/screens/edit.rs` (handles, drag, `write_drag`), `src/engine/screens.rs` (`follow_placements`), `src/gui/pages/screen_settings.rs` (the cog's panel), `crates/humanity-ship/src/machines.rs` (`MachineInstance.screen_size_m`, `screen_px_for_shape`, `SCREEN_MAX_PIXELS`, `body_size_for_display`), `data/gui/screens.json` (`sizes`) |

The decision that required this increment is Brief 4 in
[decision-briefs.md](decision-briefs.md): whatever renders the readable web
later, the in-world MONITOR surface is engine work with value on its own. It
now exists, without Chromium or any web renderer.

## Architecture

A screen is three things bolted together:

1. **A surface**: its own `egui::Context`, its own `egui_wgpu::Renderer`, and
   its own wgpu texture at a fixed pixel size. Each frame the surface runs a
   page function with the pending synthetic events and draws the result into
   the texture. The page function is the SAME function the full-screen UI
   calls, reached through `gui::dispatch::draw_tool_page`, against the SAME
   `GuiState`. That is what makes the wall inventory the player's inventory:
   there is one `GuiState`, and both the full-screen page and the wall page
   read and write it.

   **THE RULE THAT COMES WITH THAT (BUG-078, v0.1323.0): a page draw may not
   write `active_page`.** Sharing one `GuiState` is the feature, and it is
   also the trap - whatever a page writes while drawing on a wall, it writes
   on behalf of the whole application. The bedroom's standing mirror is a
   screen whose source is `profile`, and `profile::draw` used to set
   `active_page = GuiPage::Real` as its way of aliasing itself onto the Real
   page. So entering the world drew the mirror, the mirror navigated the app
   onto the Profile page, and Play was unusable for two releases. Nothing
   said a word: the page rendered correctly and the world loaded correctly.
   The gate is `no_page_drawn_on_a_wall_screen_changes_the_app_page` in
   `src/gui/screen_surface.rs`, which walks every screen the shipped home
   declares. If a page needs to navigate, do it from the main UI's own frame.

2. **A material**: PBR material type 24, whose albedo texture is the surface's
   texture bound BY VIEW (`Renderer::add_material_with_albedo_view`). The
   scene samples the page's pixels directly. There is no readback and no
   copy per frame; the surface's render target and the material's albedo are
   the same GPU texture. The shader shows it as an EMITTER: the sampled
   colour times the def's `brightness` replaces the lit result, so neither
   the sun nor the shadow map darkens a display, while aerial haze and
   underwater extinction still apply so a distant screen stays part of the
   scene. A screen is opaque: no alpha cutout, no `fs_shadow` band, it casts
   a solid shadow with the depth-only shadow pipeline.

3. **A quad**: a two-triangle mesh on one face of the machine body, inset by
   the bezel and 2 mm proud of the face, with the world-space rectangle
   (`QuadGeom`) the input ray is tested against. The mesh is built in the
   body's local space and drawn with the body's position and yaw, so the
   display follows the body through every editor move.

### The one dispatch table

Before this increment the list of "pages the app can draw" lived only inside
the `match` in `lib.rs`'s per-frame egui closure. `gui::dispatch::draw_tool_page`
now holds every plain tool page (a page drawn as `page::draw(ctx, theme,
gui_state)`), and `lib.rs` delegates to it, keeping inline only the two arms
that need `EngineState` (the title screen and the in-game `None` page with the
HUD). The match has no wildcard: adding a `GuiPage` variant without deciding
where it draws does not compile, and `dispatch_table_covers_every_page_except_the_engine_bound_ones`
runs every page through the table under a headless context. A page the main
UI can show, a screen can show, by construction.

`gui::dispatch::page_id` / `page_from_id` are the data-file names of pages
("inventory", "tasks", "chat", "watch", ...). They are a complete table,
separate from the seven-entry boot-page dropdown mapping in `gui/mod.rs`, and
an unknown id resolves to `None` rather than a fallback page: a typo in
home.ron logs a warning and shows a plain "No page named ..." notice on the
wall, never the mission dashboard and never a crash.

### The texture-swap rule

An `egui::TextureHandle` belongs to the context that created it. `GuiState`
holds three (`image_cache`, `watch_texture`, `link_device_qr`), created by
whichever context ran the page that made them. A surface keeps its own
`ScreenTextures` set and swaps it into `GuiState` for exactly the span of its
page draw. The swap is a guard struct (`TextureSwap`) whose `Drop` swaps
back, so a panic inside a page unwinds through the guard and the main UI
never ends up holding a screen's handles; `texture_swap_restores_when_the_page_panics`
proves that path. The chat image cache is polled against the swapped-in
cache and the screen's context, so chat images render on a wall.

Why the surface has its own context at all: egui memory (scroll positions,
open headers, focused text field) lives in the context. Two wall screens
showing the inventory can be scrolled to different places, and neither moves
the full-screen inventory.

## Input model

`engine::screens::update` runs once per frame before the scene passes:

- **Mouse-look (cursor grabbed):** the ray is the camera's eye and forward
  vector. You point at a screen by looking at it.
- **Cursor free** (Alt held, the F10 sidebar): the ray is the cursor's world
  ray from `Camera::pick_ray`, unless the main UI has an interactive widget
  under the cursor, in which case the world gets no ray.
- The nearest screen the ray hits within its reach is the hovered screen; its
  page receives `PointerMoved` at the hit's (u, v) in pixels. Leaving it sends
  `PointerGone`. Screens beyond reach still render but ignore input. The reach
  was a fixed **3.5 m** until 2026-11-06; it now grows with the screen's size,
  and with the aim assist on, the pointer goes to the nearest clickable widget
  within a cone of the crosshair (`Screens::aim_at`; see "Aim assist, any
  size, and each screen's own settings" below).
- **Click:** a primary press or release while a screen is hovered goes to
  that screen (`Screens::route_button`, which returns true when a screen
  took the event). On a press that return is what makes `lib.rs` withhold the
  press from the camera controller, so the game's own click action skips it;
  the release still reaches the controller so a held-button state can never
  stick, the same rule the in-world modals use. The surface hands the event
  to its provider too (`ScreenSurface::button`), so a clip can pause on click.
  A press with no screen under the ray releases keyboard focus.
- **Wheel:** a notch over a hovered screen scrolls its page and does not
  reach the camera (`route_scroll`).
- **Keyboard focus:** the last-clicked screen holds focus. While its page
  reports `wants_keyboard_input` (a text field is focused), key presses and
  typed text go to that page and no gameplay key handler runs (so typing "i"
  into a wall screen's search box does not open the inventory). **Escape**
  releases focus instead of opening the menu. Releases always fall through so
  the held-key trackers clear.
- Screens take no input while a menu page covers the world, in the
  construction editor, the showroom, an in-world modal, or the death screen.

The pure geometry is tested without a GPU: `corners_map_to_unit_uv_as_seen_from_the_front`
pins that (0, 0) is the top-left corner AS SEEN BY SOMEONE FACING THE SCREEN,
u runs to their right and v runs down (egui's y-down), a ray from behind
misses, and `axes_are_right_handed_for_a_front_viewer` pins the handedness
(`v_axis x u_axis == normal`, so text is never mirrored).

## Data shape

A screen is a property of a machine def, and the page shown is a property of
the placed instance:

```ron
// data/machines/home.ron, catalog
"wall_screen": (
    shape: "box",
    size: (1.2, 0.7, 0.05),
    color: (0.08, 0.08, 0.1),      // the bezel colour
    label: "Wall screen",
    category: "Displays",
    stats: [(kind: "display", value: "1280 x 720 touchscreen", status: "ok")],
    power: None, ports: [], storage: [],
    auto_recipe: None, container_type: None, model: None,
    screen: Some((
        source: "inventory",        // default source: a page id, or a scheme (see Sources)
        px: (1280, 720),            // offscreen render size, fixed per def
        face: "front",              // front | back | left | right | top
        bezel_m: 0.02,              // display inset from the face edges
        brightness: 1.2,            // emissive strength
    )),
),

// instances
(
    id: "wall_screen_2",
    machine: "wall_screen",
    room: "common",
    offset: (50.85, 1.2, 40.0),
    rotation: 90.0,
    zone: "home",
    screen_source: Some("page:tasks"),   // this one shows the tasks board
),
```

Face convention: "front" is the box's -Z face at rotation 0 (glTF forward,
[model-pipeline.md](../game/model-pipeline.md)), rotated by the instance yaw
about Y like the body. Rotation 180 faces +Z, rotation 90 faces -X. "left"
and "right" are the viewer's left and right when facing the front; "top" is
read by a viewer standing at the front looking down.

Shipped: `wall_screen`, `desk_monitor` and `standing_mirror` in both
`home.ron` and `home_solo.ron`, under the "Displays" and "Furniture" palette
categories. The 2026-09-19 home redesign spread the placed screens across the
rooms that use them rather than hanging a wall of them in one booth; the table
of what is where and why is in [player-home.md](player-home.md). Placing a screen
in the construction editor works like any other machine; the page comes from
the def until the instance's `screen_source` is set in the file.

### Sources

The `source` string names what a screen shows. One field, a scheme prefix,
so every kind of content a display can carry is data, not a field per kind
(infinite-of-x). `ScreenSource::parse` in `src/gui/screen_surface.rs` turns
it into a variant once, when the surface is created:

| string | shows | drawn by |
|---|---|---|
| `inventory` or `page:inventory` | a native page (any `gui::dispatch::page_id`) | the surface itself, through `draw_tool_page` |
| `watch:<stream id>` | an MJPEG live stream (the Watch page's decoder); the id is the publisher's registered name, lower case | `engine::screens::live::LiveProvider` (rung 3, shipped) |
| `camera:<machine instance id>` | the game world from a placed camera post's pose | `engine::screens::camera::CameraProvider` (rung 3, shipped) |
| `video:<path>` | a WebM clip through the purpose-built player | the video provider (rung 5) |
| `web:<url>` | the readable web view, when `readable_web` is on | the web provider (rung 6) |

Anything else, a page id that does not exist or an unknown scheme, parses
to `Unknown` and the screen draws a notice naming the string; a typo in
home.ron never takes the world down. A kind whose provider is not wired
yet draws a "not wired yet" notice the same way.

A **provider** (`ScreenProvider`) owns the per-screen state of a non-page
source (the decoder, the viewer, the browsing history) and takes over the
surface's per-frame draw. `engine::screens::provider_for` is the one
registry: a match from source to provider, one arm per kind, each kind in
its own file under `src/engine/screens/`, so the rungs that add them never
edit each other. A provider draws in one of two ways, both methods on the
surface it is handed: `run_and_render` for egui content (a status page, the
web view) and `write_pixels` for finished frames (a decoded video or stream
frame). `write_pixels` with a frame of another size resizes the surface to
match and `frame_surfaces` rebinds the scene material to the new texture the
same frame, so a 1920 x 1080 stream on a 1280 x 720 wall shows every pixel.
Providers also report `status()` fields into the dev IPC's done file (a
stream's connection state, a clip's position, a web view's url), which is
what lets the rig wait for and assert on them (`complete_screen_request` merges them under the base fields, which win a name clash, and leaves out null-valued ones). Two more hooks serve the rig:
`load_state()` (static / loading / ready / error, what `wait_ready` polls)
and `link_rects()` (the links drawn last frame, what `link` clicks).

## Web sources (rung 6, integration)

`web:<url>` puts the readable web (`docs/design/readable-web.md`) on a
wall. The provider (`src/engine/screens/web.rs`, `WebProvider`) owns one
`WebViewState` per screen, the same widget the Browser page hosts, so a wall
has its own history, its own in-flight fetch and its own status line; two
walls showing two sites never share a page, and neither touches the Browser
page's view. What the player sees is the view's toolbar (Back, Forward,
Reload, the address row with Go, Open in browser; the "Sites" button is
hidden because a wall has no card list to return to), the status line, and
the page in a scroll area with clickable links. Looking at a link and
clicking navigates the wall, exactly as in the Browser page.

The rules that make it safe to hang a web page on a wall:

- **One navigation per screen.** The provider navigates to its url on the
  first frame it draws while in-app web reading is on, and never again on
  its own (`navigated` is the guard). The view's fetch runs on a background
  thread and is polled once per frame; that is the whole per-frame cost.
  Reload, Back, Forward and link clicks are the player's, through the view.
- **Off means off.** `AppConfig.readable_web` (Settings > Privacy, "Read
  websites inside HumanityOS") is off by default. While it is off the
  screen draws a notice ("In-app web reading is off", the url it would
  show, and the exact switch) and NEVER calls the view: no navigate, no
  `show`, so no fetch can be dispatched. The promise the Browser page makes,
  that a person who never turns the switch on never has the app fetch a
  page, holds on the wall. Turning the switch on later starts the one
  navigation on the next frame; turning it off again stops the view being
  drawn at all. `off_switch_draws_a_notice_and_never_fetches` in `web.rs`
  proves the off case through the view's own fetch state.
- **The sites database is enforced on a wall (2026-09-25).** The affiliate
  disclosure line for the current page's site is drawn above the page on
  the wall too, and the placement gate runs on every frame before the view
  draws: a screen placed on a site whose terms forbid being shown inside
  other software never navigates and says why; a link or typed address to
  such a site is refused before it is fetched; an unreviewed or unlisted
  site is shown with a one-line note. The gate itself lives in the view,
  so the Browser page runs the identical check. Detail:
  `docs/design/readable-web.md`, "The placement gate".

Status for the dev IPC: `{url, title, status, links}`, where the title is
the page's first heading (else its `<title>`, else the url), the status is
one of `unframed`, `off`, `idle`, `fetching`, `ready` or `error: <reason>`,
and `links` lists the hrefs drawn last frame (first 32, in `link` index
order) so a rig can choose a link by where it goes; the screens gate only
follows links that stay on our own site.

Shipped placement: `wall_screen_6` in `home.ron`, in the console room above
the desk on its east wall, source `web:https://united-humanity.us`. This is the
screen `scripts/verify-screens.js` parks in front of, so its id and its source
are load-bearing: renaming either quietly shrinks that gate.

## Live and camera sources (rung 3)

Two providers, one file each under `src/engine/screens/`, both built by
`provider_for` and both reachable from the data file alone: a wall screen
whose `screen_source` is `watch:shaostoul` shows that live stream, one whose
source is `camera:camera_post_1` shows what that camera post sees. The
operator's ask: "live camera feeds of in-game events".

### The live stream: one viewer per screen

`watch:<stream id>` shows the MJPEG stream the relay publishes under a
registered name (`src/relay/live.rs`: the stream id IS the publisher's
registered name, lower case, which is what the Watch page's directory
lists). The provider reuses the Watch page's decoder, `net::live_viewer::
LiveViewer`, which keeps only the NEWEST decoded frame and hands it to one
caller through `take_latest`.

That last fact is the rule: **each screen owns its own viewer**. The Watch
page's `GuiState::watch_viewer` is one shared slot, and a shared viewer
would split its frames between consumers, so two walls on one stream would
each show every other frame, or nothing when the Watch page is open. The
rung-1 critic flagged exactly that. `LiveProvider` therefore starts its own
`LiveViewer` on its first framed tick, against the configured server
(`GuiState::server_url`, the same way `pages::watch::start_watching`
does), and drops it with the surface (a dropped viewer stops its thread).
Two walls showing one stream cost two sockets, and each gets every frame.

Each framed tick the provider takes the newest frame and writes it into the
surface with `write_pixels`, **letterboxed to the surface's own pixel
size** (`live::letterbox_into`, nearest-neighbour, about a millisecond at
1280 x 720). The surface never changes size, so the wall's quad never
stretches the picture: a 16:9 stream on a 16:10 desk monitor gets black
bars, not a squeeze. A frame that already matches the surface is written
directly.

While no frame has arrived the surface shows a status page through
`run_and_render`; when the viewer's thread ends the viewer is dropped and a
new one is opened after `live::RETRY_AFTER` (15 s), so a wall comes back on
its own when the streamer goes live again. The status page is redrawn only
when its text changes, and **never over a live picture**: the stream runs
slower than the game (a 20 fps stream on a 60 fps game brings a new frame
one tick in three), and `run_and_render` clears the texture before it
draws, so a status page drawn on a no-new-frame tick would strobe the wall.
`live::next_display` is the per-tick decision, pure and pinned by
`a_live_picture_stays_up_between_stream_frames_while_connected`: a new
frame is written; no new frame with the picture up and the viewer connected
keeps the picture untouched; anything else shows the status page. The
picture is forgotten when the viewer is dropped and when a new one is
started (the retry), so a reconnect shows "Connecting to" again before its
first frame. `status()` reports `{stream, connected, frames, heading,
detail, may_connect}`, merged into `debug/screen_done.json`: a rig can read
what the wall SAYS without reading its pixels.

#### What the wall says when it cannot show a stream

`live::live_status(server, stream_id, last_error)` is that decision, pure,
and it answers two things at once: the words, and whether to connect at
all. It exists because of a wall the operator photographed in the console
room on 2026-09-18, reading:

> Could not connect: URL error: No host name in the URL Trying again shortly.

Nobody was signed in, so `GuiState::server_url` was empty, so the composed
address was `/ws/live/sub/shaostoul`, which has no host name. The screen
retried that address forever. A URL with no host can never work, so the
answer is not a better error message, it is not building the URL:

| what is true | the wall says | connects |
|---|---|---|
| no server set | "No server set" plus where to set one (the Server field in Chat's connection panel) | **no** |
| a server, nothing wrong yet | "Connecting to `<stream>`" | yes |
| the relay says nobody is publishing that name | "Stream offline" / No one is streaming as "`<stream>`" right now. Trying again shortly. | yes |
| the publisher stopped | "Stream offline" / The stream ended. Trying again shortly. | yes |
| the stream is at its viewer ceiling | "Stream full" plus the reason | yes |
| the socket failed | "Cannot reach the server" plus the reason the viewer reported | yes |

The case is decided on `net::live_viewer::EndReason`, a small enum the
viewer thread stores beside its human sentence, never by matching on the
English: a reworded sentence would otherwise silently change which wall a
screen thinks it is standing at. This follows the project norm in CLAUDE.md
("Saying what we will not build, and why"): a wall a person hits names its
reason.

Two things the rig found while proving this (2026-09-18):

- **A stopped publisher left its viewers hanging.** Every viewer holds its
  own `Arc<LiveStream>`, so dropping the publisher's copy never closed the
  broadcast channel, and a viewer parked on `rx.recv()` waited forever: the
  wall sat on the last frame, still reporting itself connected. The
  publisher's teardown now broadcasts a zero-length sentinel frame
  (`relay::live::END_OF_STREAM`, unambiguous because a real frame is at
  least a 9-byte header) and `viewer_loop` closes on it. Pinned by
  `a_viewer_is_closed_when_the_publisher_stops`.
- **A memoized status page could not be searched.** The page is only
  redrawn when its words change, and the dev IPC's `find` reads the shapes
  of a run, so `find` reported that plainly readable words were not there.
  The provider now runs when `core.find_pending()`, the same rule the video
  screen already followed.

### The in-game camera: a camera post and a world screen

`camera:<instance id>` shows the game world from a placed **camera post**.
The post is an ordinary catalog machine with one new def field:

```ron
"camera_post": (
    shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.22),
    label: "Camera post", category: "Displays",
    camera: Some((0.0, -8.0, 70.0)),   // (yaw, pitch, fov) in degrees
    camera_px: (640, 360),             // the size the camera RENDERS at (default)
),
```

`MachineDef.camera` / `PlacedMachine.camera` is `(yaw_deg, pitch_deg,
fov_deg)`. `camera_px` (default `(640, 360)`, `CAMERA_PX_DEFAULT`; each
side clamped to at least 1) is the pixel size the camera's view is
rendered at, carried into `CameraPose::px`: the wall screen's surface is
RESIZED to it on the camera's first picture, whatever the wall def's own
`screen.px` says. **The trade (2026-09-18 measurement at the operator's
settings):** the cost of a camera view is per pixel. A camera re-rendering
the console room into the wall's 1280 x 720 surface cost 17 ms whenever
its wall was in view, a full extra scene pass in a 10 fps room. A 640 x 360
render is a quarter of those pixels for roughly a quarter of the cost; the
wall quad's physical size is unchanged and the sampler scales the picture
up, so a security feed reads as a security feed. A def that wants a sharp
camera pays for it explicitly by raising `camera_px`. Yaw 0 is the body's front (-Z at rotation 0) and the placed
instance's `rotation` ADDS to it, so turning the post in the editor turns
the camera; pitch is positive up (a mounted camera looks a little down);
fov is the vertical field of view. `PlacedMachine::camera_pose`
(`crates/humanity-ship/src/machines.rs`) resolves the lens: just below the machine's top, on its
FRONT face (so the post's own body is behind the near plane), yaw = def yaw
+ rotation, pitch clamped to 89 degrees either way, fov to 10..150.
`engine::screens::camera_posts_from` collects every post's pose on every
placement rebuild AND every count-unchanged move (`Screens::update_poses`),
so the provider always resolves the CURRENT placements: a post dragged in
the editor pans its screen on its next render, a deleted post turns its
screen into a "No camera post named ..." notice.

A camera screen is a **world screen**: its provider does not draw content,
the engine renders the world for it. `ScreenProvider::world_render` returns
the screen's scheduling state (last render, interval) and
`ScreenProvider::render_world` is handed a `WorldRender` handle
(`engine::screens::camera::EngineWorld`, over the engine state and this
frame's draw lists) that resolves a post's pose and renders a view from any
pose into any target. The provider builds a renderer `Camera` at the pose
with the surface's aspect (`camera::camera_from_pose`; the yaw sign is
proven by `camera_forward_matches_the_machine_front`, not read off the
formulas) and has the world rendered STRAIGHT INTO its surface texture. For
that the surface is created in the scene's swapchain format
(`ScreenProvider::surface_format`): the scene pipelines were built for that
format and can draw into no other. The readback (`read_rgba`) and
`write_pixels` swizzle for a BGRA format, so the dev IPC snapshot of a
camera screen is a correct PNG.

**The camera budget** (`camera::CAMERA_INTERVAL`, `camera::pick_due`):

- A camera screen renders at most every 100 ms (10 Hz). A skipped tick
  keeps the last image; the texture persists.
- At most ONE camera renders per frame across all camera screens. Among the
  surfaces chosen this frame (`frame_surfaces`: nearest four in range, the
  hovered one, the IPC target) the due world screens are served in
  starvation order: never rendered first, then the oldest render, ties to
  the nearest. Three cameras in a room cycle 0, 1, 2 and none renders on
  two consecutive frames (`due_cameras_render_one_per_frame_in_starvation_order`).
- The render size is the camera post's `camera_px` (default 640 x 360), not
  the wall's `screen.px`: the provider resizes its surface to the pose's
  size on the first picture (`ScreenSurface::resize`, through the
  `WorldRender::device` handle) and `frame_surfaces` rebinds the wall's
  material to the new texture the same frame. The wall quad's physical size
  is unchanged; the sampler scales the picture up. The cost of a view is
  per pixel, so this is the camera's one cost knob (see the def field
  above for the measured trade). The re-render publishes under its own
  frame-cost keys, `gpu.screen_sky` / `gpu.screen_scene` /
  `gpu.screen_transparent` / `gpu.screen_overlay` / `gpu.screen_lines`
  (`frame_costs::SceneView::Screen`, one key per pass it runs), so it shows
  on the Performance page as "In-world screens" instead of hiding inside
  the live frame's numbers.
- The passes are the sky (stars) and the scene lists (opaque, transparent,
  overlay, ring lines): `ViewPasses::SceneOnly`. The planet/cloud pass,
  celestial lines, god rays and SSAO are NOT run for a camera, for two
  reasons that are both about the cloud renderer: it keeps per-frame
  temporal history (screen-space reprojection, a re-anchor order, a
  translation baseline) fitted to the PLAYER's camera, and a foreign pose
  at 10 Hz would poison that history for the live frame (the orders are
  `take()`n by whichever celestial pass runs first in a frame, and a camera
  screen renders before the live frame's pass); and it is the frame's most
  expensive pass by an order of magnitude. A camera post inside the
  homestead sees the station and a star sky through any window; a planet
  in a camera's window is a later rung (a second temporal history keyed per
  camera).
- The star pass uses the live frame's **daylight gate**
  (`ipc::sky_daylight`, the v0.1059 rule: inside an atmosphere with the sun
  more than about 6 degrees up, the stars are skipped because the sky
  washes them out). One function decides it for the live frame, the camera
  screens and the hi-res screenshot, so a camera looking out a window by
  day shows no stars while the window shows none; before this the view path
  hard-coded "night" (inherited from the screenshot, where the sky pass hid
  the mistake) and a daytime camera rendered stars. A camera still draws no
  sky of its own (the planet/cloud pass is what draws it, and that pass is
  not run for a camera, above), so a daytime window on a camera is the
  black behind the station, not blue; that is the later rung.

**A camera never sees its own screen.** The camera's surface texture is
the render target of its view, and its own display quad's material samples
that same texture. wgpu refuses a texture that is both a render attachment
and a sampled texture within one pass (a validation error that would take
the frame down), and the draw list is not culled per camera, so
`camera::without_own_quads` leaves the target screen's quads out of the
opaque list the camera renders; every other object, other screens' quads
included, stays. A camera pointed at its own monitor shows the monitor's
body with no picture on it.

**Ordering.** The render happens inside `frame_surfaces`, before the
surfaces' own `frame` and before the live frame's scene pass, so the scene
pass samples THIS frame's view, never last frame's; the provider's `frame`
only ever draws its notice, and only when the text changes, never over a
rendered view. `frame_surfaces` takes `SceneDrawLists`, this frame's draw
lists as built so far; they are still in the HOME frame at that point
(lib.rs adds `station_off` in place further down the frame), which is the
frame the camera posts are in. The sun shadow map is the live frame's (fitted
to the player's view), so geometry outside the player's view is lit without
its shadow on a camera; and the light list is the previous frame's. Both are
right while aboard the station, which is the only place a camera screen is
within framing range.

**`render_view_onto` is shared with the screenshot.** The sky pass and the
scene passes were refactored OUT of `capture_hires_screenshot`
(`src/engine/ipc.rs`) into `render_view_onto(state, camera, target, size,
lists, passes)`. The hi-res screenshot calls it with `ViewPasses::Everything`
on a copy of the live camera with the capture's aspect (the live camera is
never touched; before this the path set and restored `state.camera.aspect`
around the passes), then reads the target back to its PNG exactly as
before; the done file contract is unchanged. The camera provider calls it
through `EngineWorld::render_view` with `ViewPasses::SceneOnly`, which
returns whether it rendered (false, and the surface untouched, when the
world is not loaded), and the provider counts a render only when it did
(`CameraProvider::record_render`, pinned by `a_skipped_render_is_not_counted`),
so `renders > 0` in the done file means a picture was drawn. Both views
bind a depth buffer of their own size for the duration: the renderer keeps
ONE spare depth texture (`Renderer::begin_view_depth` / `end_view_depth`,
`capture::ViewDepth`) and swaps it in for the passes while the window's own
depth buffer is parked untouched and put back before returning. A camera
screen at steady state costs no depth allocations per render (the spare is
kept at the camera's size); the hi-res screenshot drops the spare after its
one render so an 8K depth buffer does not linger. Before this the path
recreated the shared buffer at the view size and again at the window size
on every render, twenty full-screen allocations a second for one 10 Hz
camera. `set_depth_target_size` also no longer recreates a buffer that
already has the requested size.

**Shipped data** (`data/machines/home.ron`): `camera_post` in both home
catalogs under "Displays"; `camera_post_1` on the open garden floor west of
the variety-tower grid, looking east down the tower rows and a little down;
`wall_screen_3` in the console room shows `camera:camera_post_1`, and
`wall_screen_4` is the great room's television showing `watch:shaostoul`. `shipped_homes_place_a_camera_post_and_the_console_screens_name_it`
pins that every `camera:` screen names a post that is actually placed.

**Dev IPC.** `debug/screen_request.json` works on both kinds like any
screen; the done file carries the provider's `status()` fields: a live
screen's `stream`, `connected`, `frames`; a camera screen's `camera`,
`renders`, `live`, and `camera_error` (the notice text, present only
when the post could not be found). The merge (`ipc_parse::merge_provider_status`)
skips null-valued provider fields and never lets a provider key shadow the
IPC's own, so a successful request is never `{"ok": true, ..., "error": null}`
next to the IPC's own failure shape `{"ok": false, "error": "..."}`; the
provider's field is named `camera_error` for the same reason, so neither
rule alone carries the guarantee. A `snapshot` of a camera screen is the
rendered view as the wall shows it.

A provider that places SOUND in the world gets the world context once per
frame through `ScreenProvider::world_update(&mut ScreenWorld)`: its screen's
centre, the listener's position and right-hand direction, and the audio
manager when the machine has an audio device. `frame_surfaces` calls it for
EVERY surface with a provider, framed or not, before the framing loop: a clip
keeps sounding from its screen while the player faces away, so its volume
and pan must follow the listener every frame, independent of the framing
budget. A provider with nothing to place leaves the default no-op.

## Video sources (rung 5, integration)

`video:<path>` plays a clip through the purpose-built player
([media-player.md](media-player.md)) on the screen, on loop, with its sound
placed at the screen; a click on the picture toggles pause. The operator's
words: "movies on displays", then, looking at the console room, "should we
play a video that's stored on my PC?". Provider:
`src/engine/screens/video.rs` (`VideoProvider`). Shipped: `wall_screen_5`
over the workshop benches starts on
`data/media/demo_colour_bar.webm` (the synthetic 2 s colour-bar clip;
provenance in `data/media/README.md`) and the player can point it at any
video on the machine from inside the game.

**The control strip and the picker (2026-09-18).** The screen draws through
egui over its own frame texture, so one draw path serves every state: the
picture fitted into the display's rectangle with black bars, a strip over
its bottom edge (Open, Play/Pause, the file name, the time), the file picker
as a window on the screen, and the notices. The strip shows while the screen
is being looked at, while paused, while a conversion runs, while the picker
is open and while there is nothing to show; it hides over a playing film,
and a hidden strip with no new frame draws nothing at all, so a playing film
costs one texture upload per decoded frame. Open raises the same file picker
widget the chat attach button uses, filtered to the video extensions in
`data/media/ingest.json`. The chosen file is remembered per screen in
`AppConfig::screen_media`; the data file's source is the default.

**A video disc in the drive (2026-09-18).** The operator asked: "Can I play
a DVD that's in my PC on an in-game display?" A disc we can read as files,
we play. Put a disc in, press Open on the screen, and the disc shows up in
the picker's quick row as "Disc (F:)"; opening it shows a **Play disc**
button, because a film on a disc is a folder of numbered files nobody
should have to understand. The button hands the FOLDER to the same open
path everything else takes; `src/media/dvd.rs` then finds the `VIDEO_TS`
folder (the drive root or the folder itself, either is accepted), picks the
main title (the biggest title set, with its `_0` menu left out, parts in
order), and hands the whole title to ffmpeg as one film through its
`concat:` protocol, so a two-part title plays as one film and not as its
first half. From there ffmpeg decodes it as it plays, as it does any file
the player does not decode itself (since 2026-11-13; until then the title
was converted whole into a media cache first, which the proof below used).

A **copy-protected commercial disc does not play, and never will**. Those
discs are encrypted, and HumanityOS does not break disc protection: there
is no circumvention in the codebase and none is suggested anywhere. Before
converting, the disc's first title file is read and its packet headers are
checked; a disc that marks its packets as scrambled, or whose files cannot
be read at all, gets one plain sentence on the screen saying the disc is
copy protected, that this app does not break disc protection, and that a
disc you burned yourself or any unprotected disc plays here. A conversion
that fails later on a disc gets the same sentence appended, so the screen
never shows a bare decoder error. What DOES play: a disc you burned, a
camcorder disc, a data disc with ordinary video files on it (pick the files
one at a time), an unprotected video disc. A disc IMAGE (`.iso`) is
refused with what to do instead: an image is a filesystem in a file, not a
video stream. Menus, several camera angles, seamless branching and
subtitles are not followed; the main title plays, beginning to end.

Proven on the real screen (2026-09-18, the verify-screens rig kept open,
`verify-screens` itself 12/12 with 0 panics): `wall_screen_5` pointed at
the synthetic `VIDEO_TS` fixture through the dev IPC's `video_open`
reported `media.disc: true` and `convert_why: "is a video disc: title 1 in
2 part(s), MPEG-2 video"`, converted, and played a film of **2.024 s**,
which is BOTH parts (one part alone is a second, so the chain really was
joined); two snapshots a second apart differed by **104,218 of 921,600
pixels** with the clock moving 0.80 s to 1.99 s. Opening the same disc
again was a cache hit with `transcodes` still 1. A copy of the same disc
with its packet headers marked scrambled drew the refusal on the wall,
with `transcodes` still 1 (nothing was converted) and the Open button
still there. What a REAL disc in the operator's drive would still prove,
and this cannot: that a pressed disc's `VIDEO_TS` lists and reads the way
a folder on a hard disk does, that a title spread over four or five
1 GB parts converts in reasonable time, and that a genuinely CSS-encrypted
disc trips the same refusal (its packets are marked the same way, but only
a real one proves it).

**Anything ffmpeg can read.** The player itself decodes only AV1 + Opus
(the codec policy: no patented decoders are shipped), and on Windows asks
the computer's own decoders for an ordinary MP4. A chosen file neither
plays is decoded by the machine's ffmpeg as it plays (since 2026-11-13:
raw pictures and sound through two pipes, nothing converted ahead or kept;
`src/media/stream.rs`), with an honest message naming the fix when ffmpeg
is missing. Until 2026-11-13 it was converted once into a media cache
first, which is what this proof used: an H.264 + AAC MP4 fixture handed to `wall_screen_5` through
the dev IPC converted, played (76,832 of 921,600 pixels changed between two
snapshots a second apart), and opened again from the cache with no second
ffmpeg run.

- **Path rule.** The path after `video:` is resolved against the game DATA
  dir first (`data/media/x.webm`, the distributed and moddable tree), then
  the data dir's parent (the dev repo root, so a checkout can name
  `tests/fixtures/media/x.webm`), the same order GLB models use
  (`resolve_model_path`, [model-pipeline.md](../game/model-pipeline.md)).
  A file in neither place, or one the player refuses (any codec but AV1 +
  Opus), draws an error page naming the path and the problem; it is tried
  once, never per frame, and never takes the world down.
- **Opened on the first frame,** not when the world loads: a screen the
  player never looks at never spawns a decode thread. The decode thread is
  the player's own (`media-decode`); nothing decodes on the render thread.
  Each framed tick the provider calls the player's `poll()`, which is the
  ONLY gate on frames (the newest frame whose pts is at or before the clock,
  never one ahead of it), and writes what it gets with `write_pixels`.
- **Every pixel, never stretched.** `write_pixels` resizes the surface to the
  clip's own frame size, so the wall shows the clip at its native
  resolution and the GPU sampler scales it on the quad. When the clip's
  aspect differs from the display's (the def's `px`, which the def author
  matched to the physical display), the frame is centred at 1:1 in a canvas
  of the display's aspect over opaque black bars (`letterbox_layout`,
  `compose_letterbox`), so a 4:3 clip on a 16:9 wall gets side bars, never
  a stretch. Equal aspects write the frame's bytes straight through, no copy.
- **Looping, only while playing.** When the player's clock reaches the
  clip's declared end AND the clip is meant to be playing, the provider
  calls `seek_to_start` and `play` again; the loop count is in `status()`.
  A clip paused on its last frame stays on its last frame (the wrap is gated
  on the wanted state in `advance`; without the gate a pause that landed at
  the end rewound on the next tick, the paused page read "Paused at 0.0 s"
  and `loops` counted a wrap nobody saw). The sound loops on kira's side:
  the stream is attached with a loop region over its whole length
  (`VideoPlayer::attach_audio_with`, `AudioAttach { looping: true, .. }`),
  because a kira stream that runs off its end is removed from the mixer and
  can no longer be resumed or seeked. The audio wraps a few milliseconds
  before the picture (the stream ends at the Opus sample count, the clock
  at the container duration); the seek inside `seek_to_start` re-aligns
  them, a phase glitch of under 20 ms on the fixture's tone.
- **Click to pause.** `on_button` toggles on the PRESS (a click is one
  toggle, not two). The surface routes a button event only to the provider
  it holds, so a click on a page screen or any other kind never reaches a
  clip. While paused the surface shows a one-line "Paused at X s of Y s.
  Click to play." page through `run_and_render` (the frame is bytes in a
  texture, not egui, so there is no overlay to draw on it) and the last
  frame is kept in memory; play re-writes it at once, before the next frame
  is due. A click before the clip has opened is honoured when it opens.
- **The texture is the clip's size while playing and the def's while
  showing a notice.** `write_pixels` sizes the surface to the clip (320 x
  180 for the demo); a notice laid out at that size and upscaled four times
  onto a 1.2 m wall would be a blur, so before the paused or error page is
  drawn the surface is resized back to the def's `px` (remembered from the
  first frame, before anything resized it), and the next written frame
  resizes it to the clip again. `resize` is a no-op when the size already
  matches, so a paused clip does not reallocate every frame, and
  `frame_surfaces` rebinds the scene material after either change. The
  decision (which page, at what size, or a frame, or keep) is
  `VideoProvider::plan_frame`, GPU-free and unit-tested; `frame` is the
  GPU glue over it.
- **Input while playing is dropped, not queued.** The look ray reports a
  `PointerMoved` every frame it moves across a screen, and the core's event
  queue is only ever drained by an egui run. A playing clip never runs egui
  (its picture is bytes), so the backlog used to grow without bound for as
  long as the player watched and was replayed in one run on the click that
  paused. Now `plan_frame` drops the core's queued input every playing
  tick (`ScreenCore::drop_pending_events`, which also forgets the pointer
  so the next report re-announces it), and `write_pixels` drops it for any
  provider that writes bytes. The notice paths keep their events: the run
  that draws the notice consumes them.
- **Sound starts muted (2026-09-27).** A screen plays its picture on its own,
  and its sound waits for the player: the strip shows Unmute on a clip with a
  sound track, and a click turns it on (the button then reads Mute). No
  stream is attached until the first Unmute; a Mute after that keeps the
  stream and sends silence, so Unmute resumes in step with the picture. The
  state is not saved, so every boot is quiet. This is the rule browsers apply
  to autoplaying video, and it came from the operator: the workshop screen's
  demo clip, a looping 440 Hz test tone, sounded on every boot. Separately,
  an instance launched in the background by a script or agent opens no audio
  device at all (`lib.rs`, `engine::launch_focus`).
- **Sound placed at the screen.** The clip's Opus track plays through kira
  as a streaming sound (`AudioManager::play_stream`), which is a plain stereo
  stream, not an emitter in a 3D scene (the engine has no kira spatial scene
  or listener yet; `audio/spatial.rs` is a stub). So the placement is done
  the way `play_spatial` does it for one-shots, updated live: linear volume
  falloff to silence at 50 m (`AUDIO_MAX_DISTANCE_M`, the same law), and a
  stereo pan from the screen's BEARING relative to the listener's right
  vector (centre straight ahead or behind, swung 0.7 of the way to one ear
  for a screen beside you), sent through `VideoPlayer::set_audio_mix` with a
  60 ms tween and only when the value moved past `MIX_EPSILON`. The one-shot
  path pans by a world-axis offset, which is fine for a third of a second of
  footstep and wrong for a film the player turns away from, hence the
  bearing. Volume = master x sfx x falloff (`compose_mix`), read live, so
  the Settings sliders govern a film on the wall like any other world
  sound. When the engine gains a real spatial scene, the stream should
  route to an emitter at the screen and this math goes away.
- **The attach order, and the mix the stream starts at.** `frame_surfaces`
  calls `world_update` BEFORE `frame` on every tick, so on the tick whose
  first frame opens the player the world update finds no player; the sound
  attaches on the NEXT tick, exactly once, and later ticks only re-send a
  mix that moved. Nothing is decided until both the player and an audio
  device exist, so on a machine with no device the attach stays pending
  (never marked done) rather than being skipped. The stream is attached AT
  the mix that tick's listener position implies (`AudioAttach { volume,
  panning }` bakes it into the sound data, so the first sample plays at
  it): a stream attached at master volume and then tweened down to master
  x sfx x falloff spent its first 60 ms up to 33 times too loud with the
  sfx slider at 10 percent and a screen entering range at 35 m, a click
  every time. The whole seam is pure and unit-tested without a device:
  `compose_mix`, `mix_moved` (the epsilon gate) and `SoundLink::step`
  (attach once at the first mix, then `Send` or `Hold`); `world_update`
  only decides when to step and makes the kira calls the step names.
- **One world update per surface.** The loop in `frame_surfaces` walks the
  quads and relies on exactly one quad per surface, which `sync_screens`
  guarantees by construction (one placement makes one surface and one
  quad). A surface with several quads (a two-sided display) would need
  that loop to dedupe by surface index or its provider would be updated
  twice a frame; not done because the case does not exist and a per-frame
  seen-set would be a cost paid for nothing.
- **Removal.** Dropping the provider (the machine removed, the source
  changed) drops the player, which stops and joins the decode thread and
  stops the kira sound (`VideoPlayer::drop`), so no decoder runs for a wall
  that is gone and no soundtrack plays from nowhere.
- **Not framed, still playing.** A video screen behind the camera or beyond
  40 m is not framed, so its picture freezes while its sound carries on (the
  kira thread runs regardless). When it is framed again the player's queue
  holds four stale frames; `poll` drops the stale ones and the decoder
  catches up in a burst (instant for the fixture, a second or so for 1080p).
  While a menu page covers the world nothing is framed and no placement
  runs; the sound holds its last volume and pan.
- **`status()`**: `path`, `resolved`, `playing`, `paused`, `position_s`,
  `duration_s`, `loops`, `audio`, `error`, merged into
  `debug/screen_done.json`.

Not done in this rung, on purpose: **synchronised playback between players**
(designed 2026-09-25 as a one-shot "jump to where they are", see "Screens
are per viewer" below), **subtitles**, a **file picker** (the
source is the data file's string), inline **volume** per screen, and true 3D
spatial audio (above).

Tests (`src/engine/screens/video.rs`, GPU-free): `provider_for` resolves the
scheme; the path rule on a scratch tree (data dir wins, root serves the
rest, neither is `None`); a press toggles pause and a release does not, on
the pure state and on the real player's clock; the error page for a missing
file names the path (read back from the egui shapes, not from the setup) and
for the VP8 fixture names the codec; frames over the shipped clip arrive at
320 x 180, in pts order, and the clip loops within 2.6 s; the letterbox
layout keeps the frame 1:1 at the display's aspect (identity, side bars,
top and bottom bars, degenerate sizes); the composed canvas has the frame's
bytes at the offset over opaque black; the placement law (centre ahead,
mirror-symmetric swing, linear fade, silent at 50 m); the mix composition
(each slider really in it) and the epsilon gate; the sound link attaches
once at the first mix and never twice; the stream starts at the placed mix
(0.03 for the reviewer's sfx-at-10-percent, 35 m case, not 1.0); the attach
waits for both the player and a device through the real `world_update`
with `audio: None`; a playing clip drops the queued input every tick and a
paused one hands it to the notice run; a clip paused on its last frame does
not rewind (loops stays 0, the position stays at the end) and wraps on the
first playing tick; notices are planned at the def's px with the core at
the clip's size; the shipped demo clip is byte-identical to the media
fixture. Two ignored device tests drive the real audio path by hand: the
provider attaches once at the composed mix and follows the listener
(observed 2026-09-17: attached at 0.0006 with master 0.02 and sfx 0.1 at
35 m, then 0.0012 and pan 0.85 after a step to the side), and the media
suite's looping test (below, in [media-player.md](media-player.md)).
Proven able to fail: the compose and loop tests each caught a real bug
while this was built (ghost pixels under the bars when two layouts share a
canvas size; the pre-wrap frame stamped with the next loop number), and
the pause and error-page tests were broken on purpose and observed failing
at their named assertions; details in the FEATURES entry.

## Dev IPC (permanent tooling)

Drop `debug/screen_request.json` while the game runs:

```json
{"screen": "wall_screen_1", "action": "click", "uv": [0.5, 0.2]}
{"screen": "wall_screen_1", "action": "scroll", "uv": [0.5, 0.5], "dy": -3}
{"screen": "wall_screen_1", "action": "text", "uv": [0.5, 0.5], "text": "hello"}
{"screen": "wall_screen_1", "action": "hover", "uv": [0.1, 0.1]}
{"screen": "wall_screen_1", "action": "snapshot"}
{"screen": "wall_screen_1", "find": {"text": "Home"}}
{"screen": "wall_screen_6", "action": "wait_ready"}
{"screen": "wall_screen_6", "link": {"index": 0}}
```

The event goes through the same `ScreenCore` methods the look ray uses
(never a side path), and `debug/screen_done.json` comes back as
`{"ok", "screen", "source", "kind", "action", "wants_keyboard", "hover_widget",
"cursor_icon", "focused", "uv", "png"}` plus the provider's `status()` fields
merged at the top level (a web screen adds `url`, `title`, `status`); `png` is
present for `snapshot`, which reads the surface texture back to
`debug/screen_<id>_N.png`; `uv` is present for the pointer verbs and says
where the event landed. A click is three frames, hover then press then
release (the sequence the headless click test proved against re-interacted
rows, which register nothing on a same-frame move + press), and the done
file is written after the surface has drawn the frame the last event
landed in. `hover_widget` is whether egui reported a layer
under the pointer after that frame (egui does not expose per-widget hover
publicly); `cursor_icon` distinguishes a text field (Text) or a link
(PointingHand) from plain content. The request file is consumed even on
error, and an error writes `{"ok": false, "error"}`. That holds when the
request's screen disappears mid-flight too: a placement rebuild
(`sync_screens`, an editor change) or a surface index that no longer exists
abandons the request through `Screens::abandon_ipc`, which clears the
in-flight record and its payload and writes `{"ok": false, "error"}` naming
the cause, so a rig reads a failure at once instead of waiting out its own
timeout.

The three verbs a rig needs so it never guesses a pixel:

- **`find`** answers where a drawn text is: `{"found", "text", "matches",
  "uv", "rect_px"}`. egui exposes no label lookup (widget rects carry ids,
  not text), so the core scans the frame's own shapes for text galleys:
  an exact match wins, then a text that starts with the query, then one
  that contains it, first in drawing order within a tier; text clipped
  out of a scroll area is skipped. `found: false` is an answer, not an
  error. On the inventory, `"Home"` lands on the container header "Home
  (Silverdale, WA ...)", not the person row "You  (Home)".
- **`link`** clicks the Nth link the provider drew last frame: the rect is
  clipped to the surface, its centre becomes the uv, and the hover, press
  and release go through the surface on three frames like any click. No
  such link is `{"ok": false}` with the count the content drew. Which N:
  the provider's status lists the hrefs in the same order (`links`), so a
  rig picks by where the link goes instead of trusting index 0.
- **`wait_ready`** completes only once the provider reports ready or
  failed (`load_state()`), bounded by `WAIT_READY_LIMIT` (15 s; a timeout
  is `{"ok": false}` with the status fields, never a pass). While it waits
  the request stays in flight, which keeps the surface framed so a web
  view keeps polling its fetch. Static content (a page, the off notice)
  completes at once. `waited_ms` says how long it took.

`debug/camera_request.json` with `{"station": "home", "screen":
"wall_screen_6"}` parks the camera 2 m (`distance_m`) straight out from
that screen's centre, at its height, looking back at it: the pose is
computed from the screen's quad, so any screen in any room can be framed
without a typed coordinate. Unknown ids fail with the placed ids listed.
`"look_uv": [u, v]` (2026-11-06) keeps that spot and looks at that point of
the picture instead ((0, 0) its top-left as seen from the front), so a rig
can rest the crosshair near a widget and capture the aim assist's line.

The MAIN egui context has its own sibling, `debug/ui_request.json`
(2026-09-18): `f10` / `escape` run the F10 sidebar's key rules through the
same `engine::input` functions the keys call, `pointer_move` / `click` push
synthetic pointer events into the main context's pending input (window
pixels, three frames for a click), `find` locates a drawn text on the main
frame through this module's `find_text_in_shapes` (so both IPCs answer a
lookup the same way), `state` just reports, and
`debug/ui_request_done.json` answers with the cursor state on both sides of
the winit boundary, the sidebar flags, egui's `wants_pointer_input` and
`is_pointer_over_area`, and a named `cloud_dev_*` flag before and after.
Table row and field list: `docs/dev/performance-profiling.md`, "The debug
file protocol". A rig that wants a wall screen AND the sidebar in one run
uses both files; each is one request at a time.

### The screens rig

`just verify-screens` (`scripts/verify-screens.js`) is the gate that proves
the wall screens are interactive with nobody at the keyboard. It refuses
while ANY HumanityOS.exe is running (one GPU), refuses a stale exe, boots
the release binary in its own portable rig (`.probe-rig/screens`, with
`readable_web: true` written into the rig's config.json before boot), enters
the world through autopilot, parks facing `wall_screen_6`, and then:

1. inventory (`wall_screen_1`): first collapse the sections above the
   places (`find` "Status", `click` its title; then "Equipment" if needed)
   until "Home" is drawn, because a 1280 x 720 wall shows the Status cards
   first and the container header sits below the fold (the section title
   toggles its section: `widgets::section_disclosure` draws it as a
   non-selectable label, so the press is a click, not a text-selection
   drag). Then `find` "Home" (the container header),
   `hover` at the answer, `find` "Garage" (the child container that is
   drawn only while Home is open), snapshot, `click` at the answer, `find`
   "Garage" again, snapshot. The hover comes FIRST so both snapshots carry
   the pointer in the same place: egui's floating scrollbar fades in under
   a pointer, so without that hover two frames differ by a scrollbar
   column whether or not the click did anything, and a pixel diff alone
   cannot tell a dead click from a live one (an adversarial review found
   exactly that hole, 2026-09-17). The judge requires the hover to have
   landed at the found uv, the click's answer to report `hover_widget` AND
   the `PointingHand` cursor (the inventory sets it from `row.hovered()`,
   so it proves the point is on the header row, not merely on the panel;
   the header's labels are non-selectable so egui's text-selection drag
   cannot override that cursor with the I-beam on the release frame, which
   it did while they were selectable),
   the child row's `found` to flip across the click, and the two PNGs to
   differ; `inventory_changed` is red unless every one of those holds;
2. web (`wall_screen_6`): `wait_ready` (status must be `ready` on our own
   host), snapshot, `link` at the first link whose href stays on our own
   host (chosen from the `links` the status reported; if the page drew no
   such link nothing is clicked and the web checks fail, because this gate
   never fetches a third-party page), `wait_ready` again (a NEW url, still
   on our host, `ready` again), snapshot; the two PNGs must differ;
3. tasks (`wall_screen_2`): one snapshot that is not a single colour;
4. zero PANIC lines in the rig's run.log.

Evidence lands in `.probe-rig/screens/runs/<stamp>/` (the PNG pairs, a
viewport capture of the console room, the manifest with every done file
verbatim, the run log). Exit 0 passed, 1 refused, 2 failed. The verdict is a
pure function of the manifest: `--dry-verdict <manifest.json>` re-judges
one without booting, and `--self-test` judges the three fixture manifests
under `tests/fixtures/screens/`: `green` must pass; `red` must fail on
exactly its nine known checks (byte-identical snapshots, a hover off the
target, a click whose cursor is not the header's, a child row that never
went away, a web view that errored on the same url, a flat tasks image, a
panic) while `inventory_find` and `inventory_snapshots` still pass;
`dead-click` is the reviewer's scenario, snapshots that differ only by a
scrollbar-like column while the child row is still found after the click,
and must fail on exactly `inventory_toggled` and `inventory_changed`. That
is how the verdict logic itself is proven able to fail. Both modes print
"DRY VERDICT (nothing was booted)".

### The live-screen rig, and the test publisher it needs

`scripts/verify-live-screen.js` is the same shape of gate for the `watch:`
screen. It could not exist before, for a plain reason: there is no
third-party stream this feature can watch. It watches OUR OWN relay, so
until somebody goes live from Studio there is nothing on the wall and "does
it work" is unanswerable. The rig supplies the missing half.

**`scripts/live-publish.js`** is a publisher with no game attached. It
derives a throwaway Dilithium3 identity through the repo's own chain (the
vendored noble bundle the chat client ships, whose ML-DSA-65 keygen is
pinned to the Rust side by `scripts/pq-kat.mjs`), and:

1. with `--register`, claims a name over the ordinary chat socket using the
   two-phase identify challenge every client uses (`identify` then
   `identify_challenge{nonce}` then `identify_response{sig_b64}` over
   `hum/identify/v1\n<nonce>\n<pubkey>`). This is needed because the relay
   resolves a stream id from the publisher's REGISTERED NAME, so an
   unregistered key gets `unauthorized` at the publisher socket. No
   authentication was added or bypassed to make this work;
2. sends the auth frame `{key, timestamp, sig, title, chat}` on
   `/ws/live/pub`, where `sig` is hex of Dilithium3 over
   `live_publish\n<timestamp>` (`relay::live::authenticate`);
3. pushes `[1 byte tag 1][8 bytes PTS micros BE][JPEG]` at a chosen size,
   rate and duration. The pattern is a sliding diagonal stripe field, a
   sweeping bar, an orbiting disc and the frame number in readable digits,
   so a wall that is live changes nearly every pixel every frame and a
   human glancing at it can see the count advance. JPEG encoding is
   `sharp`, already this repo's one devDependency.

It **refuses any server that is not loopback** unless given
`--allow-remote`. united-humanity.us is the operator's live service; its
streaming switch and its stream names are the operator's decision, not a
test script's.

The rig then, in one boot: starts a relay of its own
(`HumanityOS.exe --headless`, a spare port, a database it deleted first, its
own working directory so it cannot write into the repository's `data/`),
publishes as `shaostoul` (the name `wall_screen_4`'s source already points
at, so no data file is edited), points the game at THAT relay through the
autopilot's `server_url`, parks in front of the wall, and checks eleven
things:

- the relay answered `/health` and `/api/live` listed the stream;
- the screen reports `connected` with frames written;
- **the picture moves**: two snapshots a second apart, against a STATIC
  screen measured over the same second as the floor. The live wall must
  change at least a tenth of its pixels and more than ten times what the
  static one did. Measured 2026-09-18: 99.4% of 921600 pixels against 0.00%;
- with the publisher stopped, the wall says "Stream offline" naming this
  stream, with no parse error in the words, and the heading is really drawn
  (a `find`, not a pixel guess);
- with the server cleared, the wall says "No server set" and reports
  `may_connect: false`, **and no socket was opened**: the screen logs one
  line per socket it opens, and the rig reads the tail of run.log written
  after the server was cleared, where that line must appear zero times.
  Reading a tail is the point, so the sockets the run legitimately opened
  earlier cannot mask the one it must not open now;
- zero PANIC lines.

Evidence lands in `.probe-rig/live-screen/runs/<stamp>/` (the live pair, the
control pair, a PNG of each message page, a viewport capture, the relay's
own log, the manifest). `--dry-verdict <manifest.json>` re-judges one
without booting. Exit 0 passed, 1 refused, 2 failed.

**The server-wide streaming switch does not gate this path.** Server
Settings' video-streaming toggle (`video_streaming_enabled`) is read by
`handle_stream_start`, the CHAT announcement that someone went live, and
alongside the per-role `can_stream`. The binary fanout at `/ws/live/pub`
never reads it; what gates that route is the `live_video` feature switch in
`data/server-config.json`, which defaults to on. So a fresh self-hosted
relay accepts a publisher with the Server Settings toggle still off, and
turning it on is about the chat announcement, not about whether frames
flow.

## Performance budget

- Surfaces are fixed-size (the def's `px`, or for a resized screen the shape's,
  `screen_px_for_shape`), allocated once per placed screen and reused across
  editor rebuilds by instance id; a move recomputes geometry only. Only a
  changed source recreates a surface; a page surface whose pixel size changed
  (a resize in the build editor, 2026-11-06) is resized in place and keeps its
  context, and the screen being dragged is drawn every frame meanwhile.
  The two exceptions resize IN PLACE (`ScreenSurface::resize`, same context,
  new texture): a pixel provider writing frames of another size, and a
  camera screen taking its post's `camera_px` (default 640 x 360) on its
  first picture, a quarter of a 1280 x 720 wall's pixels for about a
  quarter of the re-render's cost.
- Every screen pass is on the Performance page: `gpu.screen_ui` (each
  framed screen's egui pass, summed) and the camera re-render's
  `gpu.screen_sky` / `gpu.screen_scene` / `gpu.screen_transparent` /
  `gpu.screen_overlay` / `gpu.screen_lines`. Each has a same-named
  `cpu.screen_*` submission stage (`cpu.screen_sky` is the stage guard
  around `render_view_onto`'s star pass; the others sit in the renderer
  functions the view calls), so the no-timestamp fallback can read every
  one of them; together they are the "In-world screens" rows (GPU and CPU)
  of `data/performance/budget_systems.ron`.
- At most **4 surfaces re-run their page per frame** (`MAX_FRAMES_PER_TICK`),
  the nearest within **40 m** (`FRAME_RANGE_M`) and in front of the camera,
  plus the hovered one and any IPC target. A surface not framed keeps its
  last image; a distant screen is a frozen page, not a blank one.
- No per-frame copies: the material samples the surface texture directly.
- The theme is re-applied to each framed surface every frame (two small
  style writes), so a Settings theme edit reaches every wall the same frame.

## Known limits (v1)

- **Shared `thread_local` page state.** Some pages keep state in a
  `thread_local` (the inventory page's `InventoryPageState` is one). That is
  per thread, not per context, so the main UI and a screen showing the same
  page share it: opening the inventory's garden editor on a wall opens it in
  the full-screen inventory too. Acceptable for rung 1; the fix is moving
  such state into egui memory (`ctx.data`), page by page.
- **No VR ray yet.** The pointing ray is the camera look ray or the desktop
  cursor; a hand-controller ray is a later rung.
- **Clicks are one-frame stale.** Events are routed at winit event time
  against the hover computed by the last `update`; at 60 fps that is 16 ms.
- Pages that spawn background work when drawn (the Watch directory fetch,
  chat image loads) do the same on a screen. That is correct (the wall
  really is the app) but worth knowing when counting threads.

## The ladder above this

Each is a separate increment on the same surface:

- **Live feed and in-game camera:** shipped, rung 3 (the section above).
- **Video:** shipped (rung 5, the "Video sources" section above): decoded
  frames through `write_pixels`, looping, click to pause, sound placed at the
  screen.
- **The readable web:** shipped (rung 6, the "Web sources" section above).
  The monitor surface did not change; the thing drawn into it did, exactly
  as planned. Still wanted on top of it: a VR-controller ray, and
  distance-based suspend of a wall's fetches.
- **A real browser engine on a screen:** MEASURED, not built.
  [embedded-browser.md](embedded-browser.md) is a spike that ran Chromium
  offscreen at 1280 x 720 in a separate process and costed the whole route.
  One such screen costs the frame 0.064 ms of GPU and about 0.26 ms of CPU,
  which is cheaper than a page screen, so the frame budget is not the
  obstacle. The obstacles are that the official builds ship without H.264, so
  Twitch video cannot decode, and that paid streaming services need a signed
  Widevine path only Google grants. YouTube and Rumble play. Nothing from that
  spike is wired in; it is two standalone crates under `tools/`.
- ~~A placement gate on the sites database~~ BUILT 2026-09-25 (see "The
  sites database is enforced on a wall" above).
- **A planet in a camera's window:** the cloud pass for a second pose needs
  its own temporal history, keyed per camera.

### Screens are per viewer; "sync" means the same source, not the same pixels (operator, 2026-09-25)

Two decisions, both the operator's:

1. **Every screen is drawn by the viewer's own copy of the app, from that
   viewer's own data.** Nothing on a screen is ever sent to another player.
   That is how it already works (no screen content passes through `src/net/`),
   and it is now a rule, for three reasons: a wall can never leak one
   player's private information to another; each person reading a page in
   their own app is ordinary viewing, while one screen shown to others is
   closer to the "public display" and "any other network" clauses the
   2026-09-25 terms research found (`docs/reference/findings/2026-09-25-site-embed-terms.md`);
   and nothing is streamed, which keeps within the bandwidth budget.
2. **"Synced" screens share a SOURCE, and only our own.** A synced display
   (the development task list, the announcements channel, the devlog, and
   similar) is every viewer's app showing the same page from our own
   domains, each drawing it itself. When synced displays are built, the
   code refuses a synced source outside `own_domains`.

**Video sync is a one-shot "jump to where they are", not a shared stream.**
Verbatim: "clicking sync has the player load the URL and grab the same
timestamp, if possible but, each view experience is individual instead of
shared. That way we don't infringe on YouTube ads or whatever." So:

- A player presses **Sync** on someone else's screen (or a sync offer in
  chat). What travels is three values: the source URL, the position in
  seconds, and when that position was read (relay time, so the two clocks
  do not have to agree), plus whether it was playing.
- The receiver's own app loads that URL in its own player and seeks to
  `position + (now - read_at)` when the other was playing, or to `position`
  when paused. From then on the two playbacks are independent: pause, seek
  and ads are each viewer's own. Pressing Sync again re-aligns.
- For a platform video the platform's own player does the playing, with
  its own advertising intact, and most platforms take a start time in the
  URL (YouTube `t=`, Twitch `t=` on past broadcasts), so even the "Open in
  browser" fallback lands at the right moment. For our own AV1 player it is
  a load and a seek (seek shipped in v0.1325.0). A live stream has no
  position to share; Sync there just opens the same stream.
- It replaces the earlier plan of lockstep playback with a shared
  play/pause/seek state, which needed a continuously running shared clock
  and would have made one person's pause everyone's pause.

## Aim assist, any size, and each screen's own settings (2026-11-06)

The operator, verbatim: "Let's add like a proximity for clicking in-game screen buttons. Like
within X degrees highlight the nearest widget, like play or whatever. Let's draw a line between
the crosshair and the gui element it is interacting with. That way I don't have to have perfect
aim on a tiny play button on a screen very far away. Also, can we make the in-game displays
customize width/height wise? That way I could have a 100" 16:9 TV or a 200" 32:9 TV. I feel like
we could add the scaling lines for the TV to the edges of the TV in build mode. Just aim at the
edge and drag. Then click on the central area of the screen allows moving it around. Then there
could be like a cog like at the top right of the screen for editing that screens settings. So I
could say this screen always display this youtube/twitch stream whenever they livestream. Or this
screen only allows shopping at amazon. Or this screen always displays this HumanityOS page. Or
whatever imaginable."

This section is the whole feature, then the rungs it is built in. Rungs 1 and 2 are built.

### 1. Aim assist: the nearest widget in a cone

**What the player sees.** While the crosshair (or the free cursor) is near a screen, the nearest
thing on it that can be clicked is outlined in the theme's accent colour, and a thin line runs
from the crosshair to it. A click goes to that thing. When the crosshair is already on a control,
that control is outlined and no line is drawn (there is nothing to bridge).

**Where the widgets come from.** egui records every widget it laid out in a pass, with its id, its
rectangle after clipping, what it senses (click, drag, hover) and whether it is enabled. A screen's
own context exposes the finished pass through `ctx.viewport(|v| v.prev_pass.widgets)`, so the
candidates are exactly what egui itself would hit-test: no list of buttons kept by hand, and every
page on every screen is covered by construction. A candidate is a widget that senses a CLICK, is
enabled, has a rectangle of at least one pixel after clipping (a row scrolled out of view is gone),
and is the topmost layer at its own centre (a button under an open window is not offered).

**How it picks.** The look ray meets the screen's PLANE, whether or not it lands inside the
display. For each candidate the nearest point of its rectangle to that meeting point is found (in
the screen's own pixels, which is the plane in metres scaled), turned back into a world point, and
measured as the ANGLE between the look ray and the direction from the eye to that point. The
smallest angle inside the cone wins. Measuring in angles rather than pixels or metres is the
point: a 1 cm button on a far screen and a 5 cm one on a near screen are equally easy to hit,
because what the player controls is where they look, not a distance on the wall. A ray that misses
the display by a little still picks a widget near the edge, so a control in the very corner of a
far screen is reachable without landing on the screen first.

- **Inside wins.** A control the crosshair is on has angle 0 and wins at once; among nested ones
  (a button inside a clickable row) the smallest rectangle wins, which is the one egui's own
  hit-test would pick. The pointer then goes where the crosshair really is, not to the control's
  centre, so a slider or a text field gets the exact spot.
- **Snapped, the pointer goes to the nearest point of the target, a few pixels inside it**, never
  its centre: a wide row is clicked near where the player aimed, and a small button inside a wide
  row is not clicked by aiming at the row's far end.
- **Hysteresis, so it never flickers.** The current target is kept while it stays within the cone
  times `exit_scale` (1.25), and a rival takes over only when its angle is below the current one
  times `switch_ratio` (0.7). Between two buttons, the crosshair crosses a dead band in the middle
  where the old choice holds, instead of a line where the two swap every frame. A rival the
  crosshair is ON always takes over (its angle is 0).
- **A held button freezes the target.** While the press is down the pointer moves by the look
  ray's movement since the press, from where the press landed. A click is press and release with
  the pointer still (egui refuses a click that moved more than 6 px), so the look ray's small
  jitter never turns a click into a drag, and a real drag (a slider) still moves.
- **The cone** is the player's: Settings > Controls, "Aim assist on in-world screens" (on by
  default, for accessibility: a tremor or a gamepad stick should not decide whether a button can
  be pressed) and "Aim assist cone" in degrees (`cone_deg_default` 3, from 0.5 to 10, in
  `data/gui/screens.json`). Off, the screens behave exactly as before.

**How far it reaches.** The old fixed 3.5 m (`REACH_M`, removed) is the floor. A screen's reach grows with
its size: `reach = clamp(max(reach_base_m, diagonal * reach_per_diagonal), .., reach_max_m)`, from
the data file (3.5 m, 1.75 m per metre of diagonal, 25 m at most). The factor is chosen so the
shipped 79-inch wall screen (a 1.74 x 0.99 m picture, 2.0 m diagonal) reaches 3.5 m as before; a
100" TV (2.5 m diagonal) reaches 4.4 m, a 200" one (5.1 m) 8.9 m.
This keeps one rule ("a screen answers from as far as it reads"), and the 25 m cap keeps a
cinema-sized wall from taking clicks meant for the world across a hall.

**Controller and mouse.** The assist runs on the pointing ray, which is the look ray while the
cursor is grabbed (what a gamepad stick or the mouse turns) and the cursor's ray while it is free.
On a gamepad (not wired in the engine yet) the outline is the only cursor a screen has, so it shows
whenever a target is chosen; with a mouse the same. A future `controller_cone_scale` widens the
cone for a stick, which is coarser than a mouse; it is not in the data file until a gamepad exists.

**Cost.** One ray against each screen's plane per frame (they already were), and for the screen
being aimed at, one walk over the widgets egui recorded last pass (a few hundred rectangles at most
on a busy page). No new pass, no readback, nothing kept between runs.

### 2. Size and shape: any width and height

A placed screen's PICTURE has a width and height in metres, chosen freely or from presets by
diagonal and aspect (`data/gui/screens.json` `sizes`: 24", 32", 55", 65", 75", 100" and 120" at
16:9; 34" and 55" at 21:9; 49" and 200" at 32:9; any number more is a data line). The body follows
the picture: on its screen face it is the picture plus the bezel on each side
(`body_size_for_display`), its depth is the def's, and it grows from its base and about its
centre, so a wall-hung screen stays on its wall.

**The surface follows the shape.** The pixel height stays the def's (720 for a wall screen), so a
page reads the same size on any screen of a kind, only bigger on a bigger one; the width follows
the aspect (`screen_px_for_shape`): 16:9 is 1280 x 720, 32:9 is 2560 x 720, 21:9 is 1680 x 720.
The cap is **2,073,600 pixels (one 1920 x 1080 frame) and 4096 wide** (`SCREEN_MAX_PIXELS`,
`SCREEN_MAX_WIDTH_PX` beside the function in `machines.rs`), scaled down evenly past it,
because a page screen's cost is its egui pass and texture (8 MB at the cap) and, since 2026-10-31,
page screens take turns, one a frame; the cap keeps any one turn inside the measured 2 to 3 ms of a
1280 x 720 page with room to spare. A screen whose source is a camera or a video still sizes its
surface to its frame as before; the shape only changes the canvas it letterboxes into. Such a
surface (a stream, a clip, a camera, the web view) is made anew at the screen's new shape when a
build-mode handle is let go, not on every frame of the drag (a new socket or decoder a frame); a
page surface is resized in place, keeping its scroll and open state. One limit stays: a camera
renders at its post's `camera_px` aspect (16:9 by default), so on a screen of another shape its
picture is stretched, as it already was on a 16:10 desk monitor; rendering at the screen's own
aspect is a rung 5 item.

### 3. Build mode: drag the edges, move it, the cog

In the build editor (B), the screen under the cursor and the selected screen show their handles,
drawn on the picture's own rectangle:

- **Edges** (a band along each side, `handle_band` of the shorter side): aim and drag to move that
  edge; the opposite edge stays put. An edge changes one dimension, so it changes the shape.
- **Corners**: drag both edges at once. **A corner keeps the aspect; Shift frees it.** A TV is
  bought by size, and its aspect is part of what it is (16:9, 32:9), so the common resize keeps
  it and the explicit shape change is the edge or Shift.
- **The middle**: drag to move it ON ITS WALL: along the wall and up or down, never in or out, so
  a wall screen cannot be dragged off its wall into the room. (Moving it to another wall is the
  machine's ordinary floor drag, from the object browser, or later the drag across a corner.)
- **Snapping**, with the editor's grid snap on: a width or height to whole 5 cm (a corner keeping
  the aspect snaps its width), a move in 5 cm steps along the wall and in height; then an edge
  within 5 cm of a neighbouring screen's edge on the same wall lines up with it (rung 5; the grid
  step is rung 2).
- **The cog**, a gear near the picture's top-right corner: click to open that screen's settings
  panel (section 4). It sits two handle widths in from the corner, clear of both edge bands, so
  the corner stays a corner and a click on the cog never starts a resize.
- **What a tap does**: a press and release without moving only selects the screen (its details
  show on the editor's right panel), as for every object; a drag starts past the editor's drag
  threshold. The handles are drawn on the picture itself: the edges as lines (the one under the
  cursor thicker, in the accent colour), the corners as squares, a cross in the middle, the cog,
  and the size and diagonal under the bottom edge.
- Limits: from 0.2 x 0.12 m to 8 x 5 m (`min_m`, `max_m` in the data file).

**The data**, per placed screen in the home's file, never per machine def:

```ron
(
    id: "wall_screen_4",
    machine: "wall_screen",
    ...
    screen_source: Some("page:tasks"),     // what it shows (existing since rung 2 of the ladder)
    screen_size_m: Some((2.214, 1.245)),   // the picture in metres; None = the def's size
)
```

Both fields are optional and default to None, so every home file written before keeps loading
as it was (no compatibility code: an absent field is the def's value). A machine def keeps its
`size` and `screen.px` as the defaults a new screen starts from.

### 4. Each screen's settings (the cog)

What a screen shows, and the rules it follows, as data entries, never code branches
(infinite-of-x). The panel writes them into the placed screen and the editor's save writes the
home file.

**What it shows**, one source string (the existing schemes):

| choice | source | status |
|---|---|---|
| A HumanityOS page, always | `page:<id>` | built (rung 2) |
| A camera post, always | `camera:<instance id>` | built (rung 2) |
| A video file on this computer | `video:<path>` | built before; chosen with the screen's own Open button |
| A web page, read by our own reader | `web:<url>` | built before; typed in the cog in rung 3 |
| A live stream on our own relay | `watch:<name>` | built before; chosen in the cog in rung 3 |
| A YouTube, Twitch or Rumble channel, whenever it is live | `embed:<platform>:<channel>` | rung 4, through the embedded-browser host |

**Rules** (rung 3 onward), a list read top to bottom, the first that applies wins, the screen's
`screen_source` when none does:

```ron
screen_rules: [
    (when: Live("twitch:somechannel"), show: "embed:twitch:somechannel"),
    (when: Schedule(days: [Mon, Tue, Wed, Thu, Fri], from: "07:00", to: "09:00"), show: "page:tasks"),
],
screen_allow_sites: ["example-shop.com"],   // a web: screen may only go to these sites
screen_changers: Household,                 // Anyone, Household or Owner
screen_sound: (on: false, volume: 0.6),
```

- **A live channel "whenever they are live".** Watching YouTube, Twitch or Rumble on a screen goes
  through the opt-in embedded-browser host and nowhere else (CLAUDE.md, the 2026-09-18 amendment;
  [embedded-browser.md](embedded-browser.md)). **Today that host is a measured spike, not built:**
  nothing in the game can show a platform's player yet. When it is built, the simplest honest
  "whenever they are live" is the platform's own embedded player for the channel, which shows the
  stream when there is one; switching the screen to something else while the channel is offline
  needs a "who is live" answer from the platform. The limits named per the project's rule:
  Twitch video cannot decode in the official browser builds (a PATENT limit, H.264 and AAC);
  YouTube's developer policies forbid embedding API credentials in open-source projects, and
  Rumble's terms forbid automated access without its written permission (both PLATFORM HOLDER'S
  RULES, `docs/reference/findings/2026-11-06-social-platform-apis.md`, YT-8 and RU-1). How each
  platform lets an app ask whether a channel is live is NOT researched yet: a dated finding comes
  before rung 4 builds it.
- **Only this site.** A `web:` screen with an allowlist navigates only within those domains (a
  link or typed address elsewhere is refused with a plain sentence). What our reader is, plainly:
  it shows a page's text, images and links, and it does not run JavaScript, so a shop's product
  pages may read but its cart, sign-in and checkout, which are scripts, do not work on it. The
  sites database's placement gate still applies first ([readable-web.md](readable-web.md)): a site
  whose terms forbid being shown inside other software is refused there.
- **A schedule**: days and a time window, in the player's local time.
- **Who may change it**: anyone in the home, the household, or only its owner; in a shared world
  the server keeps the setting and checks the signer, the same as other building in a plot.
- **Sound**: on or off and a volume per screen. A screen still starts muted on every boot unless
  its owner turned sound on for it (the 2026-09-27 rule; this makes the choice per screen).

### 5. The page fills its screen, and so do its full views (operator, 2026-11-06)

Asked about wall screens offering no Full window for chat videos (BUG-258 had taken the button
away), the operator, verbatim: "I figured the whole display would be dedicated to the whole page
that we're on tailored to that display size/aspect ratio."

So two rules:

- **A page on a wall screen lays itself out for THAT display.** A page draws at one point per
  pixel into the screen's surface, and the surface follows the screen's shape (section 2), so a
  32:9 wall's page is laid out 2560 x 720 points wide, not a 16:9 page with bars. Built with rung 2:
  nothing letterboxes a page; only a stream or a clip, whose frames have their own aspect,
  letterboxes into the screen's canvas.
- **A full view opened on a wall screen fills THAT screen, never a window on the app screen.** A
  chat video's Full window, the picture viewer and the 3D view are drawn by the screen's own run,
  under its own context, over its page (`screen_surface::draw_screen_full_views`), and closed there
  with the screen's own controls, aimed at with the look ray and the aim assist. Their state is the
  screen's own: the chat's videos were already a set per screen (`ScreenTextures::chat_video`);
  `image_viewer_url` and `chat_model_view`, which were one `GuiState` field each shared with the
  app screen (a picture pressed on a wall opened over the app screen, where nobody at the wall
  could close it), are now swapped per screen like the textures. **Built 2026-11-06**: Full window
  is offered on a wall again and opens on the wall; the picture viewer and the 3D view open on the
  wall that asked. One limit stays: the 3D view draws with the model viewer's GPU pieces, which
  live in the app screen's egui renderer; a wall screen's renderer does not have them yet, so on a
  wall the 3D view shows the viewer's own plain words ("needs the app's graphics; Download the file
  to see it") in its window on the wall. Installing those pieces in each surface's renderer is the
  next step for it (rung 3 below).

### 6. A control room: one fixed screen per page (operator, 2026-11-06)

The operator, verbatim: "I feel like we should design a control room with a fixed display screen
to each menu page. The inventory display is only ever inventory. That way we can see all of the
screens at once."

**What it is.** A room whose walls carry one screen per app page, each bound to its page by the
cog's first rule, "always this page" (`screen_source: page:<id>`), all live at once: inventory,
maps, crafting, chat, tasks, the body and health page (`real`), quests, calendar, the Homes page,
and the ship's status. The ship's status has no page of its own yet (the HUD's home systems line,
power, water and air, is its seed, `game_home_state` reads the same numbers); a page for it is part
of this rung.

**Where it sits** (recommendation, coordinated with
[mothership-master-plan.md](mothership-master-plan.md)):

1. **In the player's home**: the Homestead design's console room already holds six screens
   (inventory, tasks, the garden camera, the live stream, the video, the web: section "Shipped
   data" above), so it becomes the home's control room rather than a new room being added: its
   walls are re-laid as a screen wall, one page each. A Cabin, 3.15 x 2.5 m, has room for one
   wall of three; the master plan's plot kinds each say how many (data, below).
2. **On the ship**: the bridge, the big shared one, at the bow (the master plan's section 3.3:
   the bow is where the ship meets the outside, its hangars, arrivals and docking). The plan does
   not place a bridge yet; this recommends adding it there in the rungs after Neighbourhood 1
   (its section 6.5), with the ship-wide pages (the fleet ledger, the ship's status, the map) on
   its walls. Its screens are per viewer like every screen (section "Screens are per viewer"):
   each person at the bridge sees the pages from their own app.

**The data.** A screen wall is one entry in the home's file, expanded at load into ordinary placed
screens the way `arrays` expands into machines, so every cell has the cog, the handles and its own
saved size:

```ron
screen_walls: [
    (
        id_prefix: "control",          // cells are control_inventory, control_maps, ...
        machine: "wall_screen",
        zone: "home", room: "console-room",
        origin: (50.9, 1.0, 38.0),      // the first cell's base centre, zone-local
        along: (0.0, 0.0, 1.0),         // the wall's direction
        rotation: 90.0,                 // which way the cells face
        size_m: (1.74, 0.99),           // each picture (optional: the def's)
        gap_m: 0.1,
        rows: 2,                        // stacked, fill the row first
        pages: ["inventory", "maps", "crafting", "chat", "tasks", "real"],
    ),
],
```

A page named twice is refused with a warning (the inventory display is only ever inventory, and
one is enough); an unknown page id shows the existing "No page named" notice on its cell.

**The budget for N live page screens.** Today `frame_surfaces` frames the nearest four screens in
view within 40 m, and of those the page screens take turns, one a frame (`PAGE_FRAMES_PER_TICK`,
measured 2 to 3 ms each in the 2026-10-31 stutter survey); the hovered screen, the focused one, a
provider's and the one being dragged redraw every frame. A control room breaks the "nearest four":
twelve page screens in view would leave eight frozen. So in this rung every page screen in view
and in range joins the turns, still one page redraw a frame: N screens each redraw at
`fps / N` (twelve at 60 fps: 5 times a second; at the operator's 120 fps: 10), the one being looked
at every frame, and the frame pays the same one page draw it pays today. A page that changes
slowly (the calendar) loses nothing; one that changes fast is the one being looked at. A later
refinement spends a time budget instead of a count (about 3 ms of page draws a frame, each
screen's last draw time measured), so cheap pages refresh more often; both obey the Dev page's
"turn off all reuse" switch, which redraws every screen every frame as before. Memory: a 1280 x
720 surface is 3.7 MB of texture, twelve are 44 MB.

### The rungs

1. **Aim assist** (built 2026-11-06): the cone, the hysteresis, the line and the outline, reach by
   size, the Settings toggle and cone.
2. **Any size, the handles, the cog** (built 2026-11-06): `screen_size_m` saved and loaded, the
   surface following the shape, edge and corner drag (Shift frees the aspect), the middle drag on
   the wall, the grid snap, size presets, and the cog's panel with "a HumanityOS page" and "a
   camera" wired to the existing sources. With it (built 2026-11-06): a wall screen's full views
   (a chat video's Full window, the picture viewer, the 3D view) open on that screen, per screen.
3. **The control room**: `screen_walls` in the home file, every page screen in view taking turns,
   the home's console room re-laid as one, a ship's status page, and the model viewer's GPU pieces
   in each surface so the 3D view draws on a wall.
4. **Rules without the platforms**: the web address, our own live streams, the schedule, the
   allowlist, sound and volume, who may change it.
5. **Live channels**, after the embedded-browser host ships and a dated finding on each
   platform's live-status route. The bridge on the ship follows the master plan's own rungs.
6. **Snapping to neighbours, across corners, several screens at once, a VR ray.**
