use xtask::{parse_args, XtaskCommand};

#[test]
fn parses_layout_command() {
    let command = parse_args(["xtask", "layout"]).expect("layout command should parse");

    assert_eq!(command, XtaskCommand::Layout);
}

#[test]
fn parses_verify_command() {
    let command = parse_args(["xtask", "verify"]).expect("verify command should parse");

    assert_eq!(command, XtaskCommand::Verify);
}

#[test]
fn rejects_unknown_command_with_helpful_error() {
    let err = parse_args(["xtask", "wat"]).expect_err("unknown command should fail");

    assert!(err.contains("unknown xtask command 'wat'"));
    assert!(err.contains("layout"));
    assert!(err.contains("verify"));
}
