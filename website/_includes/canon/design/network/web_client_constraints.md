# Web Client Constraints

## Purpose
Define rules and limitations for the browser client so it remains correct and predictable.

## Constraints
- Browsers cannot reliably run background tasks.
- Storage is quota-limited and may be cleared by the user agent.
- Network connectivity may be suspended when tabs sleep.

## Required behaviors
- Relay-first for realtime.
- Store encrypted local state in IndexedDB where possible.
- Treat local cache as best-effort; server feeds remain the recovery source.
- Never require the web client to accept inbound connections.

## Security requirements
- Local key material must be protected using WebCrypto.
- Do not log decrypted private payloads.
- Use strict content security policy in the web application.
