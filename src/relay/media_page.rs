//! One Media item's own page, for people and for social sites (Media round 5, 2026-10-12;
//! docs/design/media-round-5-plan.md 3.6, decisions 3 and 10).
//!
//! - `GET /media/{id}`: the website's Media page (media.html, read from the web root on every
//!   request, never built into the exe) with the item's Open Graph tags written at its
//!   `<!-- media:og -->` line and its own `<title>`. nginx sends `^/media/[0-9]+$` here; `/media`
//!   itself stays a static page.
//! - `GET /api/media/preview/{id}`: the picture a social site shows: a public, unlabelled item's
//!   cover, else the picture itself, at most [`PREVIEW_MAX_BYTES`], inline under
//!   `default-src 'none'`.
//!
//! WHAT A CRAWLER CAN LEARN. Only what anyone could already see on the Media page: a public item on
//! a public Media page. A members-only item, any item on a members-only Media page, and a number
//! that is no item (or not a number at all) all get the SAME bare page, byte for byte, still 200
//! so a member's browser loads the page and signs in: its title says only which server it is, and
//! it asks search engines not to index it ([`hidden_page`]). A labelled item (a spoiler, flashing
//! lights, something graphic) gets its words but no picture, and its description starts with its
//! labels, so a social site never shows unasked what the label hides until clicked.
//!
//! THE ADDRESSES IN THE TAGS come from the server's own configured addresses (relay/mod.rs
//! `allowed_origins`, from ALLOWED_ORIGINS): the request's `Host` header is used only when it is
//! one of them, so a forged Host can never make a preview point anywhere else ([`site_origin`]).
//! Every value written into the page is HTML-escaped ([`escape`]).
//!
//! THE PAGE'S OWN CONTENT-SECURITY-POLICY. The relay's usual policy (relay/mod.rs `csp_value`) has
//! no `blob:` among the picture sources, and the Media page shows members-only pictures through
//! `blob:` addresses (the static pages' nginx policy allows them). So this route sets the relay's
//! policy with `blob:` added to `img-src` ([`page_csp`]), and relay/mod.rs `security_headers` keeps
//! a policy a route set itself.

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::relay::media::{labels, media_settings, Folders};
use crate::relay::media_counts::shown_to_everyone;
use crate::relay::relay::RelayState;
use crate::relay::storage::media::{MediaItem, Spot};

/// The line in media.html's head where an item's tags are written.
pub const OG_MARKER: &str = "<!-- media:og -->";

/// What a server that cannot find its Media page answers (503), in words its operator can act on.
pub const NO_MEDIA_HTML: &str = "This server cannot find its Media page (media.html in its web root). Its operator can set WEB_ROOT.";

/// What every refused preview is told, whatever was wrong, so a guess learns nothing.
pub const NO_PREVIEW: &str = "No preview.";

/// The largest picture given to social sites (decision 10): the original picture or cover only
/// when it is at most this, nothing bigger. 5,000,000 bytes rather than 5 MiB, so it is under the
/// smallest limit a site documents however it counts a megabyte (X, formerly Twitter, says "5MB").
/// Smaller copies join the previews planned in increment 4.
pub const PREVIEW_MAX_BYTES: u64 = 5_000_000;

/// The most characters of an item's description written into `og:description`.
pub const DESCRIPTION_MAX_CHARS: usize = 200;

// ── GET /media/{id} ──

/// `GET /media/{id}`. The path is taken as text, so anything that is not an item's number gets
/// the same bare page as a missing item.
pub async fn page(State(state): State<Arc<RelayState>>, Path(id): Path<String>, headers: HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let origin = site_origin(host, super::allowed_origins());
    let template = read_template(&template_paths()).await;
    page_core(&state, Folders::live(), template.as_deref(), &id, &origin)
}

/// Where media.html is looked for, in order: `$WEB_ROOT/media.html` when `WEB_ROOT` is set (the
/// deploy flattens web/pages into the web root, scripts/sync-web-root.sh), then
/// `web/pages/media.html` under the relay's working folder (/opt/Humanity on both live servers,
/// whose checkout holds it).
pub fn template_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(root) = std::env::var_os("WEB_ROOT").filter(|r| !r.is_empty()) {
        paths.push(PathBuf::from(root).join("media.html"));
    }
    paths.push(PathBuf::from("web").join("pages").join("media.html"));
    paths
}

/// The first of `paths` that can be read, read now (about 2 KB): read on every request, so a deploy
/// of the website alone shows at once, with no restart.
pub async fn read_template(paths: &[PathBuf]) -> Option<String> {
    for path in paths {
        if let Ok(text) = tokio::fs::read_to_string(path).await {
            return Some(text);
        }
    }
    None
}

/// The page for `id` (as it came in the path) on the site `origin` (from [`site_origin`]), built
/// from `template` (media.html's text, none when it could not be found: 503 with
/// [`NO_MEDIA_HTML`]). `folders` is where the preview picture is looked for, since `og:image` is
/// written only when there is one.
pub fn page_core(state: &RelayState, folders: Folders, template: Option<&str>, id: &str, origin: &str) -> Response {
    let Some(template) = template else {
        return (StatusCode::SERVICE_UNAVAILABLE, NO_MEDIA_HTML).into_response();
    };
    let body = match shown_item(state, id) {
        Some(item) => {
            let has_preview = preview_file(&item, folders).is_some();
            item_page(template, &item, origin, &server_name(state), has_preview)
        }
        None => hidden_page(template, origin),
    };
    let mut res = (StatusCode::OK, body).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if let Ok(csp) = HeaderValue::from_str(page_csp()) {
        h.insert(header::CONTENT_SECURITY_POLICY, csp);
    }
    res
}

/// The item `id` names, when everyone may see it: `id` is a number (digits only), there is such an
/// item, and it and the server's Media page are public. Anything else is none, and gets the hidden
/// page; a storage error too (logged), so even a fault says nothing.
fn shown_item(state: &RelayState, id: &str) -> Option<MediaItem> {
    if id.is_empty() || id.len() > 18 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let id: i64 = id.parse().ok()?;
    let item = state.db.media_item(id).unwrap_or_else(|e| {
        tracing::error!("Media page: item {id} could not be read ({e}); the bare page is shown");
        None
    })?;
    shown_to_everyone(&item, &media_settings(&state.db)).then_some(item)
}

/// The server's name, as `/api/server-info` gives it (api.rs `get_server_info`): the name set in
/// Server Settings, else server-config.json's, else `SERVER_NAME`, else "Humanity Relay".
fn server_name(state: &RelayState) -> String {
    let set = state.db.get_server_settings().map(|s| s.server_name).unwrap_or_default();
    if !set.trim().is_empty() {
        return set.trim().to_string();
    }
    state
        .server_config
        .get("server_name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| std::env::var("SERVER_NAME").ok())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Humanity Relay".to_string())
}

/// The host part of an origin (`https://united-humanity.us` gives `united-humanity.us`).
fn host_of(origin: &str) -> &str {
    origin.split_once("://").map_or(origin, |(_, h)| h)
}

/// Whether an origin's host is this machine itself (with any port): never a public address.
fn is_loopback(origin: &str) -> bool {
    let host = host_of(origin);
    // The name without its port: `[::1]:3210` gives `[::1]`, `localhost:3210` gives `localhost`.
    let name = match host.find(']') {
        Some(end) if host.starts_with('[') => &host[..=end],
        _ => host.split(':').next().unwrap_or(host),
    };
    matches!(name, "localhost" | "127.0.0.1" | "[::1]" | "tauri.localhost")
}

/// The site an item's page names, as `scheme://host`, from the server's own configured addresses
/// (`origins`, relay/mod.rs `allowed_origins`): the request's `Host` only when `https://<Host>` is
/// one of them (a `www.` before it is let go first, when only the bare name is configured);
/// otherwise the first https one; otherwise the first http one (a server on a home
/// network with no https). The desktop app's and this machine's own addresses (`tauri.localhost`,
/// `localhost`, `127.0.0.1`), which every relay allows, are never a public address and are used
/// only when nothing else is configured.
pub fn site_origin(host: Option<&str>, origins: &[String]) -> String {
    let web: Vec<String> = origins
        .iter()
        .map(|o| o.trim().trim_end_matches('/').to_ascii_lowercase())
        .filter(|o| o.starts_with("https://") || o.starts_with("http://"))
        .collect();
    let public: Vec<&String> = web.iter().filter(|o| !is_loopback(o)).collect();
    if let Some(host) = host.map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()) {
        // The Host as it came, then without a leading `www.` (nginx serves www.<site> as <site>,
        // and the address the page names is then the configured one, never the Host's own words).
        let bare = host.strip_prefix("www.").unwrap_or(&host);
        for wanted in [format!("https://{host}"), format!("https://{bare}")] {
            if public.iter().any(|o| **o == wanted) {
                return wanted;
            }
        }
    }
    public
        .iter()
        .find(|o| o.starts_with("https://"))
        .or_else(|| public.first())
        .map(|o| o.to_string())
        .or_else(|| web.iter().find(|o| !host_of(o).starts_with("tauri.")).cloned())
        .unwrap_or_else(|| "http://localhost".to_string())
}

/// `text` made safe inside an HTML attribute or element: `& < > " '` written as entities.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `text` with every run of white space (line breaks included) as one space, cut to at most
/// [`DESCRIPTION_MAX_CHARS`] characters ("..." in its last three when it was cut).
fn short_description(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= DESCRIPTION_MAX_CHARS {
        return flat;
    }
    let cut: String = flat.chars().take(DESCRIPTION_MAX_CHARS - 3).collect();
    format!("{}...", cut.trim_end())
}

/// media.html with `title` (already escaped) as its `<title>` and `tags` written at its
/// [`OG_MARKER`] line (before `</head>` if the marker is missing, at the top if that is too).
fn fill(template: &str, title: &str, tags: &str) -> String {
    let mut page = template.to_string();
    let title_element = format!("<title>{title}</title>");
    let mut tags = tags.to_string();
    match page.find("<title>").and_then(|a| page[a..].find("</title>").map(|b| (a, a + b + "</title>".len()))) {
        Some((a, b)) => page.replace_range(a..b, &title_element),
        None => tags = format!("{title_element}\n  {tags}"),
    }
    if let Some(at) = page.find(OG_MARKER) {
        page.replace_range(at..at + OG_MARKER.len(), &tags);
    } else if let Some(at) = page.find("</head>") {
        page.insert_str(at, &format!("{tags}\n"));
    } else {
        page.insert_str(0, &format!("{tags}\n"));
    }
    page
}

/// The bare page every hidden case gets: the same bytes for the same site, whatever the number.
pub fn hidden_page(template: &str, origin: &str) -> String {
    let site = escape(&format!("Media on {}", host_of(origin)));
    let tags = format!("<meta property=\"og:title\" content=\"{site}\">\n  <meta name=\"robots\" content=\"noindex\">");
    fill(template, &format!("{site}, HumanityOS"), &tags)
}

/// A public item's page: its title, description, address and (when it has one) picture, as Open
/// Graph tags (plan 3.6).
fn item_page(template: &str, item: &MediaItem, origin: &str, site_name: &str, has_preview: bool) -> String {
    let host = host_of(origin);
    let title = [item.title.trim(), item.original_name.trim()]
        .into_iter()
        .find(|t| !t.is_empty())
        .map_or_else(|| format!("A {} on {host}", item.kind), str::to_string);
    let said = if !item.description.trim().is_empty() {
        item.description.clone()
    } else if !item.uploader_name.trim().is_empty() {
        format!("Shared by {} on {host}.", item.uploader_name.trim())
    } else {
        format!("Shared on {host}.")
    };
    // Decision 3: a labelled item's description starts with its labels, by the names people see.
    let description = if item.labels.is_empty() {
        short_description(&said)
    } else {
        let known = labels();
        let names: Vec<&str> = item.labels.iter().map(|id| known.iter().find(|l| &l.id == id).map_or(id.as_str(), |l| l.label.as_str())).collect();
        short_description(&format!("Labelled: {}. {said}", names.join(", ")))
    };
    let url = format!("{origin}/media/{}", item.id);
    let mut tags = vec![
        format!("<link rel=\"canonical\" href=\"{}\">", escape(&url)),
        "<meta property=\"og:type\" content=\"website\">".to_string(),
        format!("<meta property=\"og:site_name\" content=\"{}\">", escape(site_name)),
        format!("<meta property=\"og:title\" content=\"{}\">", escape(&title)),
        format!("<meta property=\"og:description\" content=\"{}\">", escape(&description)),
        format!("<meta property=\"og:url\" content=\"{}\">", escape(&url)),
    ];
    if has_preview {
        tags.push(format!("<meta property=\"og:image\" content=\"{}\">", escape(&format!("{origin}/api/media/preview/{}", item.id))));
    }
    tags.push(format!("<meta name=\"twitter:card\" content=\"{}\">", if has_preview { "summary_large_image" } else { "summary" }));
    fill(template, &escape(&format!("{title}, Media on {host}")), &tags.join("\n  "))
}

/// The page's Content-Security-Policy: the relay's own (relay/mod.rs `csp_value`, whose
/// `connect-src` follows the server's configured addresses) with `blob:` added to `img-src`, as
/// the static pages' nginx policy has it, so members-only pictures shown through `blob:` addresses
/// load on an item's page as they do on /media; and with [`SHELL_VERSION_CHECK`] added to
/// `connect-src`, the one outside address the page's own scripts ask (web/shared/shell.js's check
/// for a newer release). A browser enforces this policy AND the site's nginx one, so a source left
/// out here is blocked even where nginx allows it (seen 2026-10-12: the check was refused on
/// /media/2 and nowhere else).
pub fn page_csp() -> &'static str {
    static CSP: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSP.get_or_init(|| with_connect_source(&with_blob_pictures(super::csp_value()), SHELL_VERSION_CHECK))
}

/// Where web/shared/shell.js asks for the newest release (its `fetch('https://api.github.com/...')`).
pub const SHELL_VERSION_CHECK: &str = "https://api.github.com";

/// `csp` with `source` among its `connect-src` sources (at the end; never twice).
fn with_connect_source(csp: &str, source: &str) -> String {
    let directives: Vec<String> = csp
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut words: Vec<&str> = d.split_whitespace().collect();
            if words.first() == Some(&"connect-src") && !words.contains(&source) {
                words.push(source);
            }
            words.join(" ")
        })
        .collect();
    format!("{};", directives.join("; "))
}

/// `csp` with `blob:` among its `img-src` sources (after `data:` when that is there).
fn with_blob_pictures(csp: &str) -> String {
    let directives: Vec<String> = csp
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let words: Vec<&str> = d.split_whitespace().collect();
            if words.first() != Some(&"img-src") || words.contains(&"blob:") {
                return words.join(" ");
            }
            let at = words.iter().position(|w| *w == "data:").map_or(words.len(), |p| p + 1);
            let mut words = words;
            words.insert(at, "blob:");
            words.join(" ")
        })
        .collect();
    format!("{};", directives.join("; "))
}

// ── GET /api/media/preview/{id} ──

/// The `Content-Type` of a picture the preview serves, from its stored name's ending: PNG, JPEG,
/// GIF or WebP, the Media page's pictures. Anything else has no preview.
fn picture_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

/// The picture a social site is shown for `item`, with its type: the cover when it has one, else
/// the item's own file when the item is a picture; never for a labelled item (decision 3); only
/// from the public uploads folder; at most [`PREVIEW_MAX_BYTES`]. Whether everyone may see the
/// item at all is the caller's question ([`shown_to_everyone`]), asked first.
pub fn preview_file(item: &MediaItem, folders: Folders) -> Option<(PathBuf, &'static str)> {
    if !item.labels.is_empty() {
        return None;
    }
    let name = match &item.cover {
        // The cover the uploader chose is the picture; a cover too big is no preview rather than
        // another picture in its place.
        Some(cover) => (cover.kind == "picture").then_some(cover.filename.as_str())?,
        None => (item.kind == "picture").then_some(item.upload_filename.as_str())?,
    };
    let kind = picture_type(name)?;
    if folders.spot_of(name) != Some(Spot::Public) {
        return None;
    }
    let path = folders.dir(Spot::Public).join(name);
    let size = std::fs::metadata(&path).ok()?.len();
    (size <= PREVIEW_MAX_BYTES).then_some((path, kind))
}

fn no_preview() -> Response {
    (StatusCode::NOT_FOUND, NO_PREVIEW).into_response()
}

/// `GET /api/media/preview/{id}`.
pub async fn preview(State(state): State<Arc<RelayState>>, Path(id): Path<i64>) -> Response {
    preview_core(&state, Folders::live(), id).await
}

/// [`preview`] with the folders named. Served inline with its own picture type, under
/// `Content-Security-Policy: default-src 'none'` (a picture needs nothing else), kept by shared
/// caches for five minutes. Everything else is 404 [`NO_PREVIEW`], the same words for every case.
pub async fn preview_core(state: &RelayState, folders: Folders<'_>, id: i64) -> Response {
    let item = match state.db.media_item(id) {
        Ok(Some(item)) if shown_to_everyone(&item, &media_settings(&state.db)) => item,
        Ok(_) => return no_preview(),
        Err(e) => {
            tracing::error!("Media preview: item {id} could not be read ({e})");
            return no_preview();
        }
    };
    let Some((path, kind)) = preview_file(&item, folders) else { return no_preview() };
    let Ok(bytes) = tokio::fs::read(&path).await else { return no_preview() };
    if bytes.len() as u64 > PREVIEW_MAX_BYTES {
        return no_preview(); // it grew between the look and the read
    }
    let mut res = (StatusCode::OK, bytes).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(kind));
    h.insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=300"));
    res
}

#[cfg(test)]
#[path = "media_page_tests.rs"]
mod tests;
