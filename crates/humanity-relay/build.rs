use std::process::Command;

fn main() {
    // Set BUILD_VERSION from git short hash + timestamp.
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let version = format!("{}-{}", hash.trim(), timestamp);
    println!("cargo:rustc-env=BUILD_VERSION={}", version);
    // Rebuild when any source changes.
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=client/");
}
