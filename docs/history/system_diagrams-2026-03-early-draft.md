# Humanity System Diagrams (ASCII Draft v0.1)

> **Archived (moved from docs/design/ 2026-06-30).** This is an early (March 2026)
> architecture sketch predating the full post-quantum identity cutover. It describes a
> generic "discovery/bootstrap service + device-linking" model that was never built and
> does not match the actual shipped architecture (no home servers, no accounts, no device
> linking; Dilithium3 identity derived from a BIP39 seed; relay-mediated E2EE DMs). For the
> current, accurate crypto + federation model read the "Cryptography" section of
> `CLAUDE.md`. Kept here only as a historical record of early ecosystem thinking.

## 1) High-Level Ecosystem View

```text
+-------------------+ +---------------------+ +----------------------+
| Web Client | | Native .rs App | | Immersive/Game UI |
| (united-humanity) | | (full capability) | | (Project Universe UX)|
+---------+---------+ +----------+----------+ +----------+-----------+
| | |
+----------------------------+-----------------------------+
|
v
+-------------------------------+
| Core Application Services |
| identity | messaging | market |
| presence | policy | events |
+---------------+---------------+
|
v
+-------------------------------+
| Protocol + Security Layer |
| E2EE | session keys | routing |
| peer auth | relay fallback |
+---------------+---------------+
|
+--------------+--------------+
| |
v v
+------------------------+ +------------------------+
| Discovery/Bootstrap | | Relay Infrastructure |
| minimal metadata | | encrypted payload pass |
+------------------------+ +------------------------+
2) Messaging Path (Direct + Relay Fallback)
Sender Client
|
| 1) Resolve peer via discovery/bootstrap
v
Try Direct P2P Handshake ------------------- success -------------------> Direct Encrypted Channel
|
| fail / unavailable
v
Relay Fallback (Encrypted Payload Tunnel) ---> Receiver Client
(Relay routes packets but cannot decrypt)
3) Trust Boundary Diagram
[User Device Boundary]
- private keys
- decrypted message content
- local plaintext cache (if enabled)

[Network Boundary]
- hostile by default assumption
- encrypted transport required

[Service Boundary]
- discovery service
- relay service
- account metadata service
(should not hold message plaintext)

[Admin/Moderation Boundary]
- reports, abuse metadata, policy actions
- no blanket content visibility by default
4) Identity & Device Linking Flow (Simplified)
Primary Device
|
| create account + root identity keys
v
Account Service (stores public material / metadata only)
|
| generate one-time device link token
v
Secondary Device
|
| authenticated link request + signed proof
v
Primary Device approves
|
v
Secondary device added to trusted device set
5) Product Surface Integration
[Web]
- onboarding
- light messaging
- discovery/docs

[Native App]
- full secure messaging
- key/device management
- advanced settings

[Game/Immersive]
- social hubs
- service kiosks
- missions/events
- shared identity/presence

---
