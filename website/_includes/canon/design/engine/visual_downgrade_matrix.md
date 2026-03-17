# Visual Downgrade Matrix

## Purpose
Define exactly what changes per quality tier so behavior is predictable.

| Feature | Ultra Low | Low | Medium | High | Ultra |
|---|---|---|---|---|---|
| Lighting Model | Unlit/Flat | Basic PBR | Full PBR | Full PBR+ | Full PBR+ |
| Shadows | Off/Very low | Single cascade | Multi cascade | High quality | Max quality |
| Reflections | Off | Probe only | Probe+SSR limited | SSR | SSR+advanced |
| Post Effects | Off | Minimal | Moderate | Full | Full+ |
| Particles | Minimal | Reduced | Standard | High | Max |
| Draw Distance | Short | Medium-short | Medium | Long | Max |
| Material Detail | Flat color | Low detail | Standard | High | Highest |

## Rule
- Gameplay, interaction, networking, and quest systems remain identical across tiers.
- Only rendering fidelity changes.

## Needs Decision
- Keep SSR at Medium+ or High+ only.
- Whether ultra-low defaults to single-color mode automatically on first launch for Class A devices.