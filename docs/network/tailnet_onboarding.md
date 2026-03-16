# Tailnet Onboarding for Private Co-op

Tailnet mode uses an identity-gated private network (e.g., Tailscale) to reduce direct internet exposure during alpha/beta phases.

## Why use tailnet mode

- avoids public port exposure for most players
- easier NAT traversal in many home setups
- strong identity controls (device/user approval)
- good stepping stone before public direct-internet hosting

## High-level flow

1. Host enables "Tailnet Mode" in game host settings.
2. Game checks whether local node is connected to tailnet.
3. Host generates session invite key.
4. Guests must be both:
   - approved on tailnet
   - authorized by game invite/allowlist

## Approval process

There are two independent approvals:

- **Network approval** (tailnet admin side)
  - device/user admitted to tailnet
- **Game approval** (session side)
  - invite key or allowlisted identity accepted

Both are required.

## Can approval be handled in-game?

Partially.

- In-game can guide users and show status checks.
- In-game can open deep links to tailnet auth pages.
- Final tailnet membership approval still occurs in tailnet admin controls.

## Recommended UX

Host panel:
- tailnet connected: yes/no
- tailnet name
- connected peers visible to session
- "copy invite" and "approve in game allowlist"

Join panel:
- tailnet connected: yes/no
- host reachable: yes/no
- invite validity: yes/no

## Security notes

- Tailnet mode should still use invite expiry and replay protection.
- Do not assume tailnet membership alone is sufficient authorization.
