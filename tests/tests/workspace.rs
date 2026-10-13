//! Smoke tests for the workspace layout.

use rescue_tests::workspace_root;

#[test]
fn workspace_root_has_manifest_and_toolchain() {
    let root = workspace_root();
    assert!(root.join("Cargo.toml").is_file());
    assert!(root.join("rust-toolchain.toml").is_file());
}

#[test]
fn determinism_lint_configs_exist() {
    let root = workspace_root();
    for krate in ["sim", "data"] {
        let path = root.join("crates").join(krate).join("clippy.toml");
        assert!(path.is_file(), "missing {}", path.display());
    }
}

proptest::proptest! {
    #[test]
    fn proptest_runs(x in 0u32..1000) {
        proptest::prop_assert!(x < 1000);
    }
}
