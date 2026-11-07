//! Requests the server sends to an address someone else handed it: only to the public internet
//! (2026-11-06, BUG-255). THE ONE PLACE for this in the relay. Web Push endpoints (`push_send.rs`,
//! `push_routes.rs`) and link previews (`handlers/utils.rs` `fetch_link_preview`) go through it, and
//! so must anything new that fetches or posts to an address a member, a message or another server
//! supplied (the planned "Where does this go?" link check, docs/design/outbox-and-inbox.md). Never
//! send such a request through the shared `RelayState::http_client`.
//!
//! THE HOLE THIS CLOSES. Until 2026-11-06 the relay kept any push endpoint a signed-in person gave
//! it and POSTed to it on every notice, so anyone could make the server send requests to itself
//! (127.0.0.1), into its own network (10.x, 192.168.x), or to a cloud's metadata service
//! (169.254.169.254): server-side request forgery, with a 404/410 "forget it" as a small signal
//! back. The link preview fetcher (not yet called, switched on by chat-access rung 3) had the same
//! class: it checked a name once, then let its client resolve it again at connect time and follow
//! three redirects unchecked, counted only IPv6 loopback as private, and fetched anyway when the
//! name did not resolve.
//!
//! THE RULE (`OutboundGuard::shape`, then the addresses). An address is used only when it is a web
//! address (https, or http too where `Schemes::HttpOrHttps` allows it) of at most `MAX_URL_LEN`
//! characters, printable ASCII, with no name or password in it, whose host is a NAME (never a bare
//! IP address, however it is written: `https://2130706433/` is 127.0.0.1 too, and the url crate
//! reads it so), not `localhost`, and every address that name has is public (`is_public`: not
//! loopback, private, link-local, unique-local, CGNAT, multicast, unspecified, documentation,
//! benchmarking or reserved, IPv4 and IPv6, an IPv4 address carried inside an IPv6 one judged as
//! that IPv4 address; a name with a public and a private address is refused whole; a name with no
//! address is refused). It is NOT a list of known services: a self-hosted push service (UnifiedPush,
//! ntfy, autopush) works as long as it is public.
//!
//! HOW THE ADDRESS CHECKED IS THE ADDRESS USED. A host written as an IP address never reaches a
//! resolver (hyper-util's connector parses it and connects straight to it,
//! `SocketAddrs::try_parse`), so the shape check refuses it before reqwest sees the URL, every time
//! a request is made. A name is looked up by the guard's client's own resolver (`PublicOnly`),
//! which hands the connector the addresses only when every one is public: checked and connected in
//! one step, with no second lookup for a rebinding name (DNS rebinding) to race. The client follows
//! no redirect by itself (a 3xx to http://127.0.0.1 would otherwise skip both checks) and uses no
//! proxy (a proxy looks the name up itself, past our resolver); `OutboundGuard::get` follows
//! redirects itself, each hop's address checked the same way, at most the number it is given.
//!
//! THE TEST SEAM. Tests need stand-in services, which can only listen on 127.0.0.1 over plain http.
//! `OutboundGuard::allowing_test_service(port)` lets exactly `http://127.0.0.1:<port>/...` through,
//! and `OutboundGuard::with_names` swaps the system's resolver for fixed answers. Both, and the
//! field and the variant they set, are `#[cfg(test)]`: they do not exist in a release build, in the
//! relay build the servers run, or in any build a setting or an environment variable could reach.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// The longest address the relay sends to. Browsers' push endpoints are a few hundred characters
/// (FCM's about 190, Mozilla's and Windows' up to a few hundred); 2,048 is the classic URL limit.
pub const MAX_URL_LEN: usize = 2048;

/// How long a name may take to look up before it counts as not found.
const LOOKUP_LIMIT: Duration = Duration::from_secs(10);

/// Which schemes an address may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schemes {
    /// https only: Web Push (RFC 8030 requires it; a notice's metadata never travels in the clear).
    HttpsOnly,
    /// http or https: a web page someone linked to (link previews, the link check).
    HttpOrHttps,
}

/// Why an address is refused. `reason` is the code the routes answer with (the website shows its
/// own words for a push refusal, by key: `notifications.push_refused_*` in data/i18n/en.json),
/// `words` the relay's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Longer than `MAX_URL_LEN`.
    TooLong,
    /// Not a URL, not printable ASCII, or not a web scheme at all.
    NotAWebAddress,
    /// Not https://, where only https is allowed.
    NotHttps,
    /// A user name or password written into it.
    HasUserinfo,
    /// The host is an IP address, not a name.
    AddressNotName,
    /// The name leads to an address that is not on the public internet (or is localhost).
    PrivateNetwork,
    /// The name could not be looked up, or has no address.
    UnknownName,
}

impl Refusal {
    /// The code a client matches to its words.
    pub fn reason(self) -> &'static str {
        match self {
            Refusal::TooLong => "too_long",
            Refusal::NotAWebAddress => "not_web_address",
            Refusal::NotHttps => "not_https",
            Refusal::HasUserinfo => "userinfo",
            Refusal::AddressNotName => "address",
            Refusal::PrivateNetwork => "private",
            Refusal::UnknownName => "unknown_name",
        }
    }

    /// The relay's own plain words, for an answer's `error` and the log.
    pub fn words(self) -> &'static str {
        match self {
            Refusal::TooLong => "That address is longer than this server will use (2,048 characters).",
            Refusal::NotAWebAddress => "That is not a web address this server can use.",
            Refusal::NotHttps => "That address does not use https, and this server sends this only over https.",
            Refusal::HasUserinfo => "That address has a name or password written into it, which this server does not accept.",
            Refusal::AddressNotName => "That address is a bare IP address. This server sends requests only to places reached by name.",
            Refusal::PrivateNetwork => {
                "That address leads to a private or local network. This server sends requests only to the public internet, \
                 so that nobody can use it to reach machines that are not meant to be reached from outside."
            }
            Refusal::UnknownName => "This server could not find that address's name.",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.words())
    }
}

impl std::error::Error for Refusal {}

/// Why `OutboundGuard::get` brought back no answer.
#[derive(Debug)]
pub enum FetchError {
    /// The address, or a redirect's, is one the server does not send to.
    Refused(Refusal),
    /// More redirects than allowed.
    TooManyRedirects,
    /// A redirect with no usable Location.
    BadRedirect,
    /// The request itself failed (no answer, a timeout, a TLS error...), or no client could be built.
    Failed(String),
}

/// The address's shape, checked with nothing looked up: what every request is checked against
/// first, and again before each redirect is followed. Returns the parsed URL.
pub fn url_shape(url: &str, schemes: Schemes) -> Result<url::Url, Refusal> {
    if url.len() > MAX_URL_LEN {
        return Err(Refusal::TooLong);
    }
    // Printable ASCII, no spaces: what a serialized URL (a browser's PushSubscription.endpoint, a
    // Location header) is, and what keeps each signed line of a push subscribe request one line.
    if url.is_empty() || !url.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return Err(Refusal::NotAWebAddress);
    }
    let parsed = url::Url::parse(url).map_err(|_| Refusal::NotAWebAddress)?;
    match (parsed.scheme(), schemes) {
        ("https", _) | ("http", Schemes::HttpOrHttps) => {}
        (_, Schemes::HttpsOnly) => return Err(Refusal::NotHttps),
        (_, Schemes::HttpOrHttps) => return Err(Refusal::NotAWebAddress),
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(Refusal::HasUserinfo);
    }
    match parsed.host() {
        Some(url::Host::Domain(name)) => {
            // RFC 6761 section 6.3: localhost names are the machine itself, whatever a resolver says.
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if name == "localhost" || name.ends_with(".localhost") {
                return Err(Refusal::PrivateNetwork);
            }
        }
        Some(url::Host::Ipv4(_)) | Some(url::Host::Ipv6(_)) => return Err(Refusal::AddressNotName),
        None => return Err(Refusal::NotAWebAddress),
    }
    Ok(parsed)
}

/// Whether `ip` is an address on the public internet, where the server may send a request.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_public(v4),
        IpAddr::V6(v6) => v6_public(v6),
    }
}

/// IPv4: everything but the special-purpose blocks that are not globally reachable (the IANA
/// IPv4 Special-Purpose Address Registry, RFC 6890 and its updates).
fn v4_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(o[0] == 0                                    // 0.0.0.0/8, "this network", 0.0.0.0 included
        || ip.is_private()                         // 10/8, 172.16/12, 192.168/16
        || ip.is_loopback()                        // 127/8
        || ip.is_link_local()                      // 169.254/16, a cloud's metadata service
        || (o[0] == 100 && (o[1] & 0xc0) == 64)    // 100.64/10, carrier-grade NAT (RFC 6598)
        || (o[0] == 192 && o[1] == 0 && o[2] == 0) // 192.0.0/24, IETF protocol assignments
        || ip.is_documentation()                   // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || (o[0] == 198 && (o[1] & 0xfe) == 18)    // 198.18/15, benchmarking
        || ip.is_multicast()                       // 224/4
        || o[0] >= 240) // 240/4 reserved, 255.255.255.255 broadcast
}

/// IPv6: only the global unicast block 2000::/3 (everything else is loopback, unspecified,
/// link-local, unique-local, multicast or unassigned), less its special blocks; and an IPv4
/// address carried in an IPv6 one (mapped, or NAT64 on an IPv6-only host behind DNS64) judged
/// as the IPv4 address it reaches.
fn v6_public(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_public(v4); // ::ffff:a.b.c.d
    }
    let s = ip.segments();
    let carried = |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return v4_public(carried(s[6], s[7])); // 64:ff9b::/96, NAT64 (RFC 6052)
    }
    if s[0] & 0xe000 != 0x2000 {
        return false; // outside 2000::/3: ::, ::1, ::a.b.c.d, fc00::/7, fe80::/10, ff00::/8, 64:ff9b:1::/48...
    }
    if s[0] == 0x2001 && s[1] < 0x0200 {
        return false; // 2001::/23, IETF protocol assignments (Teredo, ORCHID, benchmarking...)
    }
    if s[0] == 0x2001 && s[1] == 0x0db8 {
        return false; // 2001:db8::/32, documentation
    }
    if s[0] & 0xfff0 == 0x3ff0 {
        return false; // 3fff::/20, documentation (RFC 9637)
    }
    if s[0] == 0x2002 {
        return v4_public(carried(s[1], s[2])); // 2002::/16, 6to4: the IPv4 address it carries
    }
    true
}

/// How a name becomes addresses: the system's resolver, or in a test, fixed answers.
#[derive(Clone)]
enum Lookup {
    System,
    /// Tests only: name -> addresses, a name not listed being not found.
    #[cfg(test)]
    Fixed(Arc<std::collections::HashMap<String, Vec<IpAddr>>>),
}

impl Lookup {
    async fn addresses(&self, host: &str) -> std::io::Result<Vec<IpAddr>> {
        match self {
            Lookup::System => Ok(tokio::net::lookup_host((host, 0)).await?.map(|a| a.ip()).collect()),
            #[cfg(test)]
            Lookup::Fixed(names) => names
                .get(host)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such name")),
        }
    }
}

/// Every address `host` has, when all of them are public; refused when any is not (a name that
/// mixes a public address with a private one is refused whole), or when it has none.
async fn public_addresses(lookup: &Lookup, host: &str) -> Result<Vec<IpAddr>, Refusal> {
    let found = tokio::time::timeout(LOOKUP_LIMIT, lookup.addresses(host))
        .await
        .map_err(|_| Refusal::UnknownName)?
        .map_err(|_| Refusal::UnknownName)?;
    if found.is_empty() {
        return Err(Refusal::UnknownName);
    }
    if found.iter().any(|ip| !is_public(*ip)) {
        return Err(Refusal::PrivateNetwork);
    }
    Ok(found)
}

/// The guard's clients' resolver: the connector is handed a name's addresses only after every one
/// was checked public, so a request connects to exactly the addresses checked.
struct PublicOnly(Lookup);

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let lookup = self.0.clone();
        let host = name.as_str().to_string();
        Box::pin(async move {
            let found = public_addresses(&lookup, &host).await?;
            // Port 0: reqwest puts the URL's port (or the scheme's) in its place.
            let addrs: reqwest::dns::Addrs = Box::new(found.into_iter().map(|ip| SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}

/// The refusal inside a failed request, when the guard's resolver was what stopped it (reqwest
/// carries the resolver's error down its chain of causes).
pub fn refusal_in(e: &(dyn std::error::Error + 'static)) -> Option<Refusal> {
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(c) = cause {
        if let Some(r) = c.downcast_ref::<Refusal>() {
            return Some(*r);
        }
        cause = c.source();
    }
    None
}

/// Where the server may send requests it was handed an address for, and the clients it sends
/// them with: one per relay, `RelayState::outbound`.
pub struct OutboundGuard {
    lookup: Lookup,
    /// One client per `Schemes`, each built on first use (most relay states, the tests' included,
    /// never send one); None when it could not be built, and then nothing is sent.
    https_client: OnceLock<Option<reqwest::Client>>,
    web_client: OnceLock<Option<reqwest::Client>>,
    /// Tests only: the port of the stand-in service at http://127.0.0.1, the one loopback address
    /// let through (`allowing_test_service`).
    #[cfg(test)]
    test_service_port: Option<u16>,
}

impl Default for OutboundGuard {
    fn default() -> Self {
        Self::with_lookup(Lookup::System)
    }
}

impl OutboundGuard {
    fn with_lookup(lookup: Lookup) -> Self {
        Self {
            lookup,
            https_client: OnceLock::new(),
            web_client: OnceLock::new(),
            #[cfg(test)]
            test_service_port: None,
        }
    }

    /// The address's shape, with nothing looked up (`url_shape`): checked before every request and
    /// every redirect followed, since a host written as an IP address never reaches the resolver.
    pub fn shape(&self, url: &str, schemes: Schemes) -> Result<url::Url, Refusal> {
        #[cfg(test)]
        if let Some(u) = self.test_service(url) {
            return Ok(u);
        }
        url_shape(url, schemes)
    }

    /// Everything an address must pass to be KEPT for later (a push subscription): its shape, then
    /// every address its name has now is public. A request is still checked again when it is sent
    /// (`shape` and the client's resolver), since what a name points at can change.
    pub async fn check(&self, url: &str, schemes: Schemes) -> Result<(), Refusal> {
        let parsed = self.shape(url, schemes)?;
        #[cfg(test)]
        if self.test_service(url).is_some() {
            return Ok(());
        }
        let host = parsed.host_str().ok_or(Refusal::NotAWebAddress)?;
        public_addresses(&self.lookup, host).await.map(|_| ())
    }

    /// The client requests to checked addresses are sent with: its resolver refuses any name with a
    /// non-public address, it follows no redirect, uses no proxy, speaks only the schemes allowed,
    /// and gives up after 30 seconds (a request can set less). Always `shape` the URL first. None
    /// when it could not be built (the TLS setup failed), and then nothing is sent.
    pub fn client(&self, schemes: Schemes) -> Option<&reqwest::Client> {
        let (cell, https_only) = match schemes {
            Schemes::HttpsOnly => (&self.https_client, self.https_only()),
            Schemes::HttpOrHttps => (&self.web_client, false),
        };
        cell.get_or_init(|| {
            let built = reqwest::Client::builder()
                .dns_resolver(Arc::new(PublicOnly(self.lookup.clone())))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .https_only(https_only)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build();
            match built {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::error!("The outbound client could not be built ({e}); no request will be sent");
                    None
                }
            }
        })
        .as_ref()
    }

    /// GET `url`, following at most `max_redirects` redirects, each hop's address checked as the
    /// first was (its shape here, its addresses by the client's resolver as it connects). `tune`
    /// adds headers or a shorter time limit to each hop's request. The answer is the first that is
    /// not a redirect.
    pub async fn get(
        &self,
        url: &str,
        schemes: Schemes,
        max_redirects: usize,
        tune: impl Fn(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, FetchError> {
        let client = self.client(schemes).ok_or_else(|| FetchError::Failed("no client".into()))?;
        let mut here = self.shape(url, schemes).map_err(FetchError::Refused)?;
        let mut redirects = 0;
        loop {
            let resp = tune(client.get(here.clone())).send().await.map_err(|e| match refusal_in(&e) {
                Some(r) => FetchError::Refused(r),
                None => FetchError::Failed(e.to_string()),
            })?;
            if !resp.status().is_redirection() {
                return Ok(resp);
            }
            if redirects == max_redirects {
                return Err(FetchError::TooManyRedirects);
            }
            redirects += 1;
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or(FetchError::BadRedirect)?;
            let next = here.join(location).map_err(|_| FetchError::BadRedirect)?;
            here = self.shape(next.as_str(), schemes).map_err(FetchError::Refused)?;
        }
    }

    /// Always true outside the tests: only the stand-in service is plain http.
    fn https_only(&self) -> bool {
        #[cfg(test)]
        if self.test_service_port.is_some() {
            return false;
        }
        true
    }
}

#[cfg(test)]
impl OutboundGuard {
    /// Tests only: the system's resolver swapped for fixed answers, so a public-looking name can
    /// lead to loopback or 10.x and a test can see it refused. A name not listed is not found.
    pub(crate) fn with_names(names: &[(&str, IpAddr)]) -> Self {
        let mut map: std::collections::HashMap<String, Vec<IpAddr>> = Default::default();
        for (name, ip) in names {
            map.entry(name.to_string()).or_default().push(*ip);
        }
        Self::with_lookup(Lookup::Fixed(Arc::new(map)))
    }

    /// Tests only: requests may also go to the plain-http stand-in service at
    /// http://127.0.0.1:`port`, and to no other loopback address or port.
    pub(crate) fn allowing_test_service(self, port: u16) -> Self {
        Self { test_service_port: Some(port), https_client: OnceLock::new(), web_client: OnceLock::new(), ..self }
    }

    fn test_service(&self, url: &str) -> Option<url::Url> {
        let port = self.test_service_port?;
        url::Url::parse(url)
            .ok()
            .filter(|u| u.scheme() == "http" && u.host() == Some(url::Host::Ipv4(Ipv4Addr::LOCALHOST)) && u.port() == Some(port))
    }
}

/// The tests; their stand-in services (a tripwire, a small web server) are shared with the link
/// preview tests (handlers/link_preview_tests.rs).
#[cfg(test)]
#[path = "outbound_tests.rs"]
pub(crate) mod tests;
