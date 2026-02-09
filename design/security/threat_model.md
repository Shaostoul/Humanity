# Threat Model

## Purpose
Enumerate realistic threats and define security goals and non-goals for longevity and simplicity.

## Assets to protect
- Account integrity (prevent impersonation)
- Private space confidentiality (message contents)
- Moderation legitimacy (prevent forged authority)
- Availability (resist spam and disruption)
- Data integrity (prevent tampering)
- User safety (reduce harassment and abuse impact)

## Adversaries
- Spammers and automated botnets
- Coordinated harassment groups
- Malicious peers in peer-to-peer transport
- Compromised client devices
- Compromised moderator accounts
- Compromised server infrastructure
- Network adversaries observing or manipulating traffic

## Primary threats

### Identity and account threats
- Credential stuffing
- Phishing
- Session theft
- Device compromise
- Key exfiltration from weak client storage

### Abuse threats
- Sybil attacks (many fake identities)
- Spam floods
- Brigading and harassment
- Impersonation

### Data threats
- Tampering with posts/messages
- Replay of old messages
- Unauthorized access to private spaces
- Metadata exposure (who talked to whom)

### Transport threats
- NAT-based connectivity failures
- Relay abuse (flood, resource exhaustion)
- Peer poisoning (serving invalid blocks)

## Security goals
- All user actions are verifiable by signature.
- Server cannot impersonate users without client compromise.
- Private spaces support end-to-end confidentiality where feasible.
- Moderation actions are attributable and verifiable.
- Clients can enforce moderation decisions offline.
- System degrades safely under partial connectivity.

## Non-goals (explicit)
- Guaranteed deletion of replicated content across independent nodes.
- Perfect anonymity.
- Complete elimination of metadata exposure.
- Trustless global consensus.

## Mandatory mitigations
- Signed actions and signed moderation logs.
- Short-lived sessions and device revocation.
- Rate limiting at client and relay edges.
- Quarantine pathways for unknown identities in open spaces.
- Encryption for private spaces, with key rotation on membership changes where feasible.
