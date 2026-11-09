//! Resolve the project root so data/ and assets/ work from cargo run.

use std::path::{Path, PathBuf};

/// Walk up from CARGO_MANIFEST_DIR / cwd looking for `data/stars.json`.
pub fn project_root() -> PathBuf {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let crate_dir = PathBuf::from(manifest);
        // crates/frontier-sim or crates/frontier → workspace root
        if let Some(root) = crate_dir.parent().and_then(|p| p.parent()) {
            if root.join("data").join("stars.json").is_file() {
                return root.to_path_buf();
            }
        }
        if crate_dir.join("data").join("stars.json").is_file() {
            return crate_dir;
        }
    }
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if dir.join("data").join("stars.json").is_file() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    PathBuf::from(".")
}

pub fn data_dir() -> PathBuf {
    project_root().join("data")
}

pub fn assets_dir() -> PathBuf {
    project_root().join("assets")
}

pub fn original_frontier_dir() -> PathBuf {
    project_root()
        .join("Frontier - PC ORIGINAL FOR REFERENCE")
        .join("frontier")
}

pub fn exists(path: &Path) -> bool {
    path.is_file()
}
