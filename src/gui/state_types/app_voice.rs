//! The app's own voice, as a type (the owner branch's eighth review, 2026-11-22).
//!
//! Eight reviews in a row each found one more route by which text a relay chose reached a chat line
//! drawn with the app's own mark. Chasing routes one at a time could never end, so the default is
//! flipped: a line carries the app's mark ONLY when it was built HERE, from an i18n key (or one of
//! the sentences both apps pin, [`Pinned`]) and typed arguments. Nothing else can build one:
//! [`AppVoice`] has no public field and no public constructor but [`AppVoice::say`] and
//! [`AppVoice::pinned`], so `LineOrigin::App` cannot hold a plain `String`, and the drawing side
//! (widgets/app_line.rs, the HUD's chat feed) takes the `AppVoice`, never words.
//!
//! An argument that came from the network is a [`Relayed`], and a `Relayed` is never written out
//! raw (it has no accessor for its text): a name goes through `honest_name` (a name an honest relay
//! could list, else the refused-name words), any other text becomes one short line in quotation
//! marks, marked as the server's words. The only other arguments are a number this device counted
//! and another key's words, and a frame can choose neither.
//!
//! The website's twin is web/chat/app.js `appLine(key, args)`.

use humanity_core::i18n::{t, tf};
use humanity_protocol::key_card;

/// The longest quotation of the server's words an app line holds (characters), so no relay can
/// fill the app's line with a paragraph of its own.
pub const QUOTE_MAX: usize = 80;

/// Text that came from the network (a name a relay listed, a reason it gave). It can be wrapped by
/// anyone and never unwrapped: the only ways out are [`Relayed::as_name`] and
/// [`Relayed::as_quote`], both of which a relay cannot use to write a sentence.
#[derive(Clone, PartialEq, Eq)]
pub struct Relayed(String);

impl Relayed {
    /// Wrap text a frame carried (or anything derived from one).
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// As a name in the app's words: itself when an honest relay could have listed it
    /// (humanity_protocol::key_card::honest_name: 1 to 24 letters, digits, `_` or `-`, none of the
    /// app's or the server's own names), else the refused-name words. The website's twin is app.js
    /// `shownName`.
    pub fn as_name(&self) -> String {
        if key_card::honest_name(&self.0) {
            self.0.clone()
        } else {
            t("owner.name_refused").to_string()
        }
    }

    /// As a quotation inside the app's words: one line (every run of white space, a line break
    /// included, one space), at most [`QUOTE_MAX`] characters with an ellipsis after, its own
    /// quotation marks made plain so it cannot close the quotation early, then set in the words
    /// that mark it as the server's.
    pub fn as_quote(&self) -> String {
        let one_line = self.0.split_whitespace().collect::<Vec<_>>().join(" ");
        let plain: String = one_line.chars().map(|c| if matches!(c, '"' | '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{00ab}' | '\u{00bb}') { '\'' } else { c }).collect();
        let short: String = if plain.chars().count() > QUOTE_MAX { plain.chars().take(QUOTE_MAX).collect::<String>() + "\u{2026}" } else { plain };
        tf("chat.server_quote", &[("text", &short)])
    }
}

// Debug shows how the text WOULD be drawn, never a way to print it raw into a line by accident.
impl std::fmt::Debug for Relayed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Relayed({} chars)", self.0.chars().count())
    }
}

/// One argument of an app line.
pub enum Arg<'a> {
    /// A number this device counted.
    Count(u64),
    /// More of the app's own words, by i18n key.
    Words(&'static str),
    /// A name from the network ([`Relayed::as_name`]).
    Name(&'a Relayed),
    /// Other text from the network ([`Relayed::as_quote`]).
    Quote(&'a Relayed),
}

impl Arg<'_> {
    fn render(&self) -> String {
        match self {
            Arg::Count(n) => n.to_string(),
            Arg::Words(key) => t(key).to_string(),
            Arg::Name(r) => r.as_name(),
            Arg::Quote(r) => r.as_quote(),
        }
    }
}

/// The sentences both apps pin in humanity_protocol::key_card (the shared fixture
/// scripts/tests/fixtures/key-card-v1.json holds them word for word), which take a person's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinned {
    /// Why nothing was sealed to them ([`key_card::Refusal::text`]).
    NotSent(key_card::Refusal),
}

/// A line in the app's own voice. Built only by [`AppVoice::say`] and [`AppVoice::pinned`]; drawn
/// only through [`AppVoice::text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppVoice {
    key: &'static str,
    text: String,
}

impl AppVoice {
    /// The words of i18n `key` with its `{{placeholders}}` filled from `args`, each by its type.
    pub fn say(key: &'static str, args: &[(&'static str, Arg<'_>)]) -> Self {
        let filled: Vec<(&str, String)> = args.iter().map(|(name, arg)| (*name, arg.render())).collect();
        let vars: Vec<(&str, &dyn std::fmt::Display)> = filled.iter().map(|(name, value)| (*name, value as &dyn std::fmt::Display)).collect();
        Self { key, text: tf(key, &vars) }
    }

    /// One of the pinned sentences, naming `name` through [`Relayed::as_name`].
    pub fn pinned(words: Pinned, name: &Relayed) -> Self {
        match words {
            Pinned::NotSent(why) => Self { key: "key_card.refusal", text: why.text(&name.as_name()) },
        }
    }

    /// The words, as drawn.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Which words these are: the i18n key (or `key_card.refusal` for a pinned sentence).
    pub fn key(&self) -> &'static str {
        self.key
    }
}

/// A chat row in the app's own voice, in `channel` of `server`, at `ts_ms`. The ONE place an
/// App-origin [`super::ChatMessage`] is put together.
pub fn app_row(voice: AppVoice, channel: String, server: String, ts_ms: u64) -> super::ChatMessage {
    super::ChatMessage {
        sender_name: String::new(),
        sender_key: String::new(),
        content: voice.text().to_string(),
        timestamp: crate::gui::pages::chat::format_timestamp(ts_ms),
        timestamp_ms: ts_ms,
        channel,
        server,
        origin: super::LineOrigin::App(voice),
        ..Default::default()
    }
}
