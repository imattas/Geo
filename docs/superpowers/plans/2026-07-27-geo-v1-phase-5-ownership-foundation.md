# Geo v1 Phase 5 Ownership Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first ownership checker pass: lexical move checking for owned strings, arrays, and structs.

**Architecture:** This phase adds a `borrow` module that runs after type checking and before lowering. It reconstructs enough local type information from the AST to identify owned local variables, marks owned locals moved when consumed by assignment, returns, and function calls, and rejects later use of moved locals. Borrow syntax and mutable/immutable reference checking are later Phase 5 slices.

**Tech Stack:** Rust 2021, Cargo, current Geo compiler modules.

## Global Constraints

- Existing tests must keep passing.
- Geo v1 uses a modest ownership checker.
- Values have one owner by default.
- Assignment and function calls move owned strings, arrays, and structs unless borrowed.
- The checker must reject obvious use-after-move.
- The checker must not reject scalar `int`, `bool`, `char`, integer-width, pointer, or slice values as moved.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: Add Move Checker Module

**Files:**
- Create: `src/borrow.rs`
- Modify: `src/lib.rs`
- Test: `tests/borrow_tests.rs`

**Interfaces:**
- Produces: `borrow::check(program: &Program) -> Result<(), Vec<Diagnostic>>`.
- Owned types are `Type::String`, `Type::Array(_)`, and `Type::Named(_)`.
- Moving an owned local marks it unavailable.
- Reading a moved local produces `use of moved value '<name>'`.

### Task 2: Wire Checker into Driver

**Files:**
- Modify: `src/driver.rs`
- Modify: `src/typecheck.rs` only if helper exposure is needed.

**Interfaces:**
- `geo check` runs parser, type checker, then ownership checker.
- `geo emit-asm`, `build`, and `run` also run ownership checking before lowering.

### Task 3: Verify

**Files:**
- Test: `tests/borrow_tests.rs`
- Existing test suite.

**Required tests:**
- Accept moving an owned value once.
- Reject using an owned string after move.
- Reject using an owned struct after move.
- Accept repeated scalar use.

Run: `cargo test`

Expected: all tests pass.

---

## Plan Self-Review

- Spec coverage: This covers the move-checking subset of v1 Phase 5.
- Placeholder scan: No placeholders remain.
- Type consistency: `borrow::check` and owned type definitions are consistent across plan tasks.
