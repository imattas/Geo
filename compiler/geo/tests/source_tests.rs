use geo::diagnostics::Diagnostic;
use geo::source::{module_path_to_file, SourceFile};
use std::path::Path;

fn workspace_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

#[test]
fn rejects_non_geo_source_files() {
    let err = SourceFile::load(&workspace_path("examples/not_geo.txt")).unwrap_err();
    assert!(err[0].message.contains(".geo extension"));
}

#[test]
fn maps_offsets_to_source_locations() {
    let source = SourceFile {
        path: Path::new("examples/sample.geo").to_path_buf(),
        text: "fn main() -> int {\n    return 42\n}\n".to_string(),
    };

    let location = source.location(23, 6);

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 5);
    assert_eq!(location.line_text, "    return 42");
    assert_eq!(location.underline_len, 6);
}

#[test]
fn renders_diagnostic_with_source_excerpt() {
    let source = SourceFile {
        path: Path::new("examples/sample.geo").to_path_buf(),
        text: "fn main() -> int {\n    return\n}\n".to_string(),
    };
    let rendered = Diagnostic::error("expected expression")
        .with_source(source.location(23, 6))
        .with_note("return statements require a value")
        .render();

    assert!(rendered.contains("error: expected expression"));
    assert!(rendered.contains("--> examples/sample.geo:2:5"));
    assert!(rendered.contains("return"));
    assert!(rendered.contains("^^^^^^"));
    assert!(rendered.contains("note: return statements require a value"));
}

#[test]
fn maps_module_path_to_file_path() {
    let path = module_path_to_file(Path::new("pkg"), &["std".to_string(), "io".to_string()]);
    assert_eq!(path, Path::new("pkg").join("std").join("io.geo"));
}
