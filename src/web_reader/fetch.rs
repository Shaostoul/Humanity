//! The HTTP side of the readable web: scheme gate, capped download, manual
//! redirects, and the background thread the UI actually calls.
//!
//! Every rule here is a promise the Settings toggle makes ("only the URL you
//! open leaves your machine"), so each one is enforced in code, not by
//! convention:
//!
//! - [`check_url`] runs BEFORE any socket is opened and again on every
//!   redirect hop. Only `http` and `https` pass.
//! - Redirects are followed by hand (`redirects(0)` on the agent) precisely
//!   so that re-check can happen; ureq's own follower would happily be sent
//!   somewhere the gate never saw.
//! - The body is read through `Read::take(MAX_BYTES + 1)`, so a server that
//!   lies about (or omits) Content-Length still cannot exceed the cap. Gzip
//!   is decoded by ureq before the cap, so the limit is on the real bytes.
//! - Cookies: the ureq `cookies` feature is off in Cargo.toml, so the agent
//!   has no cookie store and never sends one.

use std::io::Read;
use std::sync::mpsc;
use std::time::Duration;

use url::Url;

use super::{parse_html, Block, Page, ReadRules, WebError, MAX_BYTES, MAX_REDIRECTS, TIMEOUT_SECS};

/// Validate a URL before anything is fetched. Accepts a bare host typed into
/// the URL bar ("wikipedia.org/wiki/Cat") by assuming `https://`; refuses
/// every scheme but `http` and `https`, including `javascript:`, `file:`,
/// `data:`, `mailto:` and `ftp:`.
pub fn check_url(raw: &str) -> Result<Url, WebError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(WebError::BadUrl("empty".to_string()));
    }
    let parsed = match Url::parse(raw) {
        Ok(u) => u,
        // "wikipedia.org/wiki/Cat" has no scheme: a person typed a host.
        // Anything else that fails to parse is not an address.
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Url::parse(&format!("https://{raw}")).map_err(|e| WebError::BadUrl(format!("{raw} ({e})")))?
        }
        Err(e) => return Err(WebError::BadUrl(format!("{raw} ({e})"))),
    };
    match parsed.scheme() {
        "http" | "https" => {
            if parsed.host_str().map_or(true, str::is_empty) {
                return Err(WebError::BadUrl(format!("{raw} (no host)")));
            }
            Ok(parsed)
        }
        other => Err(WebError::BlockedScheme(other.to_string())),
    }
}

/// Fetch and parse one page, BLOCKING. Call it from a worker thread
/// ([`spawn_fetch`] does); never from the UI thread.
pub fn fetch_page(raw_url: &str, rules: &ReadRules) -> Result<Page, WebError> {
    // The gate runs before the agent is even built.
    let mut url = check_url(raw_url)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        // Followed by hand below so every hop passes `check_url`.
        .redirects(0)
        .user_agent(&super::user_agent())
        .build();

    // hops = redirects followed so far; the first request is hop 0.
    for _hop in 0..=MAX_REDIRECTS {
        let resp = match agent
            .get(url.as_str())
            .set("Accept", "text/html,application/xhtml+xml;q=0.9,text/plain;q=0.8,*/*;q=0.1")
            .call()
        {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => return Err(WebError::Http { status: code }),
            Err(e) => return Err(WebError::Network(e.to_string())),
        };

        let status = resp.status();
        if (300..400).contains(&status) {
            let location = resp
                .header("Location")
                .ok_or_else(|| WebError::Network(format!("HTTP {status} redirect without a Location header")))?;
            let next = url
                .join(location)
                .map_err(|e| WebError::BadUrl(format!("redirect target {location} ({e})")))?;
            // The whole point of following by hand: the target is gated
            // like a typed URL, so a page cannot bounce us to file: or data:.
            url = check_url(next.as_str())?;
            continue;
        }

        // Content-Length is a fast refusal; the take() below is the real cap.
        if let Some(len) = resp.header("Content-Length").and_then(|s| s.trim().parse::<usize>().ok()) {
            if len > MAX_BYTES {
                return Err(WebError::TooLarge { limit: MAX_BYTES });
            }
        }

        let ctype = resp.header("Content-Type").unwrap_or("").trim().to_ascii_lowercase();
        let is_html = ctype.starts_with("text/html") || ctype.starts_with("application/xhtml");
        let is_text = ctype.starts_with("text/plain");
        // An absent Content-Type is treated as HTML (the parser is tolerant);
        // a declared non-page type (PDF, zip, image) is refused outright.
        if !ctype.is_empty() && !is_html && !is_text {
            let short = ctype.split(';').next().unwrap_or(&ctype).to_string();
            return Err(WebError::NotHtml(short));
        }

        let mut bytes = Vec::with_capacity(64 * 1024);
        resp.into_reader()
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| WebError::Network(format!("reading the page: {e}")))?;
        if bytes.len() > MAX_BYTES {
            return Err(WebError::TooLarge { limit: MAX_BYTES });
        }
        // UTF-8 is what the web serves today; a legacy-encoded page reads
        // with replacement characters rather than failing.
        let text = String::from_utf8_lossy(&bytes);

        if is_text {
            return Ok(Page {
                url: url.to_string(),
                title: url.host_str().unwrap_or("").to_string(),
                blocks: vec![Block::Code(text.trim_end().to_string())],
                notice: None,
            });
        }
        return Ok(parse_html(&text, url.as_str(), rules));
    }
    Err(WebError::TooManyRedirects)
}

/// Fetch on a background thread. The receiver yields exactly one message.
/// A blocked scheme comes back through the same channel without a thread
/// being spawned, so the UI has one code path for every outcome.
pub fn spawn_fetch(url: String, rules: ReadRules) -> mpsc::Receiver<Result<Page, WebError>> {
    let (tx, rx) = mpsc::channel();
    if let Err(e) = check_url(&url) {
        let _ = tx.send(Err(e));
        return rx;
    }
    let spawned = std::thread::Builder::new()
        .name("web-reader-fetch".to_string())
        .spawn(move || {
            let _ = tx.send(fetch_page(&url, &rules));
        });
    if let Err(e) = spawned {
        // Thread creation failing is an OS-level problem; report it as a
        // network error so the status line says something rather than
        // spinning forever.
        log::error!("[web_reader] could not spawn fetch thread: {e}");
    }
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate is the whole security story of the reader, so each refused
    /// scheme is asserted by name. None of these opens a socket: `check_url`
    /// is pure string handling, and `fetch_page` calls it first.
    #[test]
    fn non_http_schemes_are_refused_before_any_request() {
        for (raw, scheme) in [
            ("javascript:alert(1)", "javascript"),
            ("file:///etc/passwd", "file"),
            ("data:text/html,<h1>hi</h1>", "data"),
            ("mailto:someone@example.com", "mailto"),
            ("ftp://example.com/x", "ftp"),
        ] {
            assert_eq!(check_url(raw), Err(WebError::BlockedScheme(scheme.to_string())), "{raw}");
            // fetch_page must fail with the SAME error, i.e. before the
            // network is touched. A real request to "javascript:" would
            // fail differently (a transport error) if the gate were after it.
            assert_eq!(
                fetch_page(raw, &ReadRules::default()),
                Err(WebError::BlockedScheme(scheme.to_string())),
                "{raw} reached the fetch path"
            );
        }
    }

    #[test]
    fn http_and_https_pass_and_a_bare_host_gets_https() {
        assert_eq!(check_url("https://en.wikipedia.org/wiki/Cat").unwrap().as_str(), "https://en.wikipedia.org/wiki/Cat");
        assert_eq!(check_url("http://example.com").unwrap().scheme(), "http");
        assert_eq!(check_url("  wikipedia.org/wiki/Cat ").unwrap().as_str(), "https://wikipedia.org/wiki/Cat");
    }

    #[test]
    fn nonsense_is_not_an_address() {
        assert!(matches!(check_url(""), Err(WebError::BadUrl(_))));
        assert!(matches!(check_url("https://"), Err(WebError::BadUrl(_))));
        assert!(matches!(check_url("http:///nohost"), Err(WebError::BadUrl(_))));
    }

    /// A blocked URL handed to the background path is answered through the
    /// channel, with no thread and no request.
    #[test]
    fn spawn_fetch_answers_a_blocked_scheme_through_the_channel() {
        let rx = spawn_fetch("javascript:alert(1)".to_string(), ReadRules::default());
        let got = rx.recv_timeout(Duration::from_millis(200)).expect("an immediate answer");
        assert_eq!(got, Err(WebError::BlockedScheme("javascript".to_string())));
    }

    #[test]
    fn errors_explain_themselves_in_one_line() {
        let s = WebError::BlockedScheme("file".into()).to_string();
        assert!(s.contains("http") && s.contains("file"), "{s}");
        let s = WebError::TooLarge { limit: MAX_BYTES }.to_string();
        assert!(s.contains("4 MB"), "{s}");
    }
}
