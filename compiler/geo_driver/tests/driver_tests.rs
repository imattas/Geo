use clap::Parser;
use geo_driver::cli::{Cli, Command};
use std::path::PathBuf;

#[test]
fn driver_crate_exposes_the_compiler_cli_contract() {
    let cli = Cli {
        command: Command::Check {
            input: PathBuf::from("examples/return_42.geo"),
            target: Some("x86_64-linux".to_string()),
        },
    };

    assert!(matches!(cli.command, Command::Check { .. }));
}

#[test]
fn driver_exposes_pipeline_dump_commands() {
    let tokens = Cli::parse_from(["geo", "dump-tokens", "examples/hello_world.geo"]);
    let ast = Cli::parse_from(["geo", "dump-ast", "examples/hello_world.geo"]);
    let ir = Cli::parse_from(["geo", "dump-ir", "examples/hello_world.geo"]);

    assert!(matches!(tokens.command, Command::DumpTokens { .. }));
    assert!(matches!(ast.command, Command::DumpAst { .. }));
    assert!(matches!(ir.command, Command::DumpIr { .. }));
}
