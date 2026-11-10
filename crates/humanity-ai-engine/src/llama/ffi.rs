//! The C functions of src/llama/shim.c, as Rust sees them. Plain integers, floats, byte pointers and
//! opaque handles only: no struct of llama.h crosses this line (the shim's header says why).

use std::ffi::{c_char, c_void};

extern "C" {
    pub fn hai_init(backend_dir: *const c_char, verbose: i32) -> i32;
    pub fn hai_device_name(i: i32) -> *const c_char;
    pub fn hai_system_info() -> *const c_char;

    pub fn hai_model_load(path: *const c_char) -> *mut c_void;
    pub fn hai_model_free(model: *mut c_void);
    pub fn hai_model_size(model: *const c_void) -> u64;
    pub fn hai_model_n_params(model: *const c_void) -> u64;
    pub fn hai_model_desc(model: *const c_void, buf: *mut c_char, len: i32) -> i32;

    pub fn hai_context_new(model: *mut c_void, n_ctx: u32, n_threads: i32, n_batch: u32) -> *mut c_void;
    pub fn hai_context_free(ctx: *mut c_void);
    pub fn hai_context_n_ctx(ctx: *const c_void) -> u32;

    pub fn hai_chat_apply(model: *const c_void, roles: *const *const c_char, contents: *const *const c_char, n: i32, add_assistant: i32, buf: *mut c_char, len: i32) -> i32;
    pub fn hai_tokenize(model: *const c_void, text: *const c_char, text_len: i32, tokens: *mut i32, n_max: i32, add_special: i32, parse_special: i32) -> i32;
    pub fn hai_token_piece(model: *const c_void, token: i32, buf: *mut c_char, len: i32) -> i32;
    pub fn hai_is_eog(model: *const c_void, token: i32) -> i32;

    pub fn hai_decode(ctx: *mut c_void, tokens: *mut i32, n: i32) -> i32;
    pub fn hai_memory_clear(ctx: *mut c_void);
    pub fn hai_memory_keep(ctx: *mut c_void, p0: i32) -> i32;

    pub fn hai_sampler_new(model: *const c_void, temp: f32, top_k: i32, top_p: f32, min_p: f32, seed: u32, grammar: *const c_char) -> *mut c_void;
    pub fn hai_sampler_free(smpl: *mut c_void);
    pub fn hai_sample(smpl: *mut c_void, ctx: *mut c_void) -> i32;
}
