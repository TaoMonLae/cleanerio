//! Gives the Windows program its icon and version details. Does nothing on other systems.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Cleanerio");
        res.set("FileDescription", "Cleanerio");
        res.set("LegalCopyright", "Copyright (c) 2026 Min Htet Min Aung. MIT License.");
        if let Err(e) = res.compile() {
            // the program still works without its icon
            println!("cargo:warning=could not add the Windows icon: {e}");
        }
    }
}
