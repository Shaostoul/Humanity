//! Every `create_bind_group` gives its layout exactly the bindings the layout declares.
//!
//! THE FAILURE (v0.1029-v0.1038, 2026-07-28): binding 15 (the FFT ocean tile) was added to
//! the texture bind group layout in `renderer/pipeline.rs`. Two of the three
//! `create_bind_group` sites built from that layout were updated; the third, the
//! per-material albedo group in `renderer/material_bind_groups.rs`, is built LAZILY when a
//! textured material loads. wgpu rejects a bind group whose entries do not match its layout,
//! so every world entry panicked, while every menu-only boot check stayed green for ten
//! releases. The rule written afterwards ("grep the layout name for all create_bind_group
//! sites and count entries at each") only worked for a session that remembered it.
//!
//! WHY A SOURCE SCAN, NOT A HEADLESS GPU TEST. wgpu validates bind groups at run time, so a
//! test that builds every bind group against a real device would catch this too, and more
//! (a texture's view dimension, a buffer's minimum size). It was not chosen because:
//!   1. The failure was a site NOBODY REMEMBERED. A runtime test builds only the sites
//!      someone wrote into it; the next lazy site would be missing from the test exactly as
//!      it was missing from the edit. This scan finds every `create_bind_group(` in src/ by
//!      itself, lazy, test-only or not, including ones written after it.
//!   2. It runs everywhere: no GPU (the snapshot tests skip without an adapter, and CI has
//!      none), about two seconds with its own compile (0.7 s compile, 1.3 s over all 28 MB
//!      of src/, measured 2026-10-19; opt-level 1 runs faster but compiles slower, a net
//!      loss), std only, compiled standalone with rustc so it never links the native bin
//!      (the LNK1318 limit). So it sits in `just lints`, the pre-commit hook and CI instead
//!      of on the slow lib-test path.
//!   3. The lazy albedo group needs a whole renderer and a textured material load to exist
//!      at all; building it headlessly is a large harness that would itself need upkeep.
//! What it does NOT see: a resource's finer type (view dimension, sample type, minimum
//! binding size). wgpu checks those when the group is built; the probe rig's world entry
//! (`node scripts/probe-sweep.js --only blue-marble-12000km`) is the check for them.
//!
//! WHAT IT CHECKS. For each `create_bind_group(&wgpu::BindGroupDescriptor { layout, entries })`
//! in src/, it finds the `create_bind_group_layout` that made `layout` and compares the
//! binding NUMBERS on both sides (a missing one, an extra one, a duplicate), and the resource
//! KIND where both sides say it (a buffer, a texture view, a sampler). The layout is found
//! through: a definition in the same function or file (`let x = device.create_bind_group_layout`,
//! `self.x`), a struct field of another file (`pipeline.texture_bind_group_layout`), a function
//! that returns a layout (`Self::create_emission_bind_group_layout(device)`), a tuple a
//! function returns (`let (a, b, frame_bgl) = particles::build_particle_pipelines(..)`), and a
//! function PARAMETER, followed to every caller (`camera_bind_group(device, layout, ..)`,
//! `PatchArena::new(device, layout)`). A layout derived from a shader
//! (`pipeline.get_bind_group_layout(0)`) is wgpu's own and is counted, not compared.
//!
//! FAIL-CLOSED. A site whose layout it cannot find, or whose entries it cannot count (a list
//! built in a loop, an entry helper whose binding is not a number literal), FAILS and says
//! so. It never passes something it did not read. The way out is to write the site so it can
//! be read, or, with a stated reason, a row in BY_HAND below (empty on 2026-10-19).
//!
//! PROVEN RED, 2026-10-19: the `binding: 15` entry removed from the lazy albedo group in
//! src/renderer/material_bind_groups.rs (the v0.1029 shape). The run printed:
//!
//!   bind_group_entry_lint: 30 create_bind_group sites, 24 layouts; 32 comparisons (a site
//!     built from a parameter is compared once per caller, 126 bindings kind-checked), 1 shader-derived
//!   [FAIL] 1 create_bind_group site(s) do not match the layout they are built from.
//!     src/renderer/material_bind_groups.rs:143  layout `texture_bind_group_layout`
//!         (src/renderer/pipeline.rs:474, 16 bindings): missing binding 15
//!
//! And through the hardest path, binding 7 removed from `pipeline::camera_bind_group`, whose
//! layout is a parameter (found by following its four callers):
//!     src/renderer/pipeline.rs:3512  layout `camera_bind_group_layout`
//!         (src/renderer/pipeline.rs:298, 8 bindings): missing binding 7
//!
//! Restored, both pass (127 bindings kind-checked). While it was being written it also
//! refused, rather than passed, 13 sites it could not yet resolve: fail-closed working.
//! The in-file fixtures (`fixture_*`) repeat the v0.1029 shape, a wrong kind, an extra
//! binding, parameter-passed, tuple-returned and fn-made layouts, and unreadable sites, on
//! every run. `BGL_DEBUG=1` prints every layout and site exactly as the lint read them.
//!
//! Run: `just lints` (or standalone:
//!   CARGO_MANIFEST_DIR="$(pwd)" rustc --test --edition 2021 -A warnings \
//!       tests/bind_group_entry_lint.rs -o /tmp/t.exe && /tmp/t.exe)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Sites that cannot be read and are compared by hand: (file, line text that starts the
/// site's `create_bind_group(` statement, reason). A row that matches no site fails, so the
/// list cannot go stale. Empty on 2026-10-19: every site in src/ reads.
const BY_HAND: &[(&str, &str, &str)] = &[];

// ---------------------------------------------------------------------------------------
// Source text with comments, string contents and char literals blanked, so brackets and
// words in them never count. Offsets and newlines are kept, so positions match the original.
// ---------------------------------------------------------------------------------------

struct Src {
    path: String,
    text: String,
    /// Made on first use: src/ is 28 MB and an unoptimised test binary blanks it slowly, so
    /// only the files a question actually reaches (found with std's fast `contains`) are.
    blank_cell: std::cell::OnceCell<Vec<u8>>,
    fn_cell: std::cell::OnceCell<Vec<FnSpan>>,
    impl_cell: std::cell::OnceCell<Vec<(usize, usize, usize, String)>>,
    bg_cell: std::cell::OnceCell<bool>,
}

impl Src {
    fn blank(&self) -> &[u8] {
        self.blank_cell.get_or_init(|| blank_rust(&self.text))
    }

    /// Whether the file mentions create_bind_group at all (layouts and sites), asked once.
    fn builds_bind_groups(&self) -> bool {
        *self.bg_cell.get_or_init(|| self.text.contains("create_bind_group"))
    }
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn blank_rust(src: &str) -> Vec<u8> {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    fn wipe(out: &mut [u8], from: usize, to: usize) {
        for k in from..to.min(out.len()) {
            if out[k] != b'\n' {
                out[k] = b' ';
            }
        }
    }
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Most bytes start nothing; skip them first (this loop runs unoptimised).
        if !matches!(c, b'/' | b'"' | b'\'' | b'r' | b'b') {
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            let s = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            wipe(&mut out, s, i);
            continue;
        }
        if c == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let s = i;
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            wipe(&mut out, s, i);
            continue;
        }
        let boundary = i == 0 || !is_ident(b[i - 1]);
        let raw_at = if c == b'r' && boundary {
            Some(i + 1)
        } else if c == b'b' && boundary && i + 1 < b.len() && b[i + 1] == b'r' {
            Some(i + 2)
        } else {
            None
        };
        if let Some(mut j) = raw_at {
            let mut hashes = 0;
            while j < b.len() && b[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == b'"' {
                let start = j + 1;
                let mut k = start;
                'raw: while k < b.len() {
                    if b[k] == b'"' {
                        let mut n = 0;
                        while n < hashes && k + 1 + n < b.len() && b[k + 1 + n] == b'#' {
                            n += 1;
                        }
                        if n == hashes {
                            break 'raw;
                        }
                    }
                    k += 1;
                }
                wipe(&mut out, start, k);
                i = k + 1 + hashes;
                continue;
            }
        }
        if c == b'"' {
            let start = i + 1;
            let mut k = start;
            while k < b.len() && b[k] != b'"' {
                if b[k] == b'\\' {
                    k += 1;
                }
                k += 1;
            }
            wipe(&mut out, start, k);
            i = k + 1;
            continue;
        }
        if c == b'\'' {
            if i + 1 < b.len() && b[i + 1] == b'\\' {
                let mut k = i + 2;
                while k < b.len() && b[k] != b'\'' {
                    k += 1;
                }
                wipe(&mut out, i + 1, k);
                i = k + 1;
                continue;
            }
            if i + 1 < b.len() {
                let n = b[i + 1];
                let len = if n < 0x80 { 1 } else if n >= 0xF0 { 4 } else if n >= 0xE0 { 3 } else { 2 };
                if i + 1 + len < b.len() && b[i + 1 + len] == b'\'' {
                    wipe(&mut out, i + 1, i + 1 + len);
                    i += 2 + len;
                    continue;
                }
            }
            i += 1; // a lifetime
            continue;
        }
        i += 1;
    }
    out
}

fn src_of(path: &str, text: String) -> Src {
    Src { path: path.to_string(), text, blank_cell: std::cell::OnceCell::new(), fn_cell: std::cell::OnceCell::new(), impl_cell: std::cell::OnceCell::new(), bg_cell: std::cell::OnceCell::new() }
}

// ---------------------------------------------------------------------------------------
// Small structural helpers over the blanked bytes.
// ---------------------------------------------------------------------------------------

fn close_of(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn open_of(b: &[u8], close: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = close + 1;
    while i > 0 {
        i -= 1;
        match b[i] {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Items between `from` and `to` (exclusive) split on top-level commas, trimmed, empties
/// dropped. `angles` also nests `<...>` (for fn parameter lists).
fn split_top(b: &[u8], from: usize, to: usize, angles: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = from;
    let mut i = from;
    while i < to {
        let c = b[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'<' if angles => depth += 1,
            b'>' if angles && i > 0 && b[i - 1] != b'-' && b[i - 1] != b'=' => depth -= 1,
            b',' if depth == 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push((start, to));
    out.into_iter()
        .map(|(s, e)| trim(b, s, e))
        .filter(|(s, e)| s < e)
        .collect()
}

fn trim(b: &[u8], mut s: usize, mut e: usize) -> (usize, usize) {
    while s < e && b[s].is_ascii_whitespace() {
        s += 1;
    }
    while e > s && b[e - 1].is_ascii_whitespace() {
        e -= 1;
    }
    (s, e)
}

fn find_all(b: &[u8], needle: &str) -> Vec<usize> {
    let n = needle.as_bytes();
    let mut out = Vec::new();
    if n.is_empty() || b.len() < n.len() {
        return out;
    }
    let mut i = 0;
    while i + n.len() <= b.len() {
        if &b[i..i + n.len()] == n {
            out.push(i);
            i += n.len();
        } else {
            i += 1;
        }
    }
    out
}

/// Every place `needle` occurs as CODE in a whole file (not in a comment or a string).
///
/// Speed matters because `just lints` compiles without optimisation: a byte loop, and even
/// `match_indices` or `from_utf8` (both inlined into this crate), ran at about 5 MB/s here,
/// measured 2026-10-19. So the search runs on the original text, already a valid `&str`,
/// for the needle's first CHARACTER, which reaches core's precompiled memchr; `starts_with`
/// is a memcmp; and a match counts only if the blanked copy holds the same bytes there.
fn find_in(src: &Src, needle: &str) -> Vec<usize> {
    let t = src.text.as_str();
    let b = src.blank();
    let first = match needle.chars().next() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(k) = t[from..].find(first) {
        let i = from + k;
        if t[i..].starts_with(needle) && &b[i..i + needle.len()] == needle.as_bytes() {
            out.push(i);
            from = i + needle.len();
        } else {
            from = i + first.len_utf8();
        }
    }
    out
}

/// `word` at `i` as a whole identifier.
fn word_at(b: &[u8], i: usize, word: &str) -> bool {
    let w = word.as_bytes();
    i + w.len() <= b.len()
        && &b[i..i + w.len()] == w
        && (i == 0 || !is_ident(b[i - 1]))
        && (i + w.len() == b.len() || !is_ident(b[i + w.len()]))
}

fn ident_at(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut e = i;
    while e < b.len() && is_ident(b[e]) {
        e += 1;
    }
    if e > i && !b[i].is_ascii_digit() {
        Some((i, e))
    } else {
        None
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn line_of(text: &str, off: usize) -> usize {
    // std's char search (precompiled, fast) rather than a byte loop in this unoptimised binary.
    text.get(..off.min(text.len())).map_or(0, |t| t.matches('\n').count()) + 1
}

fn s(b: &[u8], r: (usize, usize)) -> String {
    String::from_utf8_lossy(&b[r.0..r.1]).to_string()
}

/// Top-level `key: value` items of a struct literal `{...}`: key -> value range. A shorthand
/// field (`layout,`) maps the key to its own range.
fn struct_fields(b: &[u8], open: usize, close: usize) -> BTreeMap<String, (usize, usize)> {
    let mut out = BTreeMap::new();
    for (is, ie) in split_top(b, open + 1, close, false) {
        if let Some((ks, ke)) = ident_at(b, is) {
            let after = skip_ws(b, ke);
            if after < ie && b[after] == b':' && !(after + 1 < ie && b[after + 1] == b':') {
                out.insert(s(b, (ks, ke)), trim(b, after + 1, ie));
            } else if ke == ie {
                out.insert(s(b, (ks, ke)), (ks, ke));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Entries.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Buffer,
    Texture,
    Sampler,
}

#[derive(Clone, Debug)]
struct Entry {
    binding: u32,
    kind: Option<Kind>,
}

fn layout_kind(text: &str) -> Option<Kind> {
    let mut kinds = Vec::new();
    if text.contains("BindingType::Buffer") {
        kinds.push(Kind::Buffer);
    }
    if text.contains("BindingType::Texture") || text.contains("BindingType::StorageTexture") {
        kinds.push(Kind::Texture);
    }
    if text.contains("BindingType::Sampler") {
        kinds.push(Kind::Sampler);
    }
    if kinds.len() == 1 {
        Some(kinds[0])
    } else {
        None
    }
}

fn resource_kind(text: &str) -> Option<Kind> {
    if text.contains("as_entire_binding") || text.contains("BindingResource::Buffer(") {
        Some(Kind::Buffer)
    } else if text.contains("BindingResource::TextureView(") {
        Some(Kind::Texture)
    } else if text.contains("BindingResource::Sampler(") {
        Some(Kind::Sampler)
    } else {
        None
    }
}

/// The body text of a free helper fn `name` in this file (for entry helpers like `uniform(0, ..)`).
fn helper_body<'a>(src: &'a Src, name: &str) -> Option<&'a str> {
    let b = src.blank();
    for at in find_in(src, "fn ") {
        let i = skip_ws(b, at + 3);
        if word_at(b, i, name) {
            let open = (i..b.len()).find(|&k| b[k] == b'{')?;
            let close = close_of(b, open)?;
            return Some(&src.text[open..close]);
        }
    }
    None
}

/// The entries of a descriptor `{...}`, read from its `entries: &[...]`.
fn read_entries(src: &Src, open: usize, close: usize, layout_side: bool) -> Result<Vec<Entry>, String> {
    let b = src.blank();
    let fields = struct_fields(b, open, close);
    let (vs, ve) = *fields.get("entries").ok_or("no `entries:` field")?;
    let lb = skip_ws(b, if b[vs] == b'&' { vs + 1 } else { vs });
    if lb >= ve || b[lb] != b'[' {
        return Err(format!("`entries: {}` is not a literal list", s(&src.text.as_bytes(), (vs, ve))));
    }
    let rb = close_of(b, lb).ok_or("unclosed entries list")?;
    let mut out = Vec::new();
    for (is, ie) in split_top(b, lb + 1, rb, false) {
        let item = &src.text[is..ie];
        let blank_item = &b[is..ie];
        // `binding: N` inside a struct literal.
        let mut binding = None;
        for at in find_all(blank_item, "binding") {
            if !word_at(blank_item, at, "binding") {
                continue;
            }
            let c = skip_ws(blank_item, at + 7);
            if c < blank_item.len() && blank_item[c] == b':' && !(c + 1 < blank_item.len() && blank_item[c + 1] == b':') {
                let d = skip_ws(blank_item, c + 1);
                let mut e = d;
                while e < blank_item.len() && blank_item[e].is_ascii_digit() {
                    e += 1;
                }
                binding = Some(if e > d && (e == blank_item.len() || !is_ident(blank_item[e])) {
                    item[d..e].parse::<u32>().map_err(|_| "bad number".to_string())
                } else {
                    Err(format!("`binding:` is not a number literal in `{}`", one_line(item)))
                });
                break;
            }
        }
        let (binding, helper) = match binding {
            Some(r) => (r?, None),
            None => {
                // An entry helper: `uniform(0, ..)`, `entry(3, ..)`, `texture(0)`.
                let (ns, ne) = ident_at(blank_item, 0).ok_or_else(|| format!("an entry the lint cannot read: `{}`", one_line(item)))?;
                let p = skip_ws(blank_item, ne);
                if p >= blank_item.len() || blank_item[p] != b'(' {
                    return Err(format!("an entry the lint cannot read: `{}`", one_line(item)));
                }
                let pc = close_of(blank_item, p).ok_or("unclosed call")?;
                let args = split_top(blank_item, p + 1, pc, false);
                let first = args.first().map(|r| &item[r.0..r.1]).unwrap_or("");
                let n = first.parse::<u32>().map_err(|_| {
                    format!("the entry helper `{}` does not take its binding as a number literal first: `{}`", &item[ns..ne], one_line(item))
                })?;
                (n, Some(item[ns..ne].to_string()))
            }
        };
        let kind = if layout_side {
            layout_kind(item).or_else(|| helper.and_then(|h| helper_body(src, &h)).and_then(layout_kind))
        } else {
            resource_kind(item)
        };
        out.push(Entry { binding, kind });
    }
    Ok(out)
}

fn one_line(t: &str) -> String {
    let j: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if j.len() > 90 {
        format!("{}...", &j[..90])
    } else {
        j
    }
}

// ---------------------------------------------------------------------------------------
// Layouts and sites.
// ---------------------------------------------------------------------------------------

struct Layout {
    file: usize,
    off: usize,
    /// The `let` name, the struct field it initialises, or `fn:<name>` for a function that
    /// returns it.
    name: String,
    entries: Result<Vec<Entry>, String>,
}

struct Site {
    file: usize,
    off: usize,
    layout_expr: String,
    layout_off: usize,
    entries: Result<Vec<Entry>, String>,
}

struct FnInfo {
    name: String,
    open: usize,
    close: usize,
    params: Vec<(String, String)>,
    has_self: bool,
    impl_type: Option<String>,
}

/// One `fn` with a body: where its name, parameter list and body sit.
#[derive(Clone, Copy)]
struct FnSpan {
    at: usize,
    ns: usize,
    ne: usize,
    po: usize,
    pc: usize,
    bo: usize,
    bc: usize,
}

/// Every fn with a body in a file, found once (a large file is asked about many times).
fn fn_spans(src: &Src) -> &[FnSpan] {
    src.fn_cell.get_or_init(|| {
        let b = src.blank();
        let mut out = Vec::new();
        for at in find_in(src, "fn") {
            if !word_at(b, at, "fn") {
                continue;
            }
            let ni = skip_ws(b, at + 2);
            let Some((ns, ne)) = ident_at(b, ni) else { continue };
            let Some(po) = (ne..b.len()).find(|&k| b[k] == b'(' || b[k] == b'{' || b[k] == b';') else { continue };
            if b[po] != b'(' {
                continue;
            }
            let Some(pc) = close_of(b, po) else { continue };
            let Some(bo) = (pc..b.len()).find(|&k| b[k] == b'{' || b[k] == b';') else { continue };
            if b[bo] != b'{' {
                continue;
            }
            let Some(bc) = close_of(b, bo) else { continue };
            out.push(FnSpan { at, ns, ne, po, pc, bo, bc });
        }
        out
    })
}

/// The innermost fn whose body holds `off`.
fn enclosing_fn(src: &Src, off: usize) -> Option<FnInfo> {
    let b = src.blank();
    let sp = fn_spans(src)
        .iter()
        .filter(|sp| sp.at <= off && sp.bo < off && off < sp.bc)
        .max_by_key(|sp| sp.bo)
        .copied()?;
    let mut params = Vec::new();
    let mut has_self = false;
    for (k, (ps, pe)) in split_top(b, sp.po + 1, sp.pc, true).into_iter().enumerate() {
        let p = s(src.text.as_bytes(), (ps, pe));
        if k == 0 && p.trim_start_matches('&').trim_start_matches("mut ").trim().starts_with("self") {
            has_self = true;
            continue;
        }
        let (n, t) = p
            .split_once(':')
            .map(|(n, t)| (n.trim().trim_start_matches("mut ").trim().to_string(), t.trim().to_string()))
            .unwrap_or((p.clone(), String::new()));
        params.push((n, t));
    }
    Some(FnInfo { name: s(b, (sp.ns, sp.ne)), open: sp.bo, close: sp.bc, params, has_self, impl_type: enclosing_impl(src, sp.at) })
}


/// Every `impl` block in a file: (where `impl` starts, its body's braces, the type it is for).
/// Found once per file.
fn impl_spans(src: &Src) -> &[(usize, usize, usize, String)] {
    src.impl_cell.get_or_init(|| {
        let b = src.blank();
        let mut out = Vec::new();
        for at in find_in(src, "impl") {
            if !word_at(b, at, "impl") {
                continue;
            }
            let Some(bo) = (at..b.len()).find(|&k| b[k] == b'{' || b[k] == b';') else { continue };
            if b[bo] != b'{' {
                continue;
            }
            let Some(bc) = close_of(b, bo) else { continue };
            let header = s(src.text.as_bytes(), (at + 4, bo));
            let mut depth = 0;
            let flat: String = header
                .chars()
                .filter(|&c| {
                    if c == '<' {
                        depth += 1;
                        false
                    } else if c == '>' {
                        depth -= 1;
                        false
                    } else {
                        depth == 0
                    }
                })
                .collect();
            let ty_part = flat.rsplit(" for ").next().unwrap_or(&flat);
            let ty = ty_part.split_whitespace().last().unwrap_or("").rsplit("::").next().unwrap_or("").to_string();
            out.push((at, bo, bc, ty));
        }
        out
    })
}

/// The type an `impl` block around `off` is for.
fn enclosing_impl(src: &Src, off: usize) -> Option<String> {
    impl_spans(src)
        .iter()
        .filter(|(at, bo, bc, _)| *at <= off && *bo < off && off < *bc)
        .max_by_key(|(_, bo, _, _)| *bo)
        .map(|(_, _, _, ty)| ty.clone())
}


fn find_layouts(srcs: &[Src]) -> Vec<Layout> {
    let mut out = Vec::new();
    for (fi, src) in srcs.iter().enumerate() {
        if !src.builds_bind_groups() {
            continue;
        }
        let b = src.blank();
        for at in find_in(src, "create_bind_group_layout") {
            let p = skip_ws(b, at + "create_bind_group_layout".len());
            if p >= b.len() || b[p] != b'(' || (at >= 3 && word_at(b, at - 3, "fn")) {
                continue;
            }
            // What the result is bound to.
            let mut j = at;
            loop {
                if j == 0 {
                    break;
                }
                let c = b[j - 1];
                if is_ident(c) || c == b'.' || c.is_ascii_whitespace() {
                    j -= 1;
                } else if c == b':' && j >= 2 && b[j - 2] == b':' {
                    j -= 2;
                } else {
                    break;
                }
            }
            let name = if j > 0 && b[j - 1] == b'=' && !(j >= 2 && b"=!<>".contains(&b[j - 2])) {
                let stmt_start = (0..j).rev().find(|&k| b[k] == b';' || b[k] == b'{' || b[k] == b'}').map_or(0, |k| k + 1);
                // From the BLANKED text: a comment before the `let` must not hide it.
                let stmt = s(b, (stmt_start, j - 1));
                let rest = stmt.trim_start().strip_prefix("let ").map(|r| r.trim_start().trim_start_matches("mut ").trim_start());
                rest.and_then(|r| r.split(|c: char| !(c.is_alphanumeric() || c == '_')).next()).filter(|n| !n.is_empty()).map(String::from)
            } else if j > 0 && b[j - 1] == b':' {
                let mut k = j - 1;
                while k > 0 && b[k - 1].is_ascii_whitespace() {
                    k -= 1;
                }
                let e = k;
                while k > 0 && is_ident(b[k - 1]) {
                    k -= 1;
                }
                Some(s(b, (k, e)))
            } else if j > 0 && b[j - 1] == b'{' {
                enclosing_fn(src, j).filter(|f| f.open == j - 1).map(|f| format!("fn:{}", f.name))
            } else {
                None
            };
            let entries = match (b[p + 1..].iter().position(|&c| c == b'{').map(|k| k + p + 1), close_of(b, p)) {
                (Some(o), Some(pc)) if o < pc => match close_of(b, o) {
                    Some(c) => read_entries(src, o, c, true),
                    None => Err("unclosed descriptor".into()),
                },
                _ => Err("the descriptor is not written inline".into()),
            };
            out.push(Layout { file: fi, off: at, name: name.unwrap_or_else(|| "?".into()), entries });
        }
    }
    out
}

fn find_sites(srcs: &[Src]) -> Vec<Site> {
    let mut out = Vec::new();
    for (fi, src) in srcs.iter().enumerate() {
        if !src.builds_bind_groups() {
            continue;
        }
        let b = src.blank();
        for at in find_in(src, "create_bind_group") {
            let p = at + "create_bind_group".len();
            if p >= b.len() || b[p] != b'(' || (at > 0 && is_ident(b[at - 1])) {
                continue;
            }
            if at >= 3 && word_at(b, at - 3, "fn") {
                continue; // a wrapper's own definition
            }
            let pc = close_of(b, p);
            let o = b[p + 1..].iter().position(|&c| c == b'{').map(|k| k + p + 1);
            let (layout_expr, layout_off, entries) = match (o, pc) {
                (Some(o), Some(pc)) if o < pc => {
                    let c = close_of(b, o).unwrap_or(pc);
                    let fields = struct_fields(b, o, c);
                    let (le, lo) = match fields.get("layout") {
                        Some(&(vs, ve)) => (s(src.text.as_bytes(), (vs, ve)), vs),
                        None => (String::new(), at),
                    };
                    (le, lo, read_entries(src, o, c, false))
                }
                _ => (String::new(), at, Err("the descriptor is not written inline".to_string())),
            };
            out.push(Site { file: fi, off: at, layout_expr, layout_off, entries });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Which layout a `layout:` expression names.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Res {
    Layout(usize),
    Auto,
    Unknown(String),
}

struct World<'a> {
    srcs: &'a [Src],
    layouts: Vec<Layout>,
    /// Field name -> files declaring `name: wgpu::BindGroupLayout` (owned: a struct field,
    /// not a `&` parameter). Built once, in one pass per file.
    fields: BTreeMap<String, Vec<usize>>,
    /// Needle -> files containing it (see `files_with`).
    seen: std::cell::RefCell<BTreeMap<String, Vec<usize>>>,
}

fn layout_fields(srcs: &[Src]) -> BTreeMap<String, Vec<usize>> {
    let mut out: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let ty = "wgpu::BindGroupLayout";
    for (fi, src) in srcs.iter().enumerate() {
        if !src.text.contains(ty) {
            continue;
        }
        let b = src.blank();
        for at in find_in(src, ty) {
            let end = at + ty.len();
            if end < b.len() && is_ident(b[end]) {
                continue; // BindGroupLayoutDescriptor, BindGroupLayoutEntry
            }
            let mut k = at;
            while k > 0 && b[k - 1].is_ascii_whitespace() {
                k -= 1;
            }
            if k == 0 || b[k - 1] != b':' || (k >= 2 && b[k - 2] == b':') {
                continue;
            }
            k -= 1;
            while k > 0 && b[k - 1].is_ascii_whitespace() {
                k -= 1;
            }
            let e = k;
            while k > 0 && is_ident(b[k - 1]) {
                k -= 1;
            }
            if k < e {
                let v = out.entry(s(b, (k, e))).or_default();
                if !v.contains(&fi) {
                    v.push(fi);
                }
            }
        }
    }
    out
}

impl<'a> World<'a> {
    /// The files whose text contains `needle`, worked out once per needle: each pass over
    /// the 28 MB of src/ costs about 0.2 s in this unoptimised binary.
    fn files_with(&self, needle: &str) -> Vec<usize> {
        if let Some(v) = self.seen.borrow().get(needle) {
            return v.clone();
        }
        let v: Vec<usize> = (0..self.srcs.len()).filter(|&i| self.srcs[i].text.contains(needle)).collect();
        self.seen.borrow_mut().insert(needle.to_string(), v.clone());
        v
    }

    fn fn_defs(&self, name: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for fi in self.files_with(&format!("fn {name}")) {
            let src = &self.srcs[fi];
            let b = src.blank();
            for at in find_in(src, "fn") {
                if word_at(b, at, "fn") {
                    let ni = skip_ws(b, at + 2);
                    if word_at(b, ni, name) {
                        out.push((fi, ni));
                    }
                }
            }
        }
        out
    }

    /// `let` statements in `file` between `from` and `to` that bind `name`, nearest last:
    /// (offset, Some(tuple index) or None, rhs fn name if the rhs is a call).
    fn lets_binding(&self, file: usize, from: usize, to: usize, name: &str) -> Vec<(usize, Option<usize>, Option<String>, usize, usize)> {
        let src = &self.srcs[file];
        let b = src.blank();
        let mut out = Vec::new();
        for at in find_in(src, "let") {
            if !word_at(b, at, "let") || at < from || at > to {
                continue;
            }
            let mut i = skip_ws(b, at + 3);
            if word_at(b, i, "mut") {
                i = skip_ws(b, i + 3);
            }
            let (idx, after) = if b[i] == b'(' {
                let Some(c) = close_of(b, i) else { continue };
                let names: Vec<String> = split_top(b, i + 1, c, false).into_iter().map(|r| s(b, r).trim_start_matches("mut ").trim().to_string()).collect();
                match names.iter().position(|n| n == name) {
                    Some(k) => (Some(k), c + 1),
                    None => continue,
                }
            } else {
                match ident_at(b, i) {
                    Some((ns, ne)) if s(b, (ns, ne)) == name => (None, ne),
                    _ => continue,
                }
            };
            let Some(eq) = (after..b.len()).find(|&k| b[k] == b'=' || b[k] == b';') else { continue };
            if b[eq] != b'=' {
                continue;
            }
            // The rhs: `path::F(` (optionally after `&`).
            let mut r = skip_ws(b, eq + 1);
            if r < b.len() && b[r] == b'&' {
                r = skip_ws(b, r + 1);
            }
            let mut last = None;
            let mut k = r;
            while let Some((ns, ne)) = ident_at(b, k) {
                last = Some((ns, ne));
                k = skip_ws(b, ne);
                if k + 1 < b.len() && b[k] == b':' && b[k + 1] == b':' {
                    k = skip_ws(b, k + 2);
                } else {
                    break;
                }
            }
            let call = match last {
                Some(r) if k < b.len() && b[k] == b'(' => Some(s(b, r)),
                _ => None,
            };
            // The rhs's end: the `;` that closes the statement.
            let mut depth = 0i32;
            let mut end = eq + 1;
            while end < b.len() {
                match b[end] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    b';' if depth == 0 => break,
                    _ => {}
                }
                if depth < 0 {
                    break;
                }
                end += 1;
            }
            out.push((at, idx, call, eq + 1, end));
        }
        out
    }

    fn resolve(&self, file: usize, off: usize, expr: &str, depth: usize) -> Vec<Res> {
        if depth > 4 {
            return vec![Res::Unknown(format!("`{expr}`: followed too many steps"))];
        }
        let e = expr.trim().trim_start_matches('&').trim();
        if e.contains("get_bind_group_layout(") {
            return vec![Res::Auto];
        }
        if e.is_empty() {
            return vec![Res::Unknown("no `layout:` field the lint can read".into())];
        }
        if e.contains('(') || e.contains('[') {
            return vec![Res::Unknown(format!("`{e}` is a call or index, not a name the lint can follow"))];
        }
        let name = e.rsplit('.').next().unwrap_or(e).trim().to_string();
        let dotted = e.contains('.');
        let src = &self.srcs[file];
        let func = enclosing_fn(src, off);

        if !dotted {
            if let Some(f) = &func {
                // A parameter: follow every caller.
                if let Some(pi) = f.params.iter().position(|(n, t)| n == &name && t.contains("BindGroupLayout")) {
                    return self.through_callers(file, f, pi, depth);
                }
                if let Some(r) = self.local(file, f.open, off, &name, depth) {
                    return vec![r];
                }
            }
            return vec![Res::Unknown(format!("`{e}` is not defined in its function, nor one of its parameters"))];
        }
        // A field: the same file's definition (a constructor), else a struct field elsewhere.
        if let Some(r) = self.local(file, 0, src.blank().len(), &name, depth) {
            return vec![r];
        }
        // The file that declares `name: wgpu::BindGroupLayout` as a struct field, and what its
        // own code binds to that name (a layout, a layout-making fn's result, a tuple element).
        let mut via_field = Vec::new();
        for fi in 0..self.srcs.len() {
            if fi != file && self.fields.get(&name).map_or(false, |fs| fs.contains(&fi)) {
                if let Some(r) = self.local(fi, 0, self.srcs[fi].blank().len(), &name, depth) {
                    via_field.push(r);
                }
            }
        }
        if via_field.len() == 1 {
            return via_field;
        }
        if via_field.len() > 1 {
            return vec![Res::Unknown(format!("`{name}` is a layout field in {} files", via_field.len()))];
        }
        let all: Vec<usize> = (0..self.layouts.len()).filter(|&k| self.layouts[k].name == name).collect();
        match all.len() {
            1 => vec![Res::Layout(all[0])],
            0 => vec![Res::Unknown(format!("no create_bind_group_layout is bound to a name `{name}`"))],
            n => vec![Res::Unknown(format!("`{name}` names {n} layouts in different files"))],
        }
    }

    /// A definition of `name` in `file` within `from..=off`: a layout bound directly, a
    /// layout-making fn's result, or a tuple element a function returns. Nearest wins.
    fn local(&self, file: usize, from: usize, off: usize, name: &str, depth: usize) -> Option<Res> {
        let direct = (0..self.layouts.len())
            .filter(|&k| self.layouts[k].file == file && self.layouts[k].name == name && self.layouts[k].off >= from && self.layouts[k].off <= off)
            .max_by_key(|&k| self.layouts[k].off)
            .map(|k| (self.layouts[k].off, Res::Layout(k)));
        let via_let = self.lets_binding(file, from, off, name).into_iter().filter_map(|(at, idx, call, rs, re)| {
            // `let layout = &self.pipeline.camera_bind_group_layout;`: a plain path, followed.
            if idx.is_none() && call.is_none() {
                let rhs = s(self.srcs[file].text.as_bytes(), trim(self.srcs[file].blank(), rs, re));
                let plain = rhs.trim_start_matches('&').trim();
                if !plain.is_empty() && plain.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.') && plain != name {
                    let r = self.resolve(file, rs, &rhs, depth + 1);
                    return Some((at, r.into_iter().next().unwrap_or(Res::Unknown("empty".into()))));
                }
                return None;
            }
            let f = call?;
            match idx {
                None => {
                    let made: Vec<usize> = (0..self.layouts.len()).filter(|&k| self.layouts[k].name == format!("fn:{f}")).collect();
                    match made.len() {
                        1 => Some((at, Res::Layout(made[0]))),
                        0 => None,
                        _ => Some((at, Res::Unknown(format!("{} functions named `{f}` make layouts", made.len())))),
                    }
                }
                Some(i) => {
                    let defs = self.fn_defs(&f);
                    if defs.len() != 1 {
                        return Some((at, Res::Unknown(format!("`{name}` comes from `{f}`, defined {} times", defs.len()))));
                    }
                    let (ffile, fat) = defs[0];
                    let fsrc = &self.srcs[ffile];
                    let fb = fsrc.blank();
                    let bo = (fat..fb.len()).find(|&k| fb[k] == b'{')?;
                    let bc = close_of(fb, bo)?;
                    let mut t = bc;
                    while t > bo && fb[t - 1].is_ascii_whitespace() {
                        t -= 1;
                    }
                    if fb[t - 1] != b')' {
                        return Some((at, Res::Unknown(format!("`{f}` does not end in a tuple"))));
                    }
                    let to = open_of(fb, t - 1)?;
                    let items = split_top(fb, to + 1, t - 1, false);
                    let (is, ie) = *items.get(i)?;
                    let r = self.resolve(ffile, is, &s(fsrc.text.as_bytes(), (is, ie)), depth + 1);
                    Some((at, r.into_iter().next().unwrap_or(Res::Unknown("empty".into()))))
                }
            }
        });
        direct.into_iter().chain(via_let).max_by_key(|(at, _)| *at).map(|(_, r)| r)
    }

    fn through_callers(&self, file: usize, f: &FnInfo, pi: usize, depth: usize) -> Vec<Res> {
        if f.has_self {
            return vec![Res::Unknown(format!("the layout is a parameter of the method `{}`, whose callers the lint does not follow", f.name))];
        }
        let mut out = Vec::new();
        // An associated fn is called as `Type::name` (or `Self::name` in its own file), so look
        // for that: a bare `new` would match nearly every file.
        let needle = match &f.impl_type {
            Some(ty) => format!("{ty}::{}", f.name),
            None => f.name.clone(),
        };
        let mut files = self.files_with(&needle);
        if !files.contains(&file) {
            files.push(file);
        }
        for ci in files {
            let src = &self.srcs[ci];
            let b = src.blank();
            for at in find_in(src, &f.name) {
                if !word_at(b, at, &f.name) {
                    continue;
                }
                let p = skip_ws(b, at + f.name.len());
                if p >= b.len() || b[p] != b'(' {
                    continue;
                }
                let before = (0..at).rev().find(|&k| !b[k].is_ascii_whitespace());
                if before.map_or(false, |k| b[k] == b'.') {
                    continue; // a method call of the same name
                }
                if at >= 3 && word_at(b, at - 3, "fn") || before.map_or(false, |k| k >= 1 && word_at(b, k - 1, "fn")) {
                    continue; // the definition
                }
                if let Some(ty) = &f.impl_type {
                    let qualified = before.map_or(false, |k| k >= 1 && b[k] == b':' && b[k - 1] == b':') && {
                        let k = before.unwrap() - 1;
                        let mut q = k;
                        while q > 0 && is_ident(b[q - 1]) {
                            q -= 1;
                        }
                        let owner = s(b, (q, k));
                        owner == *ty || (owner == "Self" && ci == file)
                    };
                    if !qualified {
                        continue;
                    }
                }
                let Some(pc) = close_of(b, p) else { continue };
                let args = split_top(b, p + 1, pc, false);
                let Some(&(as_, ae)) = args.get(pi) else {
                    out.push(Res::Unknown(format!("a call of `{}` at {}:{} has no argument {}", f.name, src.path, line_of(&src.text, at), pi + 1)));
                    continue;
                };
                out.extend(self.resolve(ci, as_, &s(src.text.as_bytes(), (as_, ae)), depth + 1));
            }
        }
        if out.is_empty() {
            out.push(Res::Unknown(format!("the layout is a parameter of `{}`, which nothing calls", f.name)));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------
// The check.
// ---------------------------------------------------------------------------------------

struct Report {
    sites: usize,
    compared: usize,
    auto: usize,
    layouts: usize,
    /// (layout name, its binding count, sites compared against it), for the anchor check.
    per_layout: BTreeMap<String, (usize, usize)>,
    /// Bindings whose kind (buffer, texture, sampler) both sides state, so was compared.
    kinds_compared: usize,
    mismatches: Vec<String>,
    unreadable: Vec<String>,
}

fn check(srcs: &[Src]) -> Report {
    let layouts = find_layouts(srcs);
    let sites = find_sites(srcs);
    let world = World { srcs, fields: layout_fields(srcs), layouts, seen: Default::default() };
    let mut r = Report { sites: sites.len(), compared: 0, auto: 0, layouts: world.layouts.len(), per_layout: BTreeMap::new(), kinds_compared: 0, mismatches: vec![], unreadable: vec![] };
    let mut by_hand_used = vec![false; BY_HAND.len()];

    for site in &sites {
        let src = &srcs[site.file];
        let line = line_of(&src.text, site.off);
        let line_text = src.text.lines().nth(line - 1).unwrap_or("").trim().to_string();
        if let Some(k) = BY_HAND.iter().position(|(f, l, _)| *f == src.path && line_text.starts_with(l)) {
            by_hand_used[k] = true;
            continue;
        }
        let at = format!("{}:{}", src.path, line);
        let site_entries = match &site.entries {
            Ok(e) => e,
            Err(why) => {
                r.unreadable.push(format!("  {at}  cannot count this bind group's entries: {why}"));
                continue;
            }
        };
        let results = world.resolve(site.file, site.layout_off, &site.layout_expr, 0);
        for res in results {
            match res {
                Res::Auto => r.auto += 1,
                Res::Unknown(why) => r.unreadable.push(format!("  {at}  cannot tell which layout this is: {why}")),
                Res::Layout(k) => {
                    let l = &world.layouts[k];
                    let lsrc = &srcs[l.file];
                    let lat = format!("{}:{}", lsrc.path, line_of(&lsrc.text, l.off));
                    let lname = l.name.trim_start_matches("fn:").to_string();
                    let lentries = match &l.entries {
                        Ok(e) => e,
                        Err(why) => {
                            r.unreadable.push(format!("  {at}  its layout `{lname}` ({lat}) cannot be counted: {why}"));
                            continue;
                        }
                    };
                    r.compared += 1;
                    let pl = r.per_layout.entry(format!("{lname} ({})", lsrc.path)).or_insert((lentries.len(), 0));
                    pl.1 += 1;
                    let want: BTreeMap<u32, Option<Kind>> = lentries.iter().map(|e| (e.binding, e.kind)).collect();
                    let mut have: BTreeMap<u32, Option<Kind>> = BTreeMap::new();
                    let mut problems = Vec::new();
                    for e in site_entries {
                        if have.insert(e.binding, e.kind).is_some() {
                            problems.push(format!("binding {} given twice", e.binding));
                        }
                    }
                    if want.len() != lentries.len() {
                        problems.push("the layout itself declares a binding twice".into());
                    }
                    let missing: Vec<String> = want.keys().filter(|b| !have.contains_key(b)).map(|b| b.to_string()).collect();
                    let extra: Vec<String> = have.keys().filter(|b| !want.contains_key(b)).map(|b| b.to_string()).collect();
                    if !missing.is_empty() {
                        problems.push(format!("missing binding {}", missing.join(", ")));
                    }
                    if !extra.is_empty() {
                        problems.push(format!("binding {} not in the layout", extra.join(", ")));
                    }
                    for (b, hk) in &have {
                        if let (Some(Some(wk)), Some(hk)) = (want.get(b), hk) {
                            r.kinds_compared += 1;
                            if wk != hk {
                                problems.push(format!("binding {b} is a {hk:?} here but a {wk:?} in the layout"));
                            }
                        }
                    }
                    if !problems.is_empty() {
                        r.mismatches.push(format!(
                            "  {at}  layout `{lname}`\n      ({lat}, {} bindings): {}",
                            lentries.len(),
                            problems.join("; ")
                        ));
                    }
                }
            }
        }
    }
    for (k, used) in by_hand_used.iter().enumerate() {
        if !used {
            r.unreadable.push(format!("  BY_HAND row {} ({}, `{}`) matches no create_bind_group site any more: delete it", k + 1, BY_HAND[k].0, BY_HAND[k].1));
        }
    }
    r.mismatches.sort();
    r.mismatches.dedup();
    r.unreadable.sort();
    r.unreadable.dedup();
    r
}

fn collect_rs(dir: &Path, root: &Path, out: &mut Vec<Src>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_rs(&p, root, out);
        } else if p.extension().map_or(false, |e| e == "rs") {
            // EVERY file, not only those that build bind groups: the caller of a function
            // that takes a layout, or the struct that declares a layout field, may live in
            // a file that never calls create_bind_group itself.
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            out.push(src_of(&rel, text));
        }
    }
}

fn failure_message(r: &Report) -> String {
    let mut m = String::new();
    if !r.mismatches.is_empty() {
        m.push_str(&format!(
            "\n\n[FAIL] {} create_bind_group site(s) do not match the layout they are built from.\n\n{}\n\n\
             Why this matters: wgpu refuses a bind group whose entries differ from its layout, and\n\
             the refusal is a panic at the moment the group is BUILT. A lazily built group (one made\n\
             when a material loads, a pass first runs, a window resizes) passes every boot check\n\
             and then crashes the game later: binding 15 on the texture layout, v0.1029-v0.1038,\n\
             panicked every world entry for ten releases while menu-only checks stayed green.\n\n\
             Fix: give the site exactly the layout's bindings (add the missing entry, or remove the\n\
             extra one), or change the layout if the site is right. Then boot into the world:\n\
             node scripts/probe-sweep.js --only blue-marble-12000km --exe target/release/HumanityOS.exe\n",
            r.mismatches.len(),
            r.mismatches.join("\n")
        ));
    }
    if !r.unreadable.is_empty() {
        m.push_str(&format!(
            "\n\n[FAIL] {} create_bind_group site(s) the lint cannot check, so it will not pass them.\n\n{}\n\n\
             Write the site so it can be read: `layout:` a name bound to a create_bind_group_layout\n\
             (a local, a struct field, a parameter of a free or associated function, a tuple a\n\
             function returns), and `entries: &[...]` written out with `binding: <number>` (or an\n\
             entry helper taking the number first). If it truly cannot be, add a row to BY_HAND in\n\
             tests/bind_group_entry_lint.rs with the reason, and check that site by hand.\n",
            r.unreadable.len(),
            r.unreadable.join("\n")
        ));
    }
    m
}

#[test]
fn every_create_bind_group_matches_its_layout() {
    let root = repo_root();
    let t0 = std::time::Instant::now();
    let mut srcs = Vec::new();
    collect_rs(&root.join("src"), &root, &mut srcs);
    if std::env::var("BGL_DEBUG").is_ok() {
        println!("TIME read {} files: {:?}", srcs.len(), t0.elapsed());
        let t = std::time::Instant::now();
        let mut bytes = 0;
        for src in &srcs {
            if src.text.contains("create_bind_group") {
                bytes += src.blank().len();
            }
        }
        println!("TIME blank {bytes} bytes: {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let mut hits = 0;
        for src in &srcs {
            if src.text.contains("create_bind_group") {
                hits += find_in(src, "create_bind_group").len();
            }
        }
        println!("TIME find_all x1 ({hits} hits): {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let n = find_layouts(&srcs).len();
        println!("TIME find_layouts ({n}): {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let n = find_sites(&srcs).len();
        println!("TIME find_sites ({n}): {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let _ = check(&srcs);
        println!("TIME check: {:?}", t.elapsed());
    }
    // BGL_DEBUG=1: every layout and site as read, for whoever extends the resolver.
    if std::env::var("BGL_DEBUG").is_ok() {
        for l in find_layouts(&srcs) {
            let n = l.entries.as_ref().map(|e| e.len().to_string()).unwrap_or_else(|e| e.clone());
            println!("LAYOUT {}:{} {} -> {}", srcs[l.file].path, line_of(&srcs[l.file].text, l.off), l.name, n);
        }
        for site in find_sites(&srcs) {
            let f = enclosing_fn(&srcs[site.file], site.layout_off);
            println!(
                "SITE {}:{} expr={:?} fn={:?} params={:?}",
                srcs[site.file].path,
                line_of(&srcs[site.file].text, site.off),
                site.layout_expr,
                f.as_ref().map(|f| &f.name),
                f.as_ref().map(|f| &f.params)
            );
        }
    }
    let r = check(&srcs);
    println!(
        "bind_group_entry_lint: {} create_bind_group sites, {} layouts; {} comparisons (a site built from a \
         parameter is compared once per caller, {} bindings kind-checked), {} shader-derived",
        r.sites, r.layouts, r.compared, r.kinds_compared, r.auto
    );
    for (name, (n, sites)) in &r.per_layout {
        println!("  {name}: {n} bindings, compared {sites} time(s)");
    }
    let mut msg = failure_message(&r);
    // A scanner that finds nothing passes everything. On 2026-10-19: 30 sites, 24 layouts,
    // 32 comparisons.
    if r.sites < 25 || r.layouts < 20 || r.compared < 25 {
        msg.push_str(&format!(
            "\n\n[FAIL] only {} create_bind_group sites, {} create_bind_group_layout calls and {} comparisons \
             found in src/ (30, 24 and 32 on 2026-10-19); the scanner is probably broken rather than the \
             code being fine.\n",
            r.sites, r.layouts, r.compared
        ));
    }
    // The incident's own layout must still be seen whole, with all three of its sites.
    let tex = r.per_layout.get("texture_bind_group_layout (src/renderer/pipeline.rs)").copied();
    if !matches!(tex, Some((n, sites)) if n >= 16 && sites >= 3) {
        msg.push_str(&format!(
            "\n\n[FAIL] the texture bind group layout (renderer/pipeline.rs) was expected with >= 16 bindings and \
             >= 3 sites compared against it (the v0.1029 incident's layout, its two eager sites and the lazy \
             albedo one); found {tex:?}. If it was renamed or split, point this anchor at its new name.\n"
        ));
    }
    assert!(msg.is_empty(), "{msg}");
}

// ---------------------------------------------------------------------------------------
// Fixtures: the past mistake and the ways a site reaches its layout, on every run.
// ---------------------------------------------------------------------------------------

fn tex_layout(n: u32) -> String {
    let entries: Vec<String> = (0..n)
        .map(|b| {
            let ty = if b == 1 { "wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)" } else { "wgpu::BindingType::Texture { multisampled: false }" };
            format!("wgpu::BindGroupLayoutEntry {{ binding: {b}, visibility: wgpu::ShaderStages::FRAGMENT, ty: {ty}, count: None }},")
        })
        .collect();
    entries.join("\n")
}

fn tex_group(skip: Option<u32>, n: u32) -> String {
    (0..n)
        .filter(|b| Some(*b) != skip)
        .map(|b| {
            let r = if b == 1 { "wgpu::BindingResource::Sampler(&s)" } else { "wgpu::BindingResource::TextureView(&v)" };
            format!("wgpu::BindGroupEntry {{ binding: {b}, resource: {r} }},")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The v0.1029 shape: a 16-binding layout on `Pipeline`, two eager sites, one lazy site.
fn v1029_fixture(lazy_skip: Option<u32>) -> Vec<Src> {
    let pipeline = format!(
        "pub struct Pipeline {{\n    pub texture_bind_group_layout: wgpu::BindGroupLayout,\n}}\n\
         impl Pipeline {{\n    pub fn new(device: &wgpu::Device) -> Self {{\n        let texture_bind_group_layout =\n            \
         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {{\n                label: Some(\"Albedo (a }} in a string)\"),\n                \
         entries: &[\n{}\n                ],\n            }});\n        Self {{ texture_bind_group_layout }}\n    }}\n}}\n",
        tex_layout(16)
    );
    let eager = format!(
        "fn init(device: &wgpu::Device, pipeline: &Pipeline) {{\n    let a = device.create_bind_group(&wgpu::BindGroupDescriptor {{\n        label: Some(\"fallback\"),\n        \
         layout: &pipeline.texture_bind_group_layout,\n        entries: &[\n{}\n        ],\n    }});\n    \
         // a comment naming create_bind_group(&x) must not count\n    let b = device.create_bind_group(&wgpu::BindGroupDescriptor {{\n        label: None,\n        \
         layout: &pipeline.texture_bind_group_layout,\n        entries: &[\n{}\n        ],\n    }});\n}}\n",
        tex_group(None, 16),
        tex_group(None, 16)
    );
    let lazy = format!(
        "impl Renderer {{\n    pub(super) fn build_albedo(&self, v: &wgpu::TextureView) -> wgpu::BindGroup {{\n        let build = |label: &str| {{\n            \
         self.device.create_bind_group(&wgpu::BindGroupDescriptor {{\n                label: Some(label),\n                \
         layout: &self.pipeline.texture_bind_group_layout,\n                entries: &[\n{}\n                ],\n            }})\n        }};\n        build(\"albedo\")\n    }}\n}}\n",
        tex_group(lazy_skip, 16)
    );
    vec![
        src_of("src/renderer/pipeline.rs", pipeline),
        src_of("src/renderer/mod.rs", eager),
        src_of("src/renderer/material_bind_groups.rs", lazy),
    ]
}

#[test]
fn fixture_v1029_lazy_site_missing_binding_15_fails() {
    let r = check(&v1029_fixture(Some(15)));
    assert_eq!(r.compared, 3, "all three sites must be compared: {:?}", r.unreadable);
    assert_eq!(r.mismatches.len(), 1, "{:?}", r.mismatches);
    assert!(r.mismatches[0].contains("material_bind_groups.rs"), "{:?}", r.mismatches);
    assert!(r.mismatches[0].contains("missing binding 15"), "{:?}", r.mismatches);
}

#[test]
fn fixture_v1029_all_sites_whole_passes() {
    let r = check(&v1029_fixture(None));
    assert_eq!((r.compared, r.layouts), (3, 1), "{:?} {:?}", r.mismatches, r.unreadable);
    assert!(r.mismatches.is_empty() && r.unreadable.is_empty(), "{:?} {:?}", r.mismatches, r.unreadable);
    assert_eq!(r.per_layout.get("texture_bind_group_layout (src/renderer/pipeline.rs)"), Some(&(16, 3)));
}

#[test]
fn fixture_wrong_kind_and_extra_binding_fail() {
    let mut f = v1029_fixture(None);
    let lazy = f[2].text.replace(
        "binding: 1, resource: wgpu::BindingResource::Sampler(&s)",
        "binding: 1, resource: wgpu::BindingResource::TextureView(&v)",
    );
    let lazy = lazy.replace("binding: 15, resource:", "binding: 16, resource:");
    f[2] = src_of("src/renderer/material_bind_groups.rs", lazy);
    let r = check(&f);
    let all = r.mismatches.join("\n");
    assert!(all.contains("missing binding 15"), "{all}");
    assert!(all.contains("binding 16 not in the layout"), "{all}");
    assert!(all.contains("binding 1 is a Texture here but a Sampler in the layout"), "{all}");
}

#[test]
fn fixture_parameter_tuple_and_layout_fn_are_followed() {
    let files = vec![
        src_of(
            "src/renderer/pipeline.rs",
            "pub struct Pipeline { pub camera_bind_group_layout: wgpu::BindGroupLayout, pub emission_bind_group_layout: wgpu::BindGroupLayout }\n\
             impl Pipeline {\n  pub fn new(device: &wgpu::Device) -> Self {\n    let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[\n      \
             wgpu::BindGroupLayoutEntry { binding: 0, ty: wgpu::BindingType::Buffer { }, count: None },\n      wgpu::BindGroupLayoutEntry { binding: 1, ty: wgpu::BindingType::Buffer { }, count: None },\n    ] });\n    \
             let emission_bind_group_layout = Self::create_emission(device);\n    Self { camera_bind_group_layout, emission_bind_group_layout }\n  }\n  \
             fn create_emission(device: &wgpu::Device) -> wgpu::BindGroupLayout {\n    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[\n      \
             wgpu::BindGroupLayoutEntry { binding: 0, ty: wgpu::BindingType::Buffer { }, count: None },\n    ] })\n  }\n}\n\
             pub fn camera_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, cam: &wgpu::Buffer) -> wgpu::BindGroup {\n  \
             device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &[\n    wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },\n  ] })\n}\n"
                .to_string(),
        ),
        src_of(
            "src/renderer/particles.rs",
            "pub fn build_particle_pipelines(device: &wgpu::Device) -> (u32, u32, wgpu::BindGroupLayout) {\n  \
             let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[uniform(0), uniform(1)] });\n  (1, 2, frame_bgl)\n}\n\
             fn uniform(b: u32) -> wgpu::BindGroupLayoutEntry { wgpu::BindGroupLayoutEntry { binding: b, ty: wgpu::BindingType::Buffer { }, count: None } }\n"
                .to_string(),
        ),
        src_of(
            "src/renderer/mod.rs",
            "fn init(device: &wgpu::Device, pipeline: &Pipeline, cam: &wgpu::Buffer) {\n  \
             let camera_bind_group = pipeline::camera_bind_group(&device, &pipeline.camera_bind_group_layout, &cam);\n  \
             let (a, b, particle_frame_bgl) =\n    particles::build_particle_pipelines(&device);\n  \
             let g = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &particle_frame_bgl, entries: &[\n    \
             wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },\n    wgpu::BindGroupEntry { binding: 1, resource: cam.as_entire_binding() },\n  ] });\n  \
             let e = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &pipeline.emission_bind_group_layout, entries: &[\n    \
             wgpu::BindGroupEntry { binding: 0, resource: cam.as_entire_binding() },\n  ] });\n  \
             let auto = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &p.get_bind_group_layout(0), entries: &[] });\n}\n"
                .to_string(),
        ),
    ];
    let r = check(&files);
    let all = format!("{}\n{}", r.mismatches.join("\n"), r.unreadable.join("\n"));
    assert_eq!((r.compared, r.auto), (3, 1), "{all}");
    // camera_bind_group gives binding 0 only; its layout (followed through the caller) has 0 and 1.
    assert_eq!(r.mismatches.len(), 1, "{all}");
    assert!(r.mismatches[0].contains("src/renderer/pipeline.rs") && r.mismatches[0].contains("missing binding 1"), "{all}");
    assert!(r.mismatches[0].contains("layout `camera_bind_group_layout`"), "{all}");
    assert!(r.unreadable.is_empty(), "{all}");
    assert_eq!(r.per_layout.get("frame_bgl (src/renderer/particles.rs)"), Some(&(2, 1)), "{:?}", r.per_layout);
    assert_eq!(r.per_layout.get("create_emission (src/renderer/pipeline.rs)"), Some(&(1, 1)), "{:?}", r.per_layout);
}

#[test]
fn fixture_unreadable_sites_fail_closed() {
    let files = vec![src_of(
        "src/renderer/odd.rs",
        "fn f(device: &wgpu::Device, entries: Vec<wgpu::BindGroupEntry>) {\n  \
         let l = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[wgpu::BindGroupLayoutEntry { binding: 0, count: None }] });\n  \
         let a = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &l, entries: &entries });\n  \
         let b = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.layout_for(3), entries: &[] });\n  \
         let c = device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &nowhere, entries: &[] });\n}\n"
            .to_string(),
    )];
    let r = check(&files);
    let all = r.unreadable.join("\n");
    assert_eq!(r.unreadable.len(), 3, "{all}");
    assert!(all.contains("`entries: &entries` is not a literal list"), "{all}");
    assert!(all.contains("is a call or index"), "{all}");
    assert!(all.contains("`nowhere` is not defined in its function"), "{all}");
    assert_eq!(r.compared, 0);
}
