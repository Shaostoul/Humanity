//! Joining a model's pieces into whole characters (rung 2, 2026-11-09).
//!
//! A language model writes in tokens, and a token's bytes can stop half way through a character
//! (one Hindi letter, an accented "é" or an emoji can take two tokens). Each `token` message the engine
//! sends must be text a person can read, so [`Utf8Stream`] holds back the bytes of a character not yet
//! finished and sends them with the piece that finishes it. Bytes that can never be a character are
//! sent as U+FFFD (the replacement mark), never dropped silently and never as broken text.

/// Bytes in, whole characters out.
#[derive(Debug, Default)]
pub struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    /// Add a piece's bytes; returns the text that is now complete (maybe empty).
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    return out;
                }
                Err(e) => {
                    let good = e.valid_up_to();
                    // The first `good` bytes were just checked to be UTF-8.
                    out.push_str(std::str::from_utf8(&self.pending[..good]).expect("checked"));
                    match e.error_len() {
                        // An unfinished character at the end: keep it for the next piece.
                        None => {
                            self.pending.drain(..good);
                            return out;
                        }
                        // Bytes that can never start a character: one mark, then go on.
                        Some(bad) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..good + bad);
                        }
                    }
                }
            }
        }
    }

    /// The end of the reply: whatever is still held, as text (an unfinished character becomes U+FFFD).
    pub fn finish(&mut self) -> String {
        let rest = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        rest
    }
}
