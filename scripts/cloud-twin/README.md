# Cloud march twins (1-D models of the shipped ray march)

Written by the v0.1271 estimator assessment (7-agent workflow, 2026-09-01) and
kept because they are the evidence for the sample-anchored march:

- `edge_bias.js`, `edge_bias2.js`: an IDEAL jittered point-sample march is
  unbiased in step length h for a cliff (E[tau] exact for h 23..928 m); only
  exp(-tau) carries a second-order Jensen term. So widening the edge was
  never the lever.
- `march_twin.js`, `march_twin2.js`, `sweep_l0.js`: a twin of the SHIPPED
  step law (coarse floor, tau-0.75 MFP refine, coarse-entry backtrack,
  trapezoid, jittered comb, SDF stride) measured per-pixel sd and bias across
  the frozen jitter phase for ramps, thin chords, warp hash, interior
  turbulence and approach distance. Result: the shipped march is biased to
  FIRST order in h because the march position and the sample position are two
  different variables and every endpoint rule mixes them. Sample-anchored:
  bias -0.43 -> -0.02, sd 0.25 -> 0.087, identical across approach distances.

Run with `node scripts/cloud-twin/<file>.js`. These are dev tooling, permanent
(the forever-development rule); the numbers above are what the map_diag 6
entry-depth channel and the jitter-factorial gates are checked against.
