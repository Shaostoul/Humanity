# Security & Privacy Architecture (Draft v0.1)

## 1. Security Goals

- Confidentiality of user communications
- Integrity of messages and transactions
- Authenticity of user/device identities
- Resilience against surveillance and metadata over-collection
- Safe recovery paths without undermining encryption guarantees

## 2. Threat Model (Initial)

Protect against:
- Passive network observers
- Malicious relay operators
- Account takeover attempts
- Client tampering / replay / session hijack
- Metadata profiling at scale

Assume:
- Some infrastructure may be hostile or compromised
- Endpoints are the strongest and weakest points simultaneously

## 3. Core Security Principles

1. **E2EE by default** for all private communications.
2. **Data minimization**: collect/store only what is strictly required.
3. **Least privilege** for services and device capabilities.
4. **Forward secrecy** for message sessions.
5. **Secure-by-default UX** (not hidden behind advanced settings).

## 4. Key Management Strategy (High-Level)

- Device-generated keypairs
- Per-conversation/session keys with rotation
- Signed device linking/unlinking
- Optional hardware-backed key storage where available
- Recovery flow that does not expose plaintext history

## 5. P2P Transport Security

- Authenticated peer handshake
- Encrypted transport channels
- Relay fallback when direct path unavailable
- Relay cannot decrypt payloads
- Opportunistic transport upgrades (relay -> direct P2P) when network conditions allow

## 6. Metadata Protection

- Store minimal routing metadata
- Short retention policies for operational logs
- Delayed/batched non-critical telemetry
- Avoid third-party ad/analytics SDKs in private surfaces
- Offer “privacy mode” with stricter metadata reduction

## 7. Platform Hardening

- Signed client releases
- Secure update pipeline
- Dependency auditing and SBOM tracking
- Secrets management and key rotation policy
- Rate-limits and abuse protections that do not require broad surveillance

## 8. Trust & Safety (without surveillance creep)

- User-level controls: block, mute, report
- Community moderation controls for shared spaces
- Cryptographic evidence trails where appropriate
- Escalation pathways that respect user privacy and legal constraints

## 9. Compliance & Governance (Future Work)

- Define data jurisdiction strategy
- Define lawful request handling process
- Publish transparency reports
- External audits for cryptography and infrastructure