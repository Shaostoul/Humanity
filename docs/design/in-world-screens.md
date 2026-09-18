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
| Data shape | `src/machines.rs` (`ScreenDef`, `MachineInstance.screen_source`), `data/machines/home.ron`, `home_solo.ron` |
| Sources and providers | `src/gui/screen_surface.rs` (`ScreenSource`, `ScreenProvider`, `WorldRender`), `src/engine/screens.rs` (`provider_for`, the registry), `src/engine/screens/live.rs` (the live stream), `src/engine/screens/camera.rs` (the in-game camera) |
| One view of the world from any pose | `src/engine/ipc.rs` (`render_view_onto`, `ViewPasses`), shared by the hi-res screenshot and the camera screens |
| Dev IPC | `src/engine/ipc.rs` (`poll_screen_request` and its two companions) |
| Hooks in the main loop | `src/lib.rs` (search "In-world screens") |

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
- The nearest screen the ray hits within **3.5 m** (`REACH_M`) is the hovered
  screen; its page receives `PointerMoved` at the hit's (u, v) in pixels.
  Leaving it sends `PointerGone`. Screens beyond reach still render but
  ignore input.
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
    power: None, ports: [], storage: [], rf_emission: 0.0,
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

Shipped: `wall_screen` and `desk_monitor` in both `home.ron` and
`home_solo.ron` under the new "Displays" palette category, and two placed
`wall_screen` instances in home.ron's common room (`wall_screen_1` inventory
on the south wall, `wall_screen_2` tasks on the east wall). Placing a screen
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
what lets the rig wait for and assert on them.

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
`run_and_render` ("Connecting to shaostoul"); when the viewer's thread ends
(the relay said "not live", the stream ended, the socket failed) the page
says "Stream offline" with the relay's reason, the viewer is dropped, and a
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
first frame. `status()` reports `{stream, connected, frames}` (frames
written to the surface), merged into `debug/screen_done.json`.

### The in-game camera: a camera post and a world screen

`camera:<instance id>` shows the game world from a placed **camera post**.
The post is an ordinary catalog machine with one new def field:

```ron
"camera_post": (
    shape: "box", size: (0.14, 1.7, 0.14), color: (0.2, 0.2, 0.22),
    label: "Camera post", category: "Displays",
    camera: Some((0.0, -8.0, 70.0)),   // (yaw, pitch, fov) in degrees
),
```

`MachineDef.camera` / `PlacedMachine.camera` is `(yaw_deg, pitch_deg,
fov_deg)`. Yaw 0 is the body's front (-Z at rotation 0) and the placed
instance's `rotation` ADDS to it, so turning the post in the editor turns
the camera; pitch is positive up (a mounted camera looks a little down);
fov is the vertical field of view. `PlacedMachine::camera_pose`
(`src/machines.rs`) resolves the lens: just below the machine's top, on its
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
- The render size is the surface's own `px`; a smaller camera screen def is
  a cheaper camera.
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
in the console room (x 47.5..51, z 44..47) `wall_screen_3` on the west wall
showing `camera:camera_post_1` and `wall_screen_4` on the north wall showing
`watch:shaostoul`. `shipped_homes_place_a_camera_post_and_the_console_screens_name_it`
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

## Dev IPC (permanent tooling)

Drop `debug/screen_request.json` while the game runs:

```json
{"screen": "wall_screen_1", "action": "click", "uv": [0.5, 0.2]}
{"screen": "wall_screen_1", "action": "scroll", "uv": [0.5, 0.5], "dy": -3}
{"screen": "wall_screen_1", "action": "text", "uv": [0.5, 0.5], "text": "hello"}
{"screen": "wall_screen_1", "action": "hover", "uv": [0.1, 0.1]}
{"screen": "wall_screen_1", "action": "snapshot"}
```

The event goes through the same `ScreenCore` methods the look ray uses
(never a side path), and `debug/screen_done.json` comes back as
`{"ok", "screen", "source", "kind", "action", "wants_keyboard", "hover_widget",
"cursor_icon", "focused", "png"}` plus the provider's `status()` fields; `png` is present for `snapshot`, which
reads the surface texture back to `debug/screen_<id>_N.png`. A click is a
press on one frame and a release on the next, and the done file is written
after the surface has drawn the frame the event landed in. `hover_widget`
is whether egui reported a layer under the pointer after that frame (egui
does not expose per-widget hover publicly); `cursor_icon` distinguishes a
text field (Text) or a link (PointingHand) from plain content. The request
file is consumed even on error, and an error writes `{"ok": false, "error"}`.

This is how the runtime verifier proves a screen works with nobody at the
keyboard: request a snapshot, read the PNG, request a click on a header,
request another snapshot, compare.

## Performance budget

- Surfaces are fixed-size (the def's `px`), allocated once per placed screen
  and reused across editor rebuilds by instance id; a move recomputes
  geometry only. Only a changed page or pixel size recreates a surface.
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
- **Video:** decoded frames through `write_pixels`, the same path the live
  frames take.
- **The readable web:** Brief 4's HTML/CSS renderer drawing into a surface
  the way an egui page does today. The monitor surface does not change; the
  thing drawn into it does.
- **A planet in a camera's window:** the cloud pass for a second pose needs
  its own temporal history, keyed per camera.
