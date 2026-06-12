//! HumanityOS — One binary to rule them all.
//!
//! Modes:
//!   Default (native feature):  Full desktop app (renderer + relay + game)
//!   --headless:                Server-only mode (relay, no GPU)
//!
//! On desktop startup, checks for a newer versioned exe and delegates to it.

// Hide the console window on Windows release builds (only for GUI mode)
#![cfg_attr(all(not(debug_assertions), feature = "native"), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let headless = args.iter().any(|a| a == "--headless");

    // ── Operator-only release-signing tooling (native builds). Runs + exits
    //    before any window/relay startup AND before find_newer_exe delegation.
    //    The private key is read from an encrypted vault via the
    //    HUMANITY_SIGNING_PASSPHRASE env var; it NEVER touches CI (audit
    //    2026-06-12 CRITICAL fix). Output goes to the files written + the exit
    //    code; on a release GUI build the console is hidden, so drive these via
    //    the `just gen-release-key` / `just sign-release` recipes. ──
    #[cfg(feature = "native")]
    {
        if let Some(i) = args.iter().position(|a| a == "--gen-release-key") {
            let pub_path = args
                .get(i + 1)
                .cloned()
                .unwrap_or_else(|| "data/release/signing_pubkeys.json".to_string());
            let vault_path = args
                .get(i + 2)
                .cloned()
                .unwrap_or_else(|| "release-signing-key.enc".to_string());
            match humanity_engine::release_update::gen_release_key(&pub_path, &vault_path) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("gen-release-key failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        if let Some(i) = args.iter().position(|a| a == "--sign-release") {
            let Some(manifest) = args.get(i + 1).cloned() else {
                eprintln!("usage: --sign-release <manifest.json> [vault.enc]");
                std::process::exit(2);
            };
            let vault_path = args
                .get(i + 2)
                .cloned()
                .unwrap_or_else(|| "release-signing-key.enc".to_string());
            match humanity_engine::release_update::sign_release(&manifest, &vault_path) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("sign-release failed: {e}");
                    std::process::exit(1);
                }
            }
        }
    }

    if headless {
        // Server-only mode: run the relay without any GPU/window
        #[cfg(feature = "relay")]
        {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(humanity_engine::relay::run_relay());
        }
        #[cfg(not(feature = "relay"))]
        {
            eprintln!("Relay feature not enabled. Build with: cargo build --features relay");
            std::process::exit(1);
        }
    } else {
        // Full desktop mode: renderer + relay + game
        #[cfg(feature = "native")]
        {
            // Check if a newer version exists and delegate to it
            if let Some(newer) = find_newer_exe() {
                launch_and_exit(&newer);
            }
            humanity_engine::run();
        }
        #[cfg(not(feature = "native"))]
        {
            eprintln!("Native feature not enabled. Build with: cargo build --features native");
            eprintln!("For server-only mode: HumanityOS --headless");
            std::process::exit(1);
        }
    }
}

/// Current version from Cargo.toml at compile time
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Scan the binaries directory for a versioned exe newer than this one.
/// Returns the path if a newer version is found.
#[cfg(feature = "native")]
fn find_newer_exe() -> Option<std::path::PathBuf> {
    let bin_dir = binaries_dir()?;
    let current = parse_version(VERSION)?;
    let current_exe = std::env::current_exe().ok()?;

    let mut best: Option<(Vec<u32>, std::path::PathBuf)> = None;

    let entries = std::fs::read_dir(&bin_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Match v{version}_HumanityOS.exe
        if let Some(ver_str) = name_str
            .strip_prefix('v')
            .and_then(|s| s.strip_suffix("_HumanityOS.exe"))
        {
            if let Some(ver) = parse_version(ver_str) {
                let entry_path = entry.path();
                if same_file(&entry_path, &current_exe) {
                    continue;
                }
                if ver > current {
                    if best.as_ref().map_or(true, |(bv, _)| ver > *bv) {
                        best = Some((ver, entry_path));
                    }
                }
            }
        }
    }

    best.map(|(_, path)| path)
}

/// Parse "0.89.0" into [0, 89, 0] for comparison
fn parse_version(s: &str) -> Option<Vec<u32>> {
    let parts: Result<Vec<u32>, _> = s.split('.').map(|p| p.parse()).collect();
    parts.ok()
}

/// Check if two paths refer to the same file
#[cfg(feature = "native")]
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// Get the directory containing versioned exes.
#[cfg(feature = "native")]
fn binaries_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let dir = std::path::PathBuf::from("C:\\Humanity");
        if dir.is_dir() { Some(dir) } else { None }
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join("Humanity")).filter(|d| d.is_dir())
    }
}

/// Launch a newer exe and exit this process
#[cfg(feature = "native")]
fn launch_and_exit(path: &std::path::Path) -> ! {
    use std::process::Command;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = Command::new(path).args(&args).spawn();
    std::process::exit(0);
}
