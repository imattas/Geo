# Geo v1 Phase 1 Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce the v1 compiler foundation modules for source management, diagnostics, target selection, and package-aware CLI plumbing.

**Architecture:** This phase preserves the existing v0.1 compiler pipeline while adding stable v1 boundaries around it. `source` owns file validation and span rendering, `target` owns platform triples and tool defaults, diagnostics gain source locations, and the driver compiles through a `CompileConfig` so later phases can add modules, runtime, object output, and Windows codegen without rewriting every command.

**Tech Stack:** Rust 2021, Cargo, `clap`, current Geo AST/parser/typechecker/IR/NASM backend.

## Global Constraints

- Geo v1 supports Linux x86-64 and Windows x86-64 as first-class targets.
- NASM assembly output remains the production path for v1.
- Direct object writing is introduced later through interfaces and a Linux ELF64 prototype.
- The compiler remains implemented in Rust for v1.
- The Rust compiler remains the authoritative compiler implementation for v1.
- v1 diagnostics must include severity, primary message, file path, line and column, source excerpt, caret underline, and optional notes.
- The default target is the host target when supported.
- Existing v0.1 examples and tests must keep passing.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

## File Structure

- Modify `src/diagnostics.rs`: add severity, optional source location, notes, and rendering.
- Create `src/source.rs`: source file validation, loading, line/column lookup, and source excerpts.
- Create `src/target.rs`: v1 target triples, ABI metadata, NASM object format, executable extension, and linker defaults.
- Modify `src/lib.rs`: export `source` and `target`.
- Modify `src/cli.rs`: add `--target` support for `check`, `emit-asm`, `build`, and `run`; add `fmt` and `test` command shells.
- Modify `src/driver.rs`: replace ad hoc source reads with `SourceFile`, add `CompileConfig`, validate targets, and route tool defaults through `Target`.
- Create `tests/source_tests.rs`: source loading and diagnostic rendering tests.
- Create `tests/target_tests.rs`: target parsing and defaults tests.
- Modify `tests/compile_tests.rs`: verify CLI target options and unsupported target rejection.

---

### Task 1: Rich Diagnostics and Source Manager

**Files:**
- Modify: `src/diagnostics.rs`
- Create: `src/source.rs`
- Modify: `src/lib.rs`
- Test: `tests/source_tests.rs`

**Interfaces:**
- Produces: `diagnostics::Severity`
- Produces: `diagnostics::SourceLocation { path: PathBuf, line: usize, column: usize, line_text: String, underline_len: usize }`
- Produces: `Diagnostic::with_source(location: SourceLocation) -> Diagnostic`
- Produces: `Diagnostic::with_note(note: impl Into<String>) -> Diagnostic`
- Produces: `Diagnostic::render() -> String`
- Produces: `source::SourceFile { path: PathBuf, text: String }`
- Produces: `SourceFile::load(path: &Path) -> Result<SourceFile, Vec<Diagnostic>>`
- Produces: `SourceFile::location(offset: usize, len: usize) -> SourceLocation`

- [ ] **Step 1: Add tests**

Create `tests/source_tests.rs`:

```rust
use geo::diagnostics::Diagnostic;
use geo::source::SourceFile;
use std::path::Path;

#[test]
fn rejects_non_geo_source_files() {
    let err = SourceFile::load(Path::new("examples/not_geo.txt")).unwrap_err();
    assert!(err[0].message.contains(".geo extension"));
}

#[test]
fn maps_offsets_to_source_locations() {
    let source = SourceFile {
        path: Path::new("examples/sample.geo").to_path_buf(),
        text: "fn main() -> int {\n    return 42\n}\n".to_string(),
    };

    let location = source.location(23, 6);

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 5);
    assert_eq!(location.line_text, "    return 42");
    assert_eq!(location.underline_len, 6);
}

#[test]
fn renders_diagnostic_with_source_excerpt() {
    let source = SourceFile {
        path: Path::new("examples/sample.geo").to_path_buf(),
        text: "fn main() -> int {\n    return\n}\n".to_string(),
    };
    let rendered = Diagnostic::error("expected expression")
        .with_source(source.location(23, 6))
        .with_note("return statements require a value")
        .render();

    assert!(rendered.contains("error: expected expression"));
    assert!(rendered.contains("--> examples/sample.geo:2:5"));
    assert!(rendered.contains("return"));
    assert!(rendered.contains("^^^^^^"));
    assert!(rendered.contains("note: return statements require a value"));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test --test source_tests`

Expected: FAIL because `geo::source` and rich diagnostic APIs do not exist.

- [ ] **Step 3: Implement diagnostics and source manager**

Update `src/diagnostics.rs` and create `src/source.rs` exactly matching the interfaces above. `Diagnostic::error` must still work for existing tests by setting severity to `Severity::Error`.

- [ ] **Step 4: Export source module**

Update `src/lib.rs`:

```rust
pub mod source;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test source_tests`

Expected: PASS.

---

### Task 2: Target Abstraction

**Files:**
- Create: `src/target.rs`
- Modify: `src/lib.rs`
- Test: `tests/target_tests.rs`

**Interfaces:**
- Produces: `target::TargetTriple`
- Produces: `target::Target { triple, abi, object_format, nasm_format, executable_extension, default_linker }`
- Produces: `Target::host() -> Target`
- Produces: `Target::parse(value: &str) -> Result<Target, Diagnostic>`
- Produces: `Target::linux_x86_64() -> Target`
- Produces: `Target::windows_x86_64() -> Target`

- [ ] **Step 1: Add tests**

Create `tests/target_tests.rs`:

```rust
use geo::target::{ObjectFormat, Target, TargetTriple};

#[test]
fn parses_linux_target() {
    let target = Target::parse("x86_64-linux").unwrap();
    assert_eq!(target.triple, TargetTriple::X86_64Linux);
    assert_eq!(target.object_format, ObjectFormat::Elf64);
    assert_eq!(target.nasm_format, "elf64");
    assert_eq!(target.default_linker, "gcc");
}

#[test]
fn parses_windows_target() {
    let target = Target::parse("x86_64-windows").unwrap();
    assert_eq!(target.triple, TargetTriple::X86_64Windows);
    assert_eq!(target.object_format, ObjectFormat::Win64);
    assert_eq!(target.nasm_format, "win64");
    assert_eq!(target.executable_extension, "exe");
}

#[test]
fn rejects_unknown_target() {
    let err = Target::parse("wasm32-browser").unwrap_err();
    assert!(err.message.contains("unsupported target"));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test --test target_tests`

Expected: FAIL because `geo::target` does not exist.

- [ ] **Step 3: Implement target module**

Create `src/target.rs` with Linux and Windows target metadata. `Target::host()` must return Windows on Windows hosts and Linux on Linux hosts; other hosts should fall back to Linux so current tests can still run cross-target emission checks.

- [ ] **Step 4: Export target module**

Update `src/lib.rs`:

```rust
pub mod target;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test target_tests`

Expected: PASS.

---

### Task 3: CLI and Driver Configuration

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/driver.rs`
- Modify: `src/main.rs`
- Test: `tests/compile_tests.rs`

**Interfaces:**
- Produces: `driver::CompileConfig { target: Target, nasm: String, linker: String, keep_temps: bool }`
- Produces: `driver::compile_to_asm(path: &Path, config: &CompileConfig) -> Result<String, Vec<Diagnostic>>`
- Preserves: `geo check`, `geo emit-asm`, `geo build`, and `geo run`
- Adds: `geo fmt <input>`
- Adds: `geo test <path>`

- [ ] **Step 1: Add CLI tests**

Append to `tests/compile_tests.rs`:

```rust
#[test]
fn cli_accepts_explicit_linux_target_for_check() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", "examples/return_42.geo", "--target", "x86_64-linux"])
        .status()
        .expect("failed to run geo");

    assert!(status.success());
}

#[test]
fn cli_rejects_unknown_target() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", "examples/return_42.geo", "--target", "wasm32-browser"])
        .output()
        .expect("failed to run geo");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported target"));
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test --test compile_tests cli_accepts_explicit_linux_target_for_check cli_rejects_unknown_target`

Expected: FAIL because `--target` is not supported.

- [ ] **Step 3: Update CLI types**

Add `target: Option<String>` to `Check`, `EmitAsm`, `Build`, and `Run`. Add `Fmt { input: PathBuf }` and `Test { path: PathBuf }` command variants.

- [ ] **Step 4: Update driver**

Use `SourceFile::load`, `Target::parse`, and `CompileConfig` for all compiler paths. `fmt` and `test` should return diagnostics saying the commands are recognized but not implemented in Phase 1.

- [ ] **Step 5: Update main diagnostic printing**

Call `diagnostic.render()` instead of printing only the message.

- [ ] **Step 6: Run tests**

Run: `cargo test`

Expected: PASS.

---

## Plan Self-Review

- Spec coverage: This plan covers Phase 1 from the v1 spec: source management, target abstraction, richer diagnostics, and package-aware CLI foundations. It intentionally leaves language features, runtime, ownership, Windows backend emission, and object writing to later phase plans.
- Placeholder scan: No placeholders remain; every task has concrete files, interfaces, tests, commands, and expected results.
- Type consistency: `SourceFile`, `SourceLocation`, `Diagnostic`, `Target`, and `CompileConfig` names are consistent across all tasks.
