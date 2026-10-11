//! What a file sent into the chat is made of on its way out, kept to ONE copy in memory (round 3
//! of docs/design/media-showcase.md, the 2026-10-11 review's finding 8). Sending a 1 GB private file
//! used to read the whole file on the UI thread (the app stopped drawing until it was read), then
//! hold the plaintext, the ciphertext and the upload's own copy (preamble + ciphertext + epilogue)
//! at once: about 3 GB.
//!
//! Now:
//! - a picked file is a [`Body::File`], its path and size: only the worker reads it ([`read`]);
//! - encryption happens in the same buffer (`dm_pq::encrypt_attachment_in_place`);
//! - the multipart request streams the framing around that buffer ([`Multipart`]), its length set,
//!   instead of copying all three into one.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use std::io::Read;

/// A file's bytes, or where to read them on the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Body {
    /// Already in memory (a pasted image, which the app encoded itself).
    Bytes(Vec<u8>),
    /// A picked file, read by the worker, not by the frame: its path and its size when picked.
    File { path: std::path::PathBuf, len: u64 },
}

impl Body {
    /// Its size, as known before it is read.
    pub(crate) fn len(&self) -> u64 {
        match self {
            Body::Bytes(b) => b.len() as u64,
            Body::File { len, .. } => *len,
        }
    }
}

/// The bytes of `body`, at most `limit` of them: a file that grew past it since it was picked is
/// refused, not read whole. On the worker.
pub(crate) fn read(body: Body, limit: u64) -> Result<Vec<u8>, String> {
    match body {
        Body::Bytes(b) => Ok(b),
        Body::File { path, len } => {
            let f = std::fs::File::open(&path).map_err(|e| format!("the file could not be read ({e})"))?;
            let mut bytes = Vec::with_capacity(len.min(limit) as usize + 16); // room for the tag
            f.take(limit + 1).read_to_end(&mut bytes).map_err(|e| format!("the file could not be read ({e})"))?;
            if bytes.len() as u64 > limit {
                return Err(format!("the file is over the {limit} limit"));
            }
            Ok(bytes)
        }
    }
}

/// One multipart upload's body, streamed: the framing before, the file's bytes (the very buffer
/// handed in, never copied), and the framing after.
pub(crate) struct Multipart {
    pub boundary: String,
    pub before: Vec<u8>,
    pub file: std::io::Cursor<Vec<u8>>,
    pub after: Vec<u8>,
}

impl Multipart {
    /// The body for `bytes` named `filename` of type `mime`, under a fresh boundary.
    pub(crate) fn new(filename: &str, mime: &str, bytes: Vec<u8>) -> Self {
        let boundary = format!(
            "HumanityOSBoundary{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
        );
        // Quotes and line breaks would break the framing.
        let safe_name: String = filename.chars().map(|c| if c == '"' || c == '\r' || c == '\n' { '_' } else { c }).collect();
        let before = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe_name}\"\r\nContent-Type: {mime}\r\n\r\n").into_bytes();
        let after = format!("\r\n--{boundary}--\r\n").into_bytes();
        Multipart { boundary, before, file: std::io::Cursor::new(bytes), after }
    }

    /// The whole body's length, for `Content-Length` (so it is sent as it is, not chunked).
    pub(crate) fn len(&self) -> u64 {
        (self.before.len() + self.file.get_ref().len() + self.after.len()) as u64
    }

    /// The body as one stream, for `ureq::Request::send`.
    pub(crate) fn reader(self) -> impl Read {
        std::io::Cursor::new(self.before).chain(self.file).chain(std::io::Cursor::new(self.after))
    }
}
