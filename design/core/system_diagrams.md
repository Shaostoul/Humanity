# Humanity System Diagrams (ASCII Draft v0.1)

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
