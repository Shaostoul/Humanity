//! The talk card's "Say anything" box and the AI engine behind it (rung 3 of
//! docs/design/embeddable-ai.md, 2026-11-12).
//!
//! WARM-UP WHEN THE CARD OPENS (4.2 and 9.3a: a cold persona costs Granite about 2 s and Ministral
//! about 4 s, once per conversation): the installed brain is checked against the catalog (on its own
//! thread, models.rs), loaded with this computer's brain threads, and handed the persona with a short
//! greeting and a one-token reply, so the engine has read the persona before the player has finished
//! typing; the real question then reuses it (9.6) and its first words come in a fraction of a second.
//!
//! STREAMING: the reply's line is shown word by word as it arrives, AND THE FILTER JUDGES EVERY PIECE
//! (8.3): a line that breaks a rule is stopped mid-word, the request cancelled, and one of the
//! character's written lines is shown instead, with the reason in words.
//!
//! FALLBACK, ALWAYS IN WORDS: no engine, a crash, a refusal, a reply outside the fixed shape, an action
//! the game did not grant, the filter, or no reply in [`REPLY_TIMEOUT`] (or a reply that stops for
//! [`STALL_TIMEOUT`]): the character says one of its
//! written lines and the card says why. The written choices stay under the box, so the conversation
//! always has a way forward. Local only: nothing here touches the network (8.1).

use std::time::{Duration, Instant};

use humanity_ai_protocol::{ModelKind, Role, Turn};

use crate::ai::client::{AiClient, Outcome};
use crate::ai::models::Prepared;
use crate::ai::talk::{self, Action, Character, Filter, Grants, TalkCounts, COUNTS};
use crate::ai::{catalog, hardware, install, models, paths, shared};
use humanity_core::i18n::{t, t_dyn, tf};

/// How long getting the model ready may take before the box says it could not: checking a 2 GB file
/// (about 2 s in the release app) and loading it (2.2 s, 9.3a), with room for a busy computer.
/// Each of the two steps (the check and load; the warm-up's reading of the persona) has this long on its
/// own. The ai-talk rig's first run, with three other agents compiling, took 79 s for both together
/// with Ministral (2026-11-13), so a stalled step, not a slow one.
pub const READY_TIMEOUT: Duration = Duration::from_secs(150);

/// How long a reply may take once its question reached the engine: Ministral's whole 40-token reply
/// is about 3 s with the persona read (9.3a), so this is a stalled engine, not a slow one.
/// It is the time to the FIRST piece; after that, [`STALL_TIMEOUT`] between pieces, so a slow computer
/// writing steadily is never cut off, while an engine that stops is (the rig's first run, under load,
/// cut a steady reply at a fixed 30 s, 2026-11-13).
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a reply that has started may go without a new piece.
pub const STALL_TIMEOUT: Duration = Duration::from_secs(15);

/// Settings > AI's "How characters talk" (`ai_talk_mode`): written lines only, even with a brain.
pub const TALK_WRITTEN: &str = "written";
/// ... or written lines and the "Say anything" box (the default; it shows only with a brain installed).
pub const TALK_CONVERSATIONAL: &str = "conversational";

/// The greeting the warm-up sends with the persona (its one-token reply is never shown).
const WARM_UP: &str = "Hello.";

/// Whether the box is offered on this card, and if not, why.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Availability {
    /// No AI model is installed: the card offers AI once (5.2 item 4).
    #[default]
    NoAi,
    /// A brain is installed, but the person chose Written lines in Settings > AI.
    WrittenChosen,
    /// A model is installed but not this app's engine version (an update waits in Settings > AI).
    EngineMissing,
    /// This client's data has no persona for the character (a crew member a server added).
    NoPersona,
    /// The box is offered.
    Offered,
}

/// What a card can offer, from what is installed and the person's choice (`ai_talk_mode`).
pub fn availability(talk_mode: &str, has_persona: bool) -> Availability {
    let dev_fake = shared::use_dev_fake();
    let installed = match (catalog::current(), paths::ai_root()) {
        (Ok(cat), Some(root)) => {
            let have = install::installed(&cat, &root);
            let brain = cat.models.iter().any(|m| m.kind == "brain" && have.models.contains_key(&m.id));
            Some((brain, have.engine.is_some()))
        }
        _ => None,
    };
    let (brain, engine) = installed.unwrap_or((false, false));
    if !dev_fake && !brain {
        return Availability::NoAi;
    }
    if talk_mode == TALK_WRITTEN {
        return Availability::WrittenChosen;
    }
    if !dev_fake && !engine {
        return Availability::EngineMissing;
    }
    if !has_persona {
        return Availability::NoPersona;
    }
    Availability::Offered
}

/// Where the box stands.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Phase {
    /// Not offered (see [`Availability`]).
    #[default]
    Off,
    /// The model is being checked or loaded.
    Preparing,
    /// The persona is being read (the warm-up request).
    Warming(u64),
    Ready,
    /// A reply is being written.
    Writing,
    /// The box cannot be used on this card now, in words (a model that no longer matches the catalog,
    /// an engine that stopped twice).
    Unavailable(String),
}

/// What the card shows as the character's line.
#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    /// A written line (a topic, a task taken, the way shown).
    Written(String),
    /// A line the AI wrote (tagged AI on the card), with the action it offers, if any.
    Ai { line: String, action: Option<Action> },
    /// The AI could not answer: one of the character's written lines, and why, in words.
    Fallback { line: String, why: String },
}

/// A question asked and not yet answered.
#[derive(Debug, Clone)]
pub struct Pending {
    pub question: String,
    pub asked: Instant,
    /// The engine request, once the question was sent (it waits while the model gets ready).
    pub request: Option<u64>,
    pub sent: Option<Instant>,
    pub first_piece: Option<Instant>,
    /// When the reply last grew (the stall clock).
    pub last_piece: Option<Instant>,
    last_len: usize,
}

/// The last reply's timing, for the card's small print, the Dev page and the ai-talk rig.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Timing {
    pub tokens: u32,
    pub ms_prefill: u32,
    pub ms_generate: u32,
    /// From the question reaching the engine to its first words on the card.
    pub ms_first_words: u32,
    /// From the question reaching the engine to the whole reply.
    pub ms_reply: u32,
}

/// The conversation (memory only, gone when the card closes, 8.1).
#[derive(Default)]
pub struct Chat {
    pub availability: Availability,
    pub phase: Phase,
    /// The text in the box.
    pub input: String,
    /// The box has the keyboard: E types a letter instead of cycling the written lines.
    pub typing: bool,
    pub turns: Vec<Turn>,
    pub persona: String,
    pub grammar: String,
    pub grants: Grants,
    /// The model this card talks to (its catalog id), and its name for the card.
    pub model: String,
    pub model_name: String,
    pub pending: Option<Pending>,
    /// The reply's line so far, as it streams.
    pub partial: String,
    pub last: Option<Timing>,
    /// The character's written lines, for a fallback.
    pub fallback_lines: Vec<String>,
    /// Requests whose outcome is to be dropped when it arrives (a cancelled reply, the warm-up after the
    /// card moved on).
    discard: Vec<u64>,
    began: Option<Instant>,
    /// The filter of 8.3 (data/ai/filter.ron), read when the card opens.
    pub filter: Option<Filter>,
    /// Talk to the developer's fake engine (Dev > AI's choice for this run) instead of an installed
    /// model; read when the card opens.
    pub dev_fake: bool,
    fallbacks: usize,
}

impl Chat {
    /// Start a card's conversation: offered or not, and when offered the persona, the grammar and the
    /// grants, and the model's preparation begins (the warm-up).
    pub fn begin(&mut self, availability: Availability, character: Option<&Character>, grants: Grants, fallback_lines: Vec<String>) {
        *self = Chat { availability, fallback_lines, dev_fake: shared::use_dev_fake(), ..Chat::default() };
        if availability != Availability::Offered {
            return;
        }
        let Some(c) = character else {
            self.availability = Availability::NoPersona;
            return;
        };
        self.persona = talk::persona(&talk::Prompt::character(), c, &grants);
        self.grammar = talk::grammar(&grants);
        self.grants = grants;
        self.filter = Some(Filter::current());
        self.phase = Phase::Preparing;
        self.began = Some(Instant::now());
    }

    pub fn offered(&self) -> bool {
        self.availability == Availability::Offered
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Ask a question (the box's Enter or Ask). It waits while the model gets ready.
    pub fn ask(&mut self, question: &str) {
        let q = question.trim();
        if q.is_empty() || !self.offered() || self.pending.is_some() || matches!(self.phase, Phase::Unavailable(_)) {
            return;
        }
        TalkCounts::bump(&COUNTS.asked);
        self.partial.clear();
        self.pending = Some(Pending { question: q.chars().take(500).collect(), asked: Instant::now(), request: None, sent: None, first_piece: None, last_piece: None, last_len: 0 });
    }

    /// Stop the reply being written (the card's Stop, or the card closing): cancelled in the engine,
    /// its outcome dropped when it comes.
    pub fn cancel(&mut self, c: &mut AiClient) {
        if let Some(p) = self.pending.take() {
            if let Some(r) = p.request {
                c.cancel(r);
                self.discard.push(r);
            }
        }
        if let Phase::Warming(r) = self.phase {
            self.discard.push(r);
        }
        self.partial.clear();
        if matches!(self.phase, Phase::Writing | Phase::Warming(_)) {
            self.phase = Phase::Ready;
        }
    }

    /// Everything the box does each frame the card is open: get the model ready, send a question that
    /// waited, read the reply as it streams (the filter on every piece), notice a timeout, and finish a
    /// reply. Returns a line to show when one is finished (an AI line, or a fallback with its reason).
    pub fn step(&mut self, c: &mut AiClient, prefer: &str) -> Option<Said> {
        for r in std::mem::take(&mut self.discard) {
            if c.take_outcome(r).is_none() && c.partial(r).is_some() {
                self.discard.push(r); // still in the engine: drop it when it ends
            }
        }
        match self.phase.clone() {
            Phase::Off | Phase::Unavailable(_) => None,
            Phase::Preparing => {
                self.prepare(c, prefer);
                None
            }
            Phase::Warming(r) => {
                if self.began.is_some_and(|b| b.elapsed() > READY_TIMEOUT) {
                    self.discard.push(r);
                    self.phase = Phase::Unavailable(t("talk.ai.ready_timeout").to_string());
                    if self.pending.is_some() {
                        TalkCounts::bump(&COUNTS.timed_out);
                        return Some(self.fall_back(t("talk.ai.timeout")));
                    }
                    return None;
                }
                match c.take_outcome(r) {
                    Some(Outcome::Answered { .. }) | Some(Outcome::Fallback { why: crate::ai::client::Fallback::Cancelled, .. }) => self.phase = Phase::Ready,
                    Some(Outcome::Fallback { why, .. }) => {
                        self.phase = Phase::Unavailable(tf("talk.ai.unavailable", &[("why", &t_dyn(why.words_key()))]));
                        if self.pending.is_some() {
                            return Some(self.fall_back(&t_dyn(why.words_key())));
                        }
                    }
                    None => {}
                }
                None
            }
            Phase::Ready => {
                if self.pending.as_ref().is_some_and(|p| p.request.is_none()) {
                    self.send(c);
                }
                None
            }
            Phase::Writing => self.read(c),
        }
    }

    fn prepare(&mut self, c: &mut AiClient, prefer: &str) {
        if self.began.is_some_and(|b| b.elapsed() > READY_TIMEOUT) {
            self.phase = Phase::Unavailable(t("talk.ai.ready_timeout").to_string());
            return;
        }
        if self.dev_fake {
            if !c.is_loaded("fake") {
                c.load("fake", "(fake)", "", ModelKind::Brain, 4096, 1);
            }
            self.model = "fake".into();
            self.model_name = t("talk.ai.fake_model").to_string();
            self.warm(c);
            return;
        }
        let (Ok(cat), Some(root)) = (catalog::current(), paths::ai_root()) else {
            self.phase = Phase::Unavailable(t("talk.ai.none").to_string());
            return;
        };
        match models::prepare_brain(&cat, &root, prefer) {
            Prepared::NotInstalled => self.phase = Phase::Unavailable(t("talk.ai.none").to_string()),
            Prepared::Checking => {}
            Prepared::Mismatch(why) => self.phase = Phase::Unavailable(tf("talk.ai.mismatch", &[("detail", &why)])),
            Prepared::Ready(ready) => {
                // One brain at a time (9.1): another one chosen since is let go before this one loads.
                for other in c.loaded_ids().into_iter().filter(|id| *id != ready.entry.id) {
                    c.unload(&other);
                }
                if !c.is_loaded(&ready.entry.id) {
                    let hw = hardware::read();
                    let threads = install::brain_threads(hw.cores, hw.threads);
                    c.set_memory_cap(models::memory_cap_mb(&ready.entry));
                    c.load(&ready.entry.id, &ready.path.display().to_string(), &ready.entry.sha256, ModelKind::Brain, ready.entry.context, threads);
                }
                self.model = ready.entry.id.clone();
                self.model_name = ready.entry.model.clone();
                self.warm(c);
            }
        }
    }

    /// Hand the engine the persona now, with a one-token reply nobody sees.
    fn warm(&mut self, c: &mut AiClient) {
        self.began = Some(Instant::now()); // the warm-up has its own clock
        let r = c.generate_turns(&self.model, &self.persona, vec![Turn { role: Role::User, text: WARM_UP.into() }], Some(self.grammar.clone()), 1);
        self.phase = Phase::Warming(r);
        shared::watch();
    }

    fn send(&mut self, c: &mut AiClient) {
        let Some(p) = self.pending.as_mut() else { return };
        let start = self.turns.len().saturating_sub(talk::HISTORY_TURNS);
        let mut turns: Vec<Turn> = self.turns[start..].to_vec();
        turns.push(Turn { role: Role::User, text: p.question.clone() });
        let r = c.generate_turns(&self.model, &self.persona, turns, Some(self.grammar.clone()), talk::MAX_TOKENS);
        p.request = Some(r);
        p.sent = Some(Instant::now());
        self.phase = Phase::Writing;
        shared::watch();
    }

    fn read(&mut self, c: &mut AiClient) -> Option<Said> {
        let p = self.pending.clone()?;
        let r = p.request?;
        if let Some(text) = c.partial(r) {
            let grew = text.len() > p.last_len;
            let line = talk::streaming_line(text).to_string();
            if let Some(pp) = self.pending.as_mut() {
                if grew {
                    pp.last_piece = Some(Instant::now());
                    pp.last_len = text.len();
                }
                if !line.is_empty() && pp.first_piece.is_none() {
                    pp.first_piece = Some(Instant::now());
                }
            }
            if let Some(hit) = self.filter.as_ref().and_then(|f| f.check(&line).err()) {
                c.cancel(r);
                self.discard.push(r);
                TalkCounts::bump(&COUNTS.filtered);
                return Some(self.fall_back(&t_dyn(&format!("ai.talk.filter.{}", hit.why))));
            }
            self.partial = line;
        }
        let p = self.pending.clone()?;
        let stalled = match p.last_piece {
            Some(last) => last.elapsed() > STALL_TIMEOUT,
            None => p.sent.is_some_and(|s| s.elapsed() > REPLY_TIMEOUT),
        };
        if stalled {
            c.cancel(r);
            self.discard.push(r);
            TalkCounts::bump(&COUNTS.timed_out);
            return Some(self.fall_back(t("talk.ai.timeout")));
        }
        match c.take_outcome(r)? {
            Outcome::Answered { text, tokens, ms_prefill, ms_generate, .. } => {
                let sent = p.sent.unwrap_or(p.asked);
                self.last = Some(Timing {
                    tokens,
                    ms_prefill,
                    ms_generate,
                    ms_first_words: p.first_piece.map(|f| f.duration_since(sent).as_millis() as u32).unwrap_or(0),
                    ms_reply: sent.elapsed().as_millis() as u32,
                });
                Some(self.finish(&text))
            }
            Outcome::Fallback { why, .. } => {
                TalkCounts::bump(&COUNTS.fell_back);
                Some(self.fall_back(&t_dyn(why.words_key())))
            }
        }
    }

    /// A whole reply: the shape and the grants, then the filter, then it is the character's line.
    pub fn finish(&mut self, raw: &str) -> Said {
        let reply = match talk::parse_reply(raw, &self.grants) {
            Ok(r) => r,
            Err(e) => {
                TalkCounts::bump(&COUNTS.refused_shape);
                log::info!("[AI] a character's reply was refused ({e:?}): {raw:?}");
                return self.fall_back(&t_dyn(e.words_key()));
            }
        };
        if let Some(hit) = self.filter.as_ref().and_then(|f| f.check(&reply.line).err()) {
            TalkCounts::bump(&COUNTS.filtered);
            return self.fall_back(&t_dyn(&format!("ai.talk.filter.{}", hit.why)));
        }
        TalkCounts::bump(&COUNTS.answered);
        let question = self.pending.take().map(|p| p.question).unwrap_or_default();
        self.turns.push(Turn { role: Role::User, text: question });
        self.turns.push(Turn { role: Role::Assistant, text: raw.trim().to_string() });
        self.partial.clear();
        self.phase = Phase::Ready;
        Said::Ai { line: reply.line, action: reply.action }
    }

    /// One of the character's written lines instead, with why in words; the question and the line join
    /// the conversation so the next answer follows on.
    pub fn fall_back(&mut self, why: &str) -> Said {
        let line = if self.fallback_lines.is_empty() {
            t("talk.ai.no_written_line").to_string()
        } else {
            self.fallbacks += 1;
            self.fallback_lines[(self.fallbacks + self.turns.len()) % self.fallback_lines.len()].clone()
        };
        if let Some(p) = self.pending.take() {
            self.turns.push(Turn { role: Role::User, text: p.question });
            let safe: String = line.chars().filter(|c| *c != '"' && *c != '\\' && !c.is_control()).collect();
            self.turns.push(Turn { role: Role::Assistant, text: format!("{{\"action\":\"none\",\"line\":\"{safe}\"}}") });
        }
        self.partial.clear();
        if !matches!(self.phase, Phase::Unavailable(_)) {
            self.phase = Phase::Ready;
        }
        Said::Fallback { line, why: why.to_string() }
    }
}
