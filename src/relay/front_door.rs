//! The website's front door (2026-11-02, docs/design/front-door.md): the update cards, the release
//! list the Devlog reads, and the release signatures the server checked itself.
//!
//! - `GET /api/front-door` answers `{ updates: [{ version, title, url, at, signed }] }`, the newest
//!   [`FRONT_DOOR_UPDATES`] releases, for the front page's row of cards.
//! - `GET /api/releases?before=<cursor>&limit=<n>` answers `{ releases: [...], next }`, the same
//!   cards a page at a time, newest first, for the Devlog. `next` is the cursor of the next older
//!   page, or null at the end. So a visitor to either page talks only to this server, never to
//!   GitHub's API (which would send every visitor's address to GitHub, the reason the fonts are
//!   self-hosted).
//! - `POST /api/release-signature { manifest, signature }`: the two files `just sign-release`
//!   uploads to a release, as text. The server keeps them only when both signatures verify against
//!   the project's compiled-in release keys (`humanity_protocol::release_signature`, the same check
//!   the desktop updater makes before it installs), and from then on that release's card says
//!   `signed: true`. The route needs no secret: only the project's keys make a pair that passes, so
//!   a forged "Signed" mark is impossible rather than merely unlikely.
//!
//! THE FILTER (design section 4.4). #announcements is a chat channel: admins and moderators can post
//! in it even when it is read only. A card is made only from a post by the Deploy Bot (`/api/send`
//! with the bot secret, CI's release step) or by this server's own key (`handlers/announce.rs`),
//! that reads "<emoji> **v<version>**: <headline>", optionally ending with ONE link, and only to
//! that version's own release page ([`release_page`]); any other link, or a web address in the
//! words, and the post is not shown at all. The card's link is always built here from the version,
//! never taken from the post, so the front door can never put another address before every
//! visitor. The headline is plain words: markdown marks taken out, at most [`HEADLINE_MAX_CHARS`].
//!
//! NO CACHE (CLAUDE.md's no-caches rule): each request is one read down `idx_messages_channel` and
//! one small lookup of signed versions. The front page asks once per visit (and again only when a
//! tab comes back after ten minutes), so nothing needs keeping between requests.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::relay::relay::RelayState;
use crate::relay::storage::front_door::ReleasePost;
use humanity_protocol::release_signature::{self, is_plain_version};

/// The channel the release posts are in.
pub const ANNOUNCEMENTS: &str = "announcements";

/// The Deploy Bot's key: `/api/send` names a bot `bot_<its name, lower case, spaces as _>`
/// (api.rs `send_message`), and CI posts as "Deploy Bot" (.github/workflows/build-desktop.yml).
pub const DEPLOY_BOT_KEY: &str = "bot_deploy_bot";

/// Where every release's own page is. A card links here and nowhere else.
pub const RELEASE_PAGE_PREFIX: &str = "https://github.com/Shaostoul/Humanity/releases/tag/";

/// How many cards the front door gets: its row shows at most nine (data/front_door/config.json
/// `card_max`, the operator's "1 to 9 card columns").
pub const FRONT_DOOR_UPDATES: usize = 9;

/// The Devlog's page of releases when it does not say, and the most it may ask for.
pub const RELEASES_PAGE_DEFAULT: usize = 25;
pub const RELEASES_PAGE_MAX: usize = 50;

/// The longest headline a card carries, in characters. The last 100 real ones were 103 at the
/// longest (2026-11-02); early posts ran to thousands, and are cut at a word with an ellipsis.
pub const HEADLINE_MAX_CHARS: usize = 280;

/// The body limit of `POST /api/release-signature`: both files at their limits, as JSON strings.
pub const SIGNATURE_BODY_LIMIT: usize = release_signature::MANIFEST_MAX_BYTES + release_signature::SIGNATURE_MAX_BYTES + 16 * 1024;

/// One card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Update {
    /// Digits and dots, no leading `v` ("0.1547.0").
    pub version: String,
    /// The headline, plain words.
    pub title: String,
    /// The release's own page, built from the version.
    pub url: String,
    /// When it was posted, milliseconds since 1970.
    pub at: u64,
    /// Whether this server checked a signature of this release by the project's keys.
    pub signed: bool,
}

/// The release page of `version` (digits and dots).
pub fn release_page(version: &str) -> String {
    format!("{RELEASE_PAGE_PREFIX}v{version}")
}

/// A release post's version and headline, or None when the post is not one the front door shows
/// (see the module's filter). Who posted it is checked by the caller.
pub fn parse_release_post(content: &str) -> Option<(String, String)> {
    let text = content.trim();
    let open = text.find("**v")?;
    // Before the version, only the post's mark (the rocket and a space): no words.
    let lead = &text[..open];
    if lead.chars().count() > 4 || lead.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    let after = &text[open + 3..];
    let close = after.find("**")?;
    let version = &after[..close];
    if !is_plain_version(version) {
        return None;
    }
    let rest = after[close + 2..].trim_start();
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim();

    // The one link a post may end with: "[words](<url>)" or "[words](url)", to this version's page.
    let mut words = rest;
    if rest.ends_with(')') {
        if let Some(mid) = rest.rfind("](") {
            let open_text = rest[..mid].rfind('[')?;
            let target = &rest[mid + 2..rest.len() - 1];
            let target = target.strip_prefix('<').and_then(|t| t.strip_suffix('>')).unwrap_or(target);
            if target != release_page(version) {
                return None;
            }
            words = rest[..open_text].trim_end();
            words = words.strip_suffix('\u{b7}').unwrap_or(words).trim_end();
        }
    }
    // Plain words only: no second link, no web address.
    let lower = words.to_ascii_lowercase();
    if words.contains("](") || lower.contains("http://") || lower.contains("https://") || lower.contains("www.") {
        return None;
    }
    let plain: String = words.replace("**", "").replace('`', "").chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if plain.is_empty() {
        return None;
    }
    Some((version.to_string(), cut_at_word(&plain, HEADLINE_MAX_CHARS)))
}

/// `s` cut to at most `max` characters, at a space where one is near the end, with an ellipsis.
fn cut_at_word(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max - 1).collect();
    let cut = match head.rfind(' ') {
        Some(at) if at > head.len() * 3 / 4 => &head[..at],
        _ => head.as_str(),
    };
    format!("{}\u{2026}", cut.trim_end())
}

/// Who may post a release card: the Deploy Bot and this server itself.
fn senders(state: &RelayState) -> Vec<String> {
    let mut out = vec![DEPLOY_BOT_KEY.to_string()];
    match state.db.get_or_create_server_keypair() {
        Ok((server_key, _)) => out.push(server_key),
        Err(e) => tracing::warn!("front door: could not read the server's own key, so only the Deploy Bot's posts are shown: {e}"),
    }
    out
}

/// The cards of one page: the posts read (newest first), parsed and filtered, each version once,
/// at most `limit`, then marked signed. Returns the cards and the cursor of the next older page.
pub fn cards_from(posts: &[ReleasePost], allowed: &[String], limit: usize, read_was_full: bool) -> (Vec<Update>, Option<i64>) {
    let mut cards: Vec<Update> = Vec::new();
    let mut last_used = None;
    for p in posts {
        if cards.len() >= limit {
            break;
        }
        if !allowed.iter().any(|k| k == &p.from_key) {
            continue;
        }
        let Some((version, title)) = parse_release_post(&p.content) else { continue };
        last_used = Some(p.id);
        // A release announced twice (a re-run of CI) shows once, its newest post.
        if cards.iter().any(|c| c.version == version) {
            continue;
        }
        cards.push(Update { url: release_page(&version), version, title, at: p.timestamp, signed: false });
    }
    let next = if cards.len() >= limit {
        last_used
    } else if read_was_full {
        // The read stopped at its limit before enough cards were found: older posts remain.
        posts.last().map(|p| p.id)
    } else {
        None
    };
    (cards, next)
}

/// Read one page of cards from the store, signed marks included.
fn read_cards(state: &RelayState, before: Option<i64>, limit: usize) -> Result<(Vec<Update>, Option<i64>), rusqlite::Error> {
    let allowed = senders(state);
    // Enough rows for a page even when some posts are not release cards, still one bounded read. A
    // read shorter than that reached the channel's oldest post.
    let rows = limit * 3 + 20;
    let posts = state.db.release_posts(ANNOUNCEMENTS, before, rows)?;
    let read_was_full = posts.len() >= rows;
    let (mut cards, next) = cards_from(&posts, &allowed, limit, read_was_full);
    let versions: Vec<&str> = cards.iter().map(|c| c.version.as_str()).collect();
    let signed = state.db.signed_versions(&versions)?;
    for c in &mut cards {
        c.signed = signed.contains(&c.version);
    }
    Ok((cards, next))
}

fn store_error(what: &str, e: rusqlite::Error) -> axum::response::Response {
    tracing::error!("front door: could not read {what}: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "The server could not read its updates." }))).into_response()
}

/// `GET /api/front-door`.
pub async fn get_front_door(State(state): State<Arc<RelayState>>) -> impl IntoResponse {
    match read_cards(&state, None, FRONT_DOOR_UPDATES) {
        Ok((updates, _)) => (StatusCode::OK, Json(serde_json::json!({ "updates": updates }))).into_response(),
        Err(e) => store_error("the front door's updates", e),
    }
}

#[derive(Debug, Deserialize)]
pub struct ReleasesQuery {
    /// The `next` of the page before; absent for the newest page.
    pub before: Option<i64>,
    pub limit: Option<usize>,
}

/// `GET /api/releases`.
pub async fn get_releases(State(state): State<Arc<RelayState>>, Query(q): Query<ReleasesQuery>) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(RELEASES_PAGE_DEFAULT).clamp(1, RELEASES_PAGE_MAX);
    match read_cards(&state, q.before.filter(|b| *b > 0), limit) {
        Ok((releases, next)) => (StatusCode::OK, Json(serde_json::json!({ "releases": releases, "next": next }))).into_response(),
        Err(e) => store_error("the release list", e),
    }
}

#[derive(Debug, Deserialize)]
pub struct SignatureRequest {
    /// `release-manifest.json`, its exact text.
    pub manifest: String,
    /// `release-manifest.json.sig.json`, its exact text.
    pub signature: String,
}

/// `POST /api/release-signature`.
pub async fn post_release_signature(State(state): State<Arc<RelayState>>, Json(req): Json<SignatureRequest>) -> impl IntoResponse {
    // The two signature checks (a Dilithium3 one is a few milliseconds) run off the async pool.
    let checked = tokio::task::spawn_blocking(move || {
        let got = release_signature::check_signed_release(req.manifest.as_bytes(), req.signature.as_bytes());
        (got, req)
    })
    .await;
    let (got, req) = match checked {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!("release signature: the check did not finish: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "The server could not check the signatures." }))).into_response();
        }
    };
    let signed = match got {
        Ok(s) => s,
        Err(why) => {
            tracing::info!("release signature: refused a pair ({why})");
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("Nothing was kept: the signatures do not check out against the project's release keys ({why}).") })),
            )
                .into_response();
        }
    };
    match state.db.store_release_signature(&signed, &req.manifest, &req.signature) {
        Ok(kept) => {
            tracing::info!("release signature: v{} checked and kept ({})", signed.version, kept.word());
            (StatusCode::OK, Json(serde_json::json!({ "ok": true, "version": signed.version, "signed": true, "kept": kept.word() }))).into_response()
        }
        Err(e) => {
            tracing::error!("release signature: v{} checked but could not be kept: {e}", signed.version);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "The signatures check out, but the server could not keep them." }))).into_response()
        }
    }
}

#[cfg(test)]
#[path = "front_door_tests.rs"]
mod tests;
