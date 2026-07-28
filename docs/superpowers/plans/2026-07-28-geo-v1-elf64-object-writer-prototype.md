# Geo v1 ELF64 Object Writer Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a direct Linux ELF64 relocatable object writer prototype for simple integer-returning functions and runtime calls.

**Architecture:** Add `src/object.rs` with a backend interface that consumes `IrProgram` and emits an ELF64 relocatable byte vector. The prototype emits `.text`, `.rela.text`, `.symtab`, `.strtab`, and `.shstrtab`; defined functions become global symbols and unresolved calls become relocation entries.

**Tech Stack:** Rust 2021, existing IR, Linux ELF64 binary structures, Cargo tests.

## Global Constraints

- Direct object writing is introduced through interfaces and a Linux ELF64 prototype.
- The prototype must include sections, symbols, and relocations sufficient for integer-returning functions and runtime calls.
- Assembly output remains the production path for v1.
- No subagents are used for this implementation.

---

### Task 1: Object Writer Tests

**Files:**
- Create: `tests/object_tests.rs`

**Interfaces:**
- Consumes: `object::emit_elf64_relocatable(program: &IrProgram) -> Vec<u8>`
- Produces: tests that assert ELF magic, relocatable file type, section names, symbol names, and call relocation entries.

- [ ] **Step 1: Add ELF header test**

Construct a program from `fn main() -> int { return 42 }`, lower it, emit object bytes, and assert:
- bytes start with `0x7f E L F`
- `e_type == 1`
- section-name table contains `.text`, `.symtab`, `.strtab`, `.shstrtab`
- string table contains `main`

- [ ] **Step 2: Add relocation test**

Construct a program with `import std.io` and `return println("Geo")`, lower it, emit object bytes, and assert:
- section-name table contains `.rela.text`
- string table contains `println`
- relocation section has nonzero payload.

### Task 2: Object Writer Implementation

**Files:**
- Create: `src/object.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `emit_elf64_relocatable(program: &IrProgram) -> Vec<u8>`

- [ ] **Step 1: Emit simple text bytes**

For each function emit `push rbp; mov rbp, rsp`, support `Const` followed by `Return` as `mov eax, imm32; pop rbp; ret`, and support `Call` by emitting `call rel32` placeholder plus relocation metadata.

- [ ] **Step 2: Emit ELF sections**

Write ELF64 header, section payloads, and section headers for null, `.text`, `.rela.text`, `.symtab`, `.strtab`, and `.shstrtab`.

- [ ] **Step 3: Emit symbols and relocations**

Add null symbol, `.text` section symbol, global function symbols, and undefined call target symbols. Emit `R_X86_64_PLT32` relocations for calls with addend `-4`.

### Task 3: Verify

- [ ] Run `cargo fmt`.
- [ ] Run `cargo test --test object_tests`.
- [ ] Run `cargo test`.

## Self-Review

- Spec coverage: Adds the object writer interface and Linux ELF64 relocatable prototype required by v1.
- Known gap: The prototype intentionally supports a subset of IR and is not yet wired as the production build path.
- Placeholder scan: No placeholders remain.
