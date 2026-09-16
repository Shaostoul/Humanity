//! The readable web: fetch a real page over HTTPS and turn it into a document
//! HumanityOS can draw itself, with no browser engine and no JavaScript.
//!
//! WHY THIS EXISTS. The long-term want is real websites on in-game monitors
//! (docs/design/decision-briefs.md, Brief 4). Embedding Chromium/CEF, Servo or
//! an OS webview was rejected there: hundreds of megabytes, a JS engine (which
//! is where tracking, popups and most of the attack surface live), and no way
//! to composite the result onto a world quad on our own terms. The call was
//! "the readable web": parse HTML with a real parser, keep the parts a person
//! reads (headings, paragraphs, links, images, lists, tables, code) and draw
//! them in egui. Cooperating sites and every one of our own pages render; a
//! site that is only a JavaScript app does not, and the view offers "open in
//! system browser" for those.
//!
//! WHAT LEAVES THE MACHINE. One HTTPS GET for the page the person opened, then
//! one GET per image the page declares (through the same image cache chat
//! uses). No cookies (the ureq `cookies` feature is not enabled, so there is
//! no cookie store to send from), no scripts, no fonts, no stylesheets, no
//! third-party beacons. The User-Agent is "HumanityOS/<version> readable-web".
//!
//! GATES. A URL is checked BEFORE any request: only `http` and `https` are
//! fetched, and the same check runs on every redirect hop, so a page cannot
//! bounce the reader to `file:`, `data:` or `javascript:`. Responses are
//! capped at [`MAX_BYTES`] and the whole fetch at [`TIMEOUT_SECS`]. The fetch
//! runs on a background thread ([`spawn_fetch`]); nothing here blocks the UI.
//!
//! LAYOUT OF THE MODULE.
//! - `dom.rs`   the arena tree html5ever builds into (our own TreeSink; the
//!              reference RcDom crate is a version behind the parser).
//! - `parse.rs` DOM to [`Page`]: the readability pass and block conversion.
//! - `fetch.rs` the HTTP side: scheme gate, redirects, caps, content type.
//!
//! The egui widget that draws a [`Page`] is `src/gui/widgets/web_view.rs`.
//! Design doc: docs/design/readable-web.md.

pub mod dom;
pub mod fetch;
pub mod parse;
pub mod sites;

pub use fetch::{check_url, fetch_page, spawn_fetch};
pub use parse::parse_html;

/// Largest response body the reader will accept, in bytes (4 MB). A readable
/// page is tens of kilobytes; the cap is there so a hostile or broken server
/// cannot make the app hold an unbounded buffer.
pub const MAX_BYTES: usize = 4 * 1024 * 1024;

/// Whole-request timeout in seconds (connect + headers + body).
pub const TIMEOUT_SECS: u64 = 10;

/// Redirect hops followed before giving up. Each hop re-runs the scheme gate.
pub const MAX_REDIRECTS: usize = 5;

/// The User-Agent sent with every page request. Plain and honest: a site can
/// tell it is us, and can serve a simpler page if it wants to.
pub fn user_agent() -> String {
    format!("HumanityOS/{} readable-web", env!("CARGO_PKG_VERSION"))
}

/// A run of styled text inside a block. Nesting is flattened: a link is one
/// inline with its whole visible text, bold inside a link stays link text.
/// That is deliberate; the reader draws text, it does not typeset it.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    /// Plain text. A `"\n"` inside it is a line break (`<br>`).
    Text(String),
    /// A clickable link. `href` is ABSOLUTE (resolved against the page URL)
    /// and always `http` or `https`; any other scheme became plain text.
    Link { href: String, text: String },
    /// `<strong>` / `<b>`.
    Strong(String),
    /// `<em>` / `<i>` / `<cite>`.
    Em(String),
    /// Inline `<code>` (block code is [`Block::Code`]).
    Code(String),
}

impl Inline {
    /// The visible text of this inline, whatever its style.
    pub fn text(&self) -> &str {
        match self {
            Inline::Text(t) | Inline::Strong(t) | Inline::Em(t) | Inline::Code(t) => t,
            Inline::Link { text, .. } => text,
        }
    }
}

/// One table cell. `header` is a `<th>`; the widget draws it bold.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub header: bool,
    pub inlines: Vec<Inline>,
}

/// One list item. `depth` is 0 for the list's own items, 1 for a list nested
/// inside an item, and so on; the widget indents by it.
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub depth: u8,
    pub inlines: Vec<Inline>,
}

/// A block of the readable document, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// `<h1>` to `<h6>`; `level` is 1 to 6.
    Heading { level: u8, inlines: Vec<Inline> },
    Paragraph(Vec<Inline>),
    List { ordered: bool, items: Vec<ListItem> },
    /// `src` is absolute and `http`/`https`. `alt` may be empty.
    Image { src: String, alt: String },
    /// `<pre>` text, whitespace preserved.
    Code(String),
    Table { rows: Vec<Vec<Cell>> },
    /// `<blockquote>`: a nested run of blocks.
    Quote(Vec<Block>),
    /// `<hr>`.
    Rule,
}

/// A fetched, parsed page: what the widget draws.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    /// The FINAL URL after redirects; relative links were resolved against it.
    pub url: String,
    /// `<title>`, or the host when the page has none.
    pub title: String,
    pub blocks: Vec<Block>,
    /// Set when `blocks` is empty, explaining why in words a reader can act
    /// on (for example: the page had only navigation and footers).
    pub notice: Option<String>,
}

/// Why a fetch did not produce a page. Every variant has a one-line
/// `Display` the status line shows verbatim.
#[derive(Debug, Clone, PartialEq)]
pub enum WebError {
    /// The URL's scheme is not `http` or `https`. Refused before any request.
    BlockedScheme(String),
    /// The text is not a URL at all.
    BadUrl(String),
    /// The body exceeded [`MAX_BYTES`].
    TooLarge { limit: usize },
    /// The server answered with something that is not a page (a PDF, a zip).
    NotHtml(String),
    /// A non-2xx status with no page to show.
    Http { status: u16 },
    /// DNS, TLS, connect, timeout: anything the transport reported.
    Network(String),
    /// More than [`MAX_REDIRECTS`] hops.
    TooManyRedirects,
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebError::BlockedScheme(s) => write!(
                f,
                "Blocked: only http and https pages can be read here (this link is \"{s}:\")"
            ),
            WebError::BadUrl(s) => write!(f, "Not a web address: {s}"),
            WebError::TooLarge { limit } => write!(
                f,
                "Page too large to read here (over {} MB). Open it in your system browser.",
                limit / (1024 * 1024)
            ),
            WebError::NotHtml(t) => write!(
                f,
                "Not a readable page (server sent \"{t}\"). Open it in your system browser."
            ),
            WebError::Http { status } => write!(f, "The site answered HTTP {status}"),
            WebError::Network(s) => write!(f, "Could not reach the site: {s}"),
            WebError::TooManyRedirects => {
                write!(f, "The site redirected more than {MAX_REDIRECTS} times; gave up")
            }
        }
    }
}

impl std::error::Error for WebError {}

/// Site-convention hints for the readability pass, from
/// `data/web/readability.json`. The HTML-spec part of readability (scripts,
/// styles, forms, nav/footer/aside are never content) is fixed in `parse.rs`;
/// what varies from site to site is which CLASSES and IDS mark screen-only
/// chrome (MediaWiki's `mw-editsection`, the widespread `noprint`). Those are
/// data, so a new site convention is a data edit, not a rebuild.
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct ReadRules {
    /// An element carrying any of these classes is dropped with its subtree.
    #[serde(default)]
    pub drop_classes: Vec<String>,
    /// An element with any of these ids is dropped with its subtree.
    #[serde(default)]
    pub drop_ids: Vec<String>,
}
