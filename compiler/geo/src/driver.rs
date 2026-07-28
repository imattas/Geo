use crate::cli::{Cli, Command};
use crate::diagnostics::Diagnostic;
use crate::source::SourceFile;
use crate::target::{ObjectFormat, Target, TargetTriple};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

#[derive(Debug, Clone)]
pub struct CompileConfig {
    pub target: Target,
    pub nasm: String,
    pub linker: String,
    pub keep_temps: bool,
    pub runtime_entry: bool,
}

pub fn run_cli(cli: Cli) -> Result<(), Vec<Diagnostic>> {
    match cli.command {
        Command::Check { input, target } => {
            let config = compile_config(target, "nasm".to_string(), None, false, false)?;
            check_source_file(&input, &config)?;
            Ok(())
        }
        Command::EmitAsm {
            input,
            output,
            target,
        } => {
            let config = compile_config(target, "nasm".to_string(), None, false, false)?;
            let asm = compile_to_asm(&input, &config)?;
            fs::write(&output, asm).map_err(|err| {
                vec![Diagnostic::error(format!(
                    "failed to write assembly file: {err}"
                ))]
            })?;
            Ok(())
        }
        Command::EmitObj {
            input,
            output,
            target,
        } => {
            let config = compile_config(target, "nasm".to_string(), None, false, false)?;
            let object = compile_to_object(&input, &config)?;
            fs::write(&output, object).map_err(|err| {
                vec![Diagnostic::error(format!(
                    "failed to write object file: {err}"
                ))]
            })?;
            Ok(())
        }
        Command::Build {
            input,
            output,
            target,
            nasm,
            linker,
            keep_temps,
        } => {
            let config = compile_config(target, nasm, linker, keep_temps, true)?;
            let output = output.unwrap_or_else(|| default_output_path(&input, &config.target));
            build_executable(&input, &output, &config)?;
            Ok(())
        }
        Command::Run {
            input,
            target,
            nasm,
            linker,
            args,
        } => {
            let config = compile_config(target, nasm, linker, false, true)?;
            let exe = temp_path(&input, "geo-run");
            build_executable(&input, &exe, &config)?;
            let status = ProcessCommand::new(&exe)
                .args(&args)
                .status()
                .map_err(|err| {
                    vec![Diagnostic::error(format!(
                        "failed to run executable: {err}"
                    ))]
                })?;
            let _ = fs::remove_file(&exe);
            std::process::exit(status.code().unwrap_or(1));
        }
        Command::Fmt { input } => {
            fmt_source_file(&input)?;
            Ok(())
        }
        Command::Test { path } => {
            let config = compile_config(None, "nasm".to_string(), None, false, false)?;
            test_geo_path(&path, &config)?;
            Ok(())
        }
    }
}

pub fn read_geo_source(path: &Path) -> Result<String, Vec<Diagnostic>> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("geo") {
        return Err(vec![Diagnostic::error(
            "Geo source files must use the .geo extension",
        )]);
    }

    SourceFile::load(path).map(|source| source.text)
}

pub fn compile_to_asm(path: &Path, config: &CompileConfig) -> Result<String, Vec<Diagnostic>> {
    let program = load_checked_program(path)?;
    let ir = crate::lower::lower(&program);
    Ok(crate::x86_64::emit_nasm_for_target_with_runtime_entry(
        &ir,
        &config.target,
        config.runtime_entry,
    ))
}

pub fn compile_to_object(path: &Path, config: &CompileConfig) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let program = load_checked_program(path)?;
    let ir = crate::lower::lower(&program);
    match config.target.object_format {
        ObjectFormat::Elf64 => Ok(crate::object::emit_elf64_relocatable(&ir)),
        ObjectFormat::Win64 => crate::object::emit_coff_x64_relocatable(&ir).ok_or_else(|| {
            vec![Diagnostic::error(
                "x86_64-windows object emission currently supports the current compiler-owned object subset",
            )]
        }),
    }
}

fn check_source_file(path: &Path, _config: &CompileConfig) -> Result<(), Vec<Diagnostic>> {
    load_checked_program(path)?;
    Ok(())
}

fn fmt_source_file(path: &Path) -> Result<(), Vec<Diagnostic>> {
    let source = SourceFile::load(path)?;
    load_checked_program(path)?;
    let formatted = format_source_text(&source.text);
    fs::write(path, formatted).map_err(|err| {
        vec![Diagnostic::error(format!(
            "failed to write formatted source: {err}"
        ))]
    })?;
    Ok(())
}

fn format_source_text(source: &str) -> String {
    let mut formatted = source
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    formatted.push('\n');
    formatted
}

fn test_geo_path(path: &Path, config: &CompileConfig) -> Result<(), Vec<Diagnostic>> {
    if path.is_dir() {
        let main = path.join("main.geo");
        if main.exists() {
            check_source_file(&main, config)?;
            return Ok(());
        }
    }

    let files = collect_geo_files(path)?;
    if files.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "no .geo files found under '{}'",
            path.display()
        ))]);
    }

    for file in files {
        check_source_file(&file, config)?;
    }
    Ok(())
}

fn collect_geo_files(path: &Path) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    if path.is_file() {
        return if path.extension().and_then(|ext| ext.to_str()) == Some("geo") {
            Ok(vec![path.to_path_buf()])
        } else {
            Err(vec![Diagnostic::error(
                "Geo source files must use the .geo extension",
            )])
        };
    }

    if !path.is_dir() {
        return Err(vec![Diagnostic::error(format!(
            "path does not exist: '{}'",
            path.display()
        ))]);
    }

    let mut files = Vec::new();
    collect_geo_files_recursive(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_geo_files_recursive(
    path: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Vec<Diagnostic>> {
    for entry in fs::read_dir(path).map_err(|err| {
        vec![Diagnostic::error(format!(
            "failed to read directory '{}': {err}",
            path.display()
        ))]
    })? {
        let entry = entry.map_err(|err| {
            vec![Diagnostic::error(format!(
                "failed to read directory entry: {err}"
            ))]
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_geo_files_recursive(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("geo") {
            files.push(path);
        }
    }
    Ok(())
}

fn load_checked_program(path: &Path) -> Result<crate::ast::Program, Vec<Diagnostic>> {
    let program = crate::resolve::load_package_entry(path)?;
    crate::typecheck::check(&program)?;
    crate::borrow::check(&program)?;
    Ok(program)
}

fn build_executable(
    input: &Path,
    output: &Path,
    config: &CompileConfig,
) -> Result<(), Vec<Diagnostic>> {
    let program = load_checked_program(input)?;
    let ir = crate::lower::lower(&program);
    if config.target.triple == TargetTriple::X86_64Windows {
        if let Some(image) = crate::pe::emit_pe64_console(&ir) {
            fs::write(output, image).map_err(|err| {
                vec![Diagnostic::error(format!(
                    "failed to write PE executable: {err}"
                ))]
            })?;
            return Ok(());
        }
    }
    if config.target.triple == TargetTriple::X86_64Linux {
        if let Some(image) = crate::elf::emit_elf64_executable(&ir) {
            fs::write(output, image).map_err(|err| {
                vec![Diagnostic::error(format!(
                    "failed to write ELF executable: {err}"
                ))]
            })?;
            return Ok(());
        }
    }
    let asm = crate::x86_64::emit_nasm_for_target_with_runtime_entry(
        &ir,
        &config.target,
        config.runtime_entry,
    );
    let asm_path = temp_path(input, "asm");
    let obj_path = temp_path(input, "o");
    fs::write(&asm_path, asm).map_err(|err| {
        vec![Diagnostic::error(format!(
            "failed to write assembly file: {err}"
        ))]
    })?;

    run_tool(
        &config.nasm,
        &[
            "-f".to_string(),
            config.target.nasm_format.to_string(),
            asm_path.to_string_lossy().to_string(),
            "-o".to_string(),
            obj_path.to_string_lossy().to_string(),
        ],
        "nasm",
    )?;
    run_tool(&config.linker, &link_args(&obj_path, output), "linker")?;

    if !config.keep_temps {
        let _ = fs::remove_file(&asm_path);
        let _ = fs::remove_file(&obj_path);
    }

    Ok(())
}

fn run_tool(tool: &str, args: &[String], label: &str) -> Result<(), Vec<Diagnostic>> {
    let output = ProcessCommand::new(tool)
        .args(args)
        .output()
        .map_err(|err| {
            vec![Diagnostic::error(format!(
                "failed to run {label} '{tool}': {err}"
            ))]
        })?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(vec![Diagnostic::error(format!("{label} failed: {stderr}"))])
    }
}

fn link_args(obj_path: &Path, output: &Path) -> Vec<String> {
    vec![
        obj_path.to_string_lossy().to_string(),
        crate::runtime::c_runtime_path()
            .to_string_lossy()
            .to_string(),
        "-o".to_string(),
        output.to_string_lossy().to_string(),
    ]
}

fn compile_config(
    target: Option<String>,
    nasm: String,
    linker: Option<String>,
    keep_temps: bool,
    runtime_entry: bool,
) -> Result<CompileConfig, Vec<Diagnostic>> {
    let target = match target {
        Some(target) => Target::parse(&target).map_err(|diagnostic| vec![diagnostic])?,
        None => Target::host(),
    };
    let linker = linker.unwrap_or_else(|| target.default_linker.to_string());
    Ok(CompileConfig {
        target,
        nasm,
        linker,
        keep_temps,
        runtime_entry,
    })
}

fn default_output_path(input: &Path, target: &Target) -> PathBuf {
    if target.executable_extension.is_empty() {
        input.with_extension("")
    } else {
        input.with_extension(target.executable_extension)
    }
}

fn temp_path(input: &Path, extension: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("geo");
    std::env::temp_dir().join(format!("{stem}-{}.{extension}", std::process::id()))
}
