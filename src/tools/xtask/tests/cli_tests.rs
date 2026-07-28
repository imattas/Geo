use xtask::{parse_args, verify_commands, XtaskCommand};

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
fn parses_from_scratch_command() {
    let command = parse_args(["xtask", "from-scratch"]).expect("from-scratch command should parse");

    assert_eq!(command, XtaskCommand::FromScratch);
}

#[test]
fn rejects_unknown_command_with_helpful_error() {
    let err = parse_args(["xtask", "wat"]).expect_err("unknown command should fail");

    assert!(err.contains("unknown xtask command 'wat'"));
    assert!(err.contains("layout"));
    assert!(err.contains("verify"));
    assert!(err.contains("from-scratch"));
}

#[test]
fn verify_includes_compiler_owned_linux_object_emission() {
    let commands = verify_commands();
    let has_emit_obj = commands.iter().any(|(program, args)| {
        *program == "cargo"
            && args.contains(&"emit-obj")
            && args.contains(&"examples/variables.geo")
            && args.contains(&"x86_64-linux")
            && args.contains(&"target/xtask-variables-linux.o")
    });

    assert!(has_emit_obj);
}
