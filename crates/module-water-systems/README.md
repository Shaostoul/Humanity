# module-water-systems

Water sourcing, treatment, routing, and risk primitives.

## Includes

- water quality state + potability thresholding
- treatment efficacy application
- routing with weighted quality merge
- shortage + contamination risk report

## Quick test

```bash
cargo test -p module-water-systems
```

## Run example scenario

```bash
cargo run -p module-water-systems --example drought_contamination
```

## Source design spec

- `design/modules/module-water-systems.md`
