# Security Audit Report

**Date:** 2026-02-11  
**Auditor:** Heron (AI security review)  
**Scope:** Full codebase audit of humanity-relay + web client + nginx config  
**Codebase:** ~8,900 lines across 7 files  
**Previous Audit:** 2026-02-10 (25 findings)

---

## Executive Summary

This is a **fresh, complete re-audit** of the Humanity Relay codebase following significant remediation work since the February 10 audit. The codebase security posture has improved substantially — **all three CRITICAL findings from the previous audit are now fixed**, along with most HIGH-severity issues. The Bot API is now authenticated (fail-closed), CORS is properly restricted, WebSocket origin checks and message size limits are in place, CSP headers are deployed, and the client-side XSS vectors through `onclick` handlers have been eliminated via data attributes and event delegation.

**Overall Risk Level: LOW-MEDIUM** — The application is now suitable for public launch. The remaining findings are defense-in-depth improvements and hardening suggestions, not blockers. There is one new MEDIUM finding (DM privacy via broadcast channel) and a few remaining items from the previous audit that warrant attention but are not urgent.

---

## Findings

### CRITICAL

**None.** All previous CRITICAL findings have been resolved.

---

### HIGH

#### H-1: DM Messages Broadcast to All Connected Clients (Privacy Concern)

- **Category:** Privacy / Authorization
- **Location:** `relay.rs` lines ~770-810 (broadcast send task), `relay.rs` DM handler

**Description:**  
Direct messages are sent via the shared broadcast channel (`broadcast_tx.send(dm_msg)`). The send task filters by checking `if to != &my_key_for_broadcast { continue; }` — meaning every DM passes through every connected client's send loop, even though it's filtered before transmission. This is architecturally correct (DMs are filtered before sending over the wire), but introduces risk:

1. A bug or regression in the send-loop filter could leak DMs to all clients
2. The broadcast channel buffer (256 messages) holds DM content in memory accessible to all subscriber tasks
3. DmHistory and DmList also flow through the broadcast channel with similar target-based filtering

The filtering logic is correctly implemented today, but the pattern is fragile for a privacy-critical feature.

**Impact:** No current data leak, but the architecture makes DM privacy depend on correct filtering in a shared broadcast path rather than isolated delivery.

**Recommendation:**  
Consider a dedicated per-client sender channel (e.g., `mpsc::Sender` per connection) for private messages (DMs, DmHistory, DmList, Private). This ensures DM content never enters the shared broadcast buffer. This is a refactor, not an emergency fix.

---

#### H-2: `server_tokens` Not Disabled in nginx

- **Category:** Information Disclosure / Hardening
- **Location:** `/etc/nginx/nginx.conf` — `server_tokens` is commented out

**Description:**  
The nginx config has `# server_tokens off;` commented out. This means nginx version information is exposed in HTTP response headers (`Server: nginx/1.x.x`) and error pages.

**Impact:** Attackers can identify the exact nginx version and target known CVEs.

**Recommendation:**  
Uncomment `server_tokens off;` in nginx.conf.

---

### MEDIUM

#### M-1: Ed25519 Signatures Not Verified Server-Side

- **Category:** Authentication / Integrity  
- **Location:** `relay.rs`, Chat message handler (line ~840+)

**Description:**  
The server passes through the `signature` field without verification. The `from` field is correctly overridden to the authenticated WebSocket peer key (preventing spoofing), but signatures are only verified client-side. This means:

1. The "✓ signed" badge can be faked by a client sending any hex string as a signature
2. The signature provides no server-enforced integrity guarantee

The client does verify signatures via `crypto.subtle.verify()`, which is good, but a malicious modified client could display a ✓ badge for forged signatures.

**Impact:** Cosmetic trust indicator can be spoofed. Since the server already enforces `from` field integrity, the practical impact is low — signatures are an additional layer, not the primary auth mechanism.

**Recommendation:**  
Add server-side Ed25519 verification using `ed25519-dalek` crate. Strip or mark unverified signatures. This can wait until post-launch.

---

#### M-2: No Server-Side Connection Limit

- **Category:** DoS  
- **Location:** `relay.rs` / `main.rs`

**Description:**  
While nginx limits concurrent connections per IP to 10 (`limit_conn ws_conn 10`), there is no global server-side cap on total WebSocket connections. Each connection allocates a broadcast receiver (256-message buffer), peer state, and two spawned tasks. An attacker with many IPs could exhaust server resources.

**Impact:** Distributed DoS via connection exhaustion.

**Recommendation:**  
Add a global `AtomicUsize` connection counter in `RelayState`. Reject WebSocket upgrades above a threshold (e.g., 500). Also consider requiring the `identify` handshake within a timeout (e.g., 30 seconds) — currently a client can connect and never identify, holding resources indefinitely.

---

#### M-3: Admin Keys in systemd Environment Variables

- **Category:** Secret Management  
- **Location:** systemd service file (on VPS)

**Description:**  
`ADMIN_KEYS` and `API_SECRET` are set as `Environment=` directives in the systemd service file. These are visible via `/proc/<pid>/environ` to root and potentially to users who can read the service file.

**Impact:** Any shell access to the VPS exposes admin keys and API secret.

**Recommendation:**  
Move secrets to `EnvironmentFile=/opt/Humanity/.env` with `chmod 600` permissions.

---

#### M-4: Kick/Ban Now Functional but Disconnect is Indirect

- **Category:** Authorization / Enforcement  
- **Location:** `relay.rs` lines ~2250-2300 (handle_mod_command), lines ~770-780 (kicked_keys check in send loop)

**Description:**  
The previous audit found that kick/ban didn't disconnect sockets. This has been **partially fixed** — kicked keys are now tracked in `kicked_keys: RwLock<HashSet<String>>`, and both the send loop and receive loop check this set to close the connection. This is a significant improvement.

However, the check in the recv task happens only when a new message arrives:
```rust
if state_clone.kicked_keys.read().await.contains(&my_key_for_recv) {
    break;
}
```

If a kicked user goes silent (sends no messages), they remain connected until the send loop's next broadcast check notices them. The send loop checks on every broadcast message, which is more reliable. Overall, this is acceptable — kicked users will be disconnected within seconds of any server activity.

**Impact:** Minor delay in kick enforcement for silent users. Functionally adequate.

**Recommendation:**  
No urgent fix needed. If perfect enforcement is desired, add a periodic check or use a cancellation token per connection.

---

#### M-5: No DM Rate Limiting Separate from Chat

- **Category:** DoS / Spam  
- **Location:** `relay.rs` DM handler (line ~2080+)

**Description:**  
The DM handler checks mute status but does not apply the Fibonacci rate limit that chat messages get. A non-muted user can spam DMs to any other user without rate limiting (aside from WebSocket frame throughput).

**Impact:** DM spam vector.

**Recommendation:**  
Apply the same rate limit logic to DM sends, or implement a separate DM-specific rate limit.

---

#### M-6: Profile Socials URLs Not Validated for `javascript:` Protocol

- **Category:** XSS (Stored)  
- **Location:** `index.html` `showViewProfileCard()` function (line ~3630+)

**Description:**  
The profile view card renders social links as `<a href="...">` tags. While the `esc()` function properly escapes HTML entities, the `href` attribute can accept `javascript:` URIs:

```javascript
if (url.startsWith('https://')) {
  html += '<a href="' + esc(url) + '"...>';
}
```

The code does check `url.startsWith('https://')` before creating a link, which effectively blocks `javascript:` URLs for the website field. Twitter and GitHub URLs are constructed from handles, not raw URLs. YouTube also checks for `https://` prefix.

However, a user could set their website to `https://` followed by injection content. Since `esc()` now escapes single quotes (`&#39;`), attribute breakout is prevented.

**Impact:** Low — the `https://` prefix check and proper escaping prevent exploitation. But the pattern could be fragile if new social fields are added without the same prefix check.

**Recommendation:**  
Add explicit URL validation for all social links — reject anything not matching `https://...` pattern. Consider using a URL parser to validate.

---

#### M-7: Upload Endpoint Does Not Require Authentication

- **Category:** Abuse / Resource Exhaustion  
- **Location:** `api.rs` `upload_file()` function

**Description:**  
The upload endpoint (`POST /api/upload`) does not require API authentication or a valid WebSocket session. Anyone on the internet can upload images (within nginx rate limits of 2/minute). The optional `?key=` parameter enables FIFO tracking but is not required.

The global disk limit of 500MB (`MAX_TOTAL_UPLOAD_BYTES`) mitigates exhaustion, and magic byte validation prevents non-image uploads. However, an attacker can still:
1. Fill the 500MB quota with garbage images
2. Generate new keys to bypass per-user FIFO limits

**Impact:** Upload quota can be exhausted by unauthenticated attackers, denying upload functionality to legitimate users.

**Recommendation:**  
Consider requiring the `key` parameter to match a currently-connected WebSocket peer, or add upload authentication. The 500MB global cap is a good backstop.

---

### LOW

#### L-1: In-Memory Rate Limit State Resets on Restart

- **Category:** Rate Limiting Bypass  
- **Location:** `relay.rs` `rate_limits: RwLock<HashMap<...>>`

**Description:**  
Rate limiting state is stored in memory only. Server restarts (including deploys) reset all rate limits and new-account tracking. An attacker who monitors server version changes (exposed via `peer_list` → `server_version`) could time bursts after restarts.

**Impact:** Minimal — nginx rate limits provide the primary protection layer and survive restarts.

---

#### L-2: Health Endpoint Reveals Operational Details

- **Category:** Information Disclosure  
- **Location:** `main.rs` `health()` handler

**Description:**  
The `/health` endpoint exposes uptime, total message count, and connected peer count. This is useful for monitoring but also for reconnaissance.

**Impact:** Minor operational intelligence leak.

**Recommendation:**  
Consider restricting to localhost or reducing exposed fields for public access.

---

#### L-3: Link Codes Are 6 Hex Characters (16.7M Possibilities)

- **Category:** Brute Force  
- **Location:** `storage.rs` `create_link_code()`

**Description:**  
Link codes are now generated using CSPRNG (`rand::rng().random()`) — **fixing the predictability issue from the previous audit**. However, the code space is still only 6 hex chars (~16.7 million possibilities). With the 5-minute expiry window and no brute-force protection on the WebSocket identify path, an attacker could attempt to guess active link codes.

At ~100 attempts/second (limited by WebSocket connection overhead), an attacker has ~30,000 attempts in 5 minutes, or about a 0.18% chance of guessing a specific code. Low probability but non-zero.

**Impact:** Very low probability of link code brute-force. The 5-minute window is the primary protection.

**Recommendation:**  
Consider increasing to 8+ characters or adding rate limiting on failed link code attempts. Not urgent.

---

#### L-4: `localStorage` Used for Various Client State

- **Category:** Client-Side Security  
- **Location:** `index.html` — multiple `localStorage.getItem/setItem` calls

**Description:**  
The client stores display name, channel preference, sound settings, block list, personal pins, and notification state in `localStorage`. This is standard web app behavior, but:

1. Any XSS vulnerability would expose all this data
2. The display name in `localStorage` enables auto-login, which could be a concern on shared computers

The Ed25519 private key is correctly stored in IndexedDB with `extractable: false` (strongest browser protection), so the critical identity is protected.

**Impact:** Non-sensitive preference data in localStorage. The auto-login behavior is a UX trade-off.

---

#### L-5: Typing Indicator Not Rate-Limited Server-Side

- **Category:** DoS  
- **Location:** `relay.rs` Typing handler

**Description:**  
Typing indicators are broadcast without rate limiting. A malicious client could flood typing indicators. The broadcast channel capacity (256) limits the practical impact — overflow would drop messages, not crash the server.

**Impact:** Minor nuisance. Could cause legitimate message drops if broadcast channel overflows.

---

#### L-6: `server_version` Exposed to All Clients

- **Category:** Information Disclosure  
- **Location:** `relay.rs` peer_list message, `build.rs`

**Description:**  
The server sends its build version (git hash + timestamp) to all clients in the `peer_list` message. The client uses this to auto-reload on version changes (good UX), but it also reveals deployment timing.

**Impact:** Minor — attackers can determine exact deploy times.

---

### INFORMATIONAL

#### I-1: CSP Allows `'unsafe-inline'` for Scripts

The current CSP includes `script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net`. The `'unsafe-inline'` directive weakens XSS protection because the entire client is a single HTML file with inline `<script>` blocks. This is an acceptable trade-off for now — moving to external JS files would enable removing `unsafe-inline` for full CSP protection.

#### I-2: Single-File Client Architecture

The entire web client (~4300 lines) is in a single `index.html` file with inline CSS and JavaScript. While this simplifies deployment (served via `ServeDir`), it prevents using strict CSP (`script-src 'self'` without `unsafe-inline`). Consider splitting into `index.html`, `styles.css`, and `app.js` for better CSP and maintainability.

#### I-3: Twemoji CDN Dependency

The client loads twemoji from `cdn.jsdelivr.net`. This is a trusted CDN, but it's a third-party dependency that could be compromised (supply chain risk). The CSP correctly restricts scripts to `self` and `cdn.jsdelivr.net`. Consider self-hosting twemoji if supply chain risk is a concern.

#### I-4: No CSRF Protection on API Endpoints

The API endpoints (`/api/send`, `/api/upload`, etc.) don't use CSRF tokens. This is acceptable because:
- `POST /api/send` requires Bearer token authentication
- `POST /api/upload` is unauthenticated but limited in scope (images only, 5MB, magic byte validated)
- CORS restricts cross-origin requests to the allowed origin list

For the upload endpoint specifically, the CORS policy prevents cross-origin uploads. Same-origin CSRF is not a concern since the attacker would need to already have XSS.

#### I-5: Database Migrations Done Inline

Schema migrations are handled inline in `Storage::open()` using `ALTER TABLE ... ADD COLUMN` with existence checks. This works for simple migrations but becomes unwieldy as the schema evolves. Consider a migration framework (e.g., `refinery`) for future development.

#### I-6: `frame-ancestors 'self' https://united-humanity.us` in CSP

The CSP allows framing from `united-humanity.us`, which is the landing page. This is intentional for the embedded chat feature. The `X-Frame-Options: SAMEORIGIN` header also applies. These are consistent and correct for the use case.

#### I-7: GitHub Webhook Secret as Query Parameter

The GitHub webhook endpoint accepts the API secret as `?secret=<token>` query parameter (in addition to the `Authorization` header). While this is needed because GitHub's webhook configuration doesn't support custom auth headers easily, query parameters can appear in server logs, browser history, and HTTP referer headers. Since this is a server-to-server call (GitHub → nginx → relay), the risk is limited to log exposure.

---

## Previously Fixed (from 2026-02-10 Audit)

| # | Previous Finding | Status | Evidence |
|---|-----------------|--------|----------|
| 1 | 🔴 Unauthenticated Bot API | ✅ **FIXED** | `check_api_auth()` in `api.rs` — Bearer token auth, fails closed if `API_SECRET` unset |
| 2 | 🔴 Stored XSS via image URL onclick | ✅ **FIXED** | Image placeholders now use `data-img-url` attribute + event delegation instead of inline onclick |
| 3 | 🟠 XSS in peer list via onclick | ✅ **FIXED** | `esc()` now escapes single quotes (`&#39;`), and peer list uses event delegation via `data-username`/`data-pubkey` attributes |
| 4 | 🟠 XSS in reactions via onclick | ✅ **FIXED** | Reaction badges use `data-target-from`/`data-target-ts`/`data-emoji` attributes + event delegation; server validates emoji (len ≤32, no `<>'"\\&`) |
| 5 | 🔴 Bot API bypasses all controls | ✅ **FIXED** | `send_message()` now checks auth, enforces 2000-char limit, validates channel exists, respects read-only |
| 6 | 🟠 Permissive CORS | ✅ **FIXED** | CORS restricted to `https://chat.united-humanity.us` and `http://localhost:3210` with specific methods/headers |
| 7 | 🟡 No WebSocket message size limit | ✅ **FIXED** | `.max_frame_size(65_536).max_message_size(131_072)` in ws_handler |
| 8 | 🟠 Missing CSP | ✅ **FIXED** | Full CSP deployed in nginx (default-src 'self', script-src whitelist, connect-src for WSS, etc.) |
| 9 | 🟡 No WebSocket origin check | ✅ **FIXED** | Origin header validated against allowlist in ws_handler; non-browser clients (no Origin) are allowed |
| 10 | 🟡 Predictable link/invite codes | ✅ **FIXED** | Now uses `rand::rng().random()` (CSPRNG) for code generation |
| 12 | 🟡 Upload trusts Content-Type | ✅ **FIXED** | Magic byte validation added for PNG, JPEG, GIF, WebP |
| 15 | 🟡 Kick doesn't disconnect | ✅ **FIXED** | `kicked_keys` HashSet + checks in both send and recv loops |
| 16 | 🟡 Disk exhaustion via uploads | ✅ **FIXED** | Global 500MB limit (`MAX_TOTAL_UPLOAD_BYTES`), per-user FIFO (4 images) |
| 21 | 🟡 Missing HSTS | ✅ **FIXED** | `Strict-Transport-Security: max-age=31536000; includeSubDomains` in nginx |
| 22 | 🟡 Emoji not validated | ✅ **FIXED** | Server-side emoji validation: length ≤32, rejects `'`, `"`, `<`, `>`, `\`, `&` |

---

## Positive Security Observations

1. ✅ **API Authentication (fail-closed)** — `check_api_auth()` returns UNAUTHORIZED if `API_SECRET` is unset or empty. This is the gold standard for API auth design.

2. ✅ **All SQL queries use parameterized queries** — Zero SQL injection risk across all 50+ queries in `storage.rs`. Every user input goes through `params![]`.

3. ✅ **Server overrides client-supplied `from` field** — Chat messages always use the authenticated WebSocket peer key as the sender. Identity spoofing is impossible.

4. ✅ **Client-side XSS prevention** — The `esc()` function correctly escapes `<`, `>`, `&`, `"`, and `'`. All user-controlled data goes through `esc()` before `innerHTML`. Event delegation eliminates inline handler injection.

5. ✅ **Name validation** — Strict ASCII alphanumeric + `_-` with 24-char limit prevents homoglyph attacks, Unicode injection, and HTML in names.

6. ✅ **Comprehensive authorization** — Every admin/mod command checks the caller's role server-side. Privilege escalation through the command interface is not possible.

7. ✅ **Ed25519 keys with `extractable: false`** — The strongest browser-side key protection available. Private keys cannot be exported from IndexedDB even via JavaScript.

8. ✅ **Privacy-preserving logging** — nginx custom log format strips IP addresses. The Rust server doesn't log IPs either. This is excellent privacy-by-design.

9. ✅ **HSTS + TLS 1.2+ + strong CSP** — The security header chain is now comprehensive: HSTS, X-Content-Type-Options, X-Frame-Options, Referrer-Policy, and CSP.

10. ✅ **WebSocket security** — Origin checking, 64KB frame / 128KB message limits, and connection-per-IP limits via nginx.

11. ✅ **Upload security** — Content-Type allowlist, magic byte validation, 5MB file limit, 500MB global disk limit, per-user FIFO, and filename sanitization.

12. ✅ **Fibonacci rate limiting** — Creative and effective. Combined with new-account slow mode (5s for first 10 minutes), this deters spam while being transparent to normal users.

13. ✅ **Auto-lockdown with grace period** — 30-second grace period prevents false lockdowns during deploy restarts. Auto-unlock when staff reconnects.

14. ✅ **Ban persistence** — Bans survive server restarts (stored in SQLite `banned_keys` table). Banned users are checked on connect.

15. ✅ **Read-only channel enforcement** — Server-side, not just client-side. Both WebSocket and API paths respect it.

16. ✅ **Proper WebSocket upgrade** — Non-browser clients (no Origin header) are allowed through, while browser connections with wrong Origin are rejected. This correctly handles native app clients.

17. ✅ **Build version auto-reload** — Clients auto-reload when server version changes, ensuring consistent client/server compatibility after deploys.

---

## Recommendations

**Priority 1 (Recommended before scaling):**
1. **Refactor DM delivery** — Move DMs to per-client sender channels instead of broadcasting through the shared channel (H-1)
2. **Uncomment `server_tokens off`** in nginx.conf (H-2) — one line change
3. **Add server-side connection limit** with configurable max (M-2)
4. **Move secrets to EnvironmentFile** in systemd (M-3)
5. **Add rate limiting to DM sends** (M-5)

**Priority 2 (Post-launch hardening):**
6. **Make upload `key` parameter required** and validate against connected peers (M-7)
7. **Add server-side Ed25519 signature verification** using `ed25519-dalek` (M-1)
8. **Validate profile social URLs** with explicit URL parsing (M-6)
9. **Increase link code length** to 8+ characters (L-3)
10. **Add typing indicator rate limiting** server-side (L-5)

**Priority 3 (Architecture improvements):**
11. **Split client into separate HTML/CSS/JS files** — enables removing `'unsafe-inline'` from CSP (I-1, I-2)
12. **Self-host twemoji** to eliminate CDN dependency (I-3)
13. **Consider a migration framework** for database schema evolution (I-5)
14. **Reduce health endpoint exposure** — restrict to localhost or minimize fields (L-2)

---

## Dependency Assessment

| Crate | Version | Risk | Notes |
|-------|---------|------|-------|
| tokio | 1.x | ✅ Low | Actively maintained, no known vulns |
| axum | 0.8 | ✅ Low | Well-maintained, latest stable series |
| tower-http | 0.6 | ✅ Low | Standard middleware crate |
| rusqlite | 0.34 (bundled) | ✅ Low | Bundled SQLite avoids system lib version issues |
| reqwest | 0.12 (rustls) | ✅ Low | Using rustls instead of OpenSSL is good practice |
| serde/serde_json | 1.x | ✅ Low | Extremely well-maintained |
| rand | 0.9 | ✅ Low | Latest version, CSPRNG by default |
| tracing | 0.1 | ✅ Low | Standard logging crate |

No known vulnerable dependency versions detected. The `edition = "2024"` Rust edition ensures modern language features and safety guarantees.

---

*End of audit. Overall: the codebase is well-built and the security posture has improved markedly since the previous audit. The application is ready for public launch with the understanding that the Priority 1 recommendations should be addressed as the user base grows.*
