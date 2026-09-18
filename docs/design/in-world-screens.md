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
| Sources and providers | `src/gui/screen_surface.rs` (`ScreenSource`, `ScreenProvider`, `ScreenWorld`), `src/engine/screens.rs` (`provider_for`, the registry), `src/engine/screens/` (one provider per kind: `video.rs`) |
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
what lets the rig wait for and assert on them (`complete_screen_request`
merges them under the base fields, which win a name clash).

A provider that places SOUND in the world gets the world context once per
frame through `ScreenProvider::world_update(&mut ScreenWorld)`: its screen's
centre, the listener's position and right-hand direction, and the audio
manager when the machine has an audio device. `frame_surfaces` calls it for
EVERY surface with a provider, framed or not, before the framing loop: a clip
keeps sounding from its screen while the player faces away, so its volume
and pan must follow the listener every frame, independent of the framing
budget. A provider with nothing to place leaves the default no-op.

### Video sources (rung 5, integration)

`video:<path>` plays a WebM clip through the purpose-built player
([media-player.md](media-player.md)) on the screen, on loop, with its sound
placed at the screen; a click on the screen toggles pause. The operator's
words: "movies on displays". Provider: `src/engine/screens/video.rs`
(`VideoProvider`). Shipped: `wall_screen_3` in home.ron's console room plays
`data/media/demo_colour_bar.webm` (the synthetic 2 s colour-bar clip;
provenance in `data/media/README.md`).

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
(two people in the same room seeing the same frame needs the relay clock
and a shared play/pause/seek state; a later rung), a **seek bar** or any
transport control beyond click-to-pause, **subtitles**, a **file picker** (the
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

- **Live feed:** a camera or stream frame written into a material's retained
  texture each frame via `Renderer::update_material_albedo_pixels` (already
  in place: one `write_texture` per frame at matching size, no reallocation).
- **Video:** SHIPPED (rung 5, "Video sources" above): decoded frames through
  `write_pixels`, looping, click to pause, sound placed at the screen.
- **The readable web:** Brief 4's HTML/CSS renderer drawing into a surface
  the way an egui page does today. The monitor surface does not change; the
  thing drawn into it does.
