fn main() {
    let mut attrs = tauri_build::Attributes::new();
    // Windows seulement (cible, pas machine de compilation) : manifeste Common Controls 6 embarqué dans
    // TOUS les binaires (y compris les tests), sinon STATUS_ENTRYPOINT_NOT_FOUND au lancement des tests.
    // Sur macOS rien de tout cela : Info.plist et paquet .app sont produits par le bundler de Tauri.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest = std::env::current_dir().unwrap().join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.to_str().unwrap());
    }
    tauri_build::try_build(attrs).expect("tauri-build");
}
