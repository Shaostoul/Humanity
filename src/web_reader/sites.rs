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
}
