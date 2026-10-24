//! Per-address limits on every HTTP route (2026-10-22, Tier 1 security residual 1).
//!
//! WHY. Until now nothing in the relay limited HTTP requests by where they came from. The bot
//! API's `POST /api/send` (API_SECRET) could be guessed at without end, every public read
//! (search, a Solana balance the relay fetches on the asker's behalf, a zk proof it checks) could
//! be asked for as fast as a script could ask, and every signed POST costs a Dilithium3 check
//! before it can be refused. nginx's `limit_req` covers the deployed servers, but only where its
//! zones are configured, and not a self-hosted relay without them. This is the relay's own floor.
//!
//! HOW. Every request goes through [`edge`], one layer over the whole router (the same reasoning
//! as mod.rs `feature_gate`: a per-handler check is how one route gets missed). It decides the
//! client's address (client_addr.rs: headers are believed only from a trusted proxy), rewrites
//! the headers so every reader after it sees that one address, sorts the route into a [`Class`],
//! and takes one request from that address's allowance for the class.
//!
//! THE ALLOWANCE is a bucket that refills at the class's steady rate up to its burst, and the
//! Fibonacci backoff the chat limiter uses (relay.rs `FIB_DELAYS`, 1, 1, 2, 3, 5, 8, 13, 21 s):
//! a request that finds the bucket empty is answered 429 with `Retry-After`, and the address is
//! held for at least the next delay in the sequence. A request during the hold moves it one step
//! further, so an address that keeps hammering waits longer each time, up to 21 s; one that waits
//! as told is not pushed further, and a minute without a refusal starts the sequence again.
//!
//! BOUNDED MEMORY. The table holds at most [`IpLimits::cap`] addresses (50,000 by default, a few
//! MB). When it is full it drops the addresses that have been quiet for two minutes, at most once
//! a second; when it is still full, a new address shares one OVERFLOW bucket per class with every
//! other address that did not fit. So a flood of addresses (an IPv6 /64 holds 2^64 of them, which
//! is why an IPv6 address counts as its /64) can neither grow the table nor escape the limit:
//! the newcomers are limited together, more strictly, never not at all.
//!
//! THE PROXY'S OWN REQUESTS. A request that resolves to a trusted proxy's own address came
//! through nginx without the header that names its client (or from a process on the same
//! machine, such as a bot). Every such request shares one bucket, so its allowance is ten times
//! the class's: a location in nginx missing `proxy_set_header X-Real-IP $remote_addr;` would
//! otherwise put every visitor in one bucket at a person's rate. The fix is that header (see
//! docs/admin/SELF-HOSTING.md); the tenfold allowance only keeps the site usable meanwhile.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::relay::client_addr::{self, TrustedProxies};
use crate::relay::relay::{RelayState, FIB_DELAYS};

/// What kind of request a route is, for its allowance. Every route is one of these: [`Class::of`]
/// has no "unlimited" answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// A route a bot or a hook authenticates with a shared secret: `POST /api/send`, the GitHub
    /// webhook, and the task and project writes (API_SECRET). The strictest, so a secret cannot be
    /// guessed at speed.
    Bot,
    /// A public route that costs the relay real work per request: full-text search, the skills
    /// search, a Solana balance (fetched from Solana's servers), a zk proof check, trust scores
    /// and proposal tallies.
    Costly,
    /// Any other request that changes something (POST, PUT, PATCH, DELETE under /api/): signed
    /// writes cost a Dilithium3 check each before a bad one can be refused.
    Write,
    /// Uploads, the pieces of a resumable upload, and another server's continuous backup kept
    /// here (/api/backup-host/): many requests for one big file.
    Upload,
    /// Any other read under /api/, an item's page under /files/, and /health.
    Read,
    /// Opening a socket: /ws (chat and federation) and /ws/live/ (video).
    Socket,
    /// Files: /uploads/ and the static fallback. A page with many pictures asks for many.
    Static,
}

/// A class's allowance: how many requests at once, and how many a minute after that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Allowance {
    pub burst: f64,
    pub per_minute: f64,
}

impl Class {
    /// Every class, for the admin view and the tests.
    pub const ALL: [Class; 7] = [Class::Bot, Class::Costly, Class::Write, Class::Upload, Class::Read, Class::Socket, Class::Static];

    /// Its name, as the admin view and the log show it.
    pub fn key(self) -> &'static str {
        match self {
            Class::Bot => "bot",
            Class::Costly => "costly",
            Class::Write => "write",
            Class::Upload => "upload",
            Class::Read => "read",
            Class::Socket => "socket",
            Class::Static => "static",
        }
    }

    /// Its allowance per address. Generous for a person, a household or a classroom behind one
    /// address; a script asking hundreds of times a second is refused within its first second.
    pub fn allowance(self) -> Allowance {
        let (burst, per_minute) = match self {
            Class::Bot => (20.0, 60.0),
            Class::Costly => (20.0, 60.0),
            Class::Write => (30.0, 120.0),
            Class::Upload => (60.0, 600.0),
            Class::Read => (240.0, 1_200.0),
            Class::Socket => (60.0, 120.0),
            Class::Static => (600.0, 3_000.0),
        };
        Allowance { burst, per_minute }
    }

    /// The class of a request, by its method and path. Read this as the audit of every route in
    /// mod.rs `build_router`: each falls in exactly one class here.
    pub fn of(method: &Method, path: &str) -> Class {
        let reads = matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
        if path == "/ws" || path.starts_with("/ws/") {
            return Class::Socket;
        }
        // Uploads, and another server's continuous backup sending its pieces here (backup_stream
        // host.rs): many requests for one big thing, from one address.
        if path == "/api/upload" || path.starts_with("/api/upload/") || path.starts_with("/api/backup-host/") {
            return Class::Upload;
        }
        if path == "/api/send" || path == "/api/github-webhook" {
            return Class::Bot;
        }
        if !reads && (path.starts_with("/api/tasks") || path.starts_with("/api/projects")) {
            return Class::Bot;
        }
        let costly = path == "/api/search"
            || path == "/api/skills/search"
            || path.starts_with("/api/v2/solana/")
            || path == "/api/v2/zk/verify"
            || path.starts_with("/api/v2/trust/")
            || (path.starts_with("/api/v2/proposals/") && (path.ends_with("/tally") || path.ends_with("/chain")));
        if costly {
            return Class::Costly;
        }
        if path.starts_with("/api/") {
            return if reads { Class::Read } else { Class::Write };
        }
        if path == "/health" || path.starts_with("/files/") {
            return Class::Read;
        }
        Class::Static
    }
}

/// The answer for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Go,
    /// Refused: try again in this many whole seconds (at least 1).
    Wait(u64),
}

/// One address's place in one class's allowance.
#[derive(Debug, Clone)]
struct Entry {
    tokens: f64,
    refilled: Instant,
    /// Refused until then: the Fibonacci hold.
    held_until: Option<Instant>,
    /// Steps along `FIB_DELAYS` the next refusal takes.
    strikes: usize,
    /// When it was last refused, so a quiet minute starts the sequence again.
    last_refused: Option<Instant>,
    seen: Instant,
}

/// The bucket name every address shares when the table is full.
pub const OVERFLOW: &str = "overflow";
/// How many addresses the table holds by default.
pub const DEFAULT_CAP: usize = 50_000;
/// A minute without a refusal starts the Fibonacci sequence again.
const CLEAN_AFTER: Duration = Duration::from_secs(60);
/// An address quiet this long holds nothing a new entry would not (its bucket is full again, its
/// strikes forgotten), so a full table may drop it.
const IDLE_AFTER: Duration = Duration::from_secs(120);
/// A full table sweeps for idle entries at most this often, so a flood of new addresses cannot
/// make every request scan the whole table.
const SWEEP_EVERY: Duration = Duration::from_secs(1);
/// The allowance of a trusted proxy's own address (see the top of this file).
const VIA_PROXY_SCALE: f64 = 10.0;

#[derive(Debug, Default)]
struct Table {
    entries: HashMap<(Class, String), Entry>,
    last_sweep: Option<Instant>,
    /// Requests refused since the start, by class, for the admin view.
    refused: HashMap<Class, u64>,
    /// Requests from addresses that did not fit, which shared the overflow bucket, since the start.
    overflowed: u64,
}

/// The relay's per-address limits (`RelayState::ip_limits`).
#[derive(Debug)]
pub struct IpLimits {
    trusted: RwLock<TrustedProxies>,
    table: Mutex<Table>,
    cap: usize,
}

impl Default for IpLimits {
    fn default() -> Self {
        IpLimits { trusted: RwLock::new(TrustedProxies::default()), table: Mutex::new(Table::default()), cap: DEFAULT_CAP }
    }
}

impl IpLimits {
    /// Limits with a table of at most `cap` addresses (tests).
    pub fn with_cap(cap: usize) -> IpLimits {
        IpLimits { cap, ..IpLimits::default() }
    }

    /// The trusted proxies in force.
    pub fn trusted(&self) -> TrustedProxies {
        self.trusted.read().map(|t| t.clone()).unwrap_or_else(|_| TrustedProxies::none())
    }

    /// Replace the trusted proxies (tests, and a future setting).
    pub fn set_trusted(&self, t: TrustedProxies) {
        if let Ok(mut held) = self.trusted.write() {
            *held = t;
        }
    }

    /// How many addresses the table holds most.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// How many entries the table holds now.
    pub fn tracked(&self) -> usize {
        self.table.lock().map(|t| t.entries.len()).unwrap_or(0)
    }

    /// Take one request of `class` from `bucket` at `now`. `via_proxy`: the address is a trusted
    /// proxy's own, so the allowance is ten times the class's.
    pub fn take(&self, class: Class, bucket: &str, via_proxy: bool, now: Instant) -> Verdict {
        let Ok(mut table) = self.table.lock() else {
            return Verdict::Go;
        };
        let mut key = (class, bucket.to_string());
        if !table.entries.contains_key(&key) && table.entries.len() >= self.cap {
            let due = table.last_sweep.map_or(true, |at| now.saturating_duration_since(at) >= SWEEP_EVERY);
            if due {
                table.last_sweep = Some(now);
                table.entries.retain(|_, e| {
                    e.held_until.is_some_and(|until| until > now) || now.saturating_duration_since(e.seen) < IDLE_AFTER
                });
            }
            if table.entries.len() >= self.cap {
                table.overflowed += 1;
                key = (class, OVERFLOW.to_string());
            }
        }
        let allowance = class.allowance();
        let scale = if via_proxy { VIA_PROXY_SCALE } else { 1.0 };
        let (burst, per_sec) = (allowance.burst * scale, allowance.per_minute * scale / 60.0);
        let entry = table.entries.entry(key).or_insert(Entry {
            tokens: burst,
            refilled: now,
            held_until: None,
            strikes: 0,
            last_refused: None,
            seen: now,
        });
        entry.seen = now;
        let elapsed = now.saturating_duration_since(entry.refilled).as_secs_f64();
        entry.tokens = (entry.tokens + elapsed * per_sec).min(burst);
        entry.refilled = now;
        if entry.last_refused.is_some_and(|at| now.saturating_duration_since(at) >= CLEAN_AFTER) {
            entry.strikes = 0;
        }
        // `strikes` points at the NEXT delay in the sequence: each refusal waits that long (or for
        // the next token, when that is longer) and moves it one step on.
        let held = entry.held_until.is_some_and(|until| until > now);
        let verdict = if !held && entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            Verdict::Go
        } else {
            // Refused: the bucket is empty, or it asked again during its hold.
            let refill = if held { 0 } else { ((1.0 - entry.tokens) / per_sec).ceil().max(1.0) as u64 };
            let wait = refill.max(FIB_DELAYS[entry.strikes]);
            entry.strikes = (entry.strikes + 1).min(FIB_DELAYS.len() - 1);
            entry.held_until = Some(now + Duration::from_secs(wait));
            entry.last_refused = Some(now);
            Verdict::Wait(wait)
        };
        if let Verdict::Wait(_) = verdict {
            *table.refused.entry(class).or_default() += 1;
        }
        verdict
    }

    /// What the admin page shows: addresses tracked and held now, the cap, requests refused by
    /// class since the start, and how many requests had to share the overflow bucket.
    pub fn summary(&self, now: Instant) -> serde_json::Value {
        let Ok(table) = self.table.lock() else {
            return serde_json::Value::Null;
        };
        let held = table.entries.values().filter(|e| e.held_until.is_some_and(|until| until > now)).count();
        let refused: serde_json::Map<String, serde_json::Value> =
            Class::ALL.iter().map(|c| (c.key().to_string(), serde_json::json!(table.refused.get(c).copied().unwrap_or(0)))).collect();
        serde_json::json!({
            "tracked": table.entries.len(),
            "held_now": held,
            "cap": self.cap,
            "refused": refused,
            "overflowed": table.overflowed,
        })
    }
}

/// The edge every request passes first (mod.rs `build_router`): decide the client's address,
/// rewrite the headers to it, and take the request from its allowance, answering 429 with
/// `Retry-After` when it is spent.
pub async fn edge(State(state): State<Arc<RelayState>>, mut req: Request, next: Next) -> Response {
    let peer = req.extensions().get::<ConnectInfo<std::net::SocketAddr>>().map(|c| c.0.ip());
    let trusted = state.ip_limits.trusted();
    let ip = client_addr::client_ip(peer, req.headers(), &trusted);
    client_addr::rewrite(req.headers_mut(), ip);
    let class = Class::of(req.method(), req.uri().path());
    let via_proxy = ip.is_some_and(|ip| trusted.trusts(ip));
    let bucket = client_addr::bucket(ip);
    match state.ip_limits.take(class, &bucket, via_proxy, Instant::now()) {
        Verdict::Go => next.run(req).await,
        Verdict::Wait(secs) => {
            tracing::debug!("Too many {} requests from {bucket}: wait {secs}s", class.key());
            too_many(secs)
        }
    }
}

/// The refusal: 429 with `Retry-After` and a sentence that says how long.
pub fn too_many(secs: u64) -> Response {
    let mut res = (
        StatusCode::TOO_MANY_REQUESTS,
        format!("Too many requests from your address. Try again in {secs} second{}.", if secs == 1 { "" } else { "s" }),
    )
        .into_response();
    if let Ok(v) = HeaderValue::from_str(&secs.to_string()) {
        res.headers_mut().insert(axum::http::header::RETRY_AFTER, v);
    }
    res
}

#[cfg(test)]
#[path = "ip_limit_tests.rs"]
mod tests;
