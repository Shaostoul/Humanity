# Humanity MVP Feature Spec (Draft v0.1)

## 1) MVP Objective

Deliver a **daily-usable core** of the Humanity ecosystem with:
- Unified identity
- Secure messaging
- Basic discovery/presence
- Web + native app continuity

MVP is not “everything.” MVP is the smallest product that proves:
1. People can use it reliably
2. Privacy architecture works in practice
3. It provides value beyond existing chat tools

---

## 2) In-Scope Features (Must Have)

## 2.1 Account + Identity
- User account creation and sign-in
- Device registration (first device + secondary device link)
- Basic profile:
- Display name
- Avatar (optional)
- Short bio/status (optional)

**Acceptance**
- User can create account and access it from at least one web and one native client.
- Device link/unlink works without data corruption.

## 2.2 Secure Messaging (Core)
- 1:1 chats with E2EE
- Small group chats (up to defined limit, e.g. 20) with E2EE
- Text messages only in MVP (attachments optional/stretch)
- Message send/receive status:
- Sent
- Delivered
- Local message history on device

**Acceptance**
- Messages are encrypted in transit and at rest (where stored).
- Relay operator cannot read plaintext.
- Users can reliably chat under normal network conditions.

## 2.3 Presence + Contacts
- Add contact by username/invite
- Contact list
- Basic presence:
- Online
- Offline
- Last seen (configurable visibility)

**Acceptance**
- Users can discover/add approved contacts.
- Presence visibility respects privacy settings.

## 2.4 Relay/Connectivity Baseline
- Direct connection attempt (P2P path) where possible
- Relay fallback when direct path fails
- Automatic reconnect/retry behavior

**Acceptance**
- Messaging remains usable under common NAT/firewall conditions.
- Failure modes are visible and recoverable.

## 2.5 Safety/Control Basics
- Block user
- Mute notifications per chat
- Report user/chat (basic abuse signal channel)

**Acceptance**
- Blocked users cannot message blocker.
- Reports are logged for moderator/admin review pipeline.

## 2.6 Cross-Surface Continuity
- Same account usable on:
- Website (light client)
- Native `.rs` app (full client)

**Acceptance**
- User identity and contact graph are consistent across both surfaces.

---

## 3) Out of Scope for MVP (Later Phases)

- Full marketplace transactions
- In-game immersive shell integration
- Rich media pipelines at scale
- Federation/community-hosted relays
- Complex social graph recommendations
- Voice/video calling
- XR/AR interfaces
- Tokenized economies / advanced fintech layers

---

## 4) Non-Functional Requirements (MVP)

## 4.1 Security & Privacy
- E2EE for private chat
- Minimal metadata retention
- No ad-tech trackers in private surfaces
- Signed builds and secure update path

## 4.2 Reliability
- Target message delivery success under normal conditions >= 99% (alpha target can be lower with tracking)
- Graceful reconnect after temporary disconnects

## 4.3 Performance
- Message send latency target:
- Direct/P2P path: “near realtime” (sub-second typical)
- Relay path: low-latency best effort
- App startup and chat-load performance budget defined during implementation

## 4.4 Usability
- New user can complete onboarding and send first secure message in <5 minutes without docs

---

## 5) User Stories (MVP Critical Path)

1. As a new user, I create an account and verify my device.
2. As a user, I add a trusted contact.
3. As a user, I send encrypted messages and receive replies in realtime.
4. As a user, I use the same account on web + native app.
5. As a user, I block/report abusive behavior.

---

## 6) Release Gates

## Alpha Gate
- Core messaging works end-to-end for internal users
- Critical crypto/security checks pass
- Known issues documented with mitigations

## Beta Gate
- Stable onboarding
- Reliability metrics acceptable
- Basic moderation flow active

## Public MVP Gate
- Incident response process ready
- Security review complete
- Core docs and support paths available