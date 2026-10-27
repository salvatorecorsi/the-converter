fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set("ProductName", "The Converter");
        resource.set("FileDescription", "The Converter — conversioni locali");
        resource.set("CompanyName", "Salvatore Corsi");
        let manifest = std::env::var("THE_CONVERTER_APP_MANIFEST")
            .unwrap_or_else(|_| "shell-extension/app.manifest".into());
        resource.set_manifest_file(&manifest);
        println!("cargo:rerun-if-changed={manifest}");
        if std::path::Path::new("assets/app.ico").exists() {
            resource.set_icon("assets/app.ico");
        }
        resource.set_icon_with_id("assets/pending.ico", "2");
        resource.set_icon_with_id("assets/failed.ico", "3");
        resource.compile().expect("Compilazione risorse Windows");
    }
    println!("cargo:rerun-if-changed=shell-extension/app.manifest");
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/pending.ico");
    println!("cargo:rerun-if-changed=assets/failed.ico");
    println!("cargo:rerun-if-env-changed=THE_CONVERTER_APP_MANIFEST");
}
