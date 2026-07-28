# Geo Native Handle Read Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add direct `std.io.file_read_to_string` support for Linux and Windows.

**Architecture:** Extend the existing native runtime dispatch and use target-native file-size, allocation, rewind, and read operations while preserving caller-owned handles.

**Tech Stack:** Rust 2021, Geo IR, x86-64 machine code, Linux syscalls, Win64 Kernel32 imports.

## Global Constraints

- Keep the compiler and runtime helper emission from scratch.
- Preserve target ABI and NUL-terminated Geo string conventions.
- Return a null pointer on failure and do not close the caller's handle.

---

### Task 1: Tests and example

**Files:** `compiler/geo/tests/elf_tests.rs`, `compiler/geo/tests/pe_tests.rs`, `examples/file_read_handle_len_exit.geo`

- [x] Add backend assertions for direct handle reads.
- [x] Add a write/reopen/read/length Geo executable.

### Task 2: Native helpers

**Files:** `compiler/geo_backend/src/elf.rs`, `compiler/geo_backend/src/pe.rs`

- [x] Emit Linux `lseek`, `mmap`, rewind, and `read` code.
- [x] Emit Windows `GetFileSize`, `VirtualAlloc`, and `ReadFile` code.
- [x] Resolve the new helper symbol in both relocation patchers.

### Task 3: Verify and document

**Files:** `.github/workflows/ci.yml`, `ROADMAP.md`, `STATUS.md`, `IMPROVEMENTS.md`

- [x] Add target builds and update the remaining-gap notes.
- [x] Run full tests, native smoke tests, push, and watch CI.
