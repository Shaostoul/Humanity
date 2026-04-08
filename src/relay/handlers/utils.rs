//! Pure utility helpers extracted from relay.rs to reduce file size.
//! These functions have no side-effects on relay state (or minimal read-only access).

use std::sync::Arc;

use crate::relay::relay::{LinkPreview, RelayState};

/// Check if an IP address is private/internal (SSRF prevention).
pub fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
                || v4.octets()[0] == 0 // 0.0.0.0/8
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unspecified()
        }
    }
}

/// Fetch and cache a link preview for a URL. Returns None on failure.
pub async fn fetch_link_preview(state: &Arc<RelayState>, url: &str) -> Option<LinkPreview> {
    // SSRF prevention: only HTTP(S).
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    // Don't fetch upload URLs from our own server.
    if url.contains("/uploads/") {
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

    // DNS resolution + SSRF check.
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            // Try to resolve the hostname to check for private IPs.
            if let Ok(addrs) = tokio::net::lookup_host(format!("{}:{}", host, parsed.port_or_known_default().unwrap_or(80))).await {
                for addr in addrs {
                    if is_private_ip(&addr.ip()) {
                        tracing::debug!("SSRF blocked: {} resolves to private IP {}", url, addr.ip());
                        return None;
                    }
                }
            }
        }
    }

    // Fetch with timeout and redirect limit.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .ok()?;

    let resp = match client.get(url)
        .header("User-Agent", "HumanityRelay/1.0 LinkPreview")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("Link preview fetch failed for {}: {}", url, e);
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

    // Limit body size to 256KB.
    let body = match resp.text().await {
        Ok(b) if b.len() <= 256 * 1024 => b,
        _ => return None,
    };

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
            description: og_desc.map(|d| if d.len() > 300 { format!("{}…", &d[..297]) } else { d }),
            image: og_image,
            site_name: og_site,
        })
    } else {
        None
    }
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
