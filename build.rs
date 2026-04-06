fn main() {
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
