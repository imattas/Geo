use clap::Parser;
use geo_driver::cli::{Cli, Command};
use geo_driver::driver::run_cli;
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

#[test]
fn semantic_diagnostics_include_function_source_locations() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("semantic_type_error.geo");
    let diagnostics = run_cli(Cli {
        command: Command::Check {
            input,
            target: Some("x86_64-linux".to_string()),
        },
    })
    .expect_err("the fixture should fail type checking");

    let source = diagnostics[0]
        .source
        .as_ref()
        .expect("semantic diagnostics should include a source location");
    assert_eq!(source.line, 1);
    assert!(source.path.ends_with("semantic_type_error.geo"));
}

#[test]
fn semantic_diagnostics_keep_imported_module_locations() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("imported_main.geo");
    let diagnostics = run_cli(Cli {
        command: Command::Check {
            input,
            target: Some("x86_64-linux".to_string()),
        },
    })
    .expect_err("the imported fixture should fail type checking");

    let source = diagnostics[0]
        .source
        .as_ref()
        .expect("imported semantic diagnostics should include a source location");
    assert_eq!(source.line, 1);
    assert!(source.path.ends_with("semantic_helper.geo"));
}
