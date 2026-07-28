# Geo v1 Multi-File Module Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve non-`std` imports by loading `.geo` files from the package root so entry programs can call imported functions and use imported structs.

**Architecture:** Add `src/resolve.rs` as a package loader/resolver layer between parsing and typechecking. It recursively loads non-`std` imports relative to the entry file directory, merges imported structs/externs/functions into one `Program`, preserves `std` imports for runtime lookup, and reports circular import chains.

**Tech Stack:** Rust 2021, existing lexer/parser/source/diagnostics/typecheck/lower pipeline, Cargo tests.

## Global Constraints

- Geo v1 supports multiple source files in a package directory.
- A module maps to a `.geo` file or directory module entry.
- Imports are explicit and acyclic in v1.
- The compiler resolves imported module symbols and standard library modules.
- No subagents are used for this implementation.

---

### Task 1: Resolver Tests

**Files:**
- Create: `tests/resolve_tests.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: `resolve::load_package_entry(path: &Path) -> Result<Program, Vec<Diagnostic>>`
- Produces: tests proving imported functions/structs are merged and cycles are rejected.

- [ ] **Step 1: Add resolver unit tests**

Add a test that writes `main.geo` importing `math`, writes `math.geo` with `fn forty_two() -> int`, calls `load_package_entry(&main)`, and asserts the merged program has both `main` and `forty_two`.

Add a test that writes `a.geo` importing `b` and `b.geo` importing `a`, then asserts `load_package_entry(&a)` errors with `circular import`.

- [ ] **Step 2: Add CLI acceptance test**

Add a test that writes `main.geo` importing `math`, then runs `geo check` and `geo emit-asm` against `main.geo`.

### Task 2: Resolver Implementation

**Files:**
- Create: `src/resolve.rs`
- Modify: `src/lib.rs`
- Modify: `src/source.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `resolve::load_package_entry(path: &Path) -> Result<Program, Vec<Diagnostic>>`

- [ ] **Step 1: Add module file resolution**

Extend source mapping to support `foo.geo` first and `foo/mod.geo` second.

- [ ] **Step 2: Implement recursive resolver**

Load the entry file, parse it, recursively load non-`std` imports, maintain `visiting` and `visited` sets by canonical path, and merge imported declarations before entry declarations.

- [ ] **Step 3: Wire driver**

Replace single-file parsing in `compile_to_asm`, `check_source_file`, and `fmt_source_file` validation with `resolve::load_package_entry`.

### Task 3: Verify

- [ ] Run `cargo fmt`.
- [ ] Run `cargo test --test resolve_tests`.
- [ ] Run focused CLI module test.
- [ ] Run `cargo test`.

## Self-Review

- Spec coverage: Implements explicit acyclic multi-file imports and imported symbol resolution for functions, structs, externs, and std imports.
- Known gap: Names remain merged into a flat namespace; qualified names and directory module entry beyond `mod.geo` are future work.
- Placeholder scan: No placeholders remain.
