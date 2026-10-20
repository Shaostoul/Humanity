//! INSTALLING THROUGH WINGET (2026-10-19), Windows' own package manager, on the person's click and
//! only after they agree in HumanityOS's own window. What may be done is set by the dated finding
//! docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md ("What it means for
//! HumanityOS", judgement call 2), and this module does exactly that:
//!
//! 1. ASK WINGET ABOUT THE PACKAGE first (`winget show`, [`show_plan`]): its publisher, its
//!    licence and licence page, and any agreements its manifest carries ([`parse_show`]). Reading
//!    is not installing.
//! 2. THE CONSENT STEP (the Tools page's details, tools_detail.rs) shows the licence, those
//!    agreements, Microsoft's two lines exactly as winget prints them ([`DISCLAIMER_OWNER`],
//!    [`DISCLAIMER_MICROSOFT`]), that Windows may ask for permission, and that winget sends
//!    Microsoft usage data; and says that pressing "I agree, install" means agreeing.
//! 3. ONLY AFTER "I AGREE" ([`Agreed`]) is winget asked to install ([`install_plan`]): that exact
//!    package (`--id <id> --exact`) from winget's own community source (`--source winget`, so the
//!    Store source and its terms never come into it), with `--accept-package-agreements` because
//!    the person has just agreed, `--no-upgrade` (a copy already there is left as it is), and
//!    `--disable-interactivity` (winget fails in plain words rather than waiting on a prompt nobody
//!    can see). For a program whose licence is not a free-software licence (`install_interactive`
//!    in the catalog: VS Code, Paint.NET, Zed) also `--interactive`, so the maker's own installer and
//!    licence page appear. NEVER `--accept-source-agreements`, `--silent` or
//!    `--ignore-security-hash`; HumanityOS never runs as administrator to skip Windows' own prompt.
//! 4. Each winget return code becomes a plain sentence ([`failure_words`], the finding's 2.7 table),
//!    with winget's own last words beside it.
//!
//! [`run`] is THE one place winget is started, on the page's worker thread, never on the UI's.
//! TEST BUILDS NEVER RUN IT: there `run` records what it was asked and answers what the test set
//! (`recorder`), and the code that starts a program is not compiled in, as with games/launch.rs.

use std::sync::mpsc::Sender;

/// winget's first line before every install that is not from the Store, quoted exactly (its
/// `InstallationDisclaimer1`, the finding's 2.2).
pub const DISCLAIMER_OWNER: &str = "This application is licensed to you by its owner.";
/// Its second (`InstallationDisclaimer2`), quoted exactly.
pub const DISCLAIMER_MICROSOFT: &str = "Microsoft is not responsible for, nor does it grant any licenses to, third-party packages.";
/// Said when winget is not on this computer.
pub const WINGET_MISSING: &str = "winget, Windows' own package manager, was not found on this computer. It comes with Microsoft's App Installer, from the Microsoft Store. Until then, Download opens the maker's own page.";

/// What winget says about a package (`winget show`), shown in the consent step. Each `None` where
/// winget did not say.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Package {
    pub id: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    /// Each line of the package's agreements, as winget prints them. Most packages have none (only
    /// verified developers may add them, the finding's 2.3).
    pub agreements: Vec<String>,
}

/// One line of winget's output as a person would read it: the text after its last carriage
/// return (winget redraws a line in place), without its progress bar's block characters, its
/// spinner, or runs of spaces.
pub fn clean_line(raw: &str) -> String {
    let raw = raw.trim_end_matches(['\r', '\n']);
    let after = raw.rsplit('\r').next().unwrap_or(raw);
    let kept: String = after.chars().filter(|c| !('\u{2580}'..='\u{259F}').contains(c) && !c.is_control()).collect();
    let words = kept.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.chars().all(|c| matches!(c, '-' | '\\' | '|' | '/')) {
        String::new()
    } else {
        words
    }
}

/// What `winget show --id <id>` printed, read: `None` when it did not find that package (its
/// "Found <name> [<id>]" line is missing). winget's labels are English ones; another language's
/// output reads as a package with only its name, and the consent step then shows the catalog's own
/// licence.
pub fn parse_show(id: &str, text: &str) -> Option<Package> {
    let lines: Vec<String> = text
        .lines()
        .map(|l| {
            let l = l.trim_end_matches('\r');
            l.rsplit('\r').next().unwrap_or(l).trim_end().to_string()
        })
        .collect();
    let found = lines.iter().find_map(|l| {
        let l = l.trim_start_matches(|c: char| !c.is_alphanumeric());
        let rest = l.strip_prefix("Found ")?;
        let open = rest.rfind(" [")?;
        (rest.ends_with(']') && rest[open + 2..rest.len() - 1].eq_ignore_ascii_case(id)).then(|| rest[..open].to_string())
    })?;
    let value = |label: &str| lines.iter().find_map(|l| l.strip_prefix(label).map(|v| v.trim().to_string())).filter(|v| !v.is_empty());
    let mut agreements = Vec::new();
    if let Some(start) = lines.iter().position(|l| l.trim_end() == "Agreements:") {
        for l in &lines[start + 1..] {
            if l.is_empty() || !l.starts_with(char::is_whitespace) {
                break;
            }
            agreements.push(l.trim().to_string());
        }
    }
    Some(Package {
        id: id.to_string(),
        name: Some(found),
        version: value("Version:"),
        publisher: value("Publisher:"),
        license: value("License:"),
        license_url: value("License Url:"),
        agreements,
    })
}

/// A command to run: winget and its arguments, each one word (no shell reads them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The program run.
    pub program: &'static str,
    pub args: Vec<String>,
}

impl Plan {
    /// The command as one line, for a person to read.
    pub fn line(&self) -> String {
        std::iter::once(self.program.to_string()).chain(self.args.iter().cloned()).collect::<Vec<_>>().join(" ")
    }

    /// Whether this is an install (rather than reading what winget says).
    pub fn installs(&self) -> bool {
        self.args.first().is_some_and(|a| a == "install")
    }
}

/// Whether `id` is shaped like a winget package id: two or more parts joined by dots, each of
/// letters, digits, `-`, `_` or `+` ("Notepad++.Notepad++", "GIMP.GIMP.3").
pub fn package_id_ok(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() <= 128 && parts.len() >= 2 && parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+')))
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// Ask winget what it knows about package `id` (reading only). `None` for an id not shaped like
/// one.
pub fn show_plan(id: &str) -> Option<Plan> {
    package_id_ok(id).then(|| Plan { program: "winget", args: words(&["show", "--id", id, "--exact", "--source", "winget", "--disable-interactivity"]) })
}

/// What the person agreed to in HumanityOS's own window: one package, by its id, and whether its
/// maker's installer is to be shown. Made by the consent step's "I agree, install" and nowhere
/// else (pages/tools.rs `agree`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreed {
    pub id: String,
    pub interactive: bool,
}

/// Install what the person agreed to (see the module's header for each switch). `None` for an id
/// not shaped like a package id.
pub fn install_plan(agreed: &Agreed) -> Option<Plan> {
    if !package_id_ok(&agreed.id) {
        return None;
    }
    let mut args = words(&["install", "--id", &agreed.id, "--exact", "--source", "winget", "--accept-package-agreements", "--no-upgrade", "--disable-interactivity"]);
    if agreed.interactive {
        args.push("--interactive".to_string());
    }
    Some(Plan { program: "winget", args })
}

/// winget's return code for "found at least one version of the package installed".
pub const ALREADY_INSTALLED: u32 = 0x8A15_0061;

/// Why a run did not finish, in plain words, and winget's code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: Option<u32>,
    pub words: String,
}

/// A winget return code in plain words (the finding's 2.7, winget's own table), with winget's own
/// last line beside it when it said one.
pub fn failure_words(code: Option<u32>, last_line: &str) -> String {
    let plain = match code {
        Some(0x8A15_0010) => "winget has no installer of it for this kind of computer.".to_string(),
        Some(0x8A15_0011) => "The installer winget downloaded was not the one it expected, so it was not run. Nothing was installed.".to_string(),
        Some(0x8A15_0041) => "Its agreements were not accepted, so nothing was installed.".to_string(),
        Some(0x8A15_0046) => "winget's package source asks for its own agreement to be accepted first. HumanityOS does not accept that for you: run winget yourself to read it, or use Download.".to_string(),
        Some(ALREADY_INSTALLED) => "winget found it already installed, and left it as it is.".to_string(),
        Some(0x8A15_010C) => "The installation was cancelled. Nothing was installed.".to_string(),
        Some(0x8A15_0109) => "Restart your computer to finish installing it.".to_string(),
        Some(0x8A15_003A) | Some(0x8A15_010F) => "This computer's own settings (its organization's policy) do not allow installing it.".to_string(),
        Some(0x8A15_0101) => "It is running: close it, then try again.".to_string(),
        Some(0x8A15_0105) => "There is not enough room on the disk. Make space, then try again.".to_string(),
        Some(0x8A15_0108) => "Its installer ran into a problem. Its maker's own website may help.".to_string(),
        Some(0x8A15_0014) => "winget did not find the package. Download opens the maker's own page.".to_string(),
        Some(c) => format!("It did not install (winget's code 0x{c:08X}). If Windows asked for permission and it was not given, nothing was installed."),
        None => "It did not install.".to_string(),
    };
    let last = clean_line(last_line);
    if last.is_empty() {
        plain
    } else {
        format!("{plain} winget said: \"{last}\"")
    }
}

/// What a run sends back while it runs: each line winget prints, then how it ended (all it
/// printed, or why it failed).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Line(String),
    Done(Result<String, Failure>),
}

/// THE ONE place winget is run: `plan`, with its output sent to `tx` line by line and its end as
/// `Done`. Blocks until winget is done, so the page calls it on a worker thread. Test builds record
/// the plan and answer what the test set, and run nothing.
pub fn run(plan: &Plan, tx: &Sender<Event>) {
    #[cfg(test)]
    {
        let answer = recorder::record(plan);
        let _ = tx.send(Event::Done(answer));
    }
    #[cfg(not(test))]
    {
        real::run(plan, tx);
    }
}

/// Running winget (not compiled into test builds).
#[cfg(not(test))]
mod real {
    use super::{clean_line, failure_words, Event, Failure, Plan, WINGET_MISSING};
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc::Sender;

    pub(super) fn run(plan: &Plan, tx: &Sender<Event>) {
        let mut cmd = Command::new(plan.program);
        cmd.args(&plan.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: winget runs without a console window of its own. An installer
            // it starts still shows its own windows, and Windows still shows its own prompt.
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let _ = tx.send(Event::Done(Err(Failure { code: None, words: WINGET_MISSING.to_string() })));
                return;
            }
            Err(e) => {
                let _ = tx.send(Event::Done(Err(Failure { code: None, words: format!("winget could not be started ({e}).") })));
                return;
            }
        };
        // Its errors are read on their own thread, so a full pipe never stalls it.
        let mut stderr = child.stderr.take();
        let errors = std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(e) = stderr.as_mut() {
                let _ = e.read_to_string(&mut s);
            }
            s
        });
        let mut all = String::new();
        let mut last = String::new();
        if let Some(mut out) = child.stdout.take() {
            let mut buf = [0u8; 4096];
            let mut pending = String::new();
            while let Ok(n) = out.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                all.push_str(&text);
                pending.push_str(&text);
                // Each line, and each redraw of a line, as it comes.
                while let Some(at) = pending.find(['\n', '\r']) {
                    let line = clean_line(&pending[..at]);
                    pending.drain(..=at);
                    if !line.is_empty() {
                        last = line.clone();
                        let _ = tx.send(Event::Line(line));
                    }
                }
                // Keep at most the last 64 KB of what it printed.
                if all.len() > 64 * 1024 {
                    let cut = all.len() - 64 * 1024;
                    let cut = (cut..all.len()).find(|i| all.is_char_boundary(*i)).unwrap_or(all.len());
                    all.drain(..cut);
                }
            }
        }
        let status = child.wait();
        let errors = errors.join().unwrap_or_default();
        if last.is_empty() {
            last = errors.lines().map(clean_line).filter(|l| !l.is_empty()).last().unwrap_or_default();
        }
        let done = match status {
            Ok(s) if s.success() => Ok(all),
            Ok(s) => {
                let code = s.code().map(|c| c as u32);
                Err(Failure { code, words: failure_words(code, &last) })
            }
            Err(e) => Err(Failure { code: None, words: format!("winget's end could not be read ({e}).") }),
        };
        let _ = tx.send(Event::Done(done));
    }
}

/// Test builds: what `run` was asked, per test thread, and what it answers.
#[cfg(test)]
pub(crate) mod recorder {
    use super::{Failure, Plan};
    use std::cell::RefCell;

    thread_local! {
        static ASKED: RefCell<Vec<Plan>> = const { RefCell::new(Vec::new()) };
        static SHOW: RefCell<Option<Result<String, Failure>>> = const { RefCell::new(None) };
        static INSTALL: RefCell<Result<String, Failure>> = RefCell::new(Ok("Successfully installed".to_string()));
    }

    /// What `winget show` answers for `id` unless a test set another: a package with a publisher,
    /// a licence and its page, and no agreements.
    pub(crate) fn show_text(id: &str) -> String {
        format!("Found Test Program [{id}]\nVersion: 1.2.3\nPublisher: Test Publisher\nLicense: GPL-3.0\nLicense Url: https://example.org/license\nInstaller:\n  Installer Type: wix\n")
    }

    pub(super) fn record(plan: &Plan) -> Result<String, Failure> {
        ASKED.with(|a| a.borrow_mut().push(plan.clone()));
        if plan.installs() {
            INSTALL.with(|r| r.borrow().clone())
        } else {
            let id = plan.args.iter().skip_while(|a| *a != "--id").nth(1).cloned().unwrap_or_default();
            SHOW.with(|r| r.borrow().clone()).unwrap_or_else(|| Ok(show_text(&id)))
        }
    }

    /// Everything `run` was asked on this thread since the last call, in order.
    pub(crate) fn recorded() -> Vec<Plan> {
        ASKED.with(|a| std::mem::take(&mut *a.borrow_mut()))
    }

    /// From now on, on this thread, `winget show` answers `answer` (`None`: the default).
    pub(crate) fn answer_show(answer: Option<Result<String, Failure>>) {
        SHOW.with(|r| *r.borrow_mut() = answer);
    }

    /// From now on, on this thread, an install answers `answer`.
    pub(crate) fn answer_install(answer: Result<String, Failure>) {
        INSTALL.with(|r| *r.borrow_mut() = answer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE SWITCHES: reading what winget says is `show` for that exact package from winget's own
    /// source; installing what the person agreed to adds `--accept-package-agreements` (they have
    /// just agreed), `--no-upgrade` and `--disable-interactivity`, and `--interactive` only for a
    /// program whose licence is not free software; nothing ever passes
    /// `--accept-source-agreements`, `--silent` or `--ignore-security-hash`; reading never accepts
    /// anything; an id not shaped like a package id has no plan.
    ///
    /// Seen red 2026-10-19 with `--interactive` added whatever `agreed.interactive` said: the free
    /// program's command line got "... --disable-interactivity --interactive".
    #[test]
    fn the_switches() {
        let show = show_plan("BlenderFoundation.Blender").expect("a plan");
        assert_eq!(show.line(), "winget show --id BlenderFoundation.Blender --exact --source winget --disable-interactivity");
        assert!(!show.installs());
        let free = install_plan(&Agreed { id: "BlenderFoundation.Blender".into(), interactive: false }).unwrap();
        assert_eq!(free.line(), "winget install --id BlenderFoundation.Blender --exact --source winget --accept-package-agreements --no-upgrade --disable-interactivity");
        assert!(!free.args.iter().any(|a| a == "--interactive"), "a free program installs without its installer's window");
        let vscode = install_plan(&Agreed { id: "Microsoft.VisualStudioCode".into(), interactive: true }).unwrap();
        assert!(vscode.installs() && vscode.args.last().map(String::as_str) == Some("--interactive"), "its maker's installer shown: {}", vscode.line());
        for plan in [&show, &free, &vscode] {
            for never in ["--accept-source-agreements", "--silent", "-h", "--ignore-security-hash", "--force"] {
                assert!(!plan.args.iter().any(|a| a == never), "{never} is never passed: {}", plan.line());
            }
        }
        assert!(!show.args.iter().any(|a| a.starts_with("--accept")), "reading accepts nothing");
        for bad in ["", "Blender", "a..b", "x.y; del *", "x.y --silent", "x.y\"&calc"] {
            assert_eq!(show_plan(bad), None, "{bad:?} is not a package id");
            assert_eq!(install_plan(&Agreed { id: bad.into(), interactive: false }), None);
        }
    }

    /// WHAT WINGET SAYS ABOUT A PACKAGE, READ: its name, version, publisher, licence and licence
    /// page, and its agreements' lines (a block of indented lines under "Agreements:", none when
    /// the package has none); a spinner left before "Found" is ignored; an answer that did not find
    /// that package reads as nothing.
    ///
    /// Seen red 2026-10-19 with the agreements block read to the end of the text (no stop at the
    /// first line that is not indented): "the agreements, and nothing after them" got the
    /// "Installer:" line too.
    #[test]
    fn what_winget_says_about_a_package() {
        let text = "\r  - \r  \\ \rFound paint.net [dotPDN.PaintDotNet]\r\nVersion: 5.1.12\r\nPublisher: dotPDN LLC\r\nLicense: Proprietary (Freeware)\r\nLicense Url: https://getpaint.net/license.html\r\nAgreements:\r\n  Terms: https://example.org/terms\r\n  Pricing: Free\r\nInstaller:\r\n  Installer Type: zip\r\n";
        let p = parse_show("dotPDN.PaintDotNet", text).expect("found");
        assert_eq!(p.name.as_deref(), Some("paint.net"));
        assert_eq!((p.version.as_deref(), p.publisher.as_deref()), (Some("5.1.12"), Some("dotPDN LLC")));
        assert_eq!((p.license.as_deref(), p.license_url.as_deref()), (Some("Proprietary (Freeware)"), Some("https://getpaint.net/license.html")));
        assert_eq!(p.agreements, vec!["Terms: https://example.org/terms".to_string(), "Pricing: Free".to_string()], "the agreements, and nothing after them");
        let none = parse_show("BlenderFoundation.Blender", &recorder::show_text("BlenderFoundation.Blender")).unwrap();
        assert!(none.agreements.is_empty());
        assert_eq!(parse_show("Other.Id", text), None, "another package's answer");
        assert_eq!(parse_show("x.y", "No package found matching input criteria."), None);
    }

    /// A FAILURE IN PLAIN WORDS: each code winget's table names becomes a sentence a person can act
    /// on, with winget's own last words beside it (its progress bar and spinner taken off); an
    /// unknown code says its number.
    ///
    /// Seen red 2026-10-19 with winget's own words left off (`last` never appended): "with winget's
    /// own words" failed.
    #[test]
    fn a_failure_in_plain_words() {
        let hash = failure_words(Some(0x8A15_0011), "Installer hash does not match; to override this check use --ignore-security-hash");
        assert!(hash.starts_with("The installer winget downloaded was not the one it expected"), "{hash}");
        assert!(hash.ends_with("winget said: \"Installer hash does not match; to override this check use --ignore-security-hash\""), "with winget's own words: {hash}");
        assert!(failure_words(Some(0x8A15_0046), "").contains("does not accept that for you"));
        assert!(failure_words(Some(0x8A15_0105), "").contains("not enough room"));
        assert_eq!(clean_line("\r  ██████▒▒▒▒  1.00 MB / 300 MB"), "1.00 MB / 300 MB");
        assert_eq!(clean_line("  - "), "");
        assert!(failure_words(Some(0x8A15_9999), "").contains("0x8A159999"));
    }
}
