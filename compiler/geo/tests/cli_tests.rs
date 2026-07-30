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
fn exposes_compiler_version_metadata() {
    let error = Cli::try_parse_from(["geo", "--version"])
        .expect_err("--version should terminate argument parsing");

    assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
    assert!(error.to_string().contains(env!("CARGO_PKG_VERSION")));
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
