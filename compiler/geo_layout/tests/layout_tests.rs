use geo_layout::{required_layout, validate_workspace, LayoutKind};
use std::path::Path;

#[test]
fn required_layout_includes_compiler_library_and_src_areas() {
    let entries = required_layout();

    assert!(entries.iter().any(
        |entry| entry.path == Path::new("compiler/geo") && entry.kind == LayoutKind::Directory
    ));
    assert!(entries
        .iter()
        .any(|entry| entry.path == Path::new("library/geo_runtime")
            && entry.kind == LayoutKind::Directory));
    assert!(entries
        .iter()
        .any(|entry| entry.path == Path::new("src/tools/xtask")
            && entry.kind == LayoutKind::Directory));
}

#[test]
fn current_checkout_matches_required_layout() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = validate_workspace(&root);

    assert!(
        report.is_ok(),
        "missing layout entries: {:?}",
        report.missing
    );
}
