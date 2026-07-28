# Geo v1 Static String Lowering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lower string literals to IR and emit NASM data labels so std/runtime calls with string literal arguments can produce assembly.

**Architecture:** String literals become static null-terminated byte data in the backend. Lowering emits a `StringConst` instruction that binds a virtual value to a generated label and string content. The x86-64 backend emits `section .data` entries and loads string addresses into stack value slots with RIP-relative `lea`.

**Tech Stack:** Rust 2021, Cargo, current Geo IR/lowering/NASM backend.

## Global Constraints

- Existing tests must keep passing.
- Static string lowering is not full owned string runtime support.
- Assembly output remains the production path for v1.
- Runtime linking for `std.io.println` is not part of this phase.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: IR and Lowering

**Files:**
- Modify: `src/ir.rs`
- Modify: `src/lower.rs`
- Modify: `tests/lower_tests.rs`

**Interfaces:**
- Extends `Instruction` with `StringConst { dst: ValueId, label: String, value: String }`.
- `Expr::String(value)` lowers to `StringConst`.

### Task 2: NASM Data Emission

**Files:**
- Modify: `src/x86_64.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Backend emits `section .data` when string constants are present.
- Backend emits one label per string constant as decimal `db` bytes followed by `0`.
- Backend lowers `StringConst` with `lea rax, [rel LABEL]` and stores the pointer in the value slot.

### Task 3: Verify

Run: `cargo test`

Expected: all tests pass.

---

## Plan Self-Review

- Spec coverage: This advances v1 runtime/string support by making string literals backend-visible.
- Placeholder scan: No placeholders remain.
- Type consistency: `Instruction::StringConst` is named consistently across lowering and backend.
