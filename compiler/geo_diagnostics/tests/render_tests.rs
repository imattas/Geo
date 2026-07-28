use geo_diagnostics::{Diagnostic, Severity, SourceLocation};
use std::path::PathBuf;

#[test]
fn renders_source_aware_error() {
    let diagnostic = Diagnostic::error("expected expression").with_source(SourceLocation {
        path: PathBuf::from("examples/bad.geo"),
        line: 2,
        column: 12,
        line_text: "    return".to_string(),
        underline_len: 1,
    });

    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.render(),
        "error: expected expression\n --> examples/bad.geo:2:12\n  |\n2 |     return\n  |            ^"
    );
}
