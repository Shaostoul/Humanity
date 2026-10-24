//! Loading the how-tos from a data folder: the category tree, the planned list, and every
//! `data/how_to/<top category>/<id>.json`, each checked (check.rs) before it is shown.
//!
//! A file the checker refuses is not shown as a how-to; it is listed with its reasons, so a person
//! who edited a file in their data folder can see why it disappeared (the page's "Could not be
//! shown" part). One finding is treated differently at run time: a review whose hash no longer
//! matches. The build refuses that in a shipped file, but in a folder a person edited the how-to is
//! still shown, with "Changed since it was checked" in place of the badge (docs/design/how-to.md).
//!
//! Budget (docs/design/how-to.md, "Sizes and budgets"): every file is read and parsed in one pass.
//! At six files plus the reference sets this takes a few milliseconds and runs on a worker thread
//! when the page first opens (pages/how_to.rs); past about 3,000 files the index comes first and
//! bodies on open.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::check::{self, Code, Context, Finding};
use super::model::{Category, CategoriesFile, HowTo, Planned, PlannedFile};

/// A how-to that passed the checker.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// Its path under data/how_to/, forward slashes.
    pub file: String,
    pub how_to: HowTo,
    /// Its hash now (canonical.rs).
    pub hash: String,
    /// Warnings, and a stale review when the review no longer matches.
    pub findings: Vec<Finding>,
    /// False when the review's hash no longer matches the words: show "Changed since it was checked".
    pub review_current: bool,
}

/// A file the checker refused.
#[derive(Debug, Clone)]
pub struct Refused {
    pub file: String,
    pub findings: Vec<Finding>,
}

/// Everything the page shows.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub categories: Vec<Category>,
    pub planned: Vec<Planned>,
    pub how_tos: Vec<Loaded>,
    pub refused: Vec<Refused>,
    /// Why the tree or the plan could not be read, when one could not.
    pub problems: Vec<String>,
    /// How long the load took, milliseconds (the startup budget).
    pub load_ms: f32,
    /// Names for the ids a how-to names, read with it so the page shows words, not ids: items
    /// (items.csv), skills (skills.csv), Curriculum topics, Library documents by slug, and
    /// jurisdictions (data/laws/laws.json, for "Only in Washington").
    pub item_names: HashMap<String, String>,
    pub skill_names: HashMap<String, String>,
    pub topic_titles: HashMap<String, String>,
    pub reading_titles: HashMap<String, String>,
    pub place_names: HashMap<String, String>,
    /// Roles (data/sources/registry.json kind role) by id, with their general name.
    pub role_names: HashMap<String, String>,
    /// The place roles resolve in, until the person's place is a saved setting (increment 7): the
    /// first locale in data/locales/ (Silverdale, where the simulation is set), its name and who
    /// fills each role there.
    pub locale: Option<Locale>,
}

/// A place's name and who fills each role there (data/locales/<id>/locale.json `authorities`).
#[derive(Debug, Clone, Default)]
pub struct Locale {
    pub name: String,
    /// Role id to (body's name, its address, its note).
    pub authorities: HashMap<String, (String, String, String)>,
}

/// Page ids and the sections a go step may open on each. The native app passes the real ones
/// (pages/how_to.rs `app_pages`); a relay or a test without them passes None and go steps are not
/// resolved.
pub type Pages = HashMap<String, Vec<String>>;

impl Library {
    /// Read and check everything under `data_dir/how_to/`.
    pub fn load(data_dir: &Path, pages: Option<Pages>) -> Library {
        let t0 = std::time::Instant::now();
        let dir = data_dir.join("how_to");
        let mut lib = Library::default();
        // The website reads the same two files (web/pages/how-to-app.js; tests/page_parity_lint.rs).
        match std::fs::read(data_dir.join("how_to/categories.json")).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<CategoriesFile>(&b).map_err(|e| e.to_string())) {
            Ok(f) => lib.categories = f.categories,
            Err(e) => lib.problems.push(format!("data/how_to/categories.json: {e}")),
        }
        match std::fs::read(data_dir.join("how_to/planned.json")).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice::<PlannedFile>(&b).map_err(|e| e.to_string())) {
            Ok(f) => lib.planned = f.planned,
            Err(e) => lib.problems.push(format!("data/how_to/planned.json: {e}")),
        }
        sort_tree(&mut lib.categories);
        let files = how_to_files(&dir);
        let mut cx = context_from(data_dir, &lib.categories, &lib.planned, pages);
        // Every file's id first, so duplicates and calls resolve whatever order the files load in.
        let mut read: Vec<(String, Vec<u8>)> = Vec::new();
        for f in files {
            let rel = rel_of(&dir, &f);
            match std::fs::read(&f) {
                Ok(b) => {
                    if let Some(id) = serde_json::from_slice::<serde_json::Value>(&b).ok().and_then(|v| v.get("id").and_then(|i| i.as_str()).map(String::from)) {
                        cx.written.entry(id).or_insert_with(|| rel.clone());
                    }
                    read.push((rel, b));
                }
                Err(e) => lib.refused.push(Refused { file: rel, findings: vec![Finding { code: Code::Json, message: format!("could not be read: {e}") }] }),
            }
        }
        for (rel, bytes) in read {
            let (findings, parsed) = check::check_file(&rel, &bytes, &cx);
            let refusing: Vec<&Finding> = check::refusals(&findings).filter(|f| f.code != Code::ReviewHashStale).collect();
            match parsed {
                Some(how_to) if refusing.is_empty() => {
                    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                    let hash = super::canonical::hash(&json).unwrap_or_default();
                    let review_current = !findings.iter().any(|f| f.code == Code::ReviewHashStale);
                    lib.how_tos.push(Loaded { file: rel, how_to, hash, findings, review_current });
                }
                _ => lib.refused.push(Refused { file: rel, findings }),
            }
        }
        lib.how_tos.sort_by(|a, b| a.how_to.title.to_lowercase().cmp(&b.how_to.title.to_lowercase()));
        read_names(data_dir, &mut lib);
        lib.load_ms = t0.elapsed().as_secs_f32() * 1000.0;
        log::info!(
            "How-to: {} how-tos, {} planned, {} refused, in {:.1} ms",
            lib.how_tos.len(),
            lib.planned.len(),
            lib.refused.len(),
            lib.load_ms
        );
        lib
    }

    /// The how-to with this id.
    pub fn find(&self, id: &str) -> Option<&Loaded> {
        self.how_tos.iter().find(|l| l.how_to.id == id)
    }

    /// The planned how-to with this id.
    pub fn find_planned(&self, id: &str) -> Option<&Planned> {
        self.planned.iter().find(|p| p.id == id)
    }

    /// The category node with this id, anywhere in the tree.
    pub fn category(&self, id: &str) -> Option<&Category> {
        fn walk<'a>(nodes: &'a [Category], id: &str) -> Option<&'a Category> {
            for n in nodes {
                if n.id == id {
                    return Some(n);
                }
                if let Some(f) = walk(&n.children, id) {
                    return Some(f);
                }
            }
            None
        }
        walk(&self.categories, id)
    }

    /// The ids of a category and everything under it.
    pub fn branch_ids(&self, id: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        fn add(n: &Category, out: &mut HashSet<String>) {
            out.insert(n.id.clone());
            for c in &n.children {
                add(c, out);
            }
        }
        if let Some(n) = self.category(id) {
            add(n, &mut out);
        }
        out
    }

    /// The path of category ids from the top of the tree down to `id`, inclusive.
    pub fn path_to(&self, id: &str) -> Vec<String> {
        fn walk(nodes: &[Category], id: &str, path: &mut Vec<String>) -> bool {
            for n in nodes {
                path.push(n.id.clone());
                if n.id == id || walk(&n.children, id, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        walk(&self.categories, id, &mut path);
        path
    }

    /// Written and planned how-tos in a branch, counted.
    pub fn counts(&self, id: &str) -> (usize, usize) {
        let ids = self.branch_ids(id);
        (
            self.how_tos.iter().filter(|l| ids.contains(&l.how_to.category)).count(),
            self.planned.iter().filter(|p| ids.contains(&p.category)).count(),
        )
    }
}

/// Every `<top>/<id>.json` under `dir` (one folder level: the folder is the top category), sorted
/// so a load is the same on every machine.
pub fn how_to_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(tops) = std::fs::read_dir(dir) else { return out };
    for top in tops.flatten() {
        let p = top.path();
        if !p.is_dir() {
            continue;
        }
        if let Ok(files) = std::fs::read_dir(&p) {
            for f in files.flatten() {
                let fp = f.path();
                let name = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // `<id>.<lang>.json` is a translation (a later rung), not a how-to.
                if fp.extension().and_then(|e| e.to_str()) == Some("json") && name.matches('.').count() == 1 {
                    out.push(fp);
                }
            }
        }
    }
    out.sort();
    out
}

fn rel_of(dir: &Path, f: &Path) -> String {
    f.strip_prefix(dir).unwrap_or(f).to_string_lossy().replace('\\', "/")
}

fn sort_tree(nodes: &mut [Category]) {
    nodes.sort_by_key(|n| n.order);
    for n in nodes.iter_mut() {
        sort_tree(&mut n.children);
    }
}

/// Category id to its top-level ancestor.
pub fn tops_of(categories: &[Category]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    fn add(n: &Category, top: &str, out: &mut HashMap<String, String>) {
        out.insert(n.id.clone(), top.to_string());
        for c in &n.children {
            add(c, top, out);
        }
    }
    for n in categories {
        add(n, &n.id, &mut out);
    }
    out
}

/// The first two columns of a CSV in data/ (id, name), without comment lines and the header.
fn csv_names(path: &Path) -> HashMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("id,"))
        .filter_map(|l| {
            let mut cols = l.split(',');
            Some((cols.next()?.trim().to_string(), cols.next()?.trim().to_string()))
        })
        .collect()
}

/// The names the page shows for the ids how-tos name.
fn read_names(data_dir: &Path, lib: &mut Library) {
    lib.item_names = csv_names(&data_dir.join("items.csv"));
    lib.skill_names = csv_names(&data_dir.join("skills").join("skills.csv"));
    if let Some(v) = json_of(&data_dir.join("curriculum").join("syllabus.json")) {
        for t in v["topics"].as_array().into_iter().flatten() {
            if let (Some(id), Some(title)) = (t["id"].as_str(), t["title"].as_str()) {
                lib.topic_titles.insert(id.to_string(), title.to_string());
            }
        }
    }
    if let Some(v) = json_of(&data_dir.join("library").join("index.json")) {
        fn walk(v: &serde_json::Value, out: &mut HashMap<String, String>) {
            match v {
                serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                serde_json::Value::Object(o) => {
                    if let (Some(f), Some(t)) = (o.get("file").and_then(|f| f.as_str()), o.get("title").and_then(|t| t.as_str())) {
                        out.insert(f.trim_end_matches(".md").replace('_', "-").to_lowercase(), t.to_string());
                    }
                    o.values().for_each(|x| walk(x, out));
                }
                _ => {}
            }
        }
        walk(&v, &mut lib.reading_titles);
    }
    if let Some(v) = json_of(&data_dir.join("laws").join("laws.json")) {
        for j in v["jurisdictions"].as_array().into_iter().flatten() {
            if let (Some(id), Some(name)) = (j["id"].as_str(), j["name"].as_str()) {
                lib.place_names.insert(id.to_string(), name.to_string());
            }
        }
    }
    if let Some(v) = json_of(&data_dir.join("sources").join("registry.json")) {
        for s in v["sources"].as_array().into_iter().flatten().filter(|s| s["kind"].as_str() == Some("role")) {
            if let (Some(id), Some(name)) = (s["id"].as_str(), s["name"].as_str()) {
                lib.role_names.insert(id.to_string(), name.to_string());
            }
        }
    }
    let mut locales: Vec<PathBuf> = std::fs::read_dir(data_dir.join("locales")).into_iter().flatten().flatten().map(|e| e.path().join("locale.json")).filter(|p| p.exists()).collect();
    locales.sort();
    if let Some(v) = locales.first().and_then(|p| json_of(p)) {
        let name = match (v["name"].as_str(), v["region"].as_str()) {
            (Some(n), Some(r)) if !r.is_empty() => format!("{n}, {r}"),
            (Some(n), _) => n.to_string(),
            _ => String::new(),
        };
        let mut loc = Locale { name, ..Locale::default() };
        if let Some(a) = v["authorities"].as_object() {
            for (role, body) in a {
                let s = |k: &str| body[k].as_str().unwrap_or("").to_string();
                loc.authorities.insert(role.clone(), (s("name"), s("url"), s("note")));
            }
        }
        lib.locale = Some(loc);
    }
}

/// The first column of a CSV in data/, without comment lines and the header.
fn csv_ids(path: &Path) -> Option<HashSet<String>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("id,"))
            .filter_map(|l| l.split(',').next().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
    )
}

fn json_of(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// The checker's context, from the data folder: every set the checker resolves against.
pub fn context_from(data_dir: &Path, categories: &[Category], planned: &[Planned], pages: Option<Pages>) -> Context {
    let mut cx = Context { categories: tops_of(categories), pages, ..Context::default() };
    cx.planned = planned.iter().map(|p| p.id.clone()).collect();
    cx.items = csv_ids(&data_dir.join("items.csv"));
    cx.recipes = csv_ids(&data_dir.join("recipes.csv"));
    cx.skills = csv_ids(&data_dir.join("skills").join("skills.csv"));
    cx.topics = json_of(&data_dir.join("curriculum").join("syllabus.json")).map(|v| {
        v["topics"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str().map(String::from)).collect()
    });
    cx.reading = json_of(&data_dir.join("library").join("index.json")).map(|v| {
        let mut out = HashSet::new();
        fn walk(v: &serde_json::Value, out: &mut HashSet<String>) {
            match v {
                serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                serde_json::Value::Object(o) => {
                    if let Some(f) = o.get("file").and_then(|f| f.as_str()) {
                        out.insert(f.trim_end_matches(".md").replace('_', "-").to_lowercase());
                    }
                    o.values().for_each(|x| walk(x, out));
                }
                _ => {}
            }
        }
        walk(&v, &mut out);
        out
    });
    cx.tags = json_of(&data_dir.join("library").join("tags.json")).map(|v| {
        v["groups"].as_array().into_iter().flatten().flat_map(|g| g["tags"].as_array().into_iter().flatten()).filter_map(|t| t["id"].as_str().map(String::from)).collect()
    });
    cx.sources = json_of(&data_dir.join("sources").join("registry.json")).map(|v| {
        v["sources"].as_array().into_iter().flatten().filter_map(|s| Some((s["id"].as_str()?.to_string(), s["kind"].as_str() == Some("role")))).collect()
    });
    cx.places = json_of(&data_dir.join("laws").join("laws.json")).map(|v| {
        v["jurisdictions"].as_array().into_iter().flatten().filter_map(|j| j["id"].as_str().map(String::from)).collect()
    });
    // A data folder beside a repository checkout (the build, the tests, a developer's run) checks
    // `in_project` paths against it; a shipped app has no source tree and skips them.
    if let Some(root) = data_dir.parent() {
        if root.join("Cargo.toml").exists() && root.join("src").is_dir() {
            cx.repo_root = Some(root.to_path_buf());
        }
    }
    cx
}
