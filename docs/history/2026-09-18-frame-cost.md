# Where the frame goes, 2026-09-18

The operator's question, verbatim: "Before we do much more we need to find
out that despite the cloud layer being off that the planet is tanking
performance to ~12FPS. Do we have diagnostics to tell us where all the
performance is being spent? Also, the F10 menu that allows me to adjust
clouds and other stuff pops up but, I can't interact with any of the stuff
inside it. We should also set it so that when I press esc it closes the F10
menu only instead of also opening up the menu but, only while the F10 menu is
up."

One day, three answers, and one of them changed what the renderer costs.

## The measurement

The diagnostics existed: `frame_costs` with real GPU timestamp queries, the
Performance page that draws them, the F2 overlay, and the probe rig with
`HUMANITY_FRAME_COSTS=1` writing a cost file per capture. What did not exist
was a measurement at the operator's real settings with the clouds off. A
measuring agent took one: 21 boots, 72 cost files, every canonical planet
vantage plus the home and the console room, every `gpu.*` and `cpu.*` key
tabulated, and the top pass at each vantage bisected with rebuild-free knobs.

The clouds were not the cost. The planet pass (`gpu.celestial`) was per-pixel
and independent of geometry: 44.5 ms on the moon with six patches drawn, 61
at the 400 km limb, 43 in the Sahara at noon, and no toggle moved it except
surface-detail octaves (9 to 11 ms) and patch count at grazing views. About
12 to 15 ns per pixel where the engine's own 40-tap fullscreen pass costs
0.12. The interior opaque pass was 23 ns per pixel per layer (a flat quad
filling the view cost 160 ms; the console room 76 to 83 ms of scene plus 14
of glass); the 211-light loop was only 8 ms of it, early-z was working, and
the camera wall added 17 ms only while in view. At forest vantages the
photoscanned near trees were 68 ms of 156, the cluster cards 38, grass 17. At
low ocean eyes the water shell was 42 to 49 ms. Unaccounted: 13 to 20 ms per
frame at light vantages with no frame-total or present-wait timer, and four
passes with no timestamp scope at all.

## The hypothesis, and the one boot that decided it

A read-only perf pass turned the numbers into mechanism. The two biggest
costs shared one thing: the module. Seven pipelines are compiled from one
14,296-line megashader and four of them use `fs_main` as their fragment
entry. H1: every fragment of every one of those pipelines pays the module's
per-invocation private storage, chiefly the cloud march's 2.9 KB
`var<private> g_bc_lc` array, reachable from `fs_main` through `cloud_layer`
even when clouds are off. The bandwidth arithmetic fit (5.9 KB per fragment
implied); the repo's own record already described the spill in the pass it
was built for. H2: the floor is outside the fragment shader. The design
named the experiment: stub the three shell branches, capture the moon, which
draws none of them, and watch the number.

The P1 agent ran it as a same-boot A/B through the shader hot reload, same
frozen clock and same GPU state on both sides: the moon's planet pass read
44.1 ms with the branches reachable and 7.4 ms with them unreachable; the
Sahara's atmosphere shell fell from 10.4 to 0.2 ms the same way. H1.

The shipped change is the standard material-permutation technique done from
one source: WGSL override constants guard the atmosphere, cloud and ocean
dispatches, and the two terrain pipelines compile with all three off through
pipeline compilation options, with a per-pipeline registry of dead branches
and tests that pin the switch declarations and the guard shape (naga ignores
an unknown override key, so a rename on either side would silently put the
branch back). At the operator's settings: moon 45.4 to 8.3 ms, Sahara 31.4
to 5.3, blue marble 5.0 to 1.1, limb 63.5 to 13.3 (10.9 to 24.1 fps), Fuji
85 to 69. The limb's remaining 26 ms is the atmosphere shell through the
untouched transparent pipeline, which is P2; the interior's floor is the same
module through the opaque pipeline, so P2 answers the console room too.

## The instrumentation

A parallel branch closed the gaps the measurement named: scopes on the screen
surface pass, the camera screen's sky pass, the particle compute and the
sky-view LUT; the camera re-render, its overlay and lines under their own
ids instead of summing into the main frame's; `cpu.frame_total`,
`cpu.present_wait` and `cpu.fps_cap_sleep`; the cloud keys, the water shell
and the screens registered on the Performance page, with a scan-based test
that holds the registry equal to what the engine records in both directions;
the camera wall at a quarter of its pixels; the NearTree recompute that
logged every frame while parked rate-limited and demoted. Its critic found a
pre-existing defect that would have defeated the deliverable: opening the
Performance page decayed every GPU row toward zero within a third of a
second, because the GPU harvest ran on UI-only frames while the CPU column
froze. Fixed, red-proven (4.0 ms read as 0.0002 after 60 UI-only frames on
the old code).

## F10 and Escape

The sidebar drew but took no input because the in-game HUD allocated a
full-screen hover rect, and egui 0.31's hit test stops at the first covering
widget rect regardless of its sense or its Area's interactable flag, so the
sidebar beneath it was unreachable (BUG-076). Found by drawing the sidebar
alone headlessly (a synthetic click flips a checkbox), then adding the HUD
and watching the same click vanish; the fix is one allocation removed and a
test that fails when it returns. Escape now closes the expanded sidebar
first and does nothing else that press, repeated Escape presses are dropped,
and the F10 toggle and the Escape rule live in `engine::input` so the key,
the tests and a new `debug/ui_request.json` IPC share one code path.

The runtime proof drove the merged build through that IPC (a click flipped
the flag and the checkbox visibly changed; the same click with the sidebar
shut changed nothing) and then posted real key messages to the game window
from a background instance: first Escape closed the sidebar with the menu
shut, second opened the menu, third returned to first person.

## BUG-077

The measurement had noticed that `vsync: false` panicked. The verifier
reproduced it without entering the world: a first-frame death. The settings
apply ran at the tail of the frame arm and `set_vsync` reconfigured the
surface there, while the frame's swapchain view was still alive; DXGI refuses
ResizeBuffers with an outstanding back-buffer reference and wgpu's fatal
handler ends the process. Anyone with `vsync: false` saved could not start
the game, and toggling it off in Settings would have persisted the crash.
The mode is now recorded and applied at the next frame's start; proven with
the rig at `vsync: false` living through the menu and the world, and the
pre-fix archive dying at 15 s in the same rig.

## What is next

P2 (the shell permutation), V1 (near-tree frustum cull with the coverage
arithmetic untouched), I1 (the console room as a real vantage, the
screen-emitter hoist), W1 (the water depth prepass, a look call), then
clustered lights, interior culling and a near-tree LOD ladder, all ranked in
`docs/design/frame-cost-arc.md`. Open: Settings clamps that make "vegetation
off" unreachable from the GUI.
