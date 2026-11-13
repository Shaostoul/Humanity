/*
 * The AI engine's one door into llama.cpp (docs/design/embeddable-ai.md, 7.1; rung 2, 2026-11-09).
 *
 * Every function here takes and returns plain integers, floats, byte buffers and opaque pointers, so
 * the Rust side (src/llama/ffi.rs) never copies one of llama.h's structs by hand: the C compiler reads
 * llama.h itself and lays out llama_model_params, llama_context_params and llama_batch exactly as the
 * pinned llama.cpp expects. That is why there is no bindgen (and no libclang) in the engine's build:
 * crates/humanity-ai-engine/llama-src/Cargo.toml says why.
 *
 * Only the C API of llama.cpp is used (include/llama.h and ggml-backend.h), never `common`, so nothing
 * from llama.cpp's vendor/ folder (HTTP, JSON, audio) is compiled into the engine.
 *
 * Names start with hai_ (HumanityOS AI). Each returns a negative number or NULL when it fails; the
 * Rust side turns that into words.
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "llama.h"
#include "ggml-backend.h"

/* llama.cpp's own log lines (hundreds while a model loads) would fill the app's log: the engine's
 * standard error goes there (src/ai/client.rs). Only warnings and errors pass, unless the engine was
 * started with --llama-log (every line, for a developer chasing a failure). A line that continues the
 * one before (GGML_LOG_LEVEL_CONT) follows the level of that line. */
static int hai_verbose = 0;
static int hai_last_shown = 0;

static void hai_log(enum ggml_log_level level, const char * text, void * user_data) {
    (void) user_data;
    int show;
    if (level == GGML_LOG_LEVEL_CONT) {
        show = hai_last_shown;
    } else {
        show = hai_verbose || level == GGML_LOG_LEVEL_WARN || level == GGML_LOG_LEVEL_ERROR;
        hai_last_shown = show;
    }
    if (show) {
        fputs(text, stderr);
        fflush(stderr);
    }
}

/* Start llama.cpp once: its log filter, then the CPU backends from the engine's own folder ONLY (never
 * the current folder, which is also the engine's, nor anything else): ggml loads every ggml-cpu-*.dll
 * there, asks each how well it fits this processor, and keeps the best (the x64 baseline, SSE 4.2,
 * AVX2, AVX-512 and so on: the portable kernels of 7.1). Returns how many devices it has afterwards. */
int32_t hai_init(const char * backend_dir, int32_t verbose) {
    hai_verbose = verbose != 0;
    llama_log_set(hai_log, NULL);
    ggml_backend_load_all_from_path(backend_dir);
    llama_backend_init();
    return (int32_t) ggml_backend_dev_count();
}

/* The name of device i (for the engine's log), or NULL. */
const char * hai_device_name(int32_t i) {
    if (i < 0 || (size_t) i >= ggml_backend_dev_count()) {
        return NULL;
    }
    return ggml_backend_dev_name(ggml_backend_dev_get((size_t) i));
}

/* What llama.cpp says about this processor and build (AVX2 = 1 and so on), for the engine's log. */
const char * hai_system_info(void) {
    return llama_print_system_info();
}

/* Load a GGUF file, on the processor only (no layer on a graphics card: 9.2, the renderer keeps the
 * GPU during play). The path is UTF-8; ggml opens it as a wide path on Windows. */
void * hai_model_load(const char * path) {
    struct llama_model_params p = llama_model_default_params();
    p.n_gpu_layers = 0;
    return llama_model_load_from_file(path, p);
}

void hai_model_free(void * model) {
    if (model) {
        llama_model_free((struct llama_model *) model);
    }
}

/* The size of the model's weights in bytes, and its parameter count. */
uint64_t hai_model_size(const void * model) {
    return llama_model_size((const struct llama_model *) model);
}

uint64_t hai_model_n_params(const void * model) {
    return llama_model_n_params((const struct llama_model *) model);
}

/* A short description ("granite 1B Q4_K - Medium"), written into buf. */
int32_t hai_model_desc(const void * model, char * buf, int32_t len) {
    if (len <= 0) {
        return -1;
    }
    return llama_model_desc((const struct llama_model *) model, buf, (size_t) len);
}

/* A context: the model's working memory for one conversation, n_ctx tokens long, using n_threads
 * threads for both reading the prompt and writing the reply. The key-value cache stays at llama.cpp's
 * default precision (16-bit). */
void * hai_context_new(void * model, uint32_t n_ctx, int32_t n_threads, uint32_t n_batch) {
    struct llama_context_params p = llama_context_default_params();
    p.n_ctx = n_ctx;
    p.n_batch = n_batch;
    p.n_ubatch = n_batch < 512 ? n_batch : 512;
    p.n_seq_max = 1;
    p.n_threads = n_threads;
    p.n_threads_batch = n_threads;
    p.no_perf = true;
    return llama_init_from_model((struct llama_model *) model, p);
}

void hai_context_free(void * ctx) {
    if (ctx) {
        llama_free((struct llama_context *) ctx);
    }
}

uint32_t hai_context_n_ctx(const void * ctx) {
    return llama_n_ctx((const struct llama_context *) ctx);
}

/* The template to format with: the model's own, except where llama.cpp's detection reads it as a
 * shape it is not (rung 3, 2026-11-12, embeddable-ai.md section 14). Ministral 3's template (a
 * Mistral template with [SYSTEM_PROMPT]) is detected as `mistral-v7`, which writes a space after each
 * marker ("[INST] text"); the model's own template writes none, the `mistral-v7-tekken` shape, which
 * is what a model with the Tekken tokenizer (the GGUF's `tokenizer.ggml.pre` is "tekken") was trained
 * on. llama.cpp names that shape but does not choose it from the template text alone, so it is chosen
 * here, by the same test llama.cpp's detection makes (src/llama-chat.cpp, llm_chat_detect_template)
 * plus the tokenizer. Every other model keeps its own template untouched. */
static const char * hai_template_for(const struct llama_model * model, const char * tmpl) {
    int mistral = strncmp(tmpl, "mistral", 7) == 0 || strstr(tmpl, "[INST]") != NULL;
    if (mistral && strstr(tmpl, "[SYSTEM_PROMPT]") != NULL) {
        char pre[32] = {0};
        if (llama_model_meta_val_str(model, "tokenizer.ggml.pre", pre, sizeof pre) > 0 && strcmp(pre, "tekken") == 0) {
            return "mistral-v7-tekken";
        }
    }
    return tmpl;
}

/* A conversation through the model's own chat template (in the GGUF; Granite 4.0's is built into
 * llama.cpp), or the named shape hai_template_for chooses for it. roles[i] is "system", "user" or
 * "assistant". Returns the prompt's length in bytes; when that is more than len, call again with a
 * buffer that large. */
int32_t hai_chat_apply(const void * model, const char ** roles, const char ** contents, int32_t n, int32_t add_assistant, char * buf, int32_t len) {
    if (n <= 0 || n > 512) {
        return -1;
    }
    const char * own = llama_model_chat_template((const struct llama_model *) model, NULL);
    if (!own) {
        return -2;
    }
    const char * tmpl = hai_template_for((const struct llama_model *) model, own);
    llama_chat_message msgs[512];
    for (int32_t i = 0; i < n; i++) {
        msgs[i].role = roles[i];
        msgs[i].content = contents[i];
    }
    return llama_chat_apply_template(tmpl, msgs, (size_t) n, add_assistant != 0, buf, len);
}

/* Text into tokens. Returns how many; when more than n_max, a negative count (call again). */
int32_t hai_tokenize(const void * model, const char * text, int32_t text_len, int32_t * tokens, int32_t n_max, int32_t add_special, int32_t parse_special) {
    const struct llama_vocab * vocab = llama_model_get_vocab((const struct llama_model *) model);
    return llama_tokenize(vocab, text, text_len, tokens, n_max, add_special != 0, parse_special != 0);
}

/* One token's text (special tokens not drawn). Returns the byte count, negative when buf is short. */
int32_t hai_token_piece(const void * model, int32_t token, char * buf, int32_t len) {
    const struct llama_vocab * vocab = llama_model_get_vocab((const struct llama_model *) model);
    return llama_token_to_piece(vocab, token, buf, len, 0, false);
}

/* The token ends the reply (end of text, end of turn). */
int32_t hai_is_eog(const void * model, int32_t token) {
    const struct llama_vocab * vocab = llama_model_get_vocab((const struct llama_model *) model);
    return llama_vocab_is_eog(vocab, token) ? 1 : 0;
}

/* Read n tokens into the working memory, after what is already there (positions are tracked by
 * llama.cpp); the next sample reads the last one's prediction. 0 on success. */
int32_t hai_decode(void * ctx, int32_t * tokens, int32_t n) {
    return llama_decode((struct llama_context *) ctx, llama_batch_get_one(tokens, n));
}

/* Forget the whole conversation. */
void hai_memory_clear(void * ctx) {
    llama_memory_clear(llama_get_memory((struct llama_context *) ctx), true);
}

/* Forget everything from position p0 on, keeping what came before (prompt reuse, 9.6). 1 if it could;
 * 0 when this kind of model cannot drop part of a conversation (then clear it and read it all). */
int32_t hai_memory_keep(void * ctx, int32_t p0) {
    return llama_memory_seq_rm(llama_get_memory((struct llama_context *) ctx), 0, p0, -1) ? 1 : 0;
}

/* How the next token is chosen: an optional fixed shape (a GBNF grammar, 8.3's "fixed output shape"),
 * then the usual narrowing (top-k, top-p, min-p), the temperature, and a draw with this seed; or the
 * single likeliest token when temp <= 0 (for repeatable tests). NULL when the grammar does not parse. */
void * hai_sampler_new(const void * model, float temp, int32_t top_k, float top_p, float min_p, uint32_t seed, const char * grammar) {
    struct llama_sampler_chain_params cp = llama_sampler_chain_default_params();
    cp.no_perf = true;
    struct llama_sampler * chain = llama_sampler_chain_init(cp);
    if (grammar && grammar[0]) {
        const struct llama_vocab * vocab = llama_model_get_vocab((const struct llama_model *) model);
        struct llama_sampler * g = llama_sampler_init_grammar(vocab, grammar, "root");
        if (!g) {
            llama_sampler_free(chain);
            return NULL;
        }
        llama_sampler_chain_add(chain, g);
    }
    if (temp <= 0.0f) {
        llama_sampler_chain_add(chain, llama_sampler_init_greedy());
    } else {
        llama_sampler_chain_add(chain, llama_sampler_init_top_k(top_k));
        llama_sampler_chain_add(chain, llama_sampler_init_top_p(top_p, 1));
        llama_sampler_chain_add(chain, llama_sampler_init_min_p(min_p, 1));
        llama_sampler_chain_add(chain, llama_sampler_init_temp(temp));
        llama_sampler_chain_add(chain, llama_sampler_init_dist(seed));
    }
    return chain;
}

void hai_sampler_free(void * smpl) {
    if (smpl) {
        llama_sampler_free((struct llama_sampler *) smpl);
    }
}

/* Choose the next token from the last decoded position (and let the grammar, if any, note it). */
int32_t hai_sample(void * smpl, void * ctx) {
    return llama_sampler_sample((struct llama_sampler *) smpl, (struct llama_context *) ctx, -1);
}
