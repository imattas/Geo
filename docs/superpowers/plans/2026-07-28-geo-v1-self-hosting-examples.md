# Geo v1 Self-Hosting Examples Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the v1 compiler-shaped example programs and verify they pass front-end checking plus Linux/Windows assembly emission.

**Architecture:** Examples stay within the currently implemented meaningful compiler subset: structs, scalar-slot arrays, string literals, loops, conditionals, and std runtime calls. Acceptance tests invoke the CLI binary so target parsing and driver paths are covered.

**Tech Stack:** Geo examples, Rust integration tests, Cargo.

## Global Constraints

- v1 includes compiler-shaped Geo examples for buffer, lexer, diagnostics, AST, file echo, and mini parser.
- Acceptance criteria require these examples to compile for Linux and Windows targets.
- The ownership checker must accept natural safe versions of these programs.
- No subagents are used for this implementation.

---

### Task 1: Add Examples

**Files:**
- Create: `examples/v1/buffer.geo`
- Create: `examples/v1/lexer.geo`
- Create: `examples/v1/diagnostics.geo`
- Create: `examples/v1/ast.geo`
- Create: `examples/v1/file_echo.geo`
- Create: `examples/v1/mini_parser.geo`

**Interfaces:**
- Consumes: existing Geo parser, typechecker, borrow checker, scalar-slot aggregate lowering, and runtime std signatures.
- Produces: stable example fixtures used by acceptance tests.

- [ ] **Step 1: Create all six example files**

Each file must define `fn main() -> int` and avoid unsupported dynamic aggregate indexing.

### Task 2: Add Acceptance Tests

**Files:**
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: `CARGO_BIN_EXE_geo`
- Produces: `v1_examples_check_and_emit_for_linux_and_windows`

- [ ] **Step 1: Add hardcoded example path list**

Use the six files under `examples/v1`.

- [ ] **Step 2: Add CLI acceptance test**

For each example, run `geo check <path>`, `geo emit-asm <path> -o <temp> --target x86_64-linux`, and `geo emit-asm <path> -o <temp> --target x86_64-windows`.

### Task 3: Verify

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Run focused acceptance test**

Run: `cargo test --test compile_tests v1_examples_check_and_emit_for_linux_and_windows -- --nocapture`

- [ ] **Step 3: Run full suite**

Run: `cargo test`

Expected: all tests pass.

## Self-Review

- Spec coverage: Covers the named self-hosting example acceptance criterion at check/emit-asm level.
- Known gap: Native linking for all runtime examples remains toolchain-dependent and separately guarded.
- Placeholder scan: No placeholders remain.
