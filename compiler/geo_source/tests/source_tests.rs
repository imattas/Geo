use geo_source::{module_path_to_file, SourceFile};
use std::fs;

#[test]
fn maps_module_path_to_directory_module_when_mod_file_exists() {
    let root = std::env::temp_dir().join(format!("geo-source-test-{}", std::process::id()));
    let module_dir = root.join("std").join("io");
    fs::create_dir_all(&module_dir).expect("create module fixture");
    fs::write(module_dir.join("mod.geo"), "fn main() {}").expect("write module fixture");

    let path = module_path_to_file(&root, &["std".to_string(), "io".to_string()]);

    assert_eq!(path, module_dir.join("mod.geo"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn computes_source_location_from_byte_offset() {
    let source = SourceFile {
        path: "sample.geo".into(),
        text: "fn main() {\n    return\n}".to_string(),
    };

    let location = source.location(16, 6);

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 5);
    assert_eq!(location.line_text, "    return");
    assert_eq!(location.underline_len, 6);
}

#[test]
fn attaches_diagnostic_spans_to_source_locations() {
    let source = SourceFile {
        path: "sample.geo".into(),
        text: "fn main() {\n    @\n}".to_string(),
    };
    let diagnostics = source.attach_diagnostics(vec![geo_diagnostics::Diagnostic::error(
        "unexpected character '@'",
    )
    .with_span(16, 1)]);

    let location = diagnostics[0].source.as_ref().expect("source location");
    assert_eq!(location.line, 2);
    assert_eq!(location.column, 5);
    assert_eq!(location.line_text, "    @");
}
