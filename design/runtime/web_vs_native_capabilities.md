# Web vs Native Capability Boundary

## Can webpage trigger native app launch?

Yes, with constraints.

## Options

1. **Custom URL protocol handler** (recommended)
   - e.g., `humanity://launch/game`
   - browser prompts and opens installed native app
   - secure allowlist of actions only

2. **Local loopback launcher bridge**
   - localhost service receives signed launch requests
   - web UI sends request to local bridge
   - bridge validates and executes allowlisted command

3. **Direct browser launch without helper**
   - not reliable/safe across browsers; not recommended

## What should remain web-accessible

- account/profile management
- docs/knowledge systems
- high-level session browser
- cosmetic editor UI (if backed by shared data model)
- low-frequency social and management features

## What should be native-app first

- custom wgpu rendering
- first-person runtime with low-latency input
- heavy physics/simulation loops
- offline authoritative play
- direct file/hardware/network controls not suitable for browser sandbox

## Recommended approach

- web app as control plane + onboarding
- native app as gameplay runtime
- shared data contracts so profile/cosmetics are consistent across both
