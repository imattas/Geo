# Geo Native Handle File IO Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement direct compiler-owned handle open/write/close APIs on Linux and Windows.

**Architecture:** Add runtime-symbol dispatch and target-specific machine-code helpers to the existing ELF64 and PE64 executable writers. Reuse the existing native import layout for Win32 calls and preserve the current fallback boundary.

**Tech Stack:** Rust 2021, Geo IR, x86-64 machine code, Linux syscalls, Win64 Kernel32 imports, Cargo, WSL.

## Global Constraints

- Keep the compiler backend from scratch and free of compiler framework dependencies.
- Preserve Linux System V and Windows x64 ABI correctness.
- Match the existing `std.io` return conventions.
- Keep tests and documentation honest about unsupported runtime operations.

---

### Task 1: Add examples and backend coverage

**Files:** `compiler/geo/tests/elf_tests.rs`, `compiler/geo/tests/pe_tests.rs`, `examples/file_handle_exit.geo`, `examples/file_append_handle_exit.geo`

- [x] Add tests for direct open/write/close symbol paths and target structures.
- [x] Add Geo examples with explicit error exits.

### Task 2: Implement ELF64 helpers

**Files:** `compiler/geo_backend/src/elf.rs`

- [x] Emit three `openat` variants with read, truncate-write, and append flags.
- [x] Emit a bounded NUL-string `write` helper with complete-write validation.
- [x] Emit a native `close` helper and map all five symbols.

### Task 3: Implement PE64 helpers

**Files:** `compiler/geo_backend/src/pe.rs`

- [x] Emit three `CreateFileA` variants with the matching access/disposition values.
- [x] Emit a `WriteFile` helper that checks the stored byte count.
- [x] Emit a `CloseHandle` helper and map all five symbols.

### Task 4: Verify and document

**Files:** `.github/workflows/ci.yml`, `ROADMAP.md`, `STATUS.md`, `IMPROVEMENTS.md`

- [x] Add both targets to CI and update capability/gap notes.
- [x] Run full tests, native Linux/Windows smoke tests, push, and watch CI.
