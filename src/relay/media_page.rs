//! One Media item's own page, for people and for social sites (Media round 5, 2026-10-12;
//! docs/design/media-round-5-plan.md 3.6).
//!
//! - `GET /media/{id}`: the website's Media page (media.html, read from the web root on every
//!   request, never built into the exe) with the item's Open Graph tags written at its
//!   `<!-- media:og -->` line. A members-only item, any item on a members-only Media page and a
//!   number that is no item all get the same bare page, byte for byte, so a crawler learns nothing.
//!   nginx sends `^/media/[0-9]+$` here; `/media` itself stays a static page. It sets its own
//!   Content-Security-Policy, which relay/mod.rs `security_headers` keeps.
//! - `GET /api/media/preview/{id}`: the picture a social site shows: a public, unlabelled item's
//!   cover, else the picture itself, at most 5 MB, inline under `default-src 'none'`.
//!
//! Both are lane 1B's: until it lands they answer 501 "Not built yet.".

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Response,
};
use std::sync::Arc;

use crate::relay::relay::RelayState;

/// The line in media.html's head where an item's tags are written.
pub const OG_MARKER: &str = "<!-- media:og -->";

/// `GET /media/{id}` (lane 1B). The path is taken as text, so anything that is not an item's number
/// gets the same bare page as a missing item.
pub async fn page(State(_state): State<Arc<RelayState>>, Path(_id): Path<String>, _headers: HeaderMap) -> Response {
    crate::relay::media_counts::not_built()
}

/// `GET /api/media/preview/{id}` (lane 1B).
pub async fn preview(State(_state): State<Arc<RelayState>>, Path(_id): Path<i64>) -> Response {
    crate::relay::media_counts::not_built()
}
