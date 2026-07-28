use geo_layout::validate_workspace;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XtaskCommand {
    Help,
    Layout,
    Status,
    Verify,
}

pub fn parse_args<I, S>(args: I) -> Result<XtaskCommand, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    match args.next().map(|arg| arg.as_ref().to_string()) {
        None => Ok(XtaskCommand::Help),
        Some(command) if command == "help" || command == "--help" || command == "-h" => {
            Ok(XtaskCommand::Help)
        }
        Some(command) if command == "layout" => Ok(XtaskCommand::Layout),
        Some(command) if command == "status" => Ok(XtaskCommand::Status),
        Some(command) if command == "verify" => Ok(XtaskCommand::Verify),
        Some(command) => Err(format!(
            "unknown xtask command '{command}'\n\n{}",
            help_text()
        )),
    }
}

pub fn run(command: XtaskCommand, root: &Path) -> Result<String, String> {
    match command {
        XtaskCommand::Help => Ok(help_text()),
        XtaskCommand::Layout => run_layout(root),
        XtaskCommand::Status => Ok(run_status(root)),
        XtaskCommand::Verify => run_verify(root),
    }
}

pub fn help_text() -> String {
    [
        "Geo workspace tasks:",
        "  layout  Validate required compiler/library/src directories",
        "  status  Print repository status summary",
        "  verify  Run formatting, tests, and target smoke checks",
    ]
    .join("\n")
}

fn run_layout(root: &Path) -> Result<String, String> {
    let report = validate_workspace(root);
    if report.is_ok() {
        return Ok("layout ok".to_string());
    }

    let mut message = String::from("layout missing required entries:\n");
    for entry in report.missing {
        let _ = writeln!(message, "- {} ({})", entry.path.display(), entry.purpose);
    }
    Err(message)
}

fn run_status(root: &Path) -> String {
    let layout = validate_workspace(root);
    let state = if layout.is_ok() { "ok" } else { "incomplete" };
    format!(
        "Geo workspace\nroot: {}\nlayout: {}\ncompiler: compiler/geo\nruntime: library/geo_runtime\nstdlib: library/std\ntools: src/tools/xtask",
        root.display(),
        state
    )
}

fn run_verify(root: &Path) -> Result<String, String> {
    run_command(root, "cargo", &["fmt", "--check"])?;
    run_command(root, "cargo", &["test", "--workspace", "--locked"])?;
    run_command(
        root,
        "cargo",
        &[
            "run",
            "--locked",
            "--quiet",
            "--",
            "check",
            "examples/return_42.geo",
            "--target",
            "x86_64-linux",
        ],
    )?;
    run_command(
        root,
        "cargo",
        &[
            "run",
            "--locked",
            "--quiet",
            "--",
            "emit-asm",
            "examples/return_42.geo",
            "--target",
            "x86_64-windows",
            "-o",
            "target/xtask-return-42-windows.asm",
        ],
    )?;
    Ok("verify ok".to_string())
}

fn run_command(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run {program}: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
