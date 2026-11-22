//! The one check every signed REQUEST to this relay passes (2026-11-20; round 2 2026-11-21).
//!
//! A request (an account export, a bug vote, a vault sync, a channel read, an admin action, a file
//! edit) proves its caller holds a key by a Dilithium3 signature over
//! `humanity_protocol::signed_request::request_preimage`: `hum/request/v2`, the AUDIENCE (the
//! address the person's app sent it to), the purpose, the time. Round 1 moved requests off the shape
//! a chat message is signed over and made each signature good once; round 2 made each name its
//! server, so a request a person sent to another server is never good here. The crate module says the
//! whole story; this file is the relay's half:
//!
//! - [`check`]: in order, the cheap checks first (a key and a signature at all; the key in its one
//!   canonical form, lowercase hex, D3; a purpose of at most 2,048 bytes; a time at most
//!   [`SIGNED_WINDOW_MS`] old and at most [`SIGNED_AHEAD_MS`] ahead), then the audience (one of this
//!   relay's own names as reached, D1) and the signature (off the async executor; a panic fails
//!   closed) together, the signature verified whatever the audience and both refused in the same
//!   words (L4, 2026-11-21), then a time not older than this relay's own start (D6, [`StartedAt`]),
//!   and only then the replay memory (D4, `replay_memory.rs`): the (key, `tag`, time, purpose) never
//!   seen before, so one signature is good once. `tag` is a `&'static str`, the route's own fixed
//!   name: request text never names an entry.
//! - The audience an HTTP request claims is its `Host` (audience.rs `scope_host`); a socket passes
//!   the [`Arrival`](crate::relay::audience::Arrival) its upgrade carried to [`check_for`]. Either way
//!   audience.rs `OwnNames::admit` decides whether the name is this relay's and may be used from
//!   where the request came (round 3: a loopback or LAN name only from a client there).
//! - [`verify`]: the signature alone, for tests and a caller that keeps its own replay memory.
//!
//! Every route maps a [`SignedRefusal`] to its own answer shape; [`SignedRefusal::plain`] is the
//! common `(StatusCode, String)` one. No route builds the signed bytes itself.

use axum::http::StatusCode;

use crate::relay::relay::RelayState;
pub use humanity_protocol::signed_request::{body_hash, request_preimage, starts_with_signing_domain, MAX_PURPOSE_BYTES, REQUEST_DOMAIN};

/// How old a signed request's timestamp may be.
pub const SIGNED_WINDOW_MS: u64 = 5 * 60 * 1000;
/// How far ahead of the relay's clock a signed request's timestamp may be: small, so a request is
/// never accepted after the replay memory (six minutes from first sight) forgets it.
pub const SIGNED_AHEAD_MS: u64 = 30 * 1000;

/// Why a signed request was refused. Each route answers in its own words and shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedRefusal {
    /// No key or no signature at all.
    Unsigned,
    /// The key is not a Dilithium3 key written in lowercase hex (D3), or the purpose is too long.
    Malformed,
    /// The time is too old, or too far in the future.
    Clock,
    /// The time is older than this relay's last start (D6): a request from before a restart, or,
    /// when the restart was more than a few seconds ago, from a device whose clock is behind this
    /// server's (by about the seconds given: round 3, C6).
    BeforeStart(Option<u64>),
    /// The time is older than this relay's start, and the relay's clock now reads earlier than that
    /// start: its clock was set back since it started (L2, 2026-11-21), so for about the minutes
    /// given it cannot tell a new request from one made before its restart. A restart clears it.
    StartAhead(u64),
    /// Not for this relay as it was reached (D1): another address, or a name the client may not use
    /// from where it is. The words are audience.rs `REFUSED_AS_REACHED` for every such case.
    Audience(String),
    /// The signature is not the key's over this request. Answered exactly as [`Self::Audience`] is
    /// (L4, 2026-11-21): the same words, code and status, the signature verified whatever the
    /// audience, so a prober's made-up signature cannot tell which names this relay answers to.
    Signature,
    /// This exact signed request was already used.
    Replay,
}

impl SignedRefusal {
    /// A short code a client can choose its own words by.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsigned => "unsigned",
            Self::Malformed => "malformed",
            Self::Clock => "clock",
            Self::BeforeStart(_) | Self::StartAhead(_) => "before_start",
            // One code for both (L4): a different code would tell a refused name from a bad signature.
            Self::Audience(_) | Self::Signature => "signature",
            Self::Replay => "replay",
        }
    }

    /// The relay's own words for it.
    pub fn words(&self) -> String {
        match self {
            Self::Unsigned => "This request needs your key and its signature.".to_string(),
            Self::Malformed => "The key must be a full key in lowercase hex, and the request no longer than 2,048 characters.".to_string(),
            Self::Clock => "The request is too old or from the future: check this device's clock and try again.".to_string(),
            Self::BeforeStart(None) => "The request was made before this server last restarted: try again.".to_string(),
            Self::BeforeStart(Some(s)) => format!(
                "Your clock is behind this server's by about {s} seconds, so this server, which restarted since that time, \
                 cannot tell this request from one made before its restart. Set this device's clock automatically (on \
                 Windows: Settings, Time & language, Set time automatically) and try again."
            ),
            Self::StartAhead(m) => format!(
                "This server's clock was set back since it started, so for about {m} more minute{} it cannot tell a new \
                 request from one made before its restart, and takes neither. Try again then. If you run this server, \
                 restarting it clears this at once; its clock needs setting automatically.",
                if *m == 1 { "" } else { "s" }
            ),
            Self::Audience(words) => words.clone(),
            Self::Signature => crate::relay::audience::REFUSED_AS_REACHED.to_string(),
            Self::Replay => "That request was already used: try again.".to_string(),
        }
    }

    /// The status a refusal answers with: a stale clock or a malformed request is the caller's
    /// request being malformed, the rest are a failure to prove the key here.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Clock | Self::Malformed => StatusCode::BAD_REQUEST,
            Self::Unsigned | Self::BeforeStart(_) | Self::StartAhead(_) | Self::Audience(_) | Self::Signature | Self::Replay => {
                StatusCode::UNAUTHORIZED
            }
        }
    }

    /// The common answer shape, `(status, words)`.
    pub fn plain(self) -> (StatusCode, String) {
        (self.status(), self.words())
    }

    /// [`Self::plain`], but a replay answers 409 Conflict: the routes that always answered a
    /// repeated request so (the vault, the Files page's) keep doing so.
    pub fn plain_replay_conflict(self) -> (StatusCode, String) {
        match self {
            Self::Replay => (StatusCode::CONFLICT, "Duplicate request rejected (replay).".to_string()),
            other => other.plain(),
        }
    }
}

impl From<SignedRefusal> for (StatusCode, String) {
    fn from(r: SignedRefusal) -> Self {
        r.plain()
    }
}

/// The relay's clock, Unix milliseconds.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// How long after a restart a request older than it is read as a slow clock rather than one made
/// before the restart and delivered late (C6).
pub const CLOCK_HINT_AFTER_MS: u64 = 10_000;

/// How long, by the clock that never steps, the wall clock must hold a forward step before the start
/// rises to it for good (L5, 2026-11-21): an hour, longer than any time server's polling interval,
/// so a bad answer replaced at the next poll, or a date set wrong by hand and put right, is gone
/// before it could count.
pub const CLOCK_STEP_CONFIRM_MS: u64 = 60 * 60 * 1000;

/// How far apart two readings of the start may be and still be the same start: a clock being
/// slewed by its time service drifts against the clock that never steps by well under this in an
/// hour (500 parts in a million is 1.8 seconds).
const START_TOLERANCE_MS: u64 = 2_000;

/// When a relay's run started (D6), by the wall clock and by a clock that never steps.
///
/// THE RULE (L2 of the review of rounds 3 and 4; L5 of the review of round 5, both 2026-11-21): a
/// request signed before the restart is not taken after it once the clock has read right at or
/// since the start (a step back after a step the clock HELD moves nothing; set right and set back within
/// the hour, it does, below), and no wall clock reading locks every
/// request out for the rest of the run unless the clock HELD it. What it does NOT promise (round 7):
/// a clock slow at the start that is never set right, or set right and set back within the hour,
/// leaves the start at the slow reading, so a request captured before the restart can be taken
/// when that clock reaches its time; doing that needs control of the server's clock, and
/// docs/reference/cryptography.md lists it. (Keeping a pending step for good once a request was
/// taken under it would close that, but any stranger with a key of his own could then keep a wrong
/// reading for good and bring back the lockout L5 removed.)
/// Each reading of the relay's own wall clock gives a start (the reading less the time since the
/// start by the clock that never steps), and only the relay's own readings move it, never a
/// request's timestamp:
/// - a reading at or behind the start changes nothing: the start never moves earlier (round 3, C6,
///   took the earlier of the two, so a clock stepped back let requests from before the restart in
///   again, used once already before it: the replay memory is not kept across a restart);
/// - a reading later than the start is a forward step, PENDING: the start is that later one at once,
///   so nothing from before it is taken while it holds, but it is kept for good only once the clock
///   has held it for [`CLOCK_STEP_CONFIRM_MS`] by the clock that never steps; a reading back at the
///   start before then ends it. Round 5 kept every forward reading for good, so one reading a year
///   ahead (a bad time server answer, a date typed wrong) set the start a year ahead and refused
///   every request until a restart (L5).
///
/// What it costs, the other way: a clock FAST at the start keeps the start late, so for as long as
/// the clock was ahead a request made after the restart is refused too, in words that say the
/// server's clock was set back and that a restart clears it (`SignedRefusal::StartAhead`); and a
/// clock slow at the start, set right and then set back again within the hour is read as a step
/// that did not hold. The relay's unit waits for the clock to be set where systemd-time-wait-sync is
/// on (docs/admin/SELF-HOSTING.md). Nothing is kept across runs.
#[derive(Debug)]
pub struct StartedAt {
    state: std::sync::Mutex<StartState>,
    mono: std::time::Instant,
}

/// [`StartedAt`]'s readings so far.
#[derive(Debug, Clone, Copy)]
struct StartState {
    /// The start kept for good, Unix milliseconds.
    kept: u64,
    /// A later start a forward step gave, not yet held long enough: (the start, milliseconds since
    /// the run began by the clock that never steps when it was first read).
    pending: Option<(u64, u64)>,
}

impl StartedAt {
    /// Now.
    pub fn now() -> Self {
        Self { state: std::sync::Mutex::new(StartState { kept: now_ms(), pending: None }), mono: std::time::Instant::now() }
    }

    /// The start as of the relay's own wall clock reading `now` (THE RULE above).
    pub fn wall_ms(&self, now: u64) -> u64 {
        self.reading(now, self.mono.elapsed().as_millis() as u64)
    }

    /// The start as of the wall clock reading `now`, taken `since_ms` after the run began by the
    /// clock that never steps (THE RULE above; tests pass the time since themselves).
    pub fn reading(&self, now: u64, since_ms: u64) -> u64 {
        let start = now.saturating_sub(since_ms);
        let mut s = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if start <= s.kept + START_TOLERANCE_MS {
            // At or behind the start kept: a step ended, or the clock was set back.
            s.pending = None;
        } else {
            match s.pending {
                Some((step, seen)) if step.abs_diff(start) <= START_TOLERANCE_MS => {
                    if since_ms.saturating_sub(seen) >= CLOCK_STEP_CONFIRM_MS {
                        s.kept = step.max(start);
                        s.pending = None;
                    }
                }
                _ => s.pending = Some((start, since_ms)),
            }
        }
        s.pending.map_or(s.kept, |(step, _)| step.max(s.kept))
    }

    /// For tests: a start the wall clock recorded as `wall_ms`, `ago` before now by the clock that
    /// never steps.
    #[cfg(test)]
    pub fn at(wall_ms: u64, ago: std::time::Duration) -> Self {
        Self { state: std::sync::Mutex::new(StartState { kept: wall_ms, pending: None }), mono: std::time::Instant::now() - ago }
    }
}

/// Whether `timestamp` is inside the window at `now`.
pub fn fresh_at(timestamp: u64, now: u64) -> bool {
    now.saturating_sub(timestamp) <= SIGNED_WINDOW_MS && timestamp.saturating_sub(now) <= SIGNED_AHEAD_MS
}

/// Whether `sig_hex` is `key_hex`'s Dilithium3 signature over the request `purpose` at `timestamp`
/// for `audience`. CPU-bound (an ML-DSA-65 verify): async callers use [`check`] or run this off the
/// executor.
pub fn verify(key_hex: &str, audience: &str, purpose: &str, timestamp: u64, sig_hex: &str) -> bool {
    let (Ok(pk), Ok(sig)) = (hex::decode(key_hex), hex::decode(sig_hex)) else { return false };
    humanity_protocol::pq_crypto::verify_dilithium(&pk, request_preimage(audience, purpose, timestamp).as_bytes(), &sig).is_ok()
}

/// The whole check for an HTTP request, its audience the request's own `Host`: signed at all, well
/// formed, fresh, made since this relay started, for this relay, signed by `key` over the request
/// `purpose` at `timestamp`, and not used before for `tag` (the route's own fixed name: one
/// signature is good once). Call it before any role, ownership or rate decision is made from `key`.
pub async fn check(state: &RelayState, key: &str, timestamp: u64, sig: &str, purpose: &str, tag: &'static str) -> Result<(), SignedRefusal> {
    let arrival = crate::relay::audience::request_arrival().unwrap_or_default();
    // A test that calls a route's function directly has no HTTP request: it signs for the test
    // audience (audience.rs `TEST_AUDIENCE`). Every product call runs under `scope_host`.
    #[cfg(test)]
    let arrival = match arrival.host {
        Some(_) => arrival,
        None => crate::relay::audience::Arrival { host: Some(crate::relay::audience::TEST_AUDIENCE.to_string()), ..arrival },
    };
    check_for(state, &arrival, key, timestamp, sig, purpose, tag).await
}

/// [`check`] with the arrival given: a socket's, read from its upgrade request's headers.
pub async fn check_for(
    state: &RelayState,
    arrival: &crate::relay::audience::Arrival,
    key: &str,
    timestamp: u64,
    sig: &str,
    purpose: &str,
    tag: &'static str,
) -> Result<(), SignedRefusal> {
    check_at(state, arrival, key, timestamp, sig, purpose, tag, now_ms()).await
}

/// [`check_for`] with the relay's clock reading `now` given (a test stands a request inside a step
/// of the clock with it; every product call passes the clock, [`now_ms`]).
#[allow(clippy::too_many_arguments)]
pub async fn check_at(
    state: &RelayState,
    arrival: &crate::relay::audience::Arrival,
    key: &str,
    timestamp: u64,
    sig: &str,
    purpose: &str,
    tag: &'static str,
    now: u64,
) -> Result<(), SignedRefusal> {
    if key.is_empty() || sig.is_empty() {
        return Err(SignedRefusal::Unsigned);
    }
    if !humanity_protocol::signed_request::is_canonical_key(key) || purpose.len() > MAX_PURPOSE_BYTES {
        return Err(SignedRefusal::Malformed);
    }
    if !fresh_at(timestamp, now) {
        return Err(SignedRefusal::Clock);
    }
    // For this relay as reached, and signed by the key: both decided before either answer is given,
    // and the signature verified whatever the audience (L4 of the review of round 5, 2026-11-21: the
    // audience was asked first, so a made-up signature for one of this relay's names got the
    // signature's own words and a guess that was not got the audience's, and the time it took told
    // the same). An HTTP request is signed for the Host it was sent to: the audience IS the arrival's
    // host. Through the owner's door only the owner's key is taken (audience.rs `admit_signer`).
    let names = crate::relay::audience::of(state);
    let host = arrival.host.as_deref().unwrap_or("");
    let admitted = names.admit_signer(host, arrival, key);
    let audience = match &admitted {
        Ok(a) => a.clone(),
        Err(_) => humanity_protocol::signed_request::canonical_audience(host).unwrap_or_default(),
    };
    let (k, s, p) = (key.to_string(), sig.to_string(), purpose.to_string());
    // ML-DSA-65 verify off the async executor; a panicked or cancelled task fails closed.
    let ok = tokio::task::spawn_blocking(move || {
        #[cfg(test)]
        note_verified(&k);
        verify(&k, &audience, &p, timestamp, &s)
    })
    .await
    .unwrap_or(false);
    admitted.map_err(SignedRefusal::Audience)?;
    if !ok {
        tracing::info!("Refused a request signed for {host} ({tag}) from {:?}: the signature does not match", arrival.peer);
        return Err(SignedRefusal::Signature);
    }
    // Made since this relay started, by its own clock's reading (never the request's time: L5),
    // asked only of a request already proven to be for here.
    let start = state.started_at.wall_ms(now);
    if timestamp < start {
        // The clock reads earlier than the start it once gave: set back since the start (L2).
        if start > now {
            return Err(SignedRefusal::StartAhead(start.saturating_sub(timestamp).div_ceil(60_000)));
        }
        // Moments after a restart, a request older than it is one made before it; long after, it is
        // a device whose clock is behind (round 3, C6), by about how old the request is.
        let behind = (now.saturating_sub(start) > CLOCK_HINT_AFTER_MS).then(|| now.saturating_sub(timestamp).div_ceil(1000));
        return Err(SignedRefusal::BeforeStart(behind));
    }
    if !state.replay.first_use(key, tag, timestamp, purpose.as_bytes()) {
        return Err(SignedRefusal::Replay);
    }
    Ok(())
}

/// Test builds only: every key a signature was verified for, so a test can see that the signature
/// is verified whatever the audience (L4; round 7 of the review, 2026-11-21: with the old order and
/// the new words every answer read the same, and only this shows the order).
#[cfg(test)]
static VERIFIED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Test builds only: note that a signature by `key` was verified ([`VERIFIED`]).
#[cfg(test)]
pub fn note_verified(key: &str) {
    VERIFIED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(key.to_string());
}

/// Test builds only: whether a signature by `key` was verified in this test run.
#[cfg(test)]
pub fn was_verified(key: &str) -> bool {
    VERIFIED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().any(|k| k == key)
}

/// For tests: a time never handed out before in this test run, the relay's clock or one past the
/// last one, so two requests a test sends in one millisecond are not refused as one (the clients'
/// own clocks do the same: `net::identity::next_request_timestamp`, pq.js `pqRequestTimestamp`).
#[cfg(test)]
pub fn next_test_timestamp() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = now_ms();
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(seen) => prev = seen,
        }
    }
}
