//! The websites database: `data/web/sites.json`, one record per site the
//! Browser page offers, with whether its pages may legally be shown inside
//! HumanityOS and any affiliate relationship.
//!
//! Schema: `schemas/web_sites.toml`. Gate: `scripts/check-web-sites.js`
//! (unique ids, http(s) URLs, a decided `embed.status` needs its evidence,
//! our own domains allowed, an affiliate tag needs its disclosure). The web
//! mirror (`web/pages/web.html`) reads the same file; `tests/page_parity_lint.rs`
//! holds both clients to it. The loader is `crate::gui::load_web_sites`.
//! Workflow for the legality fields: docs/design/readable-web.md.

/// One site in the database.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebSite {
    pub id: String,
    pub name: String,
    pub url: String,
    /// A [`WebSiteCategory::id`].
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    pub embed: WebSiteEmbed,
    pub affiliate: WebSiteAffiliate,
    #[serde(default)]
    pub notes: String,
}

/// Whether this site's pages may be shown inside HumanityOS (on the Browser
/// page or an in-world screen). `status` leaves `needs_review` only when a
/// person has read the site's terms and recorded what they found.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebSiteEmbed {
    /// "needs_review" | "allowed" | "forbidden" | "unknown".
    pub status: String,
    /// The terms clause the decision rests on, or "not yet reviewed".
    #[serde(default)]
    pub basis: String,
    pub terms_url: Option<String>,
    /// ISO date of the review, or null.
    pub reviewed_on: Option<String>,
    /// Who reviewed it, or null.
    pub reviewed_by: Option<String>,
    /// The credit line the site's licence asks for (2026-09-25), e.g.
    /// "Wikipedia, under CC BY-SA 4.0". Shown under the status line on every
    /// page of the site; most "allowed" decisions rest on it, because the
    /// licences that allow reuse almost all require attribution.
    #[serde(default)]
    pub attribution: Option<String>,
}

/// Affiliate programme fields. All null today; recorded so the day a tag is
/// added, the disclosure shown on the frame is part of the same record.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebSiteAffiliate {
    pub program: Option<String>,
    pub tag: Option<String>,
    /// Shown above every page of the site while `tag` is set. The checker
    /// requires it whenever a tag is present.
    #[serde(default)]
    pub disclosure: String,
}

/// A category of sites. `color` is one of accent, info, success, warning,
/// danger (theme tokens; the page maps them to `Theme` accessors).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct WebSiteCategory {
    pub id: String,
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_color() -> String {
    "accent".to_string()
}

/// The whole database.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct WebSites {
    pub categories: Vec<WebSiteCategory>,
    pub sites: Vec<WebSite>,
    /// URL prefixes that are ours (the live site, the forge, the repository
    /// pages). Their embed status is "allowed" by definition; the checker
    /// enforces it, so code never needs its own list of what is ours.
    #[serde(default)]
    pub own_domains: Vec<String>,
}

impl WebSites {
    /// The affiliate disclosure to show for a page at `url`, if the site it
    /// belongs to carries a tag. Matched on host so every page of the site
    /// shows the line, not only the bookmarked one.
    pub fn disclosure_for(&self, url: &str) -> Option<&str> {
        let host = host_of(url)?;
        self.sites
            .iter()
            .filter(|s| s.affiliate.tag.is_some() && !s.affiliate.disclosure.is_empty())
            .find(|s| host_of(&s.url).map_or(false, |h| h == host))
            .map(|s| s.affiliate.disclosure.as_str())
    }
}

/// What may be done with a page at a given url, from the database (the
/// embed placement gate, 2026-09-25). Every host of the readable view (the
/// Browser page and every `web:` wall screen) asks this before a
/// navigation is fetched, through `WebViewState::apply_embed_gate`.
#[derive(Debug, Clone, PartialEq)]
pub enum EmbedVerdict {
    /// Ours (an `own_domains` prefix), or reviewed and allowed.
    Allowed,
    /// The site's terms were read and do not allow its pages to be shown
    /// inside other software. Never fetched, never drawn.
    Forbidden { name: String, basis: String },
    /// Listed, but nobody has read its terms yet (`needs_review`), or they
    /// could not be found (`unknown`). Shown, with a note saying so.
    Unreviewed { name: String, terms_not_found: bool },
    /// Not in the database at all (a link off a listed site, or a typed
    /// address). Shown, with a note saying so.
    NotListed { host: String },
}

impl EmbedVerdict {
    /// The one line a host draws with the page, or None when there is
    /// nothing to say. Worded for the person reading, and plain about whose
    /// rule it is (the operator's standing rule: name a platform's rule as
    /// the platform's, never as ours).
    pub fn note(&self) -> Option<String> {
        match self {
            EmbedVerdict::Allowed => None,
            EmbedVerdict::Forbidden { name, basis } => Some(format!(
                "Not shown inside HumanityOS: {name}'s own terms do not allow its pages to be \
                 shown inside other software ({basis}). That is the site's rule, not a law and \
                 not a HumanityOS setting; your system browser can still open it."
            )),
            EmbedVerdict::Unreviewed { name, terms_not_found: false } => Some(format!(
                "Review pending: nobody has checked yet whether {name}'s terms allow its pages \
                 to be shown here."
            )),
            EmbedVerdict::Unreviewed { name, terms_not_found: true } => Some(format!(
                "{name}'s terms could not be found, so whether it allows its pages to be shown \
                 here is unknown."
            )),
            EmbedVerdict::NotListed { host } => Some(format!(
                "{host} is not in the sites list, so whether its terms allow its pages to be \
                 shown here has not been checked."
            )),
        }
    }
}

/// True when `url` is `prefix` or a page under it (the next character is a
/// path, query or fragment separator), case-insensitively. So
/// "https://github.com/Shaostoul/Humanity/issues" is under the repository
/// prefix and "https://github.com/Shaostoul/HumanityFork" is not.
fn url_under(url: &str, prefix: &str) -> bool {
    let u = url.to_ascii_lowercase();
    let p = prefix.trim_end_matches('/').to_ascii_lowercase();
    u == p || (u.starts_with(&p) && matches!(u.as_bytes().get(p.len()), Some(b'/' | b'?' | b'#')))
}

impl WebSites {
    /// The placement verdict for a page at `url`. Our own domains are
    /// allowed by prefix. Otherwise the site is found by host; when several
    /// records share a host the one whose url is the longest prefix of the
    /// page wins, and a record that is one of our own prefixes never speaks
    /// for the rest of its host (the repository record does not make every
    /// github.com page "allowed").
    pub fn embed_verdict(&self, url: &str) -> EmbedVerdict {
        if self.is_own(url) {
            return EmbedVerdict::Allowed;
        }
        let Some(host) = host_of(url) else {
            return EmbedVerdict::NotListed { host: url.to_string() };
        };
        match self.site_for(url) {
            None => EmbedVerdict::NotListed { host },
            Some(s) => match s.embed.status.as_str() {
                "allowed" => EmbedVerdict::Allowed,
                "forbidden" => EmbedVerdict::Forbidden { name: s.name.clone(), basis: s.embed.basis.clone() },
                "unknown" => EmbedVerdict::Unreviewed { name: s.name.clone(), terms_not_found: true },
                _ => EmbedVerdict::Unreviewed { name: s.name.clone(), terms_not_found: false },
            },
        }
    }

    fn is_own(&self, url: &str) -> bool {
        self.own_domains.iter().any(|d| url_under(url, d))
    }

    /// The record a page belongs to, by the rules `embed_verdict` documents:
    /// matched on host, the longest url prefix winning, and never one of our
    /// own prefixes speaking for the rest of its host.
    fn site_for(&self, url: &str) -> Option<&WebSite> {
        let host = host_of(url)?;
        let candidates: Vec<&WebSite> = self
            .sites
            .iter()
            .filter(|s| host_of(&s.url).as_deref() == Some(host.as_str()))
            .filter(|s| !self.is_own(&s.url))
            .collect();
        candidates
            .iter()
            .filter(|s| url_under(url, &s.url))
            .max_by_key(|s| s.url.len())
            .or_else(|| candidates.first())
            .copied()
    }

    /// The credit line for a page on an ALLOWED site whose record carries one
    /// (see `WebSiteEmbed::attribution`). None for our own pages, for sites
    /// without a credit, and for anything not allowed.
    pub fn attribution_for(&self, url: &str) -> Option<&str> {
        if self.is_own(url) {
            return None;
        }
        self.site_for(url)
            .filter(|s| s.embed.status == "allowed")
            .and_then(|s| s.embed.attribution.as_deref())
            .filter(|a| !a.trim().is_empty())
    }
}

/// "https://Example.com:8443/x?y#z" -> "example.com". Plain string handling:
/// the card only needs the host for its footer line.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_of_strips_scheme_path_port_and_case() {
        assert_eq!(host_of("https://Example.com:8443/x?y#z").as_deref(), Some("example.com"));
        assert_eq!(host_of("http://user@host.example/p").as_deref(), Some("host.example"));
        assert_eq!(host_of("nonsense"), None);
        assert_eq!(host_of("https://"), None);
    }

    /// The shipped database parses into these types, every category a site
    /// names exists, and no site carries a disclosure today (all tags null),
    /// so `disclosure_for` returns nothing for any of them.
    #[test]
    fn shipped_database_loads_and_has_no_affiliate_disclosures_yet() {
        let text = std::fs::read_to_string("data/web/sites.json").expect("data/web/sites.json readable");
        let db: WebSites = serde_json::from_str(&text).expect("sites.json matches the WebSites types");
        assert!(db.sites.len() >= 30, "seeded with the merged lists, got {}", db.sites.len());
        for s in &db.sites {
            assert!(db.categories.iter().any(|c| c.id == s.category), "{} names unknown category {}", s.id, s.category);
            assert!(db.disclosure_for(&s.url).is_none(), "{} has no tag, so no disclosure", s.id);
        }
        assert!(!db.own_domains.is_empty());
    }

    #[test]
    fn disclosure_matches_every_page_of_a_tagged_site_by_host() {
        let mut db = WebSites::default();
        db.sites.push(WebSite {
            id: "shop".into(),
            name: "Shop".into(),
            url: "https://shop.example/home".into(),
            category: "c".into(),
            description: String::new(),
            icon: String::new(),
            embed: WebSiteEmbed {
                status: "allowed".into(),
                basis: "x".into(),
                terms_url: None,
                reviewed_on: None,
                reviewed_by: None,
                attribution: None,
            },
            affiliate: WebSiteAffiliate {
                program: Some("p".into()),
                tag: Some("t".into()),
                disclosure: "This link supports HumanityOS.".into(),
            },
            notes: String::new(),
        });
        assert_eq!(db.disclosure_for("https://shop.example/some/other/page"), Some("This link supports HumanityOS."));
        assert_eq!(db.disclosure_for("https://other.example/"), None);
    }

    fn site(id: &str, url: &str, status: &str) -> WebSite {
        WebSite {
            id: id.into(),
            name: id.into(),
            url: url.into(),
            category: "c".into(),
            description: String::new(),
            icon: String::new(),
            embed: WebSiteEmbed {
                status: status.into(),
                basis: format!("{id} terms section 4"),
                terms_url: None,
                reviewed_on: None,
                reviewed_by: None,
                attribution: None,
            },
            affiliate: WebSiteAffiliate { program: None, tag: None, disclosure: String::new() },
            notes: String::new(),
        }
    }

    /// The placement gate's verdicts: every status, our own prefixes, the
    /// longest-prefix rule for a shared host, and the rule that our own
    /// repository record does not speak for the rest of github.com.
    #[test]
    fn embed_verdict_covers_every_status_and_shared_hosts() {
        let mut db = WebSites::default();
        db.own_domains = vec!["https://github.com/Shaostoul/Humanity".into()];
        db.sites = vec![
            site("ours", "https://github.com/Shaostoul/Humanity", "allowed"),
            site("nope", "https://nope.example/", "forbidden"),
            site("fine", "https://fine.example/", "allowed"),
            site("pending", "https://pending.example/", "needs_review"),
            site("lost", "https://lost.example/", "unknown"),
            site("wiki-en", "https://wiki.example/en", "allowed"),
            site("wiki-de", "https://wiki.example/de", "forbidden"),
        ];
        let v = |u: &str| db.embed_verdict(u);
        assert_eq!(v("https://github.com/Shaostoul/Humanity/issues/3"), EmbedVerdict::Allowed);
        assert_eq!(v("https://github.com/someone/else"), EmbedVerdict::NotListed { host: "github.com".into() });
        assert_eq!(v("https://github.com/Shaostoul/HumanityFork"), EmbedVerdict::NotListed { host: "github.com".into() });
        assert!(matches!(v("https://NOPE.example/a/b"), EmbedVerdict::Forbidden { .. }));
        assert_eq!(v("https://fine.example/x"), EmbedVerdict::Allowed);
        assert_eq!(v("https://pending.example/"), EmbedVerdict::Unreviewed { name: "pending".into(), terms_not_found: false });
        assert_eq!(v("https://lost.example/"), EmbedVerdict::Unreviewed { name: "lost".into(), terms_not_found: true });
        assert_eq!(v("https://wiki.example/en/Page"), EmbedVerdict::Allowed);
        assert!(matches!(v("https://wiki.example/de/Seite"), EmbedVerdict::Forbidden { .. }));
        assert_eq!(v("https://elsewhere.example/"), EmbedVerdict::NotListed { host: "elsewhere.example".into() });
        assert!(v("https://nope.example/").note().unwrap().contains("the site's rule, not a law"));
        assert_eq!(EmbedVerdict::Allowed.note(), None);
    }

    /// The credit line: shown for an allowed site that carries one, never
    /// for our own pages, a forbidden site or an unlisted host.
    #[test]
    fn attribution_is_given_only_for_allowed_sites_that_carry_one() {
        let mut db = WebSites::default();
        db.own_domains = vec!["https://github.com/Shaostoul/Humanity".into()];
        let mut wiki = site("wiki", "https://wiki.example/", "allowed");
        wiki.embed.attribution = Some("Wiki, under CC BY-SA 4.0".into());
        let mut nope = site("nope", "https://nope.example/", "forbidden");
        nope.embed.attribution = Some("never shown".into());
        let mut ours = site("ours", "https://github.com/Shaostoul/Humanity", "allowed");
        ours.embed.attribution = Some("never shown either".into());
        db.sites = vec![wiki, nope, ours, site("plain", "https://plain.example/", "allowed")];
        assert_eq!(db.attribution_for("https://wiki.example/a/b"), Some("Wiki, under CC BY-SA 4.0"));
        assert_eq!(db.attribution_for("https://nope.example/x"), None);
        assert_eq!(db.attribution_for("https://github.com/Shaostoul/Humanity/issues"), None);
        assert_eq!(db.attribution_for("https://plain.example/"), None);
        assert_eq!(db.attribution_for("https://elsewhere.example/"), None);
    }

    /// The 2026-09-25 decisions: nothing is left awaiting review, and every
    /// ALLOWED third-party site carries its credit line, because almost every
    /// one of those decisions rests on a licence that requires attribution.
    #[test]
    fn every_allowed_third_party_site_carries_its_credit_line() {
        let text = std::fs::read_to_string("data/web/sites.json").expect("data/web/sites.json readable");
        let db: WebSites = serde_json::from_str(&text).expect("sites.json parses");
        for s in &db.sites {
            assert_ne!(s.embed.status, "needs_review", "{} is still awaiting review", s.id);
            if s.embed.status == "allowed" && !db.is_own(&s.url) {
                assert!(db.attribution_for(&s.url).is_some(), "{} is allowed but carries no credit line", s.id);
            }
        }
    }

    /// Every shipped record gets a verdict matching its own status, so the
    /// gate agrees with what the Browser page's cards say about each site.
    #[test]
    fn every_shipped_site_gets_the_verdict_its_record_states() {
        let text = std::fs::read_to_string("data/web/sites.json").expect("data/web/sites.json readable");
        let db: WebSites = serde_json::from_str(&text).expect("sites.json parses");
        for s in &db.sites {
            let got = db.embed_verdict(&s.url);
            let ok = match s.embed.status.as_str() {
                "allowed" => got == EmbedVerdict::Allowed,
                "forbidden" => matches!(got, EmbedVerdict::Forbidden { .. }),
                _ => matches!(got, EmbedVerdict::Unreviewed { .. }),
            };
            assert!(ok, "{} ({}) got {:?}", s.id, s.embed.status, got);
        }
    }
}
