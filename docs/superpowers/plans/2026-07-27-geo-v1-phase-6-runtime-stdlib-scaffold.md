# Geo v1 Phase 6 Runtime and Standard Library Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add front-end runtime/standard-library metadata so `import std.*` exposes typed built-in functions during `geo check`.

**Architecture:** This phase creates a `runtime` module containing stable signatures for the v1 standard-library surface. The type checker imports those signatures into its callable namespace based on `import std.io`, `std.mem`, `std.process`, `std.string`, and `std.array`. This enables compiler-shaped programs to type-check against std APIs before runtime linking and backend emission are implemented.

**Tech Stack:** Rust 2021, Cargo, current Geo compiler modules.

## Global Constraints

- Existing tests must keep passing.
- Geo v1 ships a small `std` implemented over a platform runtime layer.
- Core modules are `std.io`, `std.mem`, `std.process`, `std.string`, and `std.array`.
- Runtime linking is not part of this phase.
- Imported std functions must share the normal callable namespace.
- Unknown `std.*` imports must be rejected with diagnostics.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: Runtime Metadata Module

**Files:**
- Create: `src/runtime.rs`
- Modify: `src/lib.rs`
- Test: `tests/runtime_tests.rs`

**Interfaces:**
- Produces: `runtime::RuntimeFunction { module: Vec<String>, name: String, params: Vec<Param>, return_type: Type }`.
- Produces: `runtime::functions_for_import(path: &[String]) -> Result<Vec<RuntimeFunction>, Diagnostic>`.
- Produces std signatures for `std.io`, `std.mem`, `std.process`, `std.string`, and `std.array`.

### Task 2: Type Checker Integration

**Files:**
- Modify: `src/typecheck.rs`
- Test: `tests/type_tests.rs`

**Interfaces:**
- Imported runtime functions are callable by unqualified name after `import std.<module>`.
- Imported runtime functions conflict with user functions and externs using the same name.
- Unknown std imports are rejected.

### Task 3: Verify

Run: `cargo test`

Expected: all tests pass.

---

## Plan Self-Review

- Spec coverage: This plan covers the front-end runtime/std portion of v1 Phase 6.
- Placeholder scan: No placeholders remain.
- Type consistency: `RuntimeFunction`, `functions_for_import`, and callable namespace integration are consistent.
