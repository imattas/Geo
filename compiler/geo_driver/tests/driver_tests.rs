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
