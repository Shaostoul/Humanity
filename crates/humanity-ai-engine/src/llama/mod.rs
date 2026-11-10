//! llama.cpp inside the engine: the brains' runtime (docs/design/embeddable-ai.md, 7.1; rung 2,
//! 2026-11-09). Built only with the `llama` feature (build.rs says how); without it the engine has its
//! `--fake` mode and refuses every model request in words, as in rung 1.
//!
//! What it does, in the order a reply needs it:
//! 1. [`init`] once, after the app's first message (so it runs inside the job object's limits): the
//!    best CPU kernels for this processor, from the engine's own folder only.
//! 2. [`Brain::load`]: a GGUF file the app has already checked against the catalog's SHA-256, on the
//!    processor, with a working memory of `context` tokens and `threads` threads (9.2: the app sends
//!    at most physical cores minus two, never more than four). The job object's memory cap, raised by
//!    the app before the load, is what makes a load too big for this computer fail in words.
//! 3. [`Brain::prompt`]: the persona and the turns through the model's own chat template.
//! 4. [`Brain::prefill`]: read the prompt, keeping what the previous request already read when reuse
//!    is on (9.6, llama.cpp's prompt reuse: a persona read once per conversation) and nothing when it
//!    is off.
//! 5. [`Brain::next`] again and again: one token at a time, so the run loop can stream each piece, read
//!    a `cancel` between pieces, and stop at `max_tokens`.
//!
//! Every handle is freed when its owner drops. Nothing is written to disk: no saved key-value cache, no
//! kernel cache (9.6).

mod ffi;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::Path;

use humanity_ai_protocol::{Role, Turn};

/// How many prompt tokens are read in one go (the context's batch).
pub const BATCH: usize = 256;

/// The started runtime: the devices llama.cpp found (the chosen CPU kernels).
#[derive(Debug, Clone)]
pub struct Runtime {
    pub devices: Vec<String>,
    pub system_info: String,
}

fn c_text(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: the shim returns NUL-terminated strings owned by llama.cpp that live for the process.
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

/// Start llama.cpp with the CPU kernels in `own_folder` (the engine's). Removes GGML_BACKEND_PATH from
/// the engine's environment first: ggml would load whatever library that names (ggml-backend-reg.cpp,
/// "check the environment variable GGML_BACKEND_PATH to load an out-of-tree backend"), and the engine
/// loads libraries only from its own folder (7.2).
pub fn init(own_folder: &Path, verbose: bool) -> Result<Runtime, String> {
    std::env::remove_var("GGML_BACKEND_PATH");
    let dir = CString::new(own_folder.to_string_lossy().as_bytes()).map_err(|_| "the engine's folder has a NUL in its name".to_string())?;
    // SAFETY: a NUL-terminated path that lives across the call.
    let n = unsafe { ffi::hai_init(dir.as_ptr(), i32::from(verbose)) };
    // SAFETY: indexes below the count the shim returned; each name is llama.cpp's static text.
    let devices: Vec<String> = (0..n).map(|i| c_text(unsafe { ffi::hai_device_name(i) })).collect();
    if devices.is_empty() {
        return Err(format!("no CPU kernels could be loaded from {} (a ggml-cpu-*.dll beside the engine)", own_folder.display()));
    }
    // SAFETY: llama.cpp's static text.
    let system_info = c_text(unsafe { ffi::hai_system_info() });
    Ok(Runtime { devices, system_info })
}

/// Why a load failed: llama.cpp could not read the file, or there was not enough memory for its working
/// memory (the job object's cap, or the computer's).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadError {
    File(String),
    Memory(String),
}

/// A loaded language model and its working memory for one conversation.
pub struct Brain {
    model: *mut c_void,
    ctx: *mut c_void,
    /// The tokens the working memory holds now (the last prompt and its reply), for prompt reuse.
    cached: Vec<i32>,
    pub threads: u32,
    pub n_ctx: u32,
    /// "granite 1B Q4_K - Medium", for the log and the Dev page.
    pub desc: String,
    pub weights_bytes: u64,
    pub params: u64,
}

impl Drop for Brain {
    fn drop(&mut self) {
        // SAFETY: both handles came from the shim and are freed once, the context first.
        unsafe {
            ffi::hai_context_free(self.ctx);
            ffi::hai_model_free(self.model);
        }
    }
}

/// Chooses each next token (a grammar, if given, then the usual narrowing and a draw).
pub struct Sampler(*mut c_void);

impl Drop for Sampler {
    fn drop(&mut self) {
        // SAFETY: from the shim, freed once.
        unsafe { ffi::hai_sampler_free(self.0) }
    }
}

/// How each token is chosen: the planning defaults for a short, varied line (a character's reply);
/// `temp` 0 picks the likeliest token every time (repeatable, for tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sampling {
    pub temp: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub min_p: f32,
    pub seed: u32,
}

impl Default for Sampling {
    fn default() -> Self {
        Sampling { temp: 0.7, top_k: 40, top_p: 0.95, min_p: 0.05, seed: 0x4855_4d41 }
    }
}

impl Brain {
    /// Load `path` with a working memory of `n_ctx` tokens and `threads` threads.
    pub fn load(path: &str, n_ctx: u32, threads: u32) -> Result<Brain, LoadError> {
        if !Path::new(path).is_file() {
            return Err(LoadError::File(format!("{path} is not a file")));
        }
        let c_path = CString::new(path).map_err(|_| LoadError::File("the path has a NUL in it".into()))?;
        // SAFETY: a NUL-terminated path; NULL means it failed.
        let model = unsafe { ffi::hai_model_load(c_path.as_ptr()) };
        if model.is_null() {
            return Err(LoadError::File(format!("llama.cpp could not load {path} (not a GGUF it reads, damaged, or too little memory to read it)")));
        }
        let threads = threads.clamp(1, 64);
        // SAFETY: a live model handle; NULL means the working memory could not be made.
        let ctx = unsafe { ffi::hai_context_new(model, n_ctx, threads as i32, BATCH as u32) };
        if ctx.is_null() {
            // SAFETY: the model handle is ours and freed once.
            unsafe { ffi::hai_model_free(model) };
            return Err(LoadError::Memory(format!("there was not enough memory for a {n_ctx}-token working memory")));
        }
        let mut buf = vec![0u8; 256];
        // SAFETY: a live model and a buffer of the length given.
        let n = unsafe { ffi::hai_model_desc(model, buf.as_mut_ptr() as *mut c_char, buf.len() as i32) };
        buf.truncate(n.clamp(0, 255) as usize);
        // SAFETY: live handles.
        let (weights_bytes, params, real_ctx) = unsafe { (ffi::hai_model_size(model), ffi::hai_model_n_params(model), ffi::hai_context_n_ctx(ctx)) };
        Ok(Brain { model, ctx, cached: Vec::new(), threads, n_ctx: real_ctx, desc: String::from_utf8_lossy(&buf).into_owned(), weights_bytes, params })
    }

    /// The prompt: the persona as the system message, then the turns, through the model's own chat
    /// template, ending where the model's reply begins.
    pub fn prompt(&self, persona: &str, turns: &[Turn]) -> Result<String, String> {
        let mut roles = Vec::with_capacity(turns.len() + 1);
        let mut contents = Vec::with_capacity(turns.len() + 1);
        let nul = |s: &str| CString::new(s.replace('\0', " ")).expect("NULs replaced");
        if !persona.is_empty() {
            roles.push(nul("system"));
            contents.push(nul(persona));
        }
        for t in turns {
            roles.push(nul(match t.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            }));
            contents.push(nul(&t.text));
        }
        if roles.is_empty() {
            return Err("nothing to answer".into());
        }
        let role_ptrs: Vec<*const c_char> = roles.iter().map(|c| c.as_ptr()).collect();
        let content_ptrs: Vec<*const c_char> = contents.iter().map(|c| c.as_ptr()).collect();
        let mut size = 2 * contents.iter().map(|c| c.as_bytes().len()).sum::<usize>() + 512;
        for _ in 0..2 {
            let mut buf = vec![0u8; size];
            // SAFETY: parallel arrays of n NUL-terminated strings that outlive the call, and a buffer of
            // the length given.
            let n = unsafe { ffi::hai_chat_apply(self.model, role_ptrs.as_ptr(), content_ptrs.as_ptr(), role_ptrs.len() as i32, 1, buf.as_mut_ptr() as *mut c_char, size as i32) };
            if n < 0 {
                return Err(format!("the model's chat template could not be applied ({n})"));
            }
            if (n as usize) <= size {
                buf.truncate(n as usize);
                return Ok(String::from_utf8_lossy(&buf).into_owned());
            }
            size = n as usize;
        }
        Err("the chat template's length kept changing".into())
    }

    /// Text into tokens (special tokens in the text, the template's markers, read as such).
    pub fn tokenize(&self, text: &str) -> Result<Vec<i32>, String> {
        let len = i32::try_from(text.len()).map_err(|_| "the prompt is too long".to_string())?;
        let mut tokens = vec![0i32; text.len() / 2 + 16];
        for _ in 0..2 {
            // SAFETY: the text pointer and its length, and a token buffer of the length given.
            let n = unsafe { ffi::hai_tokenize(self.model, text.as_ptr() as *const c_char, len, tokens.as_mut_ptr(), tokens.len() as i32, 1, 1) };
            if n >= 0 {
                tokens.truncate(n as usize);
                return Ok(tokens);
            }
            tokens = vec![0i32; (-n) as usize];
        }
        Err("the prompt could not be read into tokens".into())
    }

    /// Read `tokens` into the working memory, keeping the part the last request already read when
    /// `reuse` is on. Calls `between` before each batch; when it says stop, the read ends (what was read
    /// stays, and is exactly what [`Brain::cached_tokens`] counts, so a later request reuses only the
    /// part it shares). Returns how many tokens were reused, or None when stopped.
    pub fn prefill(&mut self, tokens: &[i32], reuse: bool, mut between: impl FnMut() -> bool) -> Result<Option<usize>, String> {
        if tokens.is_empty() {
            return Err("an empty prompt".into());
        }
        if tokens.len() + 8 > self.n_ctx as usize {
            return Err(format!("the conversation ({} tokens) is longer than the model's working memory ({} tokens)", tokens.len(), self.n_ctx));
        }
        // At least the last token is read again, so there is a prediction to sample from.
        let mut keep = if reuse { common_prefix(&self.cached, tokens).min(tokens.len() - 1) } else { 0 };
        // SAFETY: a live context.
        if keep == 0 || unsafe { ffi::hai_memory_keep(self.ctx, keep as i32) } == 0 {
            // SAFETY: a live context.
            unsafe { ffi::hai_memory_clear(self.ctx) };
            keep = 0;
        }
        self.cached.truncate(keep);
        let mut rest: Vec<i32> = tokens[keep..].to_vec();
        for chunk in rest.chunks_mut(BATCH) {
            if between() {
                return Ok(None);
            }
            // SAFETY: a live context and a token buffer of the length given.
            let r = unsafe { ffi::hai_decode(self.ctx, chunk.as_mut_ptr(), chunk.len() as i32) };
            if r != 0 {
                self.forget();
                return Err(format!("llama.cpp could not read the prompt (decode returned {r})"));
            }
            self.cached.extend_from_slice(chunk);
        }
        Ok(Some(keep))
    }

    /// A sampler for one reply; `grammar` (GBNF) fixes the reply's shape. Err when the grammar does not
    /// parse.
    pub fn sampler(&self, s: Sampling, grammar: Option<&str>) -> Result<Sampler, String> {
        let g = grammar.filter(|g| !g.trim().is_empty()).map(|g| CString::new(g).map_err(|_| "the grammar has a NUL in it".to_string())).transpose()?;
        // SAFETY: a live model; the grammar (or NULL) lives across the call.
        let p = unsafe { ffi::hai_sampler_new(self.model, s.temp, s.top_k, s.top_p, s.min_p, s.seed, g.as_ref().map_or(std::ptr::null(), |c| c.as_ptr())) };
        if p.is_null() {
            return Err("the reply's grammar did not parse".into());
        }
        Ok(Sampler(p))
    }

    /// The next piece of the reply: its bytes (a piece can end inside a character: [`crate::text::Utf8Stream`]
    /// joins them), or None at the end of the reply. The chosen token is read into the working memory
    /// before it is returned, so the next call predicts the one after it.
    pub fn next(&mut self, sampler: &mut Sampler) -> Result<Option<Vec<u8>>, String> {
        // SAFETY: live handles.
        let token = unsafe { ffi::hai_sample(sampler.0, self.ctx) };
        // SAFETY: a live model.
        if unsafe { ffi::hai_is_eog(self.model, token) } != 0 {
            return Ok(None);
        }
        if self.cached.len() + 1 >= self.n_ctx as usize {
            return Ok(None);
        }
        let mut buf = vec![0u8; 64];
        // SAFETY: a live model and a buffer of the length given.
        let mut n = unsafe { ffi::hai_token_piece(self.model, token, buf.as_mut_ptr() as *mut c_char, buf.len() as i32) };
        if n < 0 {
            buf = vec![0u8; (-n) as usize];
            // SAFETY: as above, with the length llama.cpp asked for.
            n = unsafe { ffi::hai_token_piece(self.model, token, buf.as_mut_ptr() as *mut c_char, buf.len() as i32) };
        }
        buf.truncate(n.max(0) as usize);
        let mut one = [token];
        // SAFETY: a live context and a one-token buffer.
        let r = unsafe { ffi::hai_decode(self.ctx, one.as_mut_ptr(), 1) };
        if r != 0 {
            self.forget();
            return Err(format!("llama.cpp could not continue the reply (decode returned {r})"));
        }
        self.cached.push(token);
        Ok(Some(buf))
    }

    /// Forget the conversation (the reuse switch turned off, or a reply stopped half way).
    pub fn forget(&mut self) {
        // SAFETY: a live context.
        unsafe { ffi::hai_memory_clear(self.ctx) };
        self.cached.clear();
    }

    /// How many tokens the working memory holds now.
    pub fn cached_tokens(&self) -> usize {
        self.cached.len()
    }
}

/// How many tokens two prompts share from the start.
pub fn common_prefix(a: &[i32], b: &[i32]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}
