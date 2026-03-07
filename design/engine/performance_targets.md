# Performance Targets

## Target Hardware Classes
- Class A (Ultra Low): older iGPU / low VRAM
- Class B (Mainstream): mid desktop/laptop GPU
- Class C (High): modern dedicated GPU

## FPS Targets
- Class A: 30 FPS stable (gameplay complete)
- Class B: 60 FPS stable
- Class C: 90+ FPS target (where possible)

## Frame Time Budgets
- 30 FPS: 33.3 ms
- 60 FPS: 16.7 ms
- 90 FPS: 11.1 ms

## Requirements
- No gameplay/system logic removed for low tier.
- Visual degradations only (materials/effects/draw distance).
- UI responsiveness target < 100 ms interaction latency.

## Profiling Gates
- Scene load spikes bounded by async streaming.
- Long-frame telemetry captured in release builds.
- Regression threshold alerts in CI perf suite.

## Needs Decision
- Minimum supported GPU API level.
- Official min-spec CPU/RAM targets.