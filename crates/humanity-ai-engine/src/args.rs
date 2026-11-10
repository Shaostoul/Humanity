//! The engine's command line. The app passes these; a person never needs to (GUI-first: Settings and
//! the Dev page start the engine).

/// How the engine runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// `--fake`: scripted lines and timing instead of a model.
    pub fake: bool,
    /// `--fake-ms-prefill <ms>`: how long reading a new persona takes (a tenth of it when the same
    /// persona comes again with reuse on, as llama.cpp's prompt reuse would).
    pub ms_prefill: u64,
    /// `--fake-ms-per-token <ms>`: the time between pieces of a reply.
    pub ms_per_token: u64,
    /// `--fake-load-mb <n>`: each `load` reserves and touches this much memory and keeps it while the
    /// model is loaded, so the job object's memory cap can be seen to work.
    pub load_mb: u64,
    /// `--fake-protocol <v>`: write messages at another protocol version, so the app can be seen to
    /// refuse a mismatched engine in words.
    pub protocol: u32,
    /// `--fake-exit-on <type>`: exit at once, with code 3, when a message of this type arrives (a
    /// crash, for the client's crash handling).
    pub exit_on: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options { fake: false, ms_prefill: 150, ms_per_token: 25, load_mb: 0, protocol: humanity_ai_protocol::PROTOCOL, exit_on: None }
    }
}

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Run(Options),
    /// `--version`: print the engine and protocol versions and exit.
    Version,
}

/// Read the arguments (without the program's own name). An argument it does not know is refused,
/// never ignored: a typo in a test's flag would otherwise test something else.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, String> {
    let mut o = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let mut number = |name: &str| -> Result<u64, String> {
            let v = it.next().ok_or_else(|| format!("{name} needs a number"))?;
            v.parse::<u64>().map_err(|_| format!("{name} needs a number, not {v:?}"))
        };
        match a.as_str() {
            "--fake" => o.fake = true,
            "--version" => return Ok(Command::Version),
            "--fake-ms-prefill" => o.ms_prefill = number("--fake-ms-prefill")?,
            "--fake-ms-per-token" => o.ms_per_token = number("--fake-ms-per-token")?,
            "--fake-load-mb" => o.load_mb = number("--fake-load-mb")?,
            "--fake-protocol" => o.protocol = u32::try_from(number("--fake-protocol")?).map_err(|_| "--fake-protocol is too large".to_string())?,
            "--fake-exit-on" => o.exit_on = Some(it.next().ok_or("--fake-exit-on needs a message type")?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let fake_only = o.ms_prefill != Options::default().ms_prefill
        || o.ms_per_token != Options::default().ms_per_token
        || o.load_mb != 0
        || o.protocol != humanity_ai_protocol::PROTOCOL
        || o.exit_on.is_some();
    if fake_only && !o.fake {
        return Err("the --fake-* arguments need --fake".into());
    }
    Ok(Command::Run(o))
}
