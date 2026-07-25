# Shader organization: why one megashader, and the plan to split its SOURCE

Status: design accepted 2026-07-25 (overnight backlog #9), implementation
deferred to a dedicated session. Operator question: "Is there any particular
reason you didn't break each shader down to its own file? Wouldn't a bunch of
individual shader files be better than a single monolithic file?"

## The honest answer

There are two different things called "a shader" here, and they have
different rules:

1. **What the GPU compiles.** wgpu compiles ONE WGSL module per pipeline.
   Everything a pipeline's vertex + fragment stages call - every function,
   constant, and binding declaration - must be in that one module. WGSL has
   no include statement and naga (the compiler) has no preprocessor. So at
   the GPU level there is always exactly one big text blob per pipeline; the
   only question is how that blob is assembled.

2. **What humans edit.** Today the blob IS the source file:
   `assets/shaders/pbr_simple.wgsl` (~3,500 lines) carries the PBR core plus
   the atmosphere, clouds (three quality tiers), ocean, sky-view hybrid,
   water shading, vegetation cards, ground textures, and shadows, because
   they all share bindings, helpers, and the material-type dispatch in
   `fs_main`.

The monolith was the zero-tooling choice: no build step, no concatenation
order to get wrong, one file to hot-reload (the runtime watches it and
rebuilds pipelines in ~3 s), one file for the loader's embedded fallback,
and one file for the source-scanning tests (the ocean CPU-twin lockstep test
parses it by path, and the lint suite scans it).

Where the operator's instinct is right: 3,500 lines is past the point where
one file serves human navigation, focused diffs, or parallel agents editing
disjoint shader domains without three-way-merge hazards (the repo's own
throughput notes call this file a merge funnel). Standalone passes are
already separate files (`sky_view_lut.wgsl`, `particles.wgsl`,
`billboard_bake` inline) - the megashader is only big because its domains
genuinely share one pipeline.

## Accepted design: split the SOURCE, keep the MODULE

`assets/shaders/pbr/` gains numbered parts, concatenated in name order at
load time into the same single module the pipelines compile today:

- `00-bindings.wgsl` - groups 0-3 declarations, shared consts, vertex IO
- `10-lighting-core.wgsl` - PBR/GGX, shadows, light loop + tile lists
- `20-atmosphere.wgsl` - scattering, LUT hybrid, aerial perspective
- `30-clouds.wgsl` - the three cloud variants + noise helpers
- `40-water.wgsl` - ocean waves, water shading, sea ice
- `50-surface.wgsl` - planet surface (type 12), vegetation cards, ground
  textures
- `90-main.wgsl` - vs_main + fs_main dispatch (the if-chain stays whole;
  function bodies cannot straddle files but functions separate cleanly)

Loader changes (all three read sites must move together):
1. Runtime load: read the directory in name order, join, compile (same
   `create_shader_module` call).
2. Hot-reload: watch the directory, not one file; any part changing rebuilds
   the same pipeline set.
3. Embedded fallback: `include_str!` each part, join at compile time.

Test changes: the ocean lockstep test and the shader lints read the
concatenation (a tiny shared helper in `src/renderer/` returns the joined
source both for the compiler and for tests) - they must never go
path-by-path, or a constant moved between parts would dodge the lockstep.

## Why this is deferred rather than done tonight

The v0.782-784 incident (three consecutive unbootable releases from a
device-limit rejection no test caught) sets the verification bar for
megashader surgery: full release build, boot the exe, probe-sweep the
vantage suite, and hot-reload exercise - per split stage, not once at the
end. That is a focused daytime session, not the tail of a 14-release night.
The split is mechanical (move functions, never edit them), so the risk is
purely procedural - which is exactly why it gets fresh-session discipline.
