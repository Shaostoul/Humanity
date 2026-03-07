# Asset Budget Policy

## Objective
Keep package size small while preserving full functionality and visual clarity.

## Principles
- Mechanics > graphics in fallback tiers.
- Reuse over uniqueness where possible.
- Procedural detail over giant texture libraries.

## Budget Buckets (Initial)
- Core executable + runtime libs: target <= 250 MB
- Baseline content pack: target <= 2.5 GB
- Optional HD pack(s): separate downloadable bundles
- Audio base pack: <= 400 MB

## Texture Policy
- Avoid large unique 4K texture sets by default.
- Prefer 512/1K shared texture sets + shader variation.
- Use virtual texturing only if profiling justifies complexity.

## Mesh Policy
- Author modular kit pieces.
- Aggressive instancing for repeated architecture.
- Generate variants procedurally at runtime.

## LOD Policy
- Mandatory LODs for all medium+ assets.
- HLOD clusters for large scenes.

## Needs Decision
- Max initial install size target (hard cap).
- Whether HD packs are first-party hosted only or mirror-supported.