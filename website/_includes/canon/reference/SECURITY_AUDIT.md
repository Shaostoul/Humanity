# Security Audit — February 12, 2026

## Summary

**Auditor:** Heron (automated agent)
**Scope:** Full source audit of the Humanity relay platform (8 files, ~16,000+ lines)
**Findings:** 0 Critical, 5 High, 10 Medium, 8 Low, 7 Info

The Humanity relay demonstrates strong security fundamentals: Ed25519 cryptographic identity, parameterized SQL queries throughout, server-side signature verification, HMAC-SHA256 webhook authentication, SSRF protection on link previews, per-session upload tokens, Fibonacci rate limiting, and auto-lockdown when no moderators are online. The architecture is sound for its threat model.

The findings below are primarily defense-in-depth improvements rather than actively exploitable vulnerabilities.

---

## Findings

### CRITICAL

_None identified._

### HIGH

#### H-1: API Secret Comparison Uses Non-Constant-Time Equality
- **Location:** `api.rs:44` (`check_api_auth` function)
- **Description:** The `check_api_auth` function compares `provided != expected` using Rust's standard string equality, which short-circuits on the first differing byte. This enables timing side-channel attacks to incrementally guess the API_SECRET. Notably, the codebase already has a `constant_time_eq` function (api.rs:24) used for webhook HMAC verification but does NOT use it for API auth.
- **Impact:** An attacker on a low-latency network could theoretically recover the API_SECRET through statistical timing analysis, gaining full bot API access.
- **Recommendation:** Replace `if provided != expected` with `if !constant_time_eq(provided.as_bytes(), expected.as_bytes())` in `check_api_auth`.

#### H-2: Bot WebSocket Auth Uses Non-Constant-Time Comparison
- **Location:** `relay.rs:~1155` (Identify handler for `bot_` keys)
- **Description:** `if expected.is_empty() || provided != expected` — the bot_secret comparison in the WebSocket identify flow also uses standard string equality, same timing vulnerability as H-1.
- **Impact:** Same as H-1 but for WebSocket bot authentication.
- **Recommendation:** Use `constant_time_eq` for bot_secret comparison.

#### H-3: Unauthenticated Read APIs Expose Full Message History
- **Location:** `api.rs` — `get_messages`, `get_reactions`, `get_pins`, `get_stats`, `get_peers`, `get_tasks`, `get_listings`, `list_federation_servers`, `get_server_info`
- **Description:** Multiple GET endpoints require no authentication whatsoever. While some (like `get_server_info`, `get_listings`) are intentionally public, `get_messages` exposes the full chat history of any channel including message content, author keys, and timestamps. `get_peers` exposes all connected users' public keys. `get_stats` exposes internal metrics.
- **Impact:** Any unauthenticated party can scrape the entire message history, enumerate all users and their public keys, and monitor online status — even without a WebSocket connection. This bypasses the WebSocket's identify/origin checks.
- **Recommendation:** Add `check_api_auth` to `get_messages`, `get_peers`, and `get_stats`. Keep `get_server_info`, `get_listings`, and `list_federation_servers` public (they're designed for federation/public browsing). Consider adding optional API key auth for reactions/pins/tasks.

#### H-4: XSS via `formatBody` Post-Escape URL Injection
- **Location:** `client/index.html:3809-3833` (formatBody function, URL → HTML replacements)
- **Description:** `formatBody` correctly escapes HTML via `esc()` first, but then uses regex to convert URL patterns back into raw HTML (`<audio>`, `<video>`, `<a>`, `<img>` placeholders). Since escaping converts `&` → `&amp;`, `<` → `&lt;`, etc., the URL content is "safe" — however, the regex operates on the escaped text, and URL values from the escaped text are inserted into `src=` and `href=` attributes without re-validation. A crafted message containing a URL like `javascript:alert(1)` would be escaped to `javascript:alert(1)` (no HTML entities in that string) and then matched by the URL regex `/(?<!["=])(https?:\/\/...)/` — but wait, this regex requires `https?://` prefix, so `javascript:` URLs are actually blocked. The audio/video/document regexes also require specific file extensions. This is **mostly safe** due to the `https?://` requirement, but the file-card handler at line 3821 constructs `<a href="${url}">` where `url` comes from the regex match on escaped text that could contain encoded entities that decode differently in an href context.
- **Impact:** Low practical exploitability due to the `https?://` prefix requirement, but defense-in-depth is lacking. If any future regex is added without the protocol check, XSS would be immediate.
- **Recommendation:** Add explicit protocol validation (`url.startsWith('https://') || url.startsWith('http://')`) before inserting any URL into HTML attributes. Consider using a DOMPurify-like sanitizer as a final pass on `formatBody` output.

#### H-5: Group Messages Broadcast to All Connected Clients
- **Location:** `relay.rs:~4170-4185` (GroupMsg handler)
- **Description:** When a user sends a group message, the server broadcasts it to ALL connected clients via the broadcast channel. The comment in the code says "For now, use the broadcast and let all clients filter by group membership" — but there's no server-side filtering. Any connected client receives all group messages regardless of membership. The `GroupMessage` variant has no `target` field and no filtering in the broadcast send loop.
- **Impact:** All private group conversations are visible to any authenticated WebSocket client. Group privacy is effectively non-existent.
- **Recommendation:** Add a `target` field to `GroupMessage` or filter by group membership in the broadcast send loop (similar to how DMs are filtered by `to` field).

---

### MEDIUM

#### M-1: Uploaded Files Served Without Content-Disposition Header
- **Location:** `main.rs:104` (`.nest_service("/uploads", ...)`)
- **Description:** Uploaded files are served via `tower_http::services::ServeDir` which serves files with their detected MIME type but without a `Content-Disposition: attachment` header. HTML files are blocked by extension, but SVG files (which can contain JavaScript) are not in the blocked list. While `application/octet-stream` is allowed, the extension check prevents `.html`/`.svg` but not all dangerous types.
- **Impact:** If an attacker uploads a file that the browser interprets as HTML (e.g., via content-type sniffing), it could execute JavaScript in the context of the site's origin.
- **Recommendation:** Add `Content-Disposition: attachment` for all uploads, or add `X-Content-Type-Options: nosniff` to the upload serving route. Also add SVG to the blocked extensions list.

#### M-2: No Rate Limiting on Several API Endpoints
- **Location:** `api.rs` — `get_messages`, `get_reactions`, `get_pins`, `get_tasks`, `get_listings`, `get_peers`, `get_stats`
- **Description:** While the WebSocket has Fibonacci rate limiting and search has 1/2s rate limiting, the HTTP API endpoints have no application-level rate limiting. The comment suggests nginx handles this, but if nginx rate limits are per-IP and the server is behind a CDN/proxy, the effective rate limiting may be weaker than expected.
- **Impact:** An attacker could rapidly poll endpoints to scrape data or cause elevated database load.
- **Recommendation:** Add application-level rate limiting to API endpoints, or document the nginx rate limit dependency explicitly and ensure it's tested.

#### M-3: DM Search Exposes Private Messages in Search Results
- **Location:** `storage.rs:~1550-1600` (search_messages_full function)
- **Description:** When `channel` is None, the search function also searches `direct_messages` and includes DM content in results. The search results are sent only to the requesting user, but there's no check that the requesting user is a party to those DMs. Any authenticated user's search could return DMs between other users.
- **Impact:** Users could search for keywords and find fragments of other users' private DM conversations.
- **Recommendation:** Add a `WHERE (from_key = ?requester OR to_key = ?requester)` filter to the DM search query.

#### M-4: Broadcast Channel as Message Bus Leaks Metadata
- **Location:** `relay.rs` — entire broadcast pattern
- **Description:** The system uses a single `broadcast::channel` for all message routing. While the send loop filters messages by type (DMs to recipient only, etc.), every connected client's send loop receives every message and then decides whether to forward it. This means the server processes every message for every client, and any bug in the filtering logic would leak private data.
- **Impact:** Architecture amplifies the impact of any filtering bug. Currently functional but fragile.
- **Recommendation:** For a future refactor, consider per-user channels or a pub/sub system with topic-based routing. Document the current approach's trade-offs.

#### M-5: No Max Pin Count Per Channel
- **Location:** `storage.rs` — `pin_message` function
- **Description:** There's no limit on how many messages can be pinned per channel. A moderator (or compromised mod account) could pin thousands of messages, causing performance issues when syncing pins on connect.
- **Impact:** DoS vector through pin exhaustion, causing slow connections.
- **Recommendation:** Add a max pin count per channel (e.g., 50) and reject new pins when the limit is reached.

#### M-6: User Data Sync Has No Authentication Beyond Connection
- **Location:** `relay.rs:~1610-1660` (sync_save/sync_load handlers)
- **Description:** The `sync_save` and `sync_load` messages allow any authenticated WebSocket client to store/retrieve up to 512KB of arbitrary JSON data keyed by their public key. While this is by design (the user owns their key), there's no validation of the data content, and the 512KB limit per user could be used for abuse (500 users × 512KB = 250MB of user data).
- **Impact:** Storage exhaustion through user data abuse.
- **Recommendation:** Consider a lower default limit (e.g., 64KB) and/or rate limiting sync_save operations.

#### M-7: Link Preview Fetcher Could Be Used for Port Scanning
- **Location:** `relay.rs:~4260-4330` (fetch_link_preview function)
- **Description:** While the code has SSRF protection (private IP check after DNS resolution), it checks IPs from `lookup_host` but then makes the request via `reqwest` which does its own DNS resolution. A DNS rebinding attack could return a public IP on first lookup (passing the check) and a private IP on second lookup (used by reqwest).
- **Impact:** SSRF via DNS rebinding to access internal services.
- **Recommendation:** Use a custom DNS resolver that pins the resolved IP for the entire request, or use `reqwest`'s `resolve` feature to force the checked IP.

#### M-8: WebSocket Origin Check Allows Non-Browser Clients Without Origin
- **Location:** `main.rs:131-137` (ws_handler)
- **Description:** The origin check only fires when an `Origin` header is present. Non-browser clients (curl, custom tools) don't send Origin and bypass the check entirely. This is by design (to allow native apps and bots), but it means the origin check only protects against browser-based CSRF, not against unauthorized non-browser access.
- **Impact:** Any tool can connect to the WebSocket without origin restrictions. Combined with H-3, this means unauthenticated enumeration.
- **Recommendation:** This is acceptable given the public key auth model, but document it explicitly. Consider requiring the identify message to complete within the timeout (already done) and potentially requiring a challenge-response.

#### M-9: Profile Socials URLs Not Fully Validated on Display
- **Location:** `client/index.html:~5130-5200` (showViewProfileCard)
- **Description:** While the server validates that profile URL fields start with `https://` and handles start with alphanumerics, the client renders profile social links using innerHTML with `esc()`. The server-side validation is good, but the client trusts the server data and renders it. If the database were compromised or a future code change relaxed server validation, XSS would be possible.
- **Impact:** Low — requires server-side compromise or validation regression.
- **Recommendation:** Add client-side URL validation before rendering profile links.

#### M-10: No CSRF Protection on File Upload Endpoint
- **Location:** `api.rs` — `upload_file`
- **Description:** The upload endpoint uses a per-session token passed as a query parameter (`?token=...`). While this effectively prevents CSRF (the token is secret and per-session), the token is transmitted in the URL which may be logged by proxies, appear in Referer headers, and be stored in browser history.
- **Impact:** Upload token leakage through URL logging.
- **Recommendation:** Accept the upload token via a header (e.g., `X-Upload-Token`) instead of a query parameter.

---

### LOW

#### L-1: Error Messages Expose Internal Details
- **Location:** `api.rs` various endpoints, `relay.rs` error handling
- **Description:** Some error paths return internal error messages directly to clients, e.g., `format!("Failed to create task: {e}")` which could expose SQLite error details.
- **Impact:** Information disclosure about database schema and internal state.
- **Recommendation:** Return generic error messages to clients, log details server-side.

#### L-2: In-Memory Rate Limit State Not Bounded
- **Location:** `relay.rs` — `rate_limits`, `typing_timestamps`, `last_search_times`
- **Description:** The `rate_limits` HashMap grows without bound as new keys connect. Old entries are never cleaned up. Over time (or during an attack with many unique keys), this could consume significant memory.
- **Impact:** Memory exhaustion DoS over long server uptime.
- **Recommendation:** Periodically prune stale entries (e.g., entries older than 1 hour) or use an LRU cache.

#### L-3: Upload Token Not Invalidated on Reconnect
- **Location:** `relay.rs:~1250` (upload token generation)
- **Description:** A new upload token is generated on each connection, but old tokens from previous sessions are only removed when the peer disconnects. If a user disconnects abruptly (without the cleanup running), stale tokens remain in the `upload_tokens` map.
- **Impact:** Stale upload tokens could theoretically be reused if intercepted.
- **Recommendation:** Add a TTL to upload tokens or clear them on re-identify.

#### L-4: CORS Allows localhost Origin in Production
- **Location:** `main.rs:111` — `"http://localhost:3210"`
- **Description:** The CORS layer includes `http://localhost:3210` as an allowed origin. This is useful for development but should not be present in production.
- **Impact:** A malicious page on localhost could make cross-origin requests to the production server.
- **Recommendation:** Make the localhost origin conditional on an environment variable (e.g., only when `RUST_ENV=development`).

#### L-5: No Content Security Policy on Upload Serving
- **Location:** `main.rs:104` — uploads served via ServeDir
- **Description:** Uploaded files are served without a restrictive CSP header. While nginx likely adds CSP, the application layer doesn't enforce it.
- **Impact:** If nginx CSP is misconfigured, uploaded files could execute scripts.
- **Recommendation:** Add `Content-Security-Policy: default-src 'none'` to the uploads serving route.

#### L-6: Webhook Token Sent in Request Body, Not Header
- **Location:** `relay.rs:~205` (notify_webhook)
- **Description:** The webhook notification sends the bearer token via an Authorization header, which is correct. However, the webhook URL and token are logged at startup (`info!("Webhook configured: {}", wh.url)`). The URL itself might contain sensitive path components.
- **Impact:** Webhook URL disclosed in logs.
- **Recommendation:** Log only the domain of the webhook URL, not the full path.

#### L-7: Service Worker Caches All Non-API/WS Responses
- **Location:** `shared/sw.js:48-55` (fetch handler)
- **Description:** The service worker caches all successful responses except `/ws`, `/api/`, and `/chat`. This includes uploaded files, which could persist sensitive images in the browser cache even after the server's FIFO cleanup deletes them.
- **Impact:** Deleted uploads persist in client-side cache.
- **Recommendation:** Exclude `/uploads/` from the service worker cache, or set appropriate `Cache-Control` headers on uploads.

#### L-8: Client Stores Private Key in localStorage
- **Location:** `client/index.html` (identity management)
- **Description:** The Ed25519 private key is stored in `localStorage`, which is accessible to any JavaScript running on the same origin. While this is the standard approach for browser-based crypto identity, it means XSS or a malicious browser extension could steal private keys.
- **Impact:** Private key theft via XSS or extension compromise.
- **Recommendation:** Document this limitation. Consider using the Web Crypto API with non-extractable keys for signing operations (though this would prevent key export/backup). At minimum, educate users about the risk.

---

### INFO

#### I-1: SQL Injection Not Present — Parameterized Queries Throughout
- **Location:** `storage.rs` (entire file)
- **Description:** All SQL queries use parameterized queries (`params![]`). The search function properly escapes LIKE wildcards (`%`, `_`, `\`). No string interpolation is used in SQL. This is excellent.
- **Impact:** N/A — positive finding.

#### I-2: Server Keypair Stored in SQLite
- **Location:** `storage.rs` — `get_or_create_server_keypair`
- **Description:** The server's Ed25519 keypair for federation is stored in the SQLite database. If the database file is compromised, the server identity is compromised. This is standard for single-server deployments.
- **Recommendation:** For high-security deployments, consider storing the server private key in a separate secrets store or HSM.

#### I-3: No Message Expiry / Retention Policy
- **Description:** Messages are stored indefinitely in SQLite. There's no automatic cleanup of old messages (only admin `/wipe` commands). Over time, the database will grow without bound.
- **Recommendation:** Consider an optional retention policy (e.g., auto-delete messages older than N days) configurable via environment variable.

#### I-4: Federation Security Model Is Incomplete
- **Description:** The federation system has trust tiers and server discovery, but no actual message relay or verification between servers is implemented yet. The `/server-add` command fetches server-info over HTTP without verifying TLS certificates or server identity.
- **Recommendation:** When implementing federation relay, ensure messages are signed by origin servers, implement mutual TLS or signed challenges, and validate trust tiers before accepting federated content.

#### I-5: Markdown Formatting Applies After HTML Escaping
- **Location:** `client/index.html:3839-3847` (formatBody Step 4)
- **Description:** The `formatBody` function applies markdown transformations (bold, italic, strikethrough) after HTML escaping. Since `esc()` converts `<` to `&lt;`, the regex-generated HTML tags (`<strong>`, `<em>`, `<del>`, `<code>`) are the only raw HTML in the output. This is a safe pattern, but it's fragile — any future markdown rule that doesn't go through `esc()` first could introduce XSS.
- **Recommendation:** Add a comment documenting this invariant. Consider adding a final sanitization pass (e.g., allowlisting specific tags) as defense-in-depth.

#### I-6: No Audit Log for Admin Actions
- **Description:** Admin commands (ban, kick, verify, wipe, etc.) are logged via `tracing::info!` but not stored in a persistent audit table. If logs rotate, the history of admin actions is lost.
- **Recommendation:** Add a persistent `admin_actions` table recording who did what and when.

#### I-7: Build Version Exposed to All Clients
- **Location:** `relay.rs:~1295` (PeerList with `server_version`), `api.rs` (StatsResponse, ServerInfoResponse)
- **Description:** The exact build version is sent to all clients on connect and via public API endpoints. This helps attackers identify specific vulnerable versions.
- **Impact:** Minor information disclosure.
- **Recommendation:** Acceptable for an open-source project. Consider omitting from the public stats endpoint if desired.

---

## Recommendations

### Priority 1 (Immediate)
1. **Fix timing attacks:** Use `constant_time_eq` for API_SECRET and bot_secret comparisons (H-1, H-2)
2. **Fix group message leak:** Add server-side filtering for group messages (H-5)
3. **Fix DM search leak:** Filter DM search results to only the requesting user's conversations (M-3)

### Priority 2 (Short-term)
4. **Add auth to sensitive APIs:** Require API_SECRET for `get_messages`, `get_peers`, `get_stats` (H-3)
5. **Harden upload serving:** Add `Content-Disposition: attachment`, `X-Content-Type-Options: nosniff`, block SVG uploads (M-1)
6. **Fix DNS rebinding in link previews:** Pin resolved IPs for requests (M-7)
7. **Move upload token to header:** Use `X-Upload-Token` header instead of query param (M-10)

### Priority 3 (Medium-term)
8. **Bound in-memory state:** Add LRU/TTL to rate_limits, typing_timestamps, etc. (L-2)
9. **Add rate limiting to API endpoints:** Application-level rate limits (M-2)
10. **Add pin count limits:** Max 50 pins per channel (M-5)
11. **Remove localhost CORS in production:** Make it environment-conditional (L-4)
12. **Exclude uploads from service worker cache** (L-7)

### Priority 4 (Long-term)
13. **Add persistent admin audit log** (I-6)
14. **Add message retention policy** (I-3)
15. **Design federation security model** before implementing message relay (I-4)
16. **Consider DOMPurify** as a final sanitization pass for `formatBody` output (H-4, I-5)
