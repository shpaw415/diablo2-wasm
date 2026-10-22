//! Shared helpers for d2game integration tests.

/// Workspace root (`diablo-2-wasm/`), when the repo layout is present.
pub fn ws_root() -> Option<String> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)?
        .to_string_lossy()
        .into_owned();
    std::path::Path::new(&p).join("assets/manifest.json").is_file().then_some(p)
}

/// Output dir for proof PNGs: `engine-rs/target/d2game-proof/`.
pub fn proof_dir() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("target/d2game-proof");
    std::fs::create_dir_all(&dir).ok();
    dir.to_string_lossy().into_owned()
}
