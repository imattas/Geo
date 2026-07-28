use clap::Parser;
use geo::cli::{Cli, Command};
use std::path::PathBuf;

#[test]
fn parses_run_args_after_separator() {
    let cli = Cli::try_parse_from(["geo", "run", "main.geo", "--", "input.geo", "--emit", "-O2"])
        .expect("run args after separator should parse");

    match cli.command {
        Command::Run { input, args, .. } => {
            assert_eq!(input, PathBuf::from("main.geo"));
            assert_eq!(args, ["input.geo", "--emit", "-O2"]);
        }
        other => panic!("expected run command, got {other:?}"),
    }
}

#[test]
fn parses_emit_obj_command() {
    let cli = Cli::try_parse_from([
        "geo",
        "emit-obj",
        "examples/return_42.geo",
        "-o",
        "target/return_42.o",
        "--target",
        "x86_64-linux",
    ])
    .expect("emit-obj command should parse");

    match cli.command {
        Command::EmitObj {
            input,
            output,
            target,
        } => {
            assert_eq!(input, PathBuf::from("examples/return_42.geo"));
            assert_eq!(output, PathBuf::from("target/return_42.o"));
            assert_eq!(target, Some("x86_64-linux".to_string()));
        }
        other => panic!("expected emit-obj command, got {other:?}"),
    }
}
