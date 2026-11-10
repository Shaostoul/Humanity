//! The AI engine's protocol, version 1 (docs/design/embeddable-ai.md, section 7.3; rung 1,
//! 2026-11-09).
//!
//! HumanityOS never runs an AI model inside its own process. When a person has chosen AI in
//! Settings > AI, the app starts a separate program, the AI engine (`humanity-ai.exe`,
//! crates/humanity-ai-engine/), with its standard input and output connected, and the two talk in
//! these messages: one JSON object per line, no socket, no listener, no network (7.2). This crate is
//! the one definition both sides compile, so a field can never mean one thing to the app and another
//! to the engine.
//!
//! EVERY MESSAGE CARRIES THE PROTOCOL VERSION, as `"v"`, beside its `"type"`:
//!
//! ```text
//! {"v":1,"type":"hello","app_version":"0.1553.0"}
//! {"v":1,"type":"hello","engine_version":"1.0.0","backends":[],"cpu_features":["avx2"],"fake":true}
//! ```
//!
//! A line whose version is not ours is refused with words ([`ProtocolError::Version`], read before
//! the rest of the line is even looked at), never guessed at: an engine from another app release may
//! mean something else by the same field (7.3, "a mismatched engine is refused with words, never
//! guessed at"). The words are keys in data/i18n/en.json (`ai.protocol.*`, `ai.error.*`): this crate
//! names the keys, the app shows the words.
//!
//! Serde types only. No model, no runtime, no file access, no network: the person who never chooses AI
//! still has this crate in their app, so it holds nothing an AI could run with.

use serde::{Deserialize, Serialize};

/// The protocol version this build speaks: the `v` of every message it writes, and the only `v` it
/// reads. Raise it whenever a message's meaning or shape changes (there is no compatibility code before
/// v1.0, CLAUDE.md: the app and the engine come from the same release, pinned by the catalog, 7.4).
pub const PROTOCOL: u32 = 1;

/// The longest line either side accepts, in bytes. The largest message the design plans is a label
/// photo (`read_image`, a few MB as base64) or a chunk of speech (48 KB a second); anything longer is a
/// broken or hostile peer, refused rather than buffered without end.
pub const MAX_LINE_BYTES: usize = 32 * 1024 * 1024;

/// What a brain's conversation turn is: the person's words or the model's reply.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The person (a player talking to a character, or asking the helper).
    User,
    /// What the model said before, in this conversation.
    Assistant,
}

/// One turn of a conversation, oldest first in [`AppMessage::Generate`]'s `turns`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub role: Role,
    pub text: String,
}

/// What kind of model a `load` names: which runtime inside the engine reads it (7.1).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// A language model (a GGUF file, llama.cpp): talking characters and the helper.
    Brain,
    /// A voice (a GGUF file, audio.cpp with Pocket TTS only): the computer voice.
    Voice,
    /// A small image model (ONNX, tract): the label reader's photo model.
    ImageReader,
}

/// Where a model runs: the processor, or the graphics card through Vulkan (any maker, 9.2: never
/// during play, the renderer keeps the GPU).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Device {
    Cpu,
    Vulkan,
}

/// A backend the engine was built with (its `hello`). A fake engine has none.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Cpu,
    Vulkan,
}

/// The messages the app sends the engine (the left column of 7.3's table).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppMessage {
    /// The first message after the engine starts. The table's `protocol` field is the `v` every
    /// message carries.
    Hello { app_version: String },
    /// Load a model the app has already downloaded and checked against the catalog (6.2): the app
    /// hands the engine a path and the hash it checked, never a place to fetch from.
    Load { id: String, path: String, sha256: String, kind: ModelKind, context: u32, threads: u32, device: Device },
    /// A character's line or a helper's answer. `persona` is the project's own prompt (8.2); `turns`
    /// the conversation so far; `grammar` the fixed output shape (8.3), when there is one.
    Generate { id: String, request: u64, persona: String, turns: Vec<Turn>, grammar: Option<String>, max_tokens: u32 },
    /// Speech for one sentence, in one of the documented voices (4.5).
    Speak { voice: String, text: String, request: u64 },
    /// The label reader: a PNG, as base64.
    ReadImage { id: String, request: u64, png: String },
    /// The person walked away: stop the request and say so.
    Cancel { request: u64 },
    /// The Dev page's "turn off all reuse" switch (9.6): with `enabled` false the engine keeps no
    /// prompt or voice state between requests.
    Reuse { enabled: bool },
    /// What the engine is doing, for the Dev page.
    Status,
    Unload { id: String },
    /// Finish and exit. The app also ends the engine through its job object when the app closes.
    Shutdown,
}

/// Why the engine refused something. Each has its words in data/i18n/en.json under `ai.error.*`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The message's protocol version is not the engine's.
    Version,
    /// The line was not a message of this protocol.
    BadMessage,
    /// This engine was built without the runtime the request needs (rung 1's engine has none).
    NoBackend,
    /// A request named a model that is not loaded.
    NotLoaded,
    /// There was not enough memory (the job object's cap, or the computer's).
    OutOfMemory,
    /// The model file could not be read.
    LoadFailed,
    /// Anything else the engine could not do.
    Internal,
}

impl ErrorCode {
    /// The key of this refusal's words in data/i18n/en.json.
    pub fn words_key(self) -> &'static str {
        match self {
            ErrorCode::Version => "ai.error.version",
            ErrorCode::BadMessage => "ai.error.bad_message",
            ErrorCode::NoBackend => "ai.error.no_backend",
            ErrorCode::NotLoaded => "ai.error.not_loaded",
            ErrorCode::OutOfMemory => "ai.error.out_of_memory",
            ErrorCode::LoadFailed => "ai.error.load_failed",
            ErrorCode::Internal => "ai.error.internal",
        }
    }

    /// Every code, for the test that each has its words.
    pub const ALL: [ErrorCode; 7] = [
        ErrorCode::Version,
        ErrorCode::BadMessage,
        ErrorCode::NoBackend,
        ErrorCode::NotLoaded,
        ErrorCode::OutOfMemory,
        ErrorCode::LoadFailed,
        ErrorCode::Internal,
    ];
}

/// A model the engine holds, in its `status`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LoadedModel {
    pub id: String,
    pub kind: ModelKind,
    pub memory_mb: u64,
}

/// The engine's running totals since it started, in its `status`.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct Counters {
    pub requests: u64,
    pub tokens: u64,
    pub cancelled: u64,
    pub errors: u64,
}

/// The messages the engine sends the app (the right column of 7.3's table).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineMessage {
    /// The answer to the app's `hello`. `fake` is true for `humanity-ai --fake`, the scripted engine
    /// every test and rig uses without a model (12.1).
    Hello { engine_version: String, backends: Vec<Backend>, cpu_features: Vec<String>, fake: bool },
    Loaded { id: String, memory_mb: u64 },
    /// A refusal. `request` names the request it answers, when it answers one; `words_key` is
    /// `code.words_key()`, sent so an app reading an engine log needs no table; `detail` is for the
    /// Dev page and the log, never shown as the reason by itself.
    Error { request: Option<u64>, code: ErrorCode, words_key: String, detail: String },
    /// A piece of a reply, as it is made (shown word by word, 4.2).
    Token { request: u64, text: String },
    /// A reply or a sentence of speech is finished: how many tokens, and how long the prompt reading
    /// (prefill) and the writing took (9.3's live measurement).
    Done { request: u64, tokens: u32, ms_prefill: u32, ms_generate: u32 },
    /// A chunk of speech: 24 kHz mono 16-bit little-endian samples, as base64.
    Pcm { request: u64, samples: String },
    /// What the label reader read.
    Text { request: u64, lines: Vec<String>, confidence: f32 },
    Cancelled { request: u64 },
    /// `to` names the message answered (`reuse`, `unload`, `shutdown`).
    Ok { to: String },
    /// The answer to `status`.
    Status { memory_mb: Option<u64>, threads: u32, loaded: Vec<LoadedModel>, counters: Counters, reuse: bool, fake: bool },
}

/// Why a line could not be read as a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The line speaks another protocol version: refused before anything else in it is read.
    Version { theirs: u32, ours: u32 },
    /// A JSON object without a `v`: not this protocol at all.
    NoVersion,
    /// Not a JSON object (or longer than [`MAX_LINE_BYTES`]).
    NotJson(String),
    /// Our version, but not a message it defines (an unknown `type`, a missing field).
    Shape(String),
}

impl ProtocolError {
    /// The key of the words that explain it, in data/i18n/en.json.
    pub fn words_key(&self) -> &'static str {
        match self {
            ProtocolError::Version { .. } => "ai.protocol.version_mismatch",
            ProtocolError::NoVersion => "ai.protocol.no_version",
            ProtocolError::NotJson(_) => "ai.protocol.not_json",
            ProtocolError::Shape(_) => "ai.protocol.shape",
        }
    }

    /// The values the words fill in (`{{theirs}}`, `{{ours}}`, `{{detail}}`).
    pub fn fill(&self) -> Vec<(&'static str, String)> {
        match self {
            ProtocolError::Version { theirs, ours } => vec![("theirs", theirs.to_string()), ("ours", ours.to_string())],
            ProtocolError::NoVersion => vec![],
            ProtocolError::NotJson(d) | ProtocolError::Shape(d) => vec![("detail", d.clone())],
        }
    }

    /// The matching refusal code, for an engine answering a line it could not read.
    pub fn code(&self) -> ErrorCode {
        match self {
            ProtocolError::Version { .. } => ErrorCode::Version,
            _ => ErrorCode::BadMessage,
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Version { theirs, ours } => write!(f, "protocol version {theirs}, expected {ours}"),
            ProtocolError::NoVersion => write!(f, "no protocol version (\"v\") in the message"),
            ProtocolError::NotJson(d) => write!(f, "not a JSON message: {d}"),
            ProtocolError::Shape(d) => write!(f, "not a message of protocol {PROTOCOL}: {d}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// One message as a line (no newline), its version first. `version` is [`PROTOCOL`] for every real
/// message; the fake engine's `--fake-protocol` passes another, to see the app refuse it.
pub fn encode_versioned<T: Serialize>(message: &T, version: u32) -> String {
    // Written as text, not through serde_json's Map: a Map orders its keys by name unless a feature
    // somewhere in the build says otherwise, and the version is to stand first in every line of a log
    // whichever way the build unified its features.
    match serde_json::to_string(message) {
        // Every message type here is an internally tagged enum, so it is always an object with at least
        // its "type"; anything else would be a bug in this file, caught by the round-trip tests.
        Ok(body) if body.starts_with('{') && body.len() > 2 => format!("{{\"v\":{version},{}", &body[1..]),
        Ok(body) => format!("{{\"v\":{version},\"value\":{body}}}"),
        Err(e) => format!("{{\"v\":{version},\"encode_error\":{}}}", serde_json::Value::from(e.to_string())),
    }
}

/// One message as a line at this build's [`PROTOCOL`].
pub fn encode<T: Serialize>(message: &T) -> String {
    encode_versioned(message, PROTOCOL)
}

/// Read one line: its version first (a mismatch is refused before the rest is read), then the message.
pub fn decode<T: serde::de::DeserializeOwned>(line: &str) -> Result<T, ProtocolError> {
    if line.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::NotJson(format!("a line of {} bytes, over the {} limit", line.len(), MAX_LINE_BYTES)));
    }
    let value: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| ProtocolError::NotJson(e.to_string()))?;
    let serde_json::Value::Object(mut fields) = value else {
        return Err(ProtocolError::NotJson("a JSON value that is not an object".into()));
    };
    let v = match fields.remove("v") {
        None => return Err(ProtocolError::NoVersion),
        Some(v) => v.as_u64().ok_or(ProtocolError::NoVersion)?,
    };
    if v != u64::from(PROTOCOL) {
        return Err(ProtocolError::Version { theirs: u32::try_from(v).unwrap_or(u32::MAX), ours: PROTOCOL });
    }
    serde_json::from_value(serde_json::Value::Object(fields)).map_err(|e| ProtocolError::Shape(e.to_string()))
}

/// A line from the engine.
pub fn decode_engine(line: &str) -> Result<EngineMessage, ProtocolError> {
    decode(line)
}

/// A line from the app.
pub fn decode_app(line: &str) -> Result<AppMessage, ProtocolError> {
    decode(line)
}

impl AppMessage {
    /// The message's `type`, as written on the wire (the engine's `ok {to}` names it back).
    pub fn kind(&self) -> &'static str {
        match self {
            AppMessage::Hello { .. } => "hello",
            AppMessage::Load { .. } => "load",
            AppMessage::Generate { .. } => "generate",
            AppMessage::Speak { .. } => "speak",
            AppMessage::ReadImage { .. } => "read_image",
            AppMessage::Cancel { .. } => "cancel",
            AppMessage::Reuse { .. } => "reuse",
            AppMessage::Status => "status",
            AppMessage::Unload { .. } => "unload",
            AppMessage::Shutdown => "shutdown",
        }
    }
}

#[cfg(test)]
mod tests;
