use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XtaskCommand {
    FromScratch,
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
        Some(command) if command == "from-scratch" => Ok(XtaskCommand::FromScratch),
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
        XtaskCommand::FromScratch => run_from_scratch(root),
        XtaskCommand::Help => Ok(help_text()),
        XtaskCommand::Layout => run_layout(root),
        XtaskCommand::Status => Ok(run_status(root)),
        XtaskCommand::Verify => run_verify(root),
    }
}

pub fn help_text() -> String {
    [
        "Geo workspace tasks:",
        "  from-scratch  Enforce no compiler backend framework dependencies",
        "  layout  Validate required compiler/library/src directories",
        "  status  Print repository status summary",
        "  verify  Run formatting, tests, and target smoke checks",
    ]
    .join("\n")
}

fn run_layout(root: &Path) -> Result<String, String> {
    let missing = required_paths()
        .into_iter()
        .filter(|path| !root.join(path).exists())
        .collect::<Vec<_>>();

    if missing.is_empty() {
        return Ok("layout ok".to_string());
    }

    let mut message = String::from("layout missing required entries:\n");
    for path in missing {
        let _ = writeln!(message, "- {path}");
    }
    Err(message)
}

fn run_status(root: &Path) -> String {
    let state = if required_paths()
        .into_iter()
        .all(|path| root.join(path).exists())
    {
        "ok"
    } else {
        "incomplete"
    };
    format!(
        "Geo workspace\nroot: {}\nlayout: {}\ncompiler: compiler/geo\nfrontend: compiler/geo_syntax\nsource: compiler/geo_source\nruntime: compiler/geo_backend\nstdlib: library/std\ntools: src/tools/xtask",
        root.display(),
        state
    )
}

fn required_paths() -> Vec<&'static str> {
    vec![
        "compiler/geo",
        "compiler/geo_syntax",
        "compiler/geo_ir",
        "compiler/geo_semantic",
        "compiler/geo_codegen",
        "compiler/geo_backend",
        "compiler/geo_driver",
        "compiler/geo_diagnostics",
        "compiler/geo_source",
        "library/std",
        "src/bootstrap",
        "src/tools/xtask",
        "examples",
        "docs",
        "Cargo.toml",
    ]
}

fn run_verify(root: &Path) -> Result<String, String> {
    check_from_scratch_policy(root)?;
    for (program, args) in verify_commands() {
        run_command(root, program, &args)?;
    }
    Ok("verify ok".to_string())
}

pub fn verify_commands() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("cargo", vec!["fmt", "--check"]),
        ("cargo", vec!["test", "--workspace", "--locked"]),
        (
            "cargo",
            vec![
                "run",
                "--locked",
                "--quiet",
                "--",
                "check",
                "examples/return_42.geo",
                "--target",
                "x86_64-linux",
            ],
        ),
        (
            "cargo",
            vec![
                "run",
                "--locked",
                "--quiet",
                "--",
                "emit-obj",
                "examples/object_backend.geo",
                "--target",
                "x86_64-linux",
                "-o",
                "target/xtask-object-backend-linux.o",
            ],
        ),
        (
            "cargo",
            vec![
                "run",
                "--locked",
                "--quiet",
                "--",
                "emit-obj",
                "examples/hello_world.geo",
                "--target",
                "x86_64-linux",
                "-o",
                "target/xtask-hello-world-linux.o",
            ],
        ),
        (
            "cargo",
            vec![
                "run",
                "--locked",
                "--quiet",
                "--",
                "emit-obj",
                "examples/coff_backend.geo",
                "--target",
                "x86_64-windows",
                "-o",
                "target/xtask-coff-backend-windows.obj",
            ],
        ),
        (
            "cargo",
            vec![
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
        ),
    ]
}

fn run_from_scratch(root: &Path) -> Result<String, String> {
    check_from_scratch_policy(root)?;
    Ok("from-scratch policy ok".to_string())
}

pub fn check_from_scratch_policy(root: &Path) -> Result<(), String> {
    let forbidden = [
        "llvm",
        "llvm-sys",
        "inkwell",
        "cranelift",
        "cranelift-codegen",
        "gccjit",
        "melior",
        "mlir",
    ];
    let files = ["Cargo.toml", "Cargo.lock", "compiler/geo/Cargo.toml"];

    for file in files {
        let path = root.join(file);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            for name in forbidden {
                if declares_dependency(trimmed, name) {
                    return Err(format!(
                        "forbidden compiler backend dependency '{name}' found in {}",
                        path.display()
                    ));
                }
            }
        }
    }

    let required_native_sources = [
        "compiler/geo_syntax/src/lexer.rs",
        "compiler/geo_syntax/src/parser.rs",
        "compiler/geo_semantic/src/typecheck.rs",
        "compiler/geo_codegen/src/lower.rs",
        "compiler/geo_backend/src/elf.rs",
        "compiler/geo_backend/src/pe.rs",
        "compiler/geo_backend/src/object.rs",
    ];
    for file in required_native_sources {
        if !root.join(file).is_file() {
            return Err(format!(
                "from-scratch compiler source is missing required stage: {file}"
            ));
        }
    }

    Ok(())
}

fn declares_dependency(line: &str, name: &str) -> bool {
    let quoted = format!("\"{name}\"");
    line.starts_with(&format!("{name} ="))
        || line.starts_with(&format!("{name}="))
        || line.contains(&format!("name = {quoted}"))
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
