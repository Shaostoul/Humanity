//! DOM to [`Page`]: the readability pass and the block conversion.
//!
//! Two ideas do all the work:
//!
//! 1. READABILITY. Some elements are never content by the HTML spec (`script`,
//!    `style`, `template`, `noscript`), some are chrome around the content
//!    (`nav`, `footer`, `aside`, the landmark roles that mean the same), and
//!    forms are interaction, which the reader does not do. All of those are
//!    dropped with their subtrees. If the page marks its content with
//!    `<main>` or `<article>`, only that subtree is read. Site-specific
//!    conventions (which class means "screen-only chrome") come from
//!    `data/web/readability.json` as [`ReadRules`], not from code.
//!
//! 2. BLOCKS AND INLINES. The tree is walked once. Block elements flush the
//!    paragraph being built and emit their own block; everything else is
//!    inline and appends styled text to the paragraph under construction. A
//!    style context (inside a link, inside strong, inside em, inside code)
//!    decides which [`Inline`] each text node becomes, so nesting in any
//!    order still reads correctly. Whitespace collapses the way a browser
//!    collapses it outside `<pre>`.
//!
//! Relative URLs (links, images, `<base href>`) resolve against the page URL
//! with the `url` crate, so `/wiki/Foo`, `../x.png` and `//host/path` all
//! come out absolute.

use html5ever::tendril::TendrilSink;
use url::Url;

use super::dom::{Dom, Node, NodeData, NodeId};
use super::{Block, Cell, Inline, ListItem, Page, ReadRules};

/// Elements that are never readable content, by the HTML spec or by what
/// they are for. NOT a domain list (see docs/design/infinite-of-x.md): these
/// are the parser's fixed semantics, like the markdown reader's `#` and `-`.
/// Site conventions live in the data file instead.
const DROP_TAGS: &[&str] = &[
    // Executable or presentational, never text.
    "script", "style", "noscript", "template", "svg", "math", "canvas", "iframe", "object",
    "embed", "video", "audio", "source", "track", "map", "area",
    // Interaction, which the reader does not do.
    "form", "input", "button", "select", "option", "textarea", "label", "fieldset", "legend",
    "datalist", "output", "progress", "meter", "dialog", "menu",
    // Chrome around the content.
    "nav", "footer", "aside",
    // Document head: the title is read separately.
    "head", "title", "meta", "link", "base",
];

/// ARIA landmark roles that mean the same as the chrome tags above.
const DROP_ROLES: &[&str] = &["navigation", "banner", "contentinfo", "complementary", "search", "menu", "menubar"];

/// Block-level containers: a boundary between paragraphs, no block of their own.
const CONTAINERS: &[&str] = &[
    "div", "section", "article", "main", "body", "html", "header", "figure", "figcaption",
    "details", "summary", "center", "address", "hgroup", "search", "tbody", "thead", "tfoot",
    "tr", "td", "th", "caption", "colgroup", "col", "dl", "dd", "dt", "li",
];

/// Turn `html` into a readable page. `base_url` is the page's own URL (after
/// redirects); every relative link resolves against it.
pub fn parse_html(html: &str, base_url: &str, rules: &ReadRules) -> Page {
    let nodes = html5ever::parse_document(Dom::new(), Default::default()).one(html).into_nodes();

    let page_url = Url::parse(base_url).ok();
    // <base href> overrides the resolution base, itself resolved against the
    // page URL, as a browser does.
    let base = find_first(&nodes, 0, "base")
        .and_then(|b| nodes[b].attr("href"))
        .and_then(|href| page_url.as_ref().and_then(|u| u.join(href).ok()))
        .or(page_url.clone());

    let title = find_first(&nodes, 0, "title")
        .map(|t| collapse_ws(&raw_text(&nodes, t)).trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| page_url.as_ref().and_then(|u| u.host_str().map(str::to_string)))
        .unwrap_or_default();

    // Keep main/article when present, else read the whole body.
    let root = find_first(&nodes, 0, "main")
        .or_else(|| find_first(&nodes, 0, "article"))
        .or_else(|| find_first(&nodes, 0, "body"))
        .unwrap_or(0);

    let mut conv = Conv {
        nodes: &nodes,
        base,
        rules,
        out: Vec::new(),
        cur: Vec::new(),
        link: None,
        strong: 0,
        em: 0,
        code: 0,
        pre: 0,
        inline_only: 0,
        deferred: Vec::new(),
    };
    conv.visit(root);
    conv.flush_para();
    let mut blocks = conv.out;
    blocks.append(&mut conv.deferred);

    let notice = if blocks.is_empty() {
        Some(
            "This page has no readable content: it holds only navigation, menus, scripts or \
             forms, which the readable web leaves out. Open it in your system browser to see \
             the full page."
                .to_string(),
        )
    } else {
        None
    };

    Page { url: base_url.to_string(), title, blocks, notice }
}

/// Depth-first search for the first element named `tag`.
fn find_first(nodes: &[Node], from: NodeId, tag: &str) -> Option<NodeId> {
    if nodes[from].tag() == Some(tag) {
        return Some(from);
    }
    for &c in &nodes[from].children {
        if let Some(found) = find_first(nodes, c, tag) {
            return Some(found);
        }
    }
    None
}

/// Every descendant text node concatenated, nothing dropped, whitespace as
/// written. Used for `<title>` and `<pre>`.
fn raw_text(nodes: &[Node], from: NodeId) -> String {
    let mut out = String::new();
    fn walk(nodes: &[Node], id: NodeId, out: &mut String) {
        match &nodes[id].data {
            NodeData::Text(t) => out.push_str(t),
            NodeData::Element { .. } => {
                for &c in &nodes[id].children {
                    walk(nodes, c, out);
                }
            }
            _ => {}
        }
    }
    walk(nodes, from, &mut out);
    out
}

/// HTML whitespace collapsing: any run of spaces, tabs and newlines becomes
/// one space. Leading and trailing runs stay as one space so word boundaries
/// across inline elements survive (`<b>a</b> <i>b</i>`); paragraph edges are
/// trimmed at flush.
pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(c);
            in_ws = false;
        }
    }
    out
}

/// True when every inline is empty or whitespace.
fn inlines_blank(inl: &[Inline]) -> bool {
    inl.iter().all(|i| i.text().trim().is_empty())
}

/// Trim whitespace at both ends of an inline run and drop pieces that became
/// empty. Interior spacing is untouched.
fn trim_inlines(mut inl: Vec<Inline>) -> Vec<Inline> {
    fn set_text(i: &mut Inline, s: String) {
        match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Em(t) | Inline::Code(t) => *t = s,
            Inline::Link { text, .. } => *text = s,
        }
    }
    while let Some(first) = inl.first_mut() {
        let t = first.text().trim_start().to_string();
        if t.is_empty() {
            inl.remove(0);
        } else {
            set_text(first, t);
            break;
        }
    }
    while let Some(last) = inl.last_mut() {
        let t = last.text().trim_end().to_string();
        if t.is_empty() {
            inl.pop();
        } else {
            set_text(last, t);
            break;
        }
    }
    inl
}

struct Conv<'a> {
    nodes: &'a [Node],
    base: Option<Url>,
    rules: &'a ReadRules,
    /// Finished blocks, in order.
    out: Vec<Block>,
    /// The paragraph under construction.
    cur: Vec<Inline>,
    /// Style context for text nodes.
    link: Option<String>,
    strong: u32,
    em: u32,
    code: u32,
    /// Inside `<pre>`: whitespace is kept.
    pre: u32,
    /// Inside a heading, list item or table cell: block elements act as
    /// line breaks instead of flushing, so the item stays one run.
    inline_only: u32,
    /// Blocks that could not be placed inline (an image inside a list item
    /// or cell); emitted after the enclosing block.
    deferred: Vec<Block>,
}

impl<'a> Conv<'a> {
    fn node(&self, id: NodeId) -> &'a Node {
        &self.nodes[id]
    }

    /// Should this element and everything under it be skipped?
    fn dropped(&self, n: &Node) -> bool {
        let Some(tag) = n.tag() else { return false };
        if DROP_TAGS.contains(&tag) {
            return true;
        }
        if n.attr("hidden").is_some() || n.attr("aria-hidden") == Some("true") {
            return true;
        }
        if let Some(role) = n.attr("role") {
            if DROP_ROLES.contains(&role.trim()) {
                return true;
            }
        }
        if let Some(style) = n.attr("style") {
            let compact: String = style.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("display:none") || compact.contains("visibility:hidden") {
                return true;
            }
        }
        if let Some(classes) = n.attr("class") {
            if classes.split_whitespace().any(|c| self.rules.drop_classes.iter().any(|d| d == c)) {
                return true;
            }
        }
        if let Some(id) = n.attr("id") {
            if self.rules.drop_ids.iter().any(|d| d == id) {
                return true;
            }
        }
        false
    }

    /// Resolve `raw` against the page base; only http(s) results count.
    fn resolve(&self, raw: &str) -> Option<String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        let joined = match &self.base {
            Some(b) => b.join(raw).ok()?,
            None => Url::parse(raw).ok()?,
        };
        match joined.scheme() {
            "http" | "https" => Some(joined.to_string()),
            _ => None,
        }
    }

    /// Emit the paragraph under construction, if it has visible text.
    fn flush_para(&mut self) {
        if self.cur.is_empty() {
            return;
        }
        let inl = trim_inlines(std::mem::take(&mut self.cur));
        if !inlines_blank(&inl) {
            self.out.push(Block::Paragraph(inl));
        }
    }

    /// Add a text node to the paragraph in the current style.
    fn push_text(&mut self, raw: &str) {
        let text = if self.pre > 0 { raw.to_string() } else { collapse_ws(raw) };
        if text.is_empty() {
            return;
        }
        // No double spaces across inline boundaries, no leading space at
        // the start of a paragraph.
        let prev_ends_space = self.cur.last().map_or(true, |l| l.text().ends_with(' ') || l.text().ends_with('\n'));
        let text = if prev_ends_space && self.pre == 0 { text.trim_start().to_string() } else { text };
        if text.is_empty() {
            return;
        }
        self.push_inline(text);
    }

    /// Wrap `text` in the current style and merge with the previous inline
    /// when the style is the same.
    fn push_inline(&mut self, text: String) {
        let piece = if let Some(href) = &self.link {
            Inline::Link { href: href.clone(), text }
        } else if self.code > 0 {
            Inline::Code(text)
        } else if self.strong > 0 {
            Inline::Strong(text)
        } else if self.em > 0 {
            Inline::Em(text)
        } else {
            Inline::Text(text)
        };
        if let Some(last) = self.cur.last_mut() {
            let merged = match (last, &piece) {
                (Inline::Text(a), Inline::Text(b))
                | (Inline::Strong(a), Inline::Strong(b))
                | (Inline::Em(a), Inline::Em(b))
                | (Inline::Code(a), Inline::Code(b)) => {
                    a.push_str(b);
                    true
                }
                (Inline::Link { href: h1, text: t1 }, Inline::Link { href: h2, text: t2 }) if h1 == h2 => {
                    t1.push_str(t2);
                    true
                }
                _ => false,
            };
            if merged {
                return;
            }
        }
        self.cur.push(piece);
    }

    /// A soft line break inside an inline-only run (a `<p>` inside a cell).
    fn soft_break(&mut self) {
        if let Some(last) = self.cur.last() {
            if !last.text().ends_with('\n') {
                self.push_inline("\n".to_string());
            }
        }
    }

    /// Visit the children of `id` as one inline run and return it, leaving
    /// the outer paragraph untouched.
    fn inline_run(&mut self, id: NodeId) -> Vec<Inline> {
        let saved = std::mem::take(&mut self.cur);
        self.inline_only += 1;
        for &c in &self.node(id).children {
            self.visit(c);
        }
        self.inline_only -= 1;
        let run = std::mem::replace(&mut self.cur, saved);
        trim_inlines(run)
    }

    fn visit_children(&mut self, id: NodeId) {
        for &c in &self.node(id).children {
            self.visit(c);
        }
    }

    /// Emit a block, or defer it when inside an inline-only run.
    fn emit(&mut self, block: Block) {
        if self.inline_only > 0 {
            self.deferred.push(block);
        } else {
            self.out.push(block);
        }
    }

    /// Place the blocks an inline-only run could not hold (an image inside a
    /// list item or a table cell) right after the block that held them, so
    /// an infobox picture sits by its table rather than at the page end.
    fn drain_deferred(&mut self) {
        if self.inline_only == 0 {
            self.out.append(&mut self.deferred);
        }
    }

    fn visit(&mut self, id: NodeId) {
        let n = self.node(id);
        match &n.data {
            NodeData::Text(t) => self.push_text(t),
            NodeData::Element { .. } => {
                if self.dropped(n) {
                    return;
                }
                let tag = n.tag().unwrap_or("");
                self.visit_element(id, tag);
            }
            _ => {}
        }
    }

    fn visit_element(&mut self, id: NodeId, tag: &str) {
        let n = self.node(id);
        match tag {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag.as_bytes()[1] - b'0';
                if self.inline_only > 0 {
                    self.soft_break();
                    self.visit_children(id);
                    self.soft_break();
                    return;
                }
                self.flush_para();
                let inlines = self.inline_run(id);
                if !inlines_blank(&inlines) {
                    self.out.push(Block::Heading { level, inlines });
                }
                self.drain_deferred();
            }
            "p" => {
                if self.inline_only > 0 {
                    self.soft_break();
                    self.visit_children(id);
                    self.soft_break();
                    return;
                }
                self.flush_para();
                self.visit_children(id);
                self.flush_para();
            }
            "br" => {
                self.push_inline("\n".to_string());
            }
            "hr" => {
                if self.inline_only > 0 {
                    self.soft_break();
                    return;
                }
                self.flush_para();
                self.out.push(Block::Rule);
            }
            "ul" | "ol" => {
                if self.inline_only > 0 {
                    // A list inside a cell or item: one line per item.
                    self.collect_list_inline(id);
                    return;
                }
                self.flush_para();
                let mut items = Vec::new();
                self.collect_list(id, 0, &mut items);
                if !items.is_empty() {
                    self.out.push(Block::List { ordered: tag == "ol", items });
                }
                self.drain_deferred();
            }
            "pre" => {
                if self.inline_only > 0 {
                    self.pre += 1;
                    self.soft_break();
                    self.visit_children(id);
                    self.pre -= 1;
                    self.soft_break();
                    return;
                }
                self.flush_para();
                let text = self.pre_text(id);
                if !text.trim().is_empty() {
                    self.out.push(Block::Code(text.trim_end().to_string()));
                }
            }
            "blockquote" => {
                if self.inline_only > 0 {
                    self.soft_break();
                    self.visit_children(id);
                    self.soft_break();
                    return;
                }
                self.flush_para();
                let saved = std::mem::take(&mut self.out);
                self.visit_children(id);
                self.flush_para();
                let inner = std::mem::replace(&mut self.out, saved);
                if !inner.is_empty() {
                    self.out.push(Block::Quote(inner));
                }
            }
            "table" => {
                if self.inline_only > 0 {
                    // A table inside a cell: read it as text, row by row.
                    self.visit_children(id);
                    return;
                }
                self.flush_para();
                let rows = self.collect_table(id);
                if !rows.is_empty() {
                    self.out.push(Block::Table { rows });
                }
                self.drain_deferred();
            }
            "img" => {
                // Lazy-loading sites put the real source in data-src.
                let src = n.attr("src").filter(|s| !s.trim().is_empty()).or_else(|| n.attr("data-src"));
                let alt = n.attr("alt").map(|a| collapse_ws(a).trim().to_string()).unwrap_or_default();
                // A 1x1 image is a tracking pixel or a spacer, never a picture.
                let tiny = n.attr("width").map_or(false, |w| w.trim() == "1")
                    || n.attr("height").map_or(false, |h| h.trim() == "1");
                match src.and_then(|s| self.resolve(s)) {
                    Some(src) if !tiny => {
                        if self.inline_only == 0 {
                            self.flush_para();
                        }
                        self.emit(Block::Image { src, alt });
                    }
                    _ => {
                        if !alt.is_empty() {
                            self.push_text(&alt);
                        }
                    }
                }
            }
            "a" => {
                let href = n.attr("href").and_then(|h| self.resolve(h));
                match href {
                    // A nested link is invalid HTML; the inner one wins the
                    // text, which is what browsers show too.
                    Some(h) if self.link.is_none() => {
                        self.link = Some(h);
                        self.visit_children(id);
                        self.link = None;
                    }
                    _ => self.visit_children(id),
                }
            }
            "strong" | "b" => {
                self.strong += 1;
                self.visit_children(id);
                self.strong -= 1;
            }
            "em" | "i" | "cite" | "dfn" | "var" => {
                self.em += 1;
                self.visit_children(id);
                self.em -= 1;
            }
            "code" | "kbd" | "samp" | "tt" => {
                self.code += 1;
                self.visit_children(id);
                self.code -= 1;
            }
            "dt" => {
                // A definition term reads as a bold line of its own.
                if self.inline_only > 0 {
                    self.soft_break();
                    self.strong += 1;
                    self.visit_children(id);
                    self.strong -= 1;
                    self.soft_break();
                    return;
                }
                self.flush_para();
                self.strong += 1;
                self.visit_children(id);
                self.strong -= 1;
                self.flush_para();
            }
            t if CONTAINERS.contains(&t) => {
                if self.inline_only > 0 {
                    self.soft_break();
                    self.visit_children(id);
                    self.soft_break();
                    return;
                }
                self.flush_para();
                self.visit_children(id);
                self.flush_para();
            }
            // Everything else (span, small, sup, sub, abbr, mark, u, s, time,
            // font, q, ins, del, wbr, unknown tags) is inline pass-through.
            _ => self.visit_children(id),
        }
    }

    /// The text of a `<pre>`, whitespace kept, dropped elements skipped.
    fn pre_text(&mut self, id: NodeId) -> String {
        let saved = std::mem::take(&mut self.cur);
        self.pre += 1;
        self.inline_only += 1;
        self.visit_children(id);
        self.inline_only -= 1;
        self.pre -= 1;
        let run = std::mem::replace(&mut self.cur, saved);
        run.iter().map(|i| i.text()).collect::<String>()
    }

    /// Gather `<li>` items of a `<ul>`/`<ol>` into `items`, nested lists at
    /// depth + 1 right after the item that holds them.
    fn collect_list(&mut self, id: NodeId, depth: u8, items: &mut Vec<ListItem>) {
        for &c in &self.node(id).children {
            let child = self.node(c);
            if self.dropped(child) {
                continue;
            }
            match child.tag() {
                Some("li") => {
                    // The item's own text first (nested lists held back),
                    // then the nested lists as deeper items.
                    let mut nested = Vec::new();
                    let saved = std::mem::take(&mut self.cur);
                    self.inline_only += 1;
                    for &g in &child.children {
                        match self.node(g).tag() {
                            Some("ul") | Some("ol") => nested.push(g),
                            _ => self.visit(g),
                        }
                    }
                    self.inline_only -= 1;
                    let inlines = trim_inlines(std::mem::replace(&mut self.cur, saved));
                    if !inlines_blank(&inlines) {
                        items.push(ListItem { depth, inlines });
                    }
                    for g in nested {
                        if !self.dropped(self.node(g)) {
                            self.collect_list(g, depth.saturating_add(1), items);
                        }
                    }
                }
                // A list directly inside a list (malformed but common).
                Some("ul") | Some("ol") => self.collect_list(c, depth.saturating_add(1), items),
                // Text or other elements loose inside a list: read as an item.
                Some(_) => {
                    let inlines = self.inline_run(c);
                    if !inlines_blank(&inlines) {
                        items.push(ListItem { depth, inlines });
                    }
                }
                None => {}
            }
        }
    }

    /// A list met inside an inline-only run: one line per item.
    fn collect_list_inline(&mut self, id: NodeId) {
        for &c in &self.node(id).children {
            let child = self.node(c);
            if self.dropped(child) {
                continue;
            }
            if child.tag().is_some() {
                self.soft_break();
                self.visit_children(c);
                self.soft_break();
            }
        }
    }

    /// Rows of a `<table>`: `thead`/`tbody`/`tfoot` are transparent, each
    /// `<tr>` is a row, each `<th>`/`<td>` a cell. Empty rows are skipped.
    fn collect_table(&mut self, id: NodeId) -> Vec<Vec<Cell>> {
        let mut rows = Vec::new();
        self.collect_rows(id, &mut rows);
        rows
    }

    fn collect_rows(&mut self, id: NodeId, rows: &mut Vec<Vec<Cell>>) {
        for &c in &self.node(id).children {
            let child = self.node(c);
            if self.dropped(child) {
                continue;
            }
            match child.tag() {
                Some("thead") | Some("tbody") | Some("tfoot") => self.collect_rows(c, rows),
                Some("tr") => {
                    let mut cells = Vec::new();
                    for &g in &child.children {
                        let cell = self.node(g);
                        if self.dropped(cell) {
                            continue;
                        }
                        match cell.tag() {
                            Some("th") | Some("td") => {
                                let inlines = self.inline_run(g);
                                cells.push(Cell { header: cell.tag() == Some("th"), inlines });
                            }
                            _ => {}
                        }
                    }
                    if cells.iter().any(|c| !inlines_blank(&c.inlines)) {
                        rows.push(cells);
                    }
                }
                // A caption reads as a paragraph before the table.
                Some("caption") => {
                    let inlines = self.inline_run(c);
                    if !inlines_blank(&inlines) {
                        self.out.push(Block::Paragraph(inlines));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://example.com/docs/page.html";

    fn rules() -> ReadRules {
        ReadRules { drop_classes: vec!["noprint".into()], drop_ids: vec!["site-menu".into()] }
    }

    fn page(fixture: &str) -> Page {
        parse_html(fixture, BASE, &rules())
    }

    /// All the text of the page, for "is X anywhere in it" checks.
    fn all_text(blocks: &[Block], out: &mut String) {
        for b in blocks {
            match b {
                Block::Heading { inlines, .. } | Block::Paragraph(inlines) => {
                    for i in inlines {
                        out.push_str(i.text());
                        out.push(' ');
                    }
                }
                Block::List { items, .. } => {
                    for it in items {
                        for i in &it.inlines {
                            out.push_str(i.text());
                            out.push(' ');
                        }
                    }
                }
                Block::Image { alt, .. } => out.push_str(alt),
                Block::Code(t) => out.push_str(t),
                Block::Table { rows } => {
                    for r in rows {
                        for c in r {
                            for i in &c.inlines {
                                out.push_str(i.text());
                                out.push(' ');
                            }
                        }
                    }
                }
                Block::Quote(inner) => all_text(inner, out),
                Block::Rule => {}
            }
        }
    }

    fn text_of(p: &Page) -> String {
        let mut s = String::new();
        all_text(&p.blocks, &mut s);
        s
    }

    fn links(p: &Page) -> Vec<(String, String)> {
        let mut out = Vec::new();
        fn walk(blocks: &[Block], out: &mut Vec<(String, String)>) {
            for b in blocks {
                let inl: Vec<&Inline> = match b {
                    Block::Heading { inlines, .. } | Block::Paragraph(inlines) => inlines.iter().collect(),
                    Block::List { items, .. } => items.iter().flat_map(|i| i.inlines.iter()).collect(),
                    Block::Table { rows } => rows.iter().flatten().flat_map(|c| c.inlines.iter()).collect(),
                    Block::Quote(inner) => {
                        walk(inner, out);
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                for i in inl {
                    if let Inline::Link { href, text } = i {
                        out.push((href.clone(), text.clone()));
                    }
                }
            }
        }
        walk(&p.blocks, &mut out);
        out
    }

    // ── Fixture: a small article with every block type ──
    const ARTICLE: &str = include_str!("../../tests/fixtures/web/article.html");

    #[test]
    fn headings_paragraphs_and_title_come_through_in_order() {
        let p = page(ARTICLE);
        assert_eq!(p.title, "Silverdale, a test article");
        assert!(p.notice.is_none());
        // First block is the h1, second the lead paragraph.
        match &p.blocks[0] {
            Block::Heading { level, inlines } => {
                assert_eq!(*level, 1);
                assert_eq!(inlines, &[Inline::Text("Silverdale".into())]);
            }
            other => panic!("first block should be the h1, got {other:?}"),
        }
        match &p.blocks[1] {
            Block::Paragraph(inl) => {
                let joined: String = inl.iter().map(|i| i.text()).collect();
                assert_eq!(joined, "Silverdale is a community in Kitsap County, on Dyes Inlet.");
                // Bold inside the paragraph is its own inline, and the
                // whitespace between source lines collapsed to one space.
                assert!(inl.contains(&Inline::Strong("Silverdale".into())));
            }
            other => panic!("second block should be the lead paragraph, got {other:?}"),
        }
        let h2s: Vec<u8> = p
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Heading { level, .. } => Some(*level),
                _ => None,
            })
            .collect();
        assert_eq!(h2s, vec![1, 2, 2, 3], "heading levels in document order");
    }

    #[test]
    fn links_resolve_relative_to_the_page_url() {
        let p = page(ARTICLE);
        let l = links(&p);
        // Root-relative, sibling-relative, protocol-relative, absolute.
        assert!(l.contains(&("https://example.com/wiki/Kitsap_County".into(), "Kitsap County".into())), "{l:?}");
        assert!(l.contains(&("https://example.com/docs/nearby.html".into(), "nearby towns".into())), "{l:?}");
        assert!(l.contains(&("https://cdn.example.org/map".into(), "a map".into())), "{l:?}");
        assert!(l.contains(&("https://united-humanity.us/".into(), "HumanityOS".into())), "{l:?}");
        // A javascript: link is text, not a link. Nothing in the page links
        // to anything but http(s).
        assert!(l.iter().all(|(h, _)| h.starts_with("http")), "{l:?}");
        assert!(!l.iter().any(|(_, t)| t == "run script"), "javascript: href must not become a link: {l:?}");
        assert!(text_of(&p).contains("run script"), "its text still reads");
    }

    #[test]
    fn readability_drops_script_style_nav_footer_aside_and_forms() {
        let p = page(ARTICLE);
        let t = text_of(&p);
        for gone in [
            "SCRIPT TEXT",
            "STYLE TEXT",
            "NOSCRIPT TEXT",
            "NAV TEXT",
            "FOOTER TEXT",
            "ASIDE TEXT",
            "FORM TEXT",
            "HIDDEN TEXT",
            "NOPRINT TEXT",
            "MENU TEXT",
            "TEMPLATE TEXT",
        ] {
            assert!(!t.contains(gone), "{gone} should have been dropped; page text: {t}");
        }
        assert!(t.contains("Dyes Inlet"), "real content stays");
    }

    #[test]
    fn lists_images_code_quotes_rules_and_tables_are_typed_blocks() {
        let p = page(ARTICLE);
        let list = p.blocks.iter().find_map(|b| match b {
            Block::List { ordered, items } => Some((*ordered, items.clone())),
            _ => None,
        });
        let (ordered, items) = list.expect("a list block");
        assert!(!ordered);
        let texts: Vec<String> = items.iter().map(|i| i.inlines.iter().map(|x| x.text()).collect()).collect();
        assert_eq!(texts, vec!["Clear Creek Trail", "the mall", "the waterfront"]);
        assert_eq!(items[2].depth, 1, "the nested list item is one level deeper");

        let img = p.blocks.iter().find_map(|b| match b {
            Block::Image { src, alt } => Some((src.clone(), alt.clone())),
            _ => None,
        });
        assert_eq!(img, Some(("https://example.com/img/inlet.jpg".into(), "Dyes Inlet at dusk".into())));
        // The tracking pixel next to it did not become an image.
        let n_img = p.blocks.iter().filter(|b| matches!(b, Block::Image { .. })).count();
        assert_eq!(n_img, 1);

        let code = p.blocks.iter().find_map(|b| match b {
            Block::Code(t) => Some(t.clone()),
            _ => None,
        });
        assert_eq!(code.as_deref(), Some("fn main() {\n    println!(\"hi\");\n}"), "pre keeps its whitespace");

        assert!(p.blocks.iter().any(|b| matches!(b, Block::Rule)));

        let quote = p.blocks.iter().find_map(|b| match b {
            Block::Quote(inner) => Some(inner.clone()),
            _ => None,
        });
        let quote = quote.expect("a quote block");
        assert!(matches!(&quote[0], Block::Paragraph(inl) if inl[0].text().starts_with("A quiet place")));

        let table = p.blocks.iter().find_map(|b| match b {
            Block::Table { rows } => Some(rows.clone()),
            _ => None,
        });
        let rows = table.expect("a table block");
        assert_eq!(rows.len(), 3, "header row + two body rows");
        assert!(rows[0][0].header && rows[0][1].header);
        assert_eq!(rows[0][0].inlines[0].text(), "Year");
        assert!(!rows[1][0].header);
        assert_eq!(rows[1][1].inlines[0].text(), "19,204");
        // A link inside a cell is still a link.
        assert!(matches!(&rows[2][0].inlines[0], Inline::Link { href, .. } if href == "https://example.com/wiki/2020"));
    }

    #[test]
    fn main_wins_over_the_rest_of_the_body() {
        let html = r#"<html><head><title>T</title></head><body>
            <div>OUTSIDE MAIN</div>
            <main><p>inside main</p></main>
            <div>ALSO OUTSIDE</div></body></html>"#;
        let p = parse_html(html, BASE, &ReadRules::default());
        let t = text_of(&p);
        assert!(t.contains("inside main"));
        assert!(!t.contains("OUTSIDE MAIN") && !t.contains("ALSO OUTSIDE"), "{t}");
    }

    #[test]
    fn a_page_of_only_nav_and_footer_is_empty_with_a_clear_notice() {
        const CHROME: &str = include_str!("../../tests/fixtures/web/chrome_only.html");
        let p = page(CHROME);
        assert!(p.blocks.is_empty(), "nothing readable should remain: {:?}", p.blocks);
        let notice = p.notice.expect("an explanation for the empty page");
        assert!(notice.contains("no readable content"), "{notice}");
        assert!(notice.contains("system browser"), "tells the reader what to do next: {notice}");
    }

    #[test]
    fn base_href_changes_where_relative_links_go() {
        let html = r#"<html><head><base href="https://other.example/dir/"></head>
            <body><p><a href="x.html">x</a></p></body></html>"#;
        let p = parse_html(html, BASE, &ReadRules::default());
        assert_eq!(links(&p), vec![("https://other.example/dir/x.html".to_string(), "x".to_string())]);
    }

    #[test]
    fn whitespace_collapses_like_a_browser_and_br_breaks_lines() {
        let html = "<body><p>  one\n   two <b> three </b>  four<br>five  </p></body>";
        let p = parse_html(html, BASE, &ReadRules::default());
        let Block::Paragraph(inl) = &p.blocks[0] else { panic!("paragraph") };
        let joined: String = inl.iter().map(|i| i.text()).collect();
        assert_eq!(joined, "one two three four\nfive");
    }

    /// An image inside a table cell (Wikipedia's infobox picture) is drawn
    /// right after that table, not at the end of the page.
    #[test]
    fn an_image_inside_a_cell_follows_its_table() {
        let html = r#"<body>
            <table><tr><td><img src="/pic.png" alt="pic"></td><td>caption</td></tr></table>
            <p>after the table</p>
            <ul><li><img src="/li.png" alt="in list"> item</li></ul>
            <p>after the list</p></body>"#;
        let p = parse_html(html, BASE, &ReadRules::default());
        let kinds: Vec<&str> = p
            .blocks
            .iter()
            .map(|b| match b {
                Block::Table { .. } => "table",
                Block::Image { alt, .. } => alt.as_str(),
                Block::Paragraph(_) => "p",
                Block::List { .. } => "list",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, vec!["table", "pic", "p", "list", "in list", "p"]);
    }

    #[test]
    fn plain_text_with_no_body_markup_still_reads() {
        let p = parse_html("just words", BASE, &ReadRules::default());
        assert_eq!(p.blocks, vec![Block::Paragraph(vec![Inline::Text("just words".into())])]);
        assert_eq!(p.title, "example.com", "host stands in for a missing title");
    }
}
