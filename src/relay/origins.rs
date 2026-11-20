//! Which websites' pages a browser may call this relay from, when its owner never said
//! (BUG-329, 2026-11-20).
//!
//! `ALLOWED_ORIGINS` (relay/mod.rs `allowed_origins`) is the list the WebSocket Origin check, CORS
//! and the Content-Security-Policy are built from. Until 2026-11-20 a relay with it unset used
//! united-humanity.us's two addresses, so every other server that was never told its own names
//! let united-humanity.us's pages call it and refused its own (the same mistake as BUG-322, where
//! the call forwarder named united-humanity.us). Now an unset list names only this server:
//!
//! - its own public name, when it has one ([`own_site`]: `TURN_PUBLIC_HOST`, else
//!   `TURN_SERVER_HOST`, the settings the call forwarder already names it by), and
//! - for the WebSocket, a page this server itself served over https ([`same_origin`]: the
//!   `Origin` is `https://` plus the request's own `Host`). Every page of the website opens its
//!   socket at `location.host`, so this is exactly the server's own pages, whatever name they were
//!   served under (chat.<domain>, www.<domain>). Only https counts: a certificate binds that name
//!   to this server, so a page of someone else's site whose name was pointed at this machine
//!   (DNS rebinding) cannot pass, while plain http could.
//!
//! The desktop app's and this machine's own addresses stay allowed either way (mod.rs). The log
//! says once, at start, which setting lets other sites in ([`not_configured_line`]).

/// Whether the owner gave a list: `ALLOWED_ORIGINS` set to something other than blanks.
pub fn configured(env_val: Option<&str>) -> bool {
    env_val.is_some_and(|v| !v.trim().is_empty())
}

/// This server's own website, `https://<name>`, from the settings `var` reads (the environment,
/// or a test's map): `TURN_PUBLIC_HOST`, else `TURN_SERVER_HOST`, when it is a domain name. An IP
/// address, `localhost` or anything that is not a plain name gives nothing: no browser shows a
/// site at `https://203.0.113.4`, and a value with a scheme, a path or a space is a typo, never
/// a pattern to allow.
pub fn own_site(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    let set = |k: &str| var(k).map(|v| v.trim().to_ascii_lowercase()).filter(|v| !v.is_empty());
    let name = set("TURN_PUBLIC_HOST").or_else(|| set("TURN_SERVER_HOST"))?;
    let plain = name.contains('.')
        && name.parse::<std::net::IpAddr>().is_err()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && !name.starts_with(['.', '-'])
        && !name.ends_with(['.', '-'])
        && !name.contains("..");
    plain.then(|| format!("https://{name}"))
}

/// Whether `origin` is a page this server itself served over https: `https://` followed by the
/// request's `Host` exactly (its port included), letter case aside.
pub fn same_origin(origin: &str, host: Option<&str>) -> bool {
    let Some(host) = host.map(str::trim).filter(|h| !h.is_empty()) else { return false };
    if host.bytes().any(|b| b.is_ascii_whitespace() || b == b',' || b == b'/') {
        return false;
    }
    origin.strip_prefix("https://").is_some_and(|rest| rest.eq_ignore_ascii_case(host))
}

/// The WebSocket's decision: `origin` is on the `list`, or (only when the owner gave no list) a
/// page this server served itself over https.
pub fn accepts(list: &[String], unconfigured: bool, origin: &str, host: Option<&str>) -> bool {
    list.iter().any(|a| a == origin) || (unconfigured && same_origin(origin, host))
}

/// The line logged once at start when the owner gave no list, naming the setting.
pub fn not_configured_line(own: Option<&str>) -> String {
    let own = match own {
        Some(site) => format!("{site} (its TURN_PUBLIC_HOST) and "),
        None => String::new(),
    };
    format!(
        "ALLOWED_ORIGINS is not set: browsers may call this relay only from {own}pages this server serves itself over \
         https, never from another server's. If pages on other addresses should reach it, add \
         ALLOWED_ORIGINS=https://<this server's domain>,https://chat.<this server's domain> to the relay's .env \
         (/opt/Humanity/.env on a server set up by scripts/provision-vps.sh, which writes it) and restart the relay \
         (docs/admin/SELF-HOSTING.md, Configuration)"
    )
}

/// Whether this run's owner gave no list (read once, with the list itself in mod.rs).
pub(crate) fn unconfigured() -> bool {
    static UNSET: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *UNSET.get_or_init(|| !configured(std::env::var("ALLOWED_ORIGINS").ok().as_deref()))
}

#[cfg(test)]
#[path = "origins_tests.rs"]
mod tests;
