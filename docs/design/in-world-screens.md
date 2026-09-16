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
| Sources and providers | `src/gui/screen_surface.rs` (`ScreenSource`, `ScreenProvider`), `src/engine/screens.rs` (`provider_for`, the registry), `src/engine/screens/` (one provider per kind) |
| The web provider (rung 6) | `src/engine/screens/web.rs` (`WebProvider`) |
| Dev IPC | `src/engine/ipc.rs` (`poll_screen_request` and its two companions; `poll_camera_request`'s `station` + `screen` pose) |
| The screens rig | `scripts/verify-screens.js` (`just verify-screens`), `scripts/lib/png.js`, fixtures under `tests/fixtures/screens/` |
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
| `watch:<stream id>` | an MJPEG live stream (the Watch page's decoder) | the live provider (rung 3) |
| `camera:<machine instance id>` | the game world from a placed camera's pose | the camera provider (rung 3) |
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
what lets the rig wait for and assert on them. Two more hooks serve the rig:
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
- **The sites database still applies.** The affiliate disclosure line for
  the current page's site is drawn above the page on the wall too. Which
  sites may be placed on screens is the database's `embed.status` call, as
  the readable-web doc says; the shipped `wall_screen_3` shows our own site.

Status for the dev IPC: `{url, title, status}`, where the title is the page's
first heading (else its `<title>`, else the url) and the status is one of
`unframed`, `off`, `idle`, `fetching`, `ready` or `error: <reason>`.

Shipped placement: `wall_screen_3` in `home.ron`, on the console room's
east wall (x = 51, z centre 45.5, facing west into the room), source
`web:https://united-humanity.us`.

## Dev IPC (permanent tooling)

Drop `debug/screen_request.json` while the game runs:

```json
{"screen": "wall_screen_1", "action": "click", "uv": [0.5, 0.2]}
{"screen": "wall_screen_1", "action": "scroll", "uv": [0.5, 0.5], "dy": -3}
{"screen": "wall_screen_1", "action": "text", "uv": [0.5, 0.5], "text": "hello"}
{"screen": "wall_screen_1", "action": "hover", "uv": [0.1, 0.1]}
{"screen": "wall_screen_1", "action": "snapshot"}
{"screen": "wall_screen_1", "find": {"text": "Home"}}
{"screen": "wall_screen_3", "action": "wait_ready"}
{"screen": "wall_screen_3", "link": {"index": 0}}
```

The event goes through the same `ScreenCore` methods the look ray uses
(never a side path), and `debug/screen_done.json` comes back as
`{"ok", "screen", "source", "kind", "action", "wants_keyboard", "hover_widget",
"cursor_icon", "focused", "uv", "png"}` plus the provider's `status()` fields
merged at the top level (a web screen adds `url`, `title`, `status`); `png` is
present for `snapshot`, which reads the surface texture back to
`debug/screen_<id>_N.png`; `uv` is present for the pointer verbs and says
where the event landed. A click is a press on one frame and a release on
the next, and the done file is written after the surface has drawn the
frame the event landed in. `hover_widget` is whether egui reported a layer
under the pointer after that frame (egui does not expose per-widget hover
publicly); `cursor_icon` distinguishes a text field (Text) or a link
(PointingHand) from plain content. The request file is consumed even on
error, and an error writes `{"ok": false, "error"}`.

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
  clipped to the surface, its centre becomes the uv, and the press and
  release go through `ScreenSurface::button` like any click. No such link
  is `{"ok": false}` with the count the content drew.
- **`wait_ready`** completes only once the provider reports ready or
  failed (`load_state()`), bounded by `WAIT_READY_LIMIT` (15 s; a timeout
  is `{"ok": false}` with the status fields, never a pass). While it waits
  the request stays in flight, which keeps the surface framed so a web
  view keeps polling its fetch. Static content (a page, the off notice)
  completes at once. `waited_ms` says how long it took.

`debug/camera_request.json` with `{"station": "home", "screen":
"wall_screen_3"}` parks the camera 2 m (`distance_m`) straight out from
that screen's centre, at its height, looking back at it: the pose is
computed from the screen's quad, so any screen in any room can be framed
without a typed coordinate. Unknown ids fail with the placed ids listed.

### The screens rig

`just verify-screens` (`scripts/verify-screens.js`) is the gate that proves
the wall screens are interactive with nobody at the keyboard. It refuses
while ANY HumanityOS.exe is running (one GPU), refuses a stale exe, boots
the release binary in its own portable rig (`.probe-rig/screens`, with
`readable_web: true` written into the rig's config.json before boot), enters
the world through autopilot, parks facing `wall_screen_3`, and then:

1. inventory (`wall_screen_1`): snapshot, `find` "Home", `click` at the
   answer, snapshot; the two PNGs must differ and `hover_widget` must be
   true;
2. web (`wall_screen_3`): `wait_ready` (status must be `ready` on our own
   host), snapshot, `link 0`, `wait_ready` again (a NEW url, `ready`
   again), snapshot; the two PNGs must differ;
3. tasks (`wall_screen_2`): one snapshot that is not a single colour;
4. zero PANIC lines in the rig's run.log.

Evidence lands in `.probe-rig/screens/runs/<stamp>/` (the PNG pairs, a
viewport capture of the console room, the manifest with every done file
verbatim, the run log). Exit 0 passed, 1 refused, 2 failed. The verdict is a
pure function of the manifest: `--dry-verdict <manifest.json>` re-judges
one without booting, and `--self-test` judges the two fixture manifests
under `tests/fixtures/screens/` (a green one that must pass and a red one
that must fail on exactly its six known checks, including two byte-identical
snapshots for the differ check), which is how the verdict logic itself is
proven able to fail. Both print "DRY VERDICT (nothing was booted)".

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

- **Live feed:** a camera or stream frame written into a material's retained
  texture each frame via `Renderer::update_material_albedo_pixels` (already
  in place: one `write_texture` per frame at matching size, no reallocation).
- **Video:** decoded frames through the same updater.
- **The readable web:** shipped (rung 6, the "Web sources" section above).
  The monitor surface did not change; the thing drawn into it did, exactly
  as planned. Still wanted on top of it: a VR-controller ray, and
  distance-based suspend of a wall's fetches.
