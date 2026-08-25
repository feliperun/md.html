//! Shared scaffolding for the fuzz targets (ADR 0020): the repo directories
//! the build pipeline reads (runtime dist, themes, fonts) and a temp source
//! directory, resolved once per process.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("fuzz/ sits inside the repo")
}

pub fn runtime_dist() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| repo_root().join("runtime/dist"))
}

pub fn themes_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| repo_root().join("themes"))
}

pub fn fonts_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| repo_root().join("fonts"))
}

/// A repository-local scratch directory for the source under test (build
/// resolves relative theme/asset paths against it).
pub fn source_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = repo_root()
            .join(".runs/fuzz-work")
            .join(format!("mdhtml-fuzz-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
}
