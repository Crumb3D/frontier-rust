fn main() {
    println!("cargo:rerun-if-changed=icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() != Some("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icon.ico");
    if let Err(e) = res.compile() {
        println!("cargo:warning=exe icon not embedded: {e}");
    }
}
