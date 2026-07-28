# Geo v1 Phase 4 Modules and Externs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add front-end support for imports, module path syntax, and `extern fn` declarations so v1 runtime and standard-library work can attach to stable names.

**Architecture:** This phase extends the AST and parser with top-level imports and extern functions, adds basic duplicate/name validation in the type checker, and introduces source-loading helpers for module path to file path mapping. Full cross-file symbol resolution and runtime linking remain later steps, but `geo check` can accept package-shaped source.

**Tech Stack:** Rust 2021, Cargo, current Geo compiler modules.

## Global Constraints

- Existing tests must keep passing.
- Geo v1 supports modules and imports.
- Geo v1 supports explicit `extern fn`.
- Circular imports must eventually be rejected with diagnostics.
- Runtime linking is not part of this phase.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: Tokens and AST for Imports and Externs

**Files:**
- Modify: `src/token.rs`
- Modify: `src/lexer.rs`
- Modify: `src/ast.rs`
- Modify: `tests/lexer_tests.rs`

**Interfaces:**
- Extends: `TokenKind` with `Import` and `Extern`.
- Extends: `Program` with `imports: Vec<Import>` and `externs: Vec<ExternFunction>`.
- Produces: `Import { path: Vec<String> }`.
- Produces: `ExternFunction { name: String, params: Vec<Param>, return_type: Type }`.

---

### Task 2: Parser Support

**Files:**
- Modify: `src/parser.rs`
- Modify: `tests/parser_tests.rs`

**Interfaces:**
- Parses `import std.io`.
- Parses `extern fn puts(message: *u8) -> int` after pointer syntax is introduced as `Type::Pointer(Box<Type>)`.
- Allows imports, externs, structs, and functions in any top-level order.

---

### Task 3: Type Checker Validation

**Files:**
- Modify: `src/typecheck.rs`
- Modify: `tests/type_tests.rs`

**Interfaces:**
- Extern function names share the function namespace.
- Extern parameter and return types are validated.
- Calls can target normal functions or extern functions.

---

### Task 4: Module Path Utilities

**Files:**
- Modify: `src/source.rs`
- Test: `tests/source_tests.rs`

**Interfaces:**
- Produces: `source::module_path_to_file(root: &Path, path: &[String]) -> PathBuf`.
- Maps `["std", "io"]` to `<root>/std/io.geo`.

---

## Plan Self-Review

- Spec coverage: This plan covers the front-end half of Phase 4: imports, extern declarations, and module-path groundwork.
- Placeholder scan: No placeholders remain.
- Type consistency: `Import`, `ExternFunction`, and `Type::Pointer` are named consistently.
