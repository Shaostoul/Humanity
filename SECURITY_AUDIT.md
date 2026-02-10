# Humanity Relay — Security Audit Report

**Date:** 2026-02-10  
**Auditor:** OpenClaw Subagent (Deep Audit)  
**Scope:** Full codebase (`relay.rs`, `storage.rs`, `api.rs`, `main.rs`, `index.html`, `crypto.js`) + VPS nginx config  
**Context:** Pre-public-launch review. App is about to be announced on X/Twitter.

---

## Executive Summary

The Humanity Relay is **reasonably well-built for an MVP** — it uses parameterized SQL queries throughout, the `esc()` function is correctly applied before `innerHTML`, and the server overrides client-supplied `from` fields to prevent spoofing. However, there are **several critical and high-severity issues** that must be fixed before public launch, particularly around the unauthenticated Bot API and missing Content-Security-Policy headers.

**Overall Risk Level: MEDIUM-HIGH** — The most dangerous issues are exploitable by moderately skilled attackers and could lead to full impersonation, stored XSS via the bot API, or denial of service.

---

## Findings

### FINDING 1 — Unauthenticated Bot/Webhook API Endpoints

- **Severity:** 🔴 **CRITICAL**
- **Category:** AuthZ (Missing Authentication)
- **Location:** `api.rs` lines 56-78 (`send_message`), lines 189-256 (`github_webhook`); `main.rs` lines 60-63

**Description:**  
The Bot HTTP API (`POST /api/send`, `POST /api/github-webhook`) has **zero authentication**. Anyone on the internet can:
1. `POST /api/send` with any `from_name` to impersonate any bot or inject arbitrary messages visible to all users.
2. `POST /api/github-webhook` with a crafted payload to inject fake "GitHub" announcements into the announcements channel.
3. Send messages that bypass all rate limiting, mute checks, and content length enforcement (those only apply to WebSocket users).

**Impact:** An attacker can impersonate any bot, inject misleading/malicious system announcements, flood messages at unlimited rate, and bypass all moderation controls. Combined with FINDING 2, this becomes a stored XSS attack vector.

**Recommendation:**  
Add API key authentication to all bot endpoints. At minimum, check for a `Bearer` token in the `Authorization` header:

```rust
// In api.rs — add to send_message and github_webhook:
fn check_api_key(headers: &axum::http::HeaderMap) -> bool {
    let expected = std::env::var("API_SECRET").unwrap_or_default();
    if expected.is_empty() { return false; } // Fail closed
    headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.strip_prefix("Bearer ").unwrap_or("") == expected)
        .unwrap_or(false)
}
```

Also: apply message length limits and content validation to bot messages.

---

### FINDING 2 — Stored XSS via Bot API Message Injection

- **Severity:** 🔴 **CRITICAL**  
- **Category:** XSS (Stored Cross-Site Scripting)
- **Location:** `api.rs` line 67 (`content: req.content`) → `index.html` `formatBody()` → image URL regex

**Description:**  
The `formatBody()` function in `index.html` converts URLs matching image extensions to clickable HTML elements using regex substitution. The `esc()` function is correctly called first, but the subsequent regex replacements reintroduce HTML:

```javascript
safe = safe.replace(
  /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:png|jpe?g|gif|webp)(?:\?[^\s<]*)?)/gi,
  '<span class="img-placeholder" onclick="loadImage(this, \'$1\')">🖼️ Image (click to load)</span>'
);
```

The captured group `$1` is inserted into an `onclick` handler with single-quote delimited string. If an attacker crafts a URL containing `')` followed by JavaScript, it breaks out of the string:

```
https://evil.com/x.png?');alert(document.cookie);//
```

After `esc()`, the `'` and `)` are **not escaped** (esc() only escapes `<>&"`), so the onclick becomes:
```html
onclick="loadImage(this, 'https://evil.com/x.png?');alert(document.cookie);//')"
```

Combined with FINDING 1 (unauthenticated bot API), an attacker can inject a message via `POST /api/send` that executes arbitrary JavaScript in every connected client's browser, including stealing their Ed25519 private keys from IndexedDB.

**Impact:** Full stored XSS. Attacker can steal all users' cryptographic identities, send messages on their behalf, or redirect users to malicious sites.

**Recommendation:**  
1. Fix the regex to use safe attribute handling — never interpolate user data into `onclick` handlers:
```javascript
// Replace onclick string interpolation with data attributes:
safe = safe.replace(
  /((?:https?:\/\/[^\s<]+|\/uploads\/[^\s<]+)\.(?:png|jpe?g|gif|webp)(?:\?[^\s<]*)?)/gi,
  '<span class="img-placeholder" data-img-url="$1">🖼️ Image (click to load)</span>'
);
// Then attach click handlers via event delegation after innerHTML assignment.
```
2. Additionally, add a Content-Security-Policy header (see FINDING 8).

---

### FINDING 3 — XSS in Peer List via `onclick` Attribute

- **Severity:** 🟠 **HIGH**
- **Category:** XSS (DOM-based)
- **Location:** `index.html`, `updatePeerList()` function (approximately line 1305 in full file)

**Description:**  
The `updatePeerList()` function builds HTML with inline `onclick` handlers using escaped name/key values, but the escaping is bypassed:

```javascript
return `<div class="peer..." onclick="showUserContextMenu(event, '${escapedName}', '${escapedKey}')">
```

The `esc()` function escapes `<`, `>`, `&`, `"` but does **NOT** escape single quotes (`'`). A display name like `test')+alert(1);//` would break out of the onclick single-quoted string. 

While the server validates display names to only allow `[A-Za-z0-9_-]`, the `public_key` field flows through `escapedKey` and could theoretically contain single quotes if a specially crafted identify message is sent. In practice, hex keys won't contain `'`, but this is defense-in-depth.

**Impact:** Low practical impact due to server-side name validation, but the pattern is fragile and could break if validation changes.

**Recommendation:**  
Escape single quotes in the `esc()` function:
```javascript
function esc(str) {
  const d = document.createElement('div');
  d.textContent = str || '';
  return d.innerHTML.replace(/'/g, '&#39;');
}
```
Or better: stop using inline `onclick` handlers entirely. Use `addEventListener` with closures.

---

### FINDING 4 — XSS in `renderReactions()` via `onclick`

- **Severity:** 🟠 **HIGH**
- **Category:** XSS (Stored)
- **Location:** `index.html`, `renderReactions()` function

**Description:**  
```javascript
msgEl.innerHTML = Object.entries(reactions).map(([emoji, users]) => {
  const isMine = users.has(myKey);
  return `<span class="reaction-badge..." onclick="sendReaction('${targetFrom}', ${targetTs}, '${emoji}')">${emoji}...`;
}).join('');
```

The `emoji` variable is inserted raw into an `onclick` handler. While the client uses a fixed `REACTION_EMOJIS` array, a malicious client can send any `emoji` string via WebSocket. The server does **not validate or sanitize the emoji field** in `relay.rs` (line ~1304):

```rust
RelayMessage::Reaction { target_from, target_timestamp, emoji, .. } => {
    // No validation of emoji content!
    let reaction = RelayMessage::Reaction {
        target_from,
        target_timestamp,
        emoji,  // ← Unsanitized, broadcast to all clients
        ...
    };
```

An attacker can send a reaction with `emoji: "');alert(document.cookie);//"` and this will execute in every client that has that message visible.

**Impact:** Stored XSS via crafted WebSocket reaction messages. Arbitrary JS execution in all connected clients.

**Recommendation:**  
1. **Server-side:** Validate emoji to be a known set or limit to actual Unicode emoji characters (< 10 chars, no ASCII control chars, no quotes):
```rust
// In relay.rs, Reaction handler:
if emoji.len() > 20 || emoji.contains('\'') || emoji.contains('"') || emoji.contains('<') || emoji.contains('>') {
    continue; // Silently drop invalid reactions
}
```
2. **Client-side:** Use `data-` attributes and event delegation instead of inline onclick.

---

### FINDING 5 — `POST /api/send` Bypasses All Security Controls

- **Severity:** 🔴 **CRITICAL**
- **Category:** AuthZ / Security Bypass
- **Location:** `api.rs` lines 56-78

**Description:**  
The bot API `send_message` function:
1. Accepts any `from_name` — no authentication
2. Does **not** check rate limits
3. Does **not** check mute status
4. Does **not** enforce message length limits (no 2000-char cap)
5. Does **not** check read-only channels
6. Does **not** validate channel exists
7. Auto-creates a peer entry for any bot key, which persists until server restart

An attacker can send unlimited messages of unlimited length to any channel including read-only ones, flooding the chat and database.

**Impact:** Complete bypass of all moderation and rate-limiting. Database can be flooded, read-only channels written to, and all messages appear as legitimate bot messages.

**Recommendation:** See FINDING 1 for authentication. Additionally:
- Apply message length limits to bot messages
- Validate channel exists and is not read-only (or allow override only for authenticated bots)
- Add rate limiting to the API endpoint (nginx rate limit exists but at 10r/s which is still 600/min)

---

### FINDING 6 — CORS Permissive Policy

- **Severity:** 🟠 **HIGH**  
- **Category:** CORS
- **Location:** `main.rs` line 72 (`CorsLayer::permissive()`)

**Description:**  
`CorsLayer::permissive()` in tower-http sets:
- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: *`  
- `Access-Control-Allow-Headers: *`

This means any website on the internet can make API calls to the relay server. Combined with FINDING 1 (no API auth), any malicious webpage a user visits can silently call `POST /api/send` to inject messages.

**Impact:** A CSRF-like attack where visiting `evil-site.com` could silently post messages to Humanity Relay from the user's browser.

**Recommendation:**  
Replace with specific origin allowlist:
```rust
use tower_http::cors::{CorsLayer, Any};
let cors = CorsLayer::new()
    .allow_origin(["https://chat.united-humanity.us".parse().unwrap()])
    .allow_methods([Method::GET, Method::POST])
    .allow_headers(Any);
```

---

### FINDING 7 — No WebSocket Message Size Limit

- **Severity:** 🟡 **MEDIUM**
- **Category:** DoS (Denial of Service)
- **Location:** `main.rs` line 82 (`ws_handler`), `relay.rs` line 270

**Description:**  
The WebSocket upgrade handler uses axum's default configuration, which does not set a maximum message size. While the server checks `content.len() > 2001` for chat messages, a client can send a **single WebSocket frame** of arbitrary size (potentially gigabytes). The entire frame is buffered in memory before being parsed as JSON, which happens before the content length check.

**Impact:** A single malicious client can cause the server to OOM by sending one enormous WebSocket frame.

**Recommendation:**  
Configure the WebSocket with a max frame/message size:
```rust
// In ws_handler:
ws.max_frame_size(65536) // 64KB max frame
  .max_message_size(131072) // 128KB max message
  .on_upgrade(move |socket| handle_socket(socket, state.0))
```

---

### FINDING 8 — Missing Content-Security-Policy Header

- **Severity:** 🟠 **HIGH**
- **Category:** Security Headers
- **Location:** nginx config (`/etc/nginx/sites-available/humanity`)

**Description:**  
The nginx config includes `X-Content-Type-Options`, `X-Frame-Options`, and `Referrer-Policy`, but is missing `Content-Security-Policy` (CSP). A CSP would be the primary defense against XSS attacks (FINDINGS 2, 3, 4) even if those bugs exist, by preventing inline script execution.

**Impact:** No browser-level XSS mitigation. All XSS vulnerabilities become directly exploitable.

**Recommendation:**  
Add to nginx:
```nginx
add_header Content-Security-Policy "default-src 'self'; script-src 'self' https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src 'self' wss://chat.united-humanity.us; frame-ancestors 'none';" always;
```
Note: This requires moving all inline `<script>` code into external `.js` files (the current `index.html` has a large inline script block). This is a significant but worthwhile change. In the short term, you can use `'unsafe-inline'` for `script-src` but this defeats most CSP benefits.

---

### FINDING 9 — No WebSocket Origin Checking

- **Severity:** 🟡 **MEDIUM**
- **Category:** WebSocket Security
- **Location:** `main.rs` line 82 (`ws_handler`)

**Description:**  
The WebSocket upgrade handler does not check the `Origin` header. Combined with the permissive CORS policy, any website can establish a WebSocket connection to the relay. While messages require identifying with a key, a malicious page could:
1. Generate a key and connect as a new user
2. Flood the server with connections
3. If the user has the Humanity tab open, potentially interact with their session

**Impact:** Cross-origin WebSocket hijacking, potential DoS via connection flooding.

**Recommendation:**  
Check the Origin header in the WebSocket upgrade:
```rust
async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    state: axum::extract::State<Arc<RelayState>>,
) -> impl IntoResponse {
    let allowed_origins = ["https://chat.united-humanity.us", "http://localhost:3210"];
    let origin = headers.get("origin").and_then(|v| v.to_str().ok()).unwrap_or("");
    if !allowed_origins.contains(&origin) {
        return (StatusCode::FORBIDDEN, "Invalid origin").into_response();
    }
    ws.on_upgrade(move |socket| handle_socket(socket, state.0)).into_response()
}
```

---

### FINDING 10 — Weak Link Code Generation (Predictable Codes)

- **Severity:** 🟡 **MEDIUM**
- **Category:** Cryptography / AuthZ
- **Location:** `storage.rs` lines 210-218 (`create_link_code`)

**Description:**  
Link codes are generated using a deterministic hash of `timestamp + created_by + name`:
```rust
let raw = format!("{}{}{}", now, created_by, name);
let mut hash: u64 = 0;
for b in raw.bytes() {
    hash = hash.wrapping_mul(31).wrapping_add(b as u64);
}
let code = format!("{:06X}", hash % 0xFFFFFF);
```

This is:
1. **Only 6 hex chars** = 16.7 million possible codes. Brute-forceable.
2. **Deterministic** — if you know the user's name and approximate time, you can predict the code.
3. **Only 5-minute window** mitigates but doesn't eliminate risk.

The same issue exists for invite codes (`create_invite_code`) with 8 hex chars.

**Impact:** An attacker who knows when a link code was generated can potentially guess it and steal an identity.

**Recommendation:**  
Use cryptographically random codes:
```rust
use rand::Rng;
let code: String = rand::thread_rng()
    .sample_iter(&rand::distributions::Alphanumeric)
    .take(12)
    .map(char::from)
    .collect();
```

---

### FINDING 11 — Admin Keys Exposed in systemd Service File

- **Severity:** 🟡 **MEDIUM**
- **Category:** Information Disclosure
- **Location:** `/etc/systemd/system/humanity-relay.service`

**Description:**  
The service file contains:
```
Environment=ADMIN_KEYS=2e293bac8a7f600e...
Environment=WEBHOOK_TOKEN=5385cde4cdd9b92b...
```

Systemd environment variables are visible to any user on the system via `/proc/<pid>/environ` (if they can read the process) and in the systemd journal. This exposes admin key fingerprints and the webhook authentication token.

**Impact:** Any user with shell access to the VPS can discover admin keys and the webhook token.

**Recommendation:**  
Use an `EnvironmentFile` instead:
```ini
# /etc/systemd/system/humanity-relay.service
EnvironmentFile=/opt/Humanity/.env

# /opt/Humanity/.env (chmod 600, owned by root)
ADMIN_KEYS=2e293bac...
WEBHOOK_TOKEN=5385cde4...
```

---

### FINDING 12 — Upload Content-Type Check Is Client-Trusting

- **Severity:** 🟡 **MEDIUM**
- **Category:** Upload Security
- **Location:** `api.rs` lines 130-135

**Description:**  
The upload endpoint checks `field.content_type()` which comes from the `Content-Type` header in the multipart form data. This is entirely client-controlled. An attacker can upload an HTML file with `Content-Type: image/png`:

```bash
curl -F 'file=@evil.html;type=image/png' https://chat.united-humanity.us/api/upload
```

The file is saved with a `.png` extension (based on content-type mapping), but if the attacker can find a way to serve it without the extension, or if browsers ignore extensions, this could lead to HTML injection.

Additionally, the `/uploads/` path serves files via `ServeDir` which may set Content-Type based on the file extension, so the `.png` extension provides some protection. But the file content is unchecked.

**Impact:** Low — the extension mapping provides defense, but belt-and-suspenders is important. If any future change serves raw filenames, this becomes exploitable.

**Recommendation:**  
1. Add magic byte validation (check PNG/JPEG/GIF/WebP file headers):
```rust
let magic = &data[..4.min(data.len())];
let valid_magic = match content_type.as_str() {
    "image/png" => magic.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
    "image/jpeg" => magic.starts_with(&[0xFF, 0xD8]),
    "image/gif" => magic.starts_with(b"GIF8"),
    "image/webp" => data.len() > 12 && &data[8..12] == b"WEBP",
    _ => false,
};
if !valid_magic {
    return Err((StatusCode::BAD_REQUEST, "File content doesn't match declared type".into()));
}
```
2. Serve uploaded files with `Content-Disposition: attachment` or explicit `Content-Type` headers.

---

### FINDING 13 — No Per-Connection Limit on WebSocket Connections

- **Severity:** 🟡 **MEDIUM**
- **Category:** DoS
- **Location:** `main.rs` (server-level), nginx config `limit_conn ws_conn 10`

**Description:**  
Nginx limits concurrent connections per IP to 10 (`limit_conn ws_conn 10`). However:
1. This applies to the entire `/` location, not just WebSocket upgrades
2. An attacker using multiple IPs (e.g., Tor exit nodes, cloud functions) can open many more
3. The relay server itself has no internal connection limit — each connection spawns tasks and allocates broadcast channel subscribers

**Impact:** An attacker with multiple IPs can exhaust server memory with WebSocket connections, each of which allocates a broadcast receiver with 256 message buffer.

**Recommendation:**  
1. Add server-side connection limiting (track total WebSocket count, reject above threshold):
```rust
// In RelayState, add: pub connection_count: AtomicUsize
// In handle_connection, increment on connect, decrement on disconnect
// Reject if > MAX_CONNECTIONS (e.g., 500)
```
2. Consider requiring the identify handshake within a timeout (currently, a client can connect and never identify, holding resources indefinitely).

---

### FINDING 14 — Typing Indicator Flood Not Rate Limited

- **Severity:** 🟢 **LOW**
- **Category:** DoS
- **Location:** `relay.rs` lines 1280-1290 (Typing handler)

**Description:**  
The Typing message handler broadcasts immediately without any rate limiting:
```rust
RelayMessage::Typing { .. } => {
    let typing = RelayMessage::Typing { ... };
    let _ = state_clone.broadcast_tx.send(typing);
}
```

A malicious client can flood typing indicators. The client-side throttles to 2 seconds, but a custom WebSocket client bypasses this.

**Impact:** Minor DoS — typing indicators are lightweight, but flooding the broadcast channel (256 capacity) could cause legitimate messages to be dropped.

**Recommendation:**  
Add server-side throttle for typing indicators (max 1 per 2 seconds per key).

---

### FINDING 15 — Kick Command Doesn't Actually Disconnect the Socket

- **Severity:** 🟡 **MEDIUM**
- **Category:** AuthZ (Incomplete Enforcement)
- **Location:** `relay.rs`, `handle_mod_command()` `/kick` branch

**Description:**  
The `/kick` command removes the peer from the `peers` HashMap and broadcasts a system message, but it **does not close the WebSocket connection**. The kicked user's recv/send tasks continue running. They can still send messages (though they may not appear in the peer list). On the next message, they'll get errors because their peer entry is gone.

Similarly, `/ban` removes the peer but doesn't close the socket. The banned user can continue sending messages until they disconnect and try to reconnect.

**Impact:** Banned/kicked users can continue interacting until they manually disconnect. The ban only takes effect on reconnect.

**Recommendation:**  
Implement a mechanism to forcibly close WebSocket connections. Options:
1. Maintain a map of `public_key → CancellationToken` and trigger it on kick/ban
2. Check ban status on each message send (already done for mute, but not for ban)

---

### FINDING 16 — Disk Space Exhaustion via Uploads

- **Severity:** 🟡 **MEDIUM**
- **Category:** DoS / Resource Exhaustion
- **Location:** `api.rs` upload handler, `storage.rs` FIFO logic

**Description:**  
The per-user FIFO limits each key to 4 images. However:
1. The `?key=` parameter is optional — omitting it bypasses FIFO entirely
2. Even with FIFO, an attacker can generate new keys indefinitely (just random hex)
3. Each upload can be up to 5MB
4. There is no global disk space check

At 5MB per upload with the nginx rate limit of 2 uploads/minute, an attacker can fill ~600MB/hour. Across multiple IPs, this scales linearly.

**Impact:** Disk exhaustion leading to server crash.

**Recommendation:**  
1. Make the `key` parameter required
2. Add global disk space monitoring (check available space before writing)
3. Limit total upload directory size (e.g., 1GB)
4. Require the key to be a registered/connected user

---

### FINDING 17 — Health Endpoint Information Disclosure

- **Severity:** 🟢 **LOW**
- **Category:** Information Disclosure
- **Location:** `main.rs` lines 85-93 (`health` handler)

**Description:**  
The `/health` endpoint exposes:
```json
{
  "status": "ok",
  "uptime_seconds": 123456,
  "total_messages": 5000,
  "connected_peers": 12
}
```

This is publicly accessible and reveals:
- Server uptime (when it was last restarted — useful for timing attacks after patches)
- Total message count (business intelligence)
- Connected peer count (useful for timing attacks)

**Impact:** Minor information leakage useful for reconnaissance.

**Recommendation:**  
Either restrict `/health` to localhost/internal or remove `uptime_seconds` and `total_messages`:
```json
{ "status": "ok" }
```

---

### FINDING 18 — Signature Verification Not Enforced Server-Side

- **Severity:** 🟡 **MEDIUM**
- **Category:** Authentication / Integrity
- **Location:** `relay.rs`, Chat message handler

**Description:**  
The server **never verifies Ed25519 signatures**. The `signature` field is passed through and stored, but never checked. Signatures are only verified client-side (in the browser). This means:
1. A client can send messages with a fake `signature` field that appears valid
2. Since the server overrides the `from` field to `my_key_for_recv`, the signature mismatch isn't caught
3. The `from` override is correct (prevents spoofing), but signatures become purely cosmetic

**Impact:** The ✓ signed badge on messages can be faked. Users may place false trust in "signed" messages.

**Recommendation:**  
Either:
1. Verify signatures server-side (requires adding Ed25519 verification to the Rust server — use the `ed25519-dalek` crate)
2. Or clearly document that signatures are client-verified only and the badge is best-effort

---

### FINDING 19 — localStorage Fallback Key Not Protected

- **Severity:** 🟢 **LOW**
- **Category:** Client-Side Security
- **Location:** `crypto.js` lines 116-121

**Description:**  
When Ed25519 is not supported, the identity falls back to a random key stored in `localStorage`:
```javascript
key = localStorage.getItem('humanity_key');
```

`localStorage` is:
1. Accessible to any JavaScript running on the same origin (XSS = key theft)
2. Readable by browser extensions
3. Not encrypted on disk

The Ed25519 path stores keys in IndexedDB with `extractable: false`, which is significantly more secure. The fallback path is much weaker.

**Impact:** On browsers without Ed25519 support, identity can be stolen via XSS or local access.

**Recommendation:**  
Acceptable trade-off for MVP. Most modern browsers support Ed25519. Consider showing a stronger warning for the fallback case.

---

### FINDING 20 — Webhook Token Sent in Environment, Not Rotated

- **Severity:** 🟢 **LOW**
- **Category:** Secret Management
- **Location:** systemd service file, `relay.rs` webhook config

**Description:**  
The webhook token (`WEBHOOK_TOKEN`) is a static value in the systemd service file. It's used to authenticate outbound webhook calls. If this token is compromised, an attacker could receive webhook notifications containing all chat messages (user names + content).

**Impact:** Webhook interception reveals all chat content.

**Recommendation:**  
Rotate the webhook token periodically. Store in a separate env file with restricted permissions.

---

### FINDING 21 — No `Strict-Transport-Security` (HSTS) Header

- **Severity:** 🟡 **MEDIUM**
- **Category:** Transport Security
- **Location:** nginx config

**Description:**  
While TLS is configured with proper cipher suites and protocols (TLS 1.2+), there is no HSTS header. Users who first visit via HTTP will be redirected to HTTPS, but during that first request, they're vulnerable to downgrade attacks (SSL stripping).

**Impact:** First-visit users can be MITM'd via SSL stripping.

**Recommendation:**  
Add to nginx:
```nginx
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
```

---

### FINDING 22 — Emoji Field in Reactions Not Validated (Server)

- **Severity:** 🟡 **MEDIUM** (see FINDING 4 for full XSS chain)
- **Category:** Input Validation
- **Location:** `relay.rs` ~line 1300, Reaction handler

**Description:**  
The server broadcasts the `emoji` field from Reaction messages without any validation. This allows arbitrary string content to be broadcast as an "emoji." This is the server-side component of FINDING 4's XSS.

**Recommendation:**  
Validate the emoji field server-side. Either whitelist known emoji or limit to ≤20 bytes of valid Unicode emoji characters.

---

### FINDING 23 — channel Field Not Validated in Chat Messages

- **Severity:** 🟢 **LOW**
- **Category:** Input Validation
- **Location:** `relay.rs`, Chat handler (line ~1224)

**Description:**  
The `channel` field in Chat messages is checked for read-only status, but there's no validation that the channel actually exists. A client can post messages to a non-existent channel ID, which will be stored in the database and appear if that channel is later created.

**Impact:** Minor — messages to non-existent channels are invisible to users but waste database space.

**Recommendation:**  
Validate that the channel exists before storing the message.

---

### FINDING 24 — `ServeDir` for Uploads May Allow Directory Listing

- **Severity:** 🟢 **LOW**
- **Category:** Information Disclosure
- **Location:** `main.rs` line 70

**Description:**  
```rust
.nest_service("/uploads", tower_http::services::ServeDir::new("data/uploads"))
```

Depending on tower-http version and configuration, `ServeDir` may or may not serve directory listings. If it does, an attacker can browse all uploaded files.

**Impact:** All uploaded images become browsable via directory listing.

**Recommendation:**  
Explicitly disable directory listing:
```rust
tower_http::services::ServeDir::new("data/uploads")
    .not_found_service(tower_http::services::ServeFile::new("client/index.html"))
```
Or ensure tower-http's `ServeDir` doesn't list directories (default in recent versions is no listing, but verify).

---

### FINDING 25 — Rate Limit State Is In-Memory Only

- **Severity:** 🟢 **LOW**
- **Category:** DoS / Rate Limiting Bypass
- **Location:** `relay.rs`, `rate_limits: RwLock<HashMap<...>>`

**Description:**  
Rate limiting state is stored in memory. Server restarts reset all rate limits. An attacker who knows when the server restarts (FINDING 17 shows uptime) can time their attack.

Additionally, the rate limit is per public key, but keys are free to generate. A determined attacker can generate a new key for each burst.

**Impact:** Rate limiting is effective against casual abuse but bypassable by sophisticated attackers.

**Recommendation:**  
Acceptable for MVP. Consider IP-based rate limiting at the nginx layer (already partially in place) as the primary defense. The application-level rate limiting is a good secondary layer.

---

## Summary Table

| # | Severity | Category | Finding | Must Fix Before Launch? |
|---|----------|----------|---------|------------------------|
| 1 | 🔴 CRITICAL | AuthZ | Unauthenticated Bot API | **YES** |
| 2 | 🔴 CRITICAL | XSS | Stored XSS via image URL onclick injection | **YES** |
| 5 | 🔴 CRITICAL | AuthZ | Bot API bypasses all security controls | **YES** (same fix as #1) |
| 3 | 🟠 HIGH | XSS | Peer list onclick XSS | **YES** |
| 4 | 🟠 HIGH | XSS | Reaction emoji onclick XSS | **YES** |
| 6 | 🟠 HIGH | CORS | Permissive CORS allows any origin | **YES** |
| 8 | 🟠 HIGH | Headers | Missing Content-Security-Policy | **YES** |
| 7 | 🟡 MEDIUM | DoS | No WebSocket message size limit | **YES** |
| 9 | 🟡 MEDIUM | WS | No WebSocket origin check | **YES** |
| 10 | 🟡 MEDIUM | Crypto | Predictable link/invite codes | Recommended |
| 11 | 🟡 MEDIUM | InfoDisc | Admin keys in systemd env | Recommended |
| 12 | 🟡 MEDIUM | Upload | Content-type check trusts client | Recommended |
| 13 | 🟡 MEDIUM | DoS | No server-side connection limit | Recommended |
| 15 | 🟡 MEDIUM | AuthZ | Kick/ban doesn't close socket | Recommended |
| 16 | 🟡 MEDIUM | DoS | Disk exhaustion via uploads | Recommended |
| 18 | 🟡 MEDIUM | AuthN | Signatures not verified server-side | Can wait |
| 21 | 🟡 MEDIUM | TLS | Missing HSTS header | **YES** (easy) |
| 22 | 🟡 MEDIUM | Validation | Emoji field not validated | **YES** (same fix as #4) |
| 14 | 🟢 LOW | DoS | Typing indicator flood | Can wait |
| 17 | 🟢 LOW | InfoDisc | Health endpoint reveals too much | Can wait |
| 19 | 🟢 LOW | Client | localStorage fallback key exposure | Can wait |
| 20 | 🟢 LOW | Secrets | Static webhook token | Can wait |
| 23 | 🟢 LOW | Validation | Channel field not validated | Can wait |
| 24 | 🟢 LOW | InfoDisc | Possible upload directory listing | Can wait |
| 25 | 🟢 LOW | DoS | In-memory rate limit state | Can wait |

---

## Positive Findings (Things Done Right)

1. ✅ **All SQL queries use parameterized queries** — no SQL injection anywhere in `storage.rs`
2. ✅ **Server overrides `from` field** in Chat messages to `my_key_for_recv` — prevents identity spoofing via WebSocket
3. ✅ **`esc()` is called before `innerHTML` assignment** in `formatBody()` — correct escaping order
4. ✅ **Name validation** restricts to ASCII alphanumeric + `_-` (prevents homoglyph attacks)
5. ✅ **Ban check on connect** prevents banned users from reconnecting
6. ✅ **Mute check on send** prevents muted users from posting
7. ✅ **Read-only channel enforcement** server-side
8. ✅ **Admin command authorization** checked server-side for every command
9. ✅ **Ed25519 keys stored with extractable:false** in IndexedDB (strongest browser protection)
10. ✅ **TLS 1.2+ with strong ciphers** via Certbot config
11. ✅ **UFW firewall** restricts to ports 22, 80, 443
12. ✅ **Privacy-preserving nginx logs** (no IP addresses logged)
13. ✅ **Auto-lockdown** when no mods are online is a clever defense
14. ✅ **Upload FIFO** (4 per user) prevents individual storage abuse
15. ✅ **Fibonacci rate limiting** is creative and effective against human spammers

---

## Priority Fix Order for Launch

### Must Fix (blocking launch):
1. **Add API authentication** to `/api/send` and `/api/github-webhook` (Findings 1, 5)
2. **Fix XSS in image URL onclick** — use data attributes instead of inline handlers (Finding 2)
3. **Fix XSS in reactions** — validate emoji server-side + use data attributes client-side (Findings 4, 22)
4. **Fix XSS in peer list** — escape single quotes or use addEventListener (Finding 3)
5. **Restrict CORS** to `https://chat.united-humanity.us` (Finding 6)
6. **Set WebSocket max message size** to 128KB (Finding 7)
7. **Add HSTS header** in nginx (Finding 21) — one line
8. **Add basic CSP header** in nginx (Finding 8) — even with `unsafe-inline` it helps

### Should Fix (first week after launch):
9. Implement WebSocket origin checking (Finding 9)
10. Use cryptographic random codes for link/invite (Finding 10)
11. Move secrets to EnvironmentFile (Finding 11)
12. Add magic byte validation for uploads (Finding 12)
13. Make upload key parameter required (Finding 16)
14. Add server-side connection limits (Finding 13)

### Can Wait (first month):
15. Force-disconnect on kick/ban (Finding 15)
16. Server-side signature verification (Finding 18)
17. Reduce health endpoint information (Finding 17)
18. Typing indicator rate limiting (Finding 14)
