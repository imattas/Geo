use std::fs;
use xtask::check_from_scratch_policy;

#[test]
fn rejects_compiler_backend_framework_dependencies() {
    let root = std::env::temp_dir().join(format!("geo-from-scratch-policy-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create policy fixture");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n[dependencies]\nllvm-sys = \"1\"\n",
    )
    .expect("write policy fixture");

    let err = check_from_scratch_policy(&root).expect_err("llvm dependency should fail policy");

    assert!(err.contains("forbidden compiler backend dependency"));
    assert!(err.contains("llvm-sys"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn accepts_current_workspace_policy_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..");

    check_from_scratch_policy(&root).expect("current workspace should satisfy policy");
}
