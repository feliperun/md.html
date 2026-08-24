//! Resolve the development repository layout: the committed runtime fragments
//! live in `runtime/dist`, the themes in `themes/` and the font catalog in
//! `fonts/`, all relative to the repository root (`crates/mdhtml/..`).
//!
//! `MDHTML_ROOT` overrides the root for WASI and test hosts; the default is
//! `CARGO_MANIFEST_DIR/../..`. This is the single source of truth used by both
//! `commands` (build/check/audit/extract) and `publish`.

pub(crate) fn repository_layout() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = match std::env::var_os("MDHTML_ROOT") {
        Some(value) if !value.is_empty() => std::path::PathBuf::from(value),
        _ => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(".."),
    };
    (
        root.join("runtime").join("dist"),
        root.join("themes"),
        root.join("fonts"),
    )
}
