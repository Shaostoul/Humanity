fn main() {
    // Set BUILD_VERSION for the relay module (git hash + timestamp)
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    println!("cargo:rustc-env=BUILD_VERSION={}-{}", hash.trim(), timestamp);

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        // Version info embedded in the exe properties
        res.set("ProductName", "HumanityOS");
        res.set("FileDescription", "HumanityOS - End poverty, unite humanity");
        res.set("LegalCopyright", "Public Domain (CC0)");
        if let Err(e) = res.compile() {
            eprintln!("winres error: {}", e);
        }
    }
}
