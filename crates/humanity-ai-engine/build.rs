//! The AI engine's build script (docs/design/embeddable-ai.md, 7.1; rung 2, 2026-11-09).
//!
//! Without the `llama` feature it does nothing: the engine is pure Rust (its `--fake` mode, which the
//! app's client tests build), and no CMake or C++ compiler is needed.
//!
//! With `llama` it builds llama.cpp from the pinned source `just ai-engine` hands it
//! (HUMANITY_LLAMA_SRC, found by scripts/ai-engine.js in the crate llama-src/Cargo.toml pins), then
//! the engine's small C door (src/llama/shim.c), and links them:
//!
//! - **CPU only** (rung 2). Vulkan needs the Vulkan SDK's shader compiler to build, which this PC does
//!   not have; it comes with a later rung (the design's question 2: the GPU only outside the 3D world).
//! - **Portable kernels**: llama.cpp's own way, `GGML_BACKEND_DL` with `GGML_CPU_ALL_VARIANTS`: one
//!   `ggml-cpu-<level>.dll` per processor generation (the plain x64 baseline, SSE 4.2, Sandy Bridge's
//!   AVX, Haswell's AVX2, Skylake-X's AVX-512 and so on), and at start ggml loads the best one this
//!   processor runs (src/llama/shim.c, `hai_init`). So a processor without AVX2 still runs it, slowly,
//!   as 4.7's notice says, instead of crashing on an instruction it lacks.
//! - **No OpenMP** (its runtime, vcomp140.dll, is a separate part of the Visual C++ runtime).
//! - **The Microsoft Visual C++ runtime as a shared library** (`/MD`: MSVCP140.dll, VCRUNTIME140.dll,
//!   VCRUNTIME140_1.dll), the one HumanityOS.exe itself already needs (VCRUNTIME140.dll is in its
//!   imports, `dumpbin /dependents`, 2026-11-09), so a PC that runs the app has it. Linking it into
//!   each library instead (`/MT`) was tried first and FAILED: every library then carries its own copy of
//!   the C++ runtime, and the first model load aborted (exit 0xc0000409) before llama.cpp logged a line,
//!   the known hazard of a C++ exception or object crossing between libraries that each have their own
//!   runtime. The engine program itself links its C runtime in (+crt-static, scripts/ai-engine.js); it
//!   talks to the libraries only through C functions with plain values (src/llama/shim.c), so the two
//!   runtimes never share an object.
//! - **Nothing from llama.cpp's tools, tests, server or `common`**: only libllama and ggml.
//! - The libraries are copied beside every program cargo writes (the profile folder and its `deps/`,
//!   where the test programs run), because Windows looks for a program's libraries in its own folder.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "llama")]
    llama::build();
}

#[cfg(feature = "llama")]
mod llama {
    use std::path::{Path, PathBuf};

    /// Stop the build with words a person can act on.
    fn refuse(words: &str) -> ! {
        panic!("\n\nTHE AI ENGINE'S llama FEATURE CANNOT BUILD: {words}\nBuild it with `just ai-engine` (scripts/ai-engine.js), which finds the pinned llama.cpp source and CMake.\n");
    }

    pub fn build() {
        println!("cargo:rerun-if-env-changed=HUMANITY_LLAMA_SRC");
        println!("cargo:rerun-if-changed=src/llama/shim.c");
        let src = match std::env::var_os("HUMANITY_LLAMA_SRC") {
            Some(s) => PathBuf::from(s),
            None => refuse("HUMANITY_LLAMA_SRC is not set (the llama.cpp folder of the crate llama-src/Cargo.toml pins)."),
        };
        for needed in ["CMakeLists.txt", "LICENSE", "include/llama.h", "ggml/include/ggml-backend.h"] {
            if !src.join(needed).is_file() {
                refuse(&format!("{} has no {needed}: it is not a llama.cpp source folder.", src.display()));
            }
        }
        let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

        let mut cfg = cmake::Config::new(&src);
        cfg.profile("Release")
            .define("BUILD_SHARED_LIBS", "ON")
            .define("GGML_BACKEND_DL", "ON")
            .define("GGML_CPU_ALL_VARIANTS", "ON")
            .define("GGML_NATIVE", "OFF")
            .define("GGML_OPENMP", "OFF")
            .define("GGML_CCACHE", "OFF")
            .define("GGML_VULKAN", "OFF")
            .define("GGML_CUDA", "OFF")
            .define("GGML_BUILD_TESTS", "OFF")
            .define("GGML_BUILD_EXAMPLES", "OFF")
            .define("LLAMA_BUILD_COMMON", "OFF")
            .define("LLAMA_BUILD_TESTS", "OFF")
            .define("LLAMA_BUILD_TOOLS", "OFF")
            .define("LLAMA_BUILD_EXAMPLES", "OFF")
            .define("LLAMA_BUILD_SERVER", "OFF")
            .define("LLAMA_BUILD_APP", "OFF")
            .define("LLAMA_OPENSSL", "OFF")
            .define("LLAMA_ALL_WARNINGS", "OFF");
        if msvc {
            // The shared Visual C++ runtime (/MD; the top of this file says why not /MT); and the
            // source folder's path (the builder's user folder) trimmed from file names the code
            // embeds for its assertions, so the published libraries carry no one's user name.
            let trim = format!("/d1trimfile:{}\\", src.display());
            cfg.static_crt(false)
                .define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreadedDLL")
                .cflag(&trim)
                .cxxflag(&trim)
                // CMake's own default for these flags is "/machine:x64"; setting them replaces it, so
                // it is written again, then /Brepro: no time stamp in the libraries, so the same source
                // links to the same bytes.
                .define("CMAKE_SHARED_LINKER_FLAGS", "/machine:x64 /Brepro")
                .cflag("/Brepro")
                .cxxflag("/Brepro");
            // The optimiser, said again. With Visual Studio's generator the cmake crate writes its own
            // CMAKE_C_FLAGS_RELEASE without any /O flag (rust-lang/cmake-rs issue 240), so the
            // "Release" libraries came out UNOPTIMISED: Granite wrote 2.6 tokens a second on one thread
            // and 6 to 9 on four (2026-11-09), until these went in. llama-cpp-sys-2's build script does
            // the same for the same reason.
            for flag in ["/O2", "/Ob2", "/DNDEBUG"] {
                cfg.cflag(flag).cxxflag(flag);
            }
        }
        let out = cfg.build();
        let lib_dir = out.join("lib");
        let bin_dir = out.join("bin");

        // The engine's door into it, compiled with the engine program's own C runtime (/MT, matching
        // +crt-static): it calls llama.cpp only through C functions with plain values.
        let mut shim = cc::Build::new();
        shim.file("src/llama/shim.c").include(src.join("include")).include(src.join("ggml").join("include")).warnings(true);
        if msvc {
            shim.static_crt(true).flag("/Brepro");
        }
        shim.compile("hai_shim");

        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        for lib in ["llama", "ggml", "ggml-base"] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }

        // Beside every program cargo writes: OUT_DIR is <target>/<profile>/build/<pkg>-<hash>/out.
        let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
        let profile_dir = out_dir.ancestors().nth(3).map(Path::to_path_buf).unwrap_or_else(|| refuse("OUT_DIR is not where cargo puts it"));
        let libraries: Vec<PathBuf> = std::fs::read_dir(&bin_dir)
            .unwrap_or_else(|e| refuse(&format!("{} could not be read: {e}", bin_dir.display())))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll") || x.eq_ignore_ascii_case("so") || x.eq_ignore_ascii_case("dylib")))
            .collect();
        if !libraries.iter().any(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("ggml-cpu"))) {
            refuse(&format!("the build made no ggml-cpu library in {}", bin_dir.display()));
        }
        for dest in [profile_dir.clone(), profile_dir.join("deps"), profile_dir.join("examples")] {
            let _ = std::fs::create_dir_all(&dest);
            for lib in &libraries {
                let to = dest.join(lib.file_name().expect("a file name"));
                if let Err(e) = std::fs::copy(lib, &to) {
                    // A test program still running holds its libraries open; a stale copy beside it is
                    // replaced on the next build.
                    println!("cargo:warning=could not copy {} to {}: {e}", lib.display(), to.display());
                }
            }
        }
        // For the packaging script: where the libraries and llama.cpp's licence are.
        println!("cargo:rustc-env=HUMANITY_LLAMA_BIN={}", bin_dir.display());
        println!("cargo:rustc-env=HUMANITY_LLAMA_LICENSE={}", src.join("LICENSE").display());
    }
}
