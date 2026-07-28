# Geo Improvements

This file tracks practical improvements that would make Geo easier to expand, test, maintain, and eventually self-host.

## Highest Leverage

### 0. Keep The Compiler From Scratch

Current state: `cargo run -p xtask -- from-scratch` enforces that active Cargo manifests and lockfile do not introduce LLVM, Cranelift, MLIR, GCCJIT, or similar backend framework dependencies. `geo emit-obj --target x86_64-linux` exposes a compiler-owned ELF64 relocatable writer for constants, stack locals, System V register and stack-passed function parameters, integer arithmetic/logical/bitwise/shift operations, comparisons, labels, branches, pointer address/load/store operations, bounds-check runtime argument setup, string data, calls, symbols, and relocations in the current object subset. `geo emit-obj --target x86_64-windows` exposes a compiler-owned AMD64 COFF relocatable writer for the current object subset, including stack code, `.rdata` strings, function/data symbols, internal function calls, Windows x64 register arguments, call shadow space, and text relocations. The direct PE64 path wraps compiled Win64 machine code for current-subset programs, patches internal/data relocations, includes local bounds-check, `string_len`, `string_byte_at`, `string_find_byte`, `string_last_find_byte`, `string_index_of`, `string_compare`, `string_contains`, `string_starts_with`, `string_ends_with`, `string_eq`, `string_not_eq`, `string_less`, `string_less_or_equal`, `string_greater`, `string_greater_or_equal`, `string_is_empty`, `string_is_ascii`, `string_is_ascii_digit`, `string_is_ascii_hex_digit`, `string_is_ascii_alpha`, `string_is_ascii_lower`, `string_is_ascii_upper`, `string_is_ascii_alnum`, `string_is_ascii_identifier`, `string_is_ascii_whitespace`, `string_concat`, and console print helpers, and imports `GetStdHandle`, `WriteFile`, and `ExitProcess` when needed.

External assembler/linker tools are allowed only as temporary build-tool steps. The compiler pipeline itself must remain owned by Geo.

### 1. Split Compiler Internals Into Smaller Crates

Current state: the compiler crate is isolated at `compiler/geo`, diagnostics live in `compiler/geo_diagnostics`, and source loading lives in `compiler/geo_source`. Most syntax, semantic, IR, and backend phases still live inside the main compiler crate.

Recommended split:

- `compiler/geo_cli`: command-line entry point and user-facing commands.
- `compiler/geo_driver`: orchestration for check, build, run, test, and emit flows.
- `compiler/geo_syntax`: source files, spans, tokens, lexer, parser, AST.
- `compiler/geo_semantic`: name resolution, type checking, borrow checking.
- `compiler/geo_ir`: IR definitions, lowering, validation.
- `compiler/geo_codegen`: target lowering and backend selection.
- `compiler/geo_codegen_x86_64`: x86-64 NASM backend.
- `compiler/geo_object`: PE/ELF object writers.
- `compiler/geo_diagnostics`: diagnostics, rendering, source snippets.

Reason: self-hosting work needs stable internal boundaries. Smaller crates also reduce compile times and make testing each phase cleaner.

### 2. Add A Real Standard Library Source Tree

Current state: the compiler owns runtime metadata directly in `compiler/geo/src/runtime.rs`; the native runtime implementation lives in `library/geo_runtime`, and first-pass source-level standard library module declarations live in `library/std/src`.

Recommended layout:

```text
library/
  geo_runtime/
  std/
    io.geo
    mem.geo
    process.geo
    string.geo
    array.geo
    platform/
      linux.geo
      windows.geo
```

Reason: Geo code should eventually import stable modules like `std.io`. Keeping the runtime and source-level standard library separate avoids mixing platform ABI glue with user-facing APIs.

### 3. Expand The Bootstrap/Test Harness

Current state: Rust tests cover compiler stages and example compilation. `src/bootstrap` defines bootstrap stages and `src/tools/xtask` provides `layout`, `status`, and `verify`.

Recommended commands:

- `cargo run -p xtask -- check`
- `cargo run -p xtask -- test`
- `cargo run -p xtask -- examples`
- `cargo run -p xtask -- dist`

Reason: a compiler project needs repeatable full-stack checks: Rust tests, Geo example checks, Linux assembly emission, Windows assembly/PE emission, runtime linking, and future self-hosting samples.

### 4. Make The Backend Target Pipeline More Explicit

Current state: target support exists, including Linux and Windows x86-64 paths, NASM emission, a Linux ELF64 object writer path, and a PE writer path.

Recommended structure:

```text
IR
-> ABI lowering
-> machine-independent backend plan
-> target emitter
-> assembler/object writer
-> linker/runtime packaging
```

Reason: Windows x64 and System V AMD64 differ enough that ABI decisions should be represented before final assembly text emission.

### 5. Strengthen Diagnostics As A Product Feature

Current state: diagnostics include source-aware rendering and tests.

Recommended improvements:

- Stable diagnostic IDs such as `GEO0001`.
- Machine-readable JSON diagnostics.
- Multi-span diagnostics for imports, duplicate symbols, and borrow errors.
- Snapshot tests for important error rendering.
- Suggestions for common mistakes.

Reason: good diagnostics are core compiler quality, especially for a new language.

## Language Improvements

### Syntax And Ergonomics

- Keep canonical syntax:

```geo
import std.io

fn main() {
    println("Hello, world!")
}
```

- Treat `fn main()` as unit-returning and exit code `0`.
- Keep `fn main() -> int` for explicit exit status.
- Keep semicolons optional.
- Prefer `let` immutable and `var` mutable.
- Prefer `str` as the user-facing string type name while keeping `string` compatibility.

### Type System

- Add a real `unit` type.
- Add fixed-width integers and `usize`.
- Make integer conversions explicit.
- Add typed null or nullable design before expanding pointer-heavy APIs.
- Define string and array ownership rules before adding more collection APIs.

### Ownership And Safety

- Keep ownership lexical for v1.
- Enforce move checking for owned `str`, arrays, and structs.
- Allow many immutable borrows or one mutable borrow.
- Require `unsafe` for raw pointer dereference, pointer arithmetic, extern calls where needed, and unchecked indexing.
- Keep runtime/platform internals as the primary place for unsafe code.

## Runtime And Standard Library Improvements

### Core Runtime

- Keep runtime link integration in the compiler driver instead of a separate resolver crate.
- Stable runtime ABI for printing, allocation, process exit, panic, and file IO.
- Platform-specific implementations hidden behind a common ABI.
- Explicit runtime entry point for unit-returning `main`.
- Bounds-check trap path for strings and arrays.
- Minimal allocator abstraction with `alloc`, `realloc`, and `free`.

### Standard Library

Initial modules should be small and boring:

- `std.io`: `print`, `println`, `eprint`, file read/write.
- `std.mem`: allocation and memory copy/fill.
- `std.process`: exit, args, env basics.
- `std.string`: len, clone, concat, compare, substring.
- `std.array`: len, push, pop, get, set, slice.
- `std.fs`: paths, metadata, directory iteration, copy/rename/delete.

Reason: self-hosting examples need IO, strings, arrays, and diagnostics more than advanced abstractions.

## Backend Improvements

- Expand ELF64 object writing to cover aggregate layout and linkable runtime calls.
- Expand the Windows COFF writer beyond the current object subset and make the PE64 path handle broader runtime imports, allocation-backed strings, and external symbols through compiled machine code. The direct helper set now includes first and last substring search, non-overlapping counting, and decimal parsing; next prioritize allocation-backed string results.
- Add relocation tests for object writers.
- Harden stack-passed argument support beyond the first four Windows x64 registers.
- Add a simple register allocator after the IR and ABI boundaries are stable.
- Add debug-friendly assembly comments behind a flag.
- Add direct syscall experiments behind explicit target/runtime flags, not as the default path.

## Repository Improvements

- Add `CONTRIBUTING.md` with local verification commands.
- Add `docs/architecture/` for compiler pipeline docs.
- Add `docs/language/` for syntax and type-system reference.
- Add `docs/runtime/` for runtime ABI and standard library design.
- Add `tests/ui/` for diagnostic snapshot tests.
- Add `tests/run-pass/`, `tests/check-pass/`, and `tests/check-fail/` for Geo source tests.
- Move generated `.exe` and `.asm` artifacts out of the repository root and into `target/`.

## Tooling Improvements

- Add formatter tests for canonical syntax.
- Add `geo fmt --check`.
- Add `geo --version`.
- Add `geo dump-tokens`, `geo dump-ast`, `geo dump-ir`, and `geo dump-asm` for compiler development.
- Add CI for Windows and Linux.
- Add a small `xtask` crate for repeatable dev workflows.
- Add benchmark fixtures once backend behavior stabilizes.

## Self-Hosting Improvements

The next self-hosting target should be meaningful but narrow:

1. A Geo lexer that tokenizes a subset of Geo source.
2. A Geo diagnostic formatter that renders file, line, column, and caret.
3. A Geo AST builder for a small expression grammar.
4. A Geo mini parser that uses the lexer and AST structs.
5. A Geo file echo tool that proves file IO and buffers.

Do not start with the full compiler rewrite. First make Geo capable of writing compiler-shaped components cleanly.
