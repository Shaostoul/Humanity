//! Pure utility helpers extracted from relay.rs to reduce file size.
//! These functions have no side-effects on relay state (or minimal read-only access).

use std::sync::Arc;

use crate::relay::relay::{LinkPreview, RelayState};

/// Is `channel` one of this server's own rooms (a row in `channels`)? Pins, edits and reactions
/// carry the channel their message was in, and only a room's may be stored or broadcast: a direct
/// message (`dm:...`) or a P2P group (`p2pgroup:...`) is encrypted end to end, and its text, or who
/// reacted to whom in it, must never be kept here or sent to everyone (BUG-178, 2026-10-10).
pub fn is_room(state: &RelayState, channel: &str) -> bool {
    !channel.is_empty() && state.db.channel_exists(channel).unwrap_or(false)
}

/// How many redirects a link preview follows, each to an address checked as the first was.
const PREVIEW_REDIRECTS: usize = 3;
/// The most of a page a link preview reads; a longer page gets no preview.
const PREVIEW_MAX_BYTES: usize = 256 * 1024;

/// Fetch and cache a link preview for a URL. Returns None on failure.
///
/// The link is someone else's, so the page is fetched only on the public internet, through the
/// shared outbound guard (`relay::outbound`, BUG-255): http or https to a public name (never a
/// bare IP address or localhost), its addresses checked by the guarded client's own resolver as it
/// connects, at most three redirects, each checked the same way, and at most 256 KB read. It
/// replaces a check that looked the name up once and then let its client look it up again, follow
/// three redirects unchecked, count only IPv6 loopback as private, and fetch anyway when the name
/// did not resolve. Not called yet: chat-access rung 3 switches it on.
pub async fn fetch_link_preview(state: &Arc<RelayState>, url: &str) -> Option<LinkPreview> {
    use crate::relay::outbound::Schemes;

    // Don't fetch upload URLs from our own server.
    if url.contains("/uploads/") {
        return None;
    }
    // Only an address the server may fetch (the guard checks it again, and every redirect, as it goes).
    if let Err(why) = state.outbound.shape(url, Schemes::HttpOrHttps) {
        tracing::debug!("Link preview refused: {}", why.reason());
        return None;
    }

    // Check cache first.
    if let Ok(Some(cached)) = state.db.get_link_preview(url) {
        return Some(LinkPreview {
            url: cached.url,
            title: cached.title,
            description: cached.description,
            image: cached.image,
            site_name: cached.site_name,
        });
    }

    let tune = |r: reqwest::RequestBuilder| {
        r.header("User-Agent", "HumanityRelay/1.0 LinkPreview").timeout(std::time::Duration::from_secs(5))
    };
    let resp = match state.outbound.get(url, Schemes::HttpOrHttps, PREVIEW_REDIRECTS, tune).await {
        Ok(r) => r,
        Err(e) => {
            // Never the link itself in the log: it may be from a private room.
            tracing::debug!("Link preview not fetched: {e:?}");
            return None;
        }
    };

    // Only parse HTML responses.
    let content_type = resp.headers().get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !content_type.contains("text/html") {
        return None;
    }

    // At most PREVIEW_MAX_BYTES, read a piece at a time, so a huge page is never held whole.
    let body = read_at_most(resp, PREVIEW_MAX_BYTES).await?;

    // Parse OG tags with simple regex (avoiding heavy HTML parser dependency).
    let og_title = extract_meta(&body, "og:title")
        .or_else(|| extract_tag(&body, "title"));
    let og_desc = extract_meta(&body, "og:description")
        .or_else(|| extract_meta_name(&body, "description"));
    let og_image = extract_meta(&body, "og:image");
    let og_site = extract_meta(&body, "og:site_name");

    // Cache the result.
    let _ = state.db.cache_link_preview(
        url,
        og_title.as_deref(),
        og_desc.as_deref(),
        og_image.as_deref(),
        og_site.as_deref(),
    );

    // Only return if we have at least a title.
    if og_title.is_some() {
        Some(LinkPreview {
            url: url.to_string(),
            title: og_title,
            description: og_desc.map(|d| at_most_chars(d, 300)),
            image: og_image,
            site_name: og_site,
        })
    } else {
        None
    }
}

/// The body as text, or None when it is longer than `max` bytes or could not be read.
async fn read_at_most(mut resp: reqwest::Response, max: usize) -> Option<String> {
    let mut bytes = Vec::new();
    while let Some(piece) = resp.chunk().await.ok()? {
        if bytes.len() + piece.len() > max {
            return None;
        }
        bytes.extend_from_slice(&piece);
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `text` cut to at most `max` characters, an ellipsis marking a cut (on a character boundary:
/// slicing the bytes at 297, as this did, panics when a multi-byte character sits there).
fn at_most_chars(text: String, max: usize) -> String {
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max - 3).collect();
    format!("{cut}\u{2026}")
}


/// Extract OG meta content: <meta property="X" content="Y">
pub fn extract_meta(html: &str, property: &str) -> Option<String> {
    let pattern = format!(r#"<meta[^>]*property=["']{}["'][^>]*content=["']([^"']*)["']"#, regex::escape(property));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(&c[1]))
        .or_else(|| {
            // Also try content before property (some sites do this).
            let pattern2 = format!(r#"<meta[^>]*content=["']([^"']*)["'][^>]*property=["']{}["']"#, regex::escape(property));
            let re2 = regex::Regex::new(&pattern2).ok()?;
            re2.captures(html).map(|c| html_decode(&c[1]))
        })
}

/// Extract meta name content: <meta name="X" content="Y">
pub fn extract_meta_name(html: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"<meta[^>]*name=["']{}["'][^>]*content=["']([^"']*)["']"#, regex::escape(name));
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(&c[1]))
}

/// Extract <title>...</title>
pub fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let pattern = format!(r"<{0}[^>]*>([^<]*)</{0}>", tag);
    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(html).map(|c| html_decode(c[1].trim()))
}

/// Basic HTML entity decoding.
pub fn html_decode(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
}

/// Short key helper (Rust-side).
#[allow(non_snake_case)]
pub fn shortKey_rust(hex: &str) -> String {
    if hex.len() >= 8 { hex[..8].to_string() } else { hex.to_string() }
}

#[cfg(test)]
#[path = "link_preview_tests.rs"]
mod link_preview_tests;
