//! `humanity-ai.exe`: the AI engine's program (src/lib.rs says what it is). The app starts it with its
//! standard input and output connected; nobody needs to run it by hand.

use humanity_ai_engine::args::{parse, Command};
use humanity_ai_engine::{folder, run, ENGINE_VERSION};

fn main() {
    let command = match parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("humanity-ai: {e}");
            std::process::exit(2);
        }
    };
    match command {
        Command::Version => println!("humanity-ai {ENGINE_VERSION} (protocol {})", humanity_ai_protocol::PROTOCOL),
        Command::Run(options) => {
            let restricted = folder::restrict_library_search();
            let own = folder::own_folder().map(|p| p.display().to_string()).unwrap_or_else(|| "?".into());
            let mode = if options.fake {
                "fake"
            } else if cfg!(feature = "llama") {
                "llama.cpp (processor)"
            } else {
                "no-model"
            };
            eprintln!(
                "humanity-ai {ENGINE_VERSION}: {mode} mode, protocol {}; libraries only from {own}{}",
                options.protocol,
                if restricted { " and System32" } else { "" }
            );
            let code = run(&options, std::io::BufReader::new(std::io::stdin()), std::io::stdout());
            std::process::exit(code);
        }
    }
}
