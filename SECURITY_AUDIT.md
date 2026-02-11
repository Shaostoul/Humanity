# Security Audit Report — February 11, 2026

## Summary

**Overall Risk Level: MEDIUM** (improved from previous audit)

The Humanity relay is a public-facing WebSocket chat platform with ~15 users. The codebase shows significant security awareness with good input validation, rate limiting, and proper DM architecture (server-side filtered delivery). However, several medium-severity issues remain around signature verification, XSS vectors, and API authentication gaps.

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 1 |
| Medium | 4 |
| Low | 5 |
| Info | 5 |

---

## Critical Findings

None.

---

## High Findings

### H-1: No Server-Side Ed25519 Signature Verification
- **Severity**: HIGH
- **Component**: `relay.rs` (Chat message handling)
- **Description**: The server accepts `signature` fields on chat messages but never verifies them. Signatures are stored and forwarded to clients, but the server trusts the `from` field based solely on the WebSocket session's identified key — not cryptographic proof. A compromised or modified client can send arbitrary `from` values for non-chat message types, and the signature field provides a false sense of authenticity.
- **Impact**: While the server does enforce `from` based on the session key for chat messages (overwriting it server-side would be the fix), the signature is never validated. If a future code change trusts signatures for authorization decisions, this becomes critical. Currently, the main risk is that clients display "✓ signed" badges without the server confirming validity — a client-side-only verification that can be spoofed by modified clients sending fake signatures.
- **Recommendation**: Add `ed25519-dalek` crate and verify signatures server-side before storing/broadcasting. Reject messages with invalid signatures. Mark messages as `verified: true` in the broadcast only after server verification.
- **Status**: KNOWN (from previous audit, still present)

---

## Medium Findings

### M-1: Unauthenticated Read APIs Expose Message History and User Data
- **Severity**: MEDIUM
- **Component**: `api.rs` — `GET /api/messages`, `GET /api/peers`, `GET /api/stats`, `GET /api/reactions`, `GET /api/pins`
- **Description**: All GET API endpoints are completely unauthenticated. Anyone can poll the full message history, user list, reactions, and pins without any credentials. Only `POST /api/send` and `POST /api/upload` require authentication.
- **Impact**: An attacker can scrape all public channel messages, enumerate all users and their public keys, and monitor the platform without connecting via WebSocket (bypassing connection limits and rate limiting). While this is "public chat," the lack of any access control means automated scraping is trivial.
- **Recommendation**: Add optional API key authentication or at minimum rate limit these endpoints at the nginx level. Consider if message history should require authentication.
- **Status**: NEW

### M-2: GitHub Webhook Secret Passed as Query Parameter
- **Severity**: MEDIUM
- **Component**: `api.rs` — `github_webhook()`
- **Description**: The GitHub webhook endpoint accepts the API secret as a URL query parameter (`?secret=...`). Query parameters are logged in web server access logs, browser history, and may appear in `Referer` headers.
- **Impact**: The API secret could leak through nginx access logs (though IP is stripped, the URL with secret is logged), and through any intermediary proxy logs. Since this is the same `API_SECRET` used for bot API authentication, compromise gives full bot send access.
- **Recommendation**: Use GitHub's native webhook signature verification (`X-Hub-Signature-256` header with HMAC-SHA256) instead of passing the secret as a query parameter. Use a separate secret for the webhook.
- **Status**: NEW

### M-3: Profile Social URLs Not Fully Sanitized for XSS in Client
- **Severity**: MEDIUM
- **Component**: `client/index.html` — `showViewProfileCard()`
- **Description**: While the server validates that URL fields start with `https://` and handles are alphanumeric, the client renders profile URLs directly into `href` attributes using the `esc()` function. The `esc()` function escapes HTML entities but the server-side validation only checks for `javascript:` and `data:` prefixes. A URL like `https://evil.com/"onclick="alert(1)` would be escaped by `esc()`, BUT the profile card uses string concatenation like `'<a href="' + esc(url) + '"'` — the `esc()` function does escape quotes to `&#39;` (single quotes) but does NOT escape double quotes, only relying on `textContent` which doesn't handle attribute context.
- **Impact**: Potential stored XSS through crafted profile URLs. The `esc()` function escapes `<`, `>`, `&`, and `'` but NOT `"`. A URL containing double quotes could break out of the `href` attribute.
- **Recommendation**: Fix `esc()` to also escape double quotes (`"`→`&quot;`). Better yet, use DOM APIs (`setAttribute`) instead of string concatenation for href values. The server should also validate URLs more strictly (no quotes, no spaces).
- **Status**: NEW

### M-4: Upload Authentication Bypass — Key Parameter Not Cryptographically Verified
- **Severity**: MEDIUM
- **Component**: `api.rs` — `upload_file()`
- **Description**: The upload endpoint authenticates users by checking if the provided `?key=` parameter matches a currently connected peer. However, public keys are broadcast to all connected users via `peer_list` and `full_user_list`. Any connected user can upload files using another user's public key, bypassing per-user upload limits (4 image FIFO).
- **Impact**: A malicious user could exhaust another user's upload quota by uploading with their key, causing the victim's images to be deleted. Also allows unverified users to upload if they know a verified user's key.
- **Recommendation**: Require uploads to go through WebSocket (where identity is already established) or require a signed challenge-response for the upload endpoint. At minimum, generate per-session upload tokens.
- **Status**: NEW

---

## Low Findings

### L-1: Bot Keys (`bot_*` prefix) Bypass Ban and Name Checks
- **Severity**: LOW
- **Component**: `relay.rs` — identify handler
- **Description**: Keys starting with `bot_` skip ban checks and name registration validation. While bot messages come through the authenticated API, the WebSocket identify path also checks for this prefix. A malicious client could set their public key to `bot_something` to bypass bans.
- **Impact**: Low — a banned user could reconnect with a `bot_*` key, but they wouldn't have a registered name and the bot API requires `API_SECRET` for sending messages. However, they could observe the chat.
- **Recommendation**: Validate that `bot_*` keys can only connect through the API path, not through WebSocket identify. Or remove the `bot_` prefix exception from WebSocket connections entirely.
- **Status**: NEW

### L-2: Rate Limit State Not Persisted Across Restarts
- **Severity**: LOW
- **Component**: `relay.rs` — `RateLimitState`
- **Description**: Rate limiting uses in-memory `Instant` timestamps. After a server restart, all rate limits reset, allowing a burst of messages. The `first_seen` field (used for new account slow mode) also resets, so a 10-minute-old account gets treated as new again (but more permissively since the Fibonacci index resets to 0).
- **Impact**: Minor DoS window after restarts. An attacker could force a restart (if they know how to trigger a crash) to reset rate limits.
- **Recommendation**: Accept this as a known limitation. For the current scale (~15 users), this is fine. If scaling, consider persisting rate limit state or using a sliding window approach.
- **Status**: NEW

### L-3: No Content-Security-Policy Header
- **Severity**: LOW
- **Component**: nginx config
- **Description**: The nginx configuration sets `X-Content-Type-Options`, `Strict-Transport-Security`, and `Referrer-Policy`, but lacks a `Content-Security-Policy` header. The client loads Twemoji from a CDN (`cdn.jsdelivr.net`), making a strict CSP slightly complex but not impossible.
- **Impact**: Without CSP, any XSS vulnerability has full access to execute arbitrary scripts, access IndexedDB (private keys), and exfiltrate data.
- **Recommendation**: Add a CSP header: `default-src 'self'; script-src 'self' 'unsafe-inline' cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data: https://cdn.jsdelivr.net; connect-src 'self' wss://chat.united-humanity.us wss://united-humanity.us;`. The `unsafe-inline` for scripts is needed due to the inline `<script>` blocks.
- **Status**: NEW

### L-4: Lockdown State Not Persisted
- **Severity**: LOW
- **Component**: `relay.rs` — `lockdown` field
- **Description**: The lockdown state (registration lock) is stored in memory only. After a server restart, lockdown is always `false`, regardless of whether an admin had manually enabled it.
- **Impact**: If the server restarts while lockdown is active, registration opens up. The auto-lockdown mitigates this somewhat (no mods online → auto-lock after 30s), but a manual lockdown intended to be persistent would be lost.
- **Recommendation**: Persist lockdown state to the database.
- **Status**: NEW

### L-5: Emoji Reaction Validation Incomplete
- **Severity**: LOW
- **Component**: `relay.rs` — Reaction handler
- **Description**: Reactions are validated to be ≤32 bytes and exclude certain characters (`'`, `"`, `<`, `>`, `\`, `&`). However, this allows arbitrary Unicode sequences up to 32 bytes, which could include invisible characters, RTL override characters, or very long grapheme clusters that render poorly.
- **Impact**: Minor UI disruption. Not exploitable for XSS since the characters are escaped on render.
- **Recommendation**: Restrict reactions to a whitelist of known emoji or validate against Unicode emoji ranges.
- **Status**: NEW

---

## Informational

### I-1: Private Key Storage in IndexedDB
- **Severity**: INFO
- **Component**: `client/index.html` — `getOrCreateIdentity()`
- **Description**: Ed25519 private keys are stored in IndexedDB with `extractable: false` when Web Crypto is available. This is the correct approach — the keys are non-extractable CryptoKey objects. When Ed25519 is not supported, a random hex key is stored in `localStorage` (no signing capability).
- **Impact**: Keys are as secure as the browser's Web Crypto implementation allows. A browser extension or XSS attack could still use the key to sign messages (but not extract it).
- **Recommendation**: This is the best practice for browser-based crypto. No change needed.
- **Status**: KNOWN (acceptable)

### I-2: Broadcast Channel Messages to All — Client-Side Channel Filtering
- **Severity**: INFO
- **Component**: `relay.rs` — broadcast loop
- **Description**: Chat messages are broadcast to ALL connected WebSocket clients regardless of which channel they're viewing. The client filters by `activeChannel`. This means all users receive all channel messages.
- **Impact**: No privacy impact since channels are public. Minor bandwidth overhead. A modified client could see messages from all channels simultaneously.
- **Recommendation**: For current scale, this is fine. At scale (100+ channels), add server-side channel subscription filtering to reduce bandwidth.
- **Status**: KNOWN (acceptable at current scale)

### I-3: DM Privacy — Server-Side Filtering (FIXED from previous audit)
- **Severity**: INFO
- **Component**: `relay.rs` — DM broadcast loop filtering
- **Description**: DMs are now properly filtered server-side in the broadcast loop. The `Dm` message type is only delivered to the `to` field recipient. `DmHistory` and `DmList` messages also check `target` fields. This is a significant improvement from the previous audit where DMs were broadcast to all clients.
- **Impact**: DMs are private between sender and recipient at the transport layer.
- **Recommendation**: None — this is correctly implemented.
- **Status**: FIXED (since last audit) ✅

### I-4: Service Runs as Root (Implied)
- **Severity**: INFO
- **Component**: systemd service (`humanity-relay.service`)
- **Description**: The systemd unit file doesn't specify `User=` or `Group=`, meaning the service likely runs as root (the default for system services).
- **Impact**: If the relay process is compromised, the attacker has root access to the VPS.
- **Recommendation**: Add `User=humanity` and `Group=humanity` to the service file. Create a dedicated user. Also add `ProtectSystem=strict`, `ProtectHome=true`, `ReadWritePaths=/opt/Humanity/crates/humanity-relay/data`.
- **Status**: NEW

### I-5: Server Version Exposed to All Clients
- **Severity**: INFO
- **Component**: `relay.rs` — `PeerList` message
- **Description**: The `server_version` is sent to all clients in the `peer_list` message, and the client uses it to trigger auto-reload on version changes. This exposes the exact build version to all users.
- **Impact**: Minor information disclosure. An attacker could identify the exact build and target known vulnerabilities.
- **Recommendation**: Accept for now — the auto-reload feature is useful during active development.
- **Status**: NEW

---

## Positive Security Observations

1. **SQL Injection: SAFE** — All SQLite queries use parameterized queries (`params![]` macro). No string concatenation in SQL. ✅
2. **DM Privacy: FIXED** — DMs are now server-side filtered, not broadcast to all. Major improvement. ✅
3. **WebSocket Security: GOOD** — Origin checking, connection limits (500 max), identify timeout (30s), max frame/message sizes. ✅
4. **Rate Limiting: COMPREHENSIVE** — Fibonacci backoff for chat/DMs, new account slow mode, typing rate limit, profile update rate limit, upload rate limit (nginx + app level). ✅
5. **File Upload: STRONG** — Type validation via Content-Type AND magic bytes, 5MB size limit, per-user 4-image FIFO, 500MB global disk limit, filename sanitization (no path traversal), only verified users can upload. ✅
6. **Name Validation: GOOD** — ASCII-only names prevent homoglyph attacks, 24-char max, no special characters. ✅
7. **Input Validation: GOOD** — Message length limits, bio length limits, socials JSON validation, channel name validation, emoji sanitization. ✅
8. **nginx Configuration: SOLID** — TLS 1.2+, HSTS, no server tokens, privacy-preserving logs (no IPs), upload rate limiting, UFW firewall with minimal open ports. ✅
9. **CORS: PROPERLY CONFIGURED** — Explicit allow-list of origins for both HTTP CORS and WebSocket Origin header. ✅
10. **Moderation: WELL-DESIGNED** — Role hierarchy (admin > mod > verified > user > muted), ban persistence, kick mechanism, auto-lockdown when no staff online, report system with rate limits. ✅
11. **Link/Invite Codes: SECURE** — CSPRNG generation, time-limited, one-time use, proper cleanup of expired codes. ✅
12. **HTML Escaping**: The `esc()` function in the client escapes `<`, `>`, `&`, and `'` for most contexts. ✅ (but see M-3 for double-quote gap)

---

## Recommendations Priority

1. **HIGH — M-3: Fix `esc()` double-quote escaping** — Quick fix, prevents potential stored XSS. Add `&quot;` escaping and use DOM APIs for attribute setting.
2. **HIGH — H-1: Server-side signature verification** — Core security feature for a crypto-identity platform. Add `ed25519-dalek` verification.
3. **MEDIUM — M-4: Fix upload authentication** — Generate per-session upload tokens to prevent key impersonation.
4. **MEDIUM — M-2: Fix GitHub webhook auth** — Switch to HMAC-SHA256 signature verification.
5. **MEDIUM — M-1: Add rate limiting to GET APIs** — nginx `limit_req` on `/api/messages`, `/api/peers`, etc.
6. **LOW — L-3: Add CSP header** — Defense-in-depth against XSS.
7. **LOW — I-4: Run service as non-root** — Standard hardening.
8. **LOW — L-1: Remove bot_ prefix exception from WebSocket** — Minor hardening.
9. **LOW — L-4: Persist lockdown state** — Operational reliability.

---

*Audit performed on February 11, 2026. Covers all server-side Rust source (`main.rs`, `relay.rs`, `storage.rs`, `api.rs`), client-side JavaScript/HTML (`index.html`, `shell.js`), and VPS infrastructure (nginx, systemd, UFW).*
