# Geo v1 Target-Aware ABI Emission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make NASM emission respect the selected Linux System V or Windows x64 target ABI for register arguments.

**Architecture:** Keep `emit_nasm(&IrProgram)` as the Linux-compatible default for existing tests, and add `emit_nasm_for_target(&IrProgram, &Target)`. The backend derives argument registers from `Target.abi`; the driver uses the target-aware function.

**Tech Stack:** Rust backend module, existing `target` metadata, Cargo integration tests.

## Global Constraints

- Geo v1 supports Linux x86-64, System V ABI.
- Geo v1 supports Windows x86-64, Windows x64 ABI.
- NASM assembly output remains the stable baseline.
- No subagents are used for this implementation.

---

### Task 1: Add ABI Shape Tests

**Files:**
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: CLI `geo emit-asm --target`
- Produces: tests asserting Linux calls use `rdi/rsi` and Windows calls use `rcx/rdx`.

### Task 2: Implement Target-Aware Emission

**Files:**
- Modify: `src/x86_64.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `emit_nasm_for_target(program: &IrProgram, target: &Target) -> String`

- [ ] Add ABI register selection in `x86_64.rs`.
- [ ] Thread target-aware emission through `driver::compile_to_asm`.

### Task 3: Verify

- [ ] Run `cargo fmt`.
- [ ] Run focused ABI tests.
- [ ] Run `cargo test`.

## Self-Review

- Spec coverage: Covers first-class Linux/Windows ABI assembly shape for register arguments.
- Known gap: Stack-passed arguments and Windows shadow-space handling remain separate backend work.
- Placeholder scan: No placeholders remain.
