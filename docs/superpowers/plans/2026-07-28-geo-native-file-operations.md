# Geo Native File Operations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add direct compiler-owned append, touch, and remove file operations for Linux ELF64 and Windows PE64.

**Architecture:** Extend each native executable writer's runtime-symbol dispatch and target ABI helpers. The driver continues to choose direct emission whenever the complete program is supported; unsupported programs retain the existing assembly path until later backend milestones remove that fallback.

**Tech Stack:** Rust 2021, Geo IR, hand-emitted x86-64 machine code, Linux syscalls, Win64 Kernel32 imports, Cargo tests, WSL smoke tests.

## Global Constraints

- The compiler backend must remain written from scratch in this repository.
- Linux x86-64 and Windows x86-64 are first-class targets.
- Runtime helpers must use the existing Geo symbol ABI and return `0` on success and `1` on failure.
- Do not add a compiler-local C runtime resolver or backend framework dependency.
- Preserve the existing NASM path only as an explicit fallback for unsupported direct programs.

---

### Task 1: Add failing native backend coverage

**Files:**
- Modify: `compiler/geo/tests/elf_tests.rs`
- Modify: `compiler/geo/tests/pe_tests.rs`
- Create: `examples/append_file_exit.geo`
- Create: `examples/touch_remove_file_exit.geo`

**Interfaces:**
- The tests consume `std.io.append_file`, `std.io.touch_file`, and `std.io.remove_file`.
- The backends must recognize relocation symbols named `append_file`, `touch_file`, and `remove_file`.

- [x] Add ELF assertions that direct emission returns an executable containing the new symbols' machine code path.
- [x] Add PE assertions that direct emission includes the new runtime imports.
- [x] Add executable examples with explicit integer exit codes.
- [x] Run the focused backend tests after adding the coverage.

### Task 2: Implement Linux helpers

**Files:**
- Modify: `compiler/geo_backend/src/elf.rs`

**Interfaces:**
- `build_runtime_text` maps the three symbols to helper emitters.
- Helpers use Linux SysV arguments: path in `RDI`, data in `RSI`.

- [x] Emit `append_file` using `openat` with `O_WRONLY|O_CREAT|O_APPEND`, `write`, and `close`.
- [x] Emit `touch_file` using `openat` with `O_WRONLY|O_CREAT`, then `close`.
- [x] Emit `remove_file` using the `unlink` syscall.
- [x] Normalize success and failure to `0` and `1`.
- [x] Run focused ELF tests and the Linux examples.

### Task 3: Implement Windows helpers

**Files:**
- Modify: `compiler/geo_backend/src/pe.rs`

**Interfaces:**
- `build_compiled_text` maps the three symbols to `PeHelperRvas` entries.
- Helpers use Windows x64 arguments: path in `RCX`, data in `RDX`.

- [x] Add `DeleteFileA` to the compiler-owned import table when remove is referenced.
- [x] Emit append with `CreateFileA`, append access, `OPEN_ALWAYS`, `WriteFile`, and `CloseHandle`.
- [x] Emit touch with `CreateFileA`, write access, `OPEN_ALWAYS`, and `CloseHandle`.
- [x] Emit remove with `DeleteFileA` and normalize its BOOL result.
- [x] Run focused PE tests and the Windows examples.

### Task 4: Document and verify the milestone

**Files:**
- Modify: `ROADMAP.md`
- Modify: `STATUS.md`
- Modify: `IMPROVEMENTS.md`
- Modify: `.github/workflows/ci.yml`

- [x] Add Linux and Windows build commands for the new examples.
- [x] Add direct-runtime capability and remaining-gap notes.
- [x] Run formatting, workspace tests, xtask verification, native smoke tests, and CI.
- [ ] Commit the implementation and push the verified update.
