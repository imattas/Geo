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
