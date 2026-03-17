# First-Person Controller Contract

## Goals

- deterministic movement and stamina behavior
- renderer-agnostic controller core
- offline-first support with identical logic in host/dedicated modes

## State

- position (x,y,z)
- yaw/pitch
- stamina
- movement speed profile

## Inputs

- move_forward/back/left/right
- look_delta
- sprint toggle
- jump (future)

## Outputs

- new transform
- stamina delta
- movement event flags

## Constraints

- clamp pitch to avoid invalid camera flips
- bounded speed multipliers
- deterministic movement integration

## Integration points

- CLI adapter: textual move/look commands
- wgpu adapter: keyboard/mouse mapping
- network adapter: input replication or state snapshots
