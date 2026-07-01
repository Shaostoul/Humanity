# Network

Protocol and architecture specifications for the Humanity Network, covering transport, sync, federation, security, and real-time communication.

## Contents

Real, shipped protocol/infrastructure docs (cross-checked against `src/relay/`,
`src/net/`, and CLAUDE.md as of 2026-06-30):

- `file_sharing.md`, Peer-to-peer file sharing (forward design, not yet implemented, see status banner in the file)
- `native_voice.md`, Native (Rust desktop) voice chat implementation reference (shipped)
- `object_format.md`, Signed-objects wire format underlying P2P groups, governance, and credentials (shipped)
- `scope.md`, Network scope, surfaces, and non-goals
- `server_federation.md`, Server federation design (trust tiers + profile gossip shipped; signed root registry is forward design)
- `snapshot_delta_recovery.md`, Multiplayer snapshot/delta desync recovery
- `social_graph.md`, Follows/friends/DM model (follows + DM E2EE shipped; P2P routing tiers are forward design)
- `transport_security.md`, Transport and protocol-layer security (replay resistance, abuse controls)
- `unified_comms_sidebar_plan.md`, Unified communications sidebar plan
- `voice_video_streaming.md`, Voice, video, and livestreaming protocols (voice shipped; video/livestreaming forward design)
- `web_client_constraints.md`, Web client technical constraints

Sixteen other files in this folder (`api_and_endpoints.md`,
`authority_model.md`, `architecture.md`, `hybrid_replication.md`,
`indexing.md`, `membership_and_roles.md`, `memory_sync.md`,
`notifications_model.md`, `object_type_schemas.md`, `offline_first_sync.md`,
`protocol_versioning.md`, `realtime_relay_protocol.md`,
`realtime_transport.md`, `space_creation_and_governance_objects.md`,
`space_policy_format.md`, `tailnet_onboarding.md`) were removed 2026-06-30 —
they described a "spaces"/generic-object-forum/session-token/Tailscale design
that was never built. The real system that got shipped instead is P2P groups
(`docs/design/p2p-groups.md`), governance proposals/votes and credentials
(`src/relay/storage/governance.rs`, `credentials.rs`), and simple
channel/server membership (`server_members` table) — see `object_format.md`
above for the actual signed-object substrate those use.
