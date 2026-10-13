//! Shared helpers for the cross-crate integration tests in `tests/tests/`.
//!
//! Integration tests that need the user's ROM must skip cleanly when no ROM is
//! configured, so CI (which has no ROM) stays green.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

/// Root directory of the cargo workspace.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests/ lives inside the workspace root")
        .to_path_buf()
}
