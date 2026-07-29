# Geo Improvements

This file tracks practical improvements that would make Geo easier to expand, test, maintain, and eventually self-host.

## Highest Leverage

### 0. Keep The Compiler From Scratch

Current state: `cargo run -p xtask -- from-scratch` enforces that active Cargo manifests and lockfile do not introduce LLVM, Cranelift, MLIR, GCCJIT, or similar backend framework dependencies. `geo emit-obj --target x86_64-linux` exposes a compiler-owned ELF64 relocatable writer for constants, stack locals, System V register and stack-passed function parameters, integer arithmetic/logical/bitwise/shift operations, comparisons, labels, branches, pointer address/load/store operations, bounds-check runtime argument setup, string data, calls, symbols, and relocations in the current object subset. `geo emit-obj --target x86_64-windows` exposes a compiler-owned AMD64 COFF relocatable writer for the current object subset, including stack code, `.rdata` strings, function/data symbols, internal function calls, Windows x64 argument registers, call shadow space, and text relocations. The direct ELF64 and PE64 paths now include compiler-owned file reads, default-file reads, line input, allocation, `alloc_copy`, and `mem_copy`/`mem_move`/`mem_zero`/`mem_fill`/`mem_find`/`mem_compare`/`mem_equal`/`mem_is_zero`/`mem_reverse` buffer primitives; the Windows path emits the required Win32 APIs directly and the Linux path emits syscalls directly.
The PE64 path now also emits `std.io.write_file`, `std.io.append_file`, `std.io.touch_file`, `std.io.remove_file`, and the handle open/read/write/flush/seek/close operations directly through compiler-owned Win32 calls.
Both executable backends now also emit native `int_to_string`, `usize_to_string`, and `bool_to_string` formatters, with Linux execution and Windows PE64 build coverage.

External assembler/linker tools are allowed only as temporary build-tool steps. The compiler pipeline itself must remain owned by Geo.

### 1. Split Compiler Internals Into Smaller Crates

Current state: syntax, IR, semantic analysis, lowering, target backends/object writers, and driver orchestration are owned by dedicated compiler crates. `compiler/geo` remains the compatibility/library shell and binary entry point; diagnostics and source loading remain dedicated crates.

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

Current state: the compiler owns runtime metadata directly in `compiler/geo_semantic/src/runtime.rs`; native runtime emission lives in `compiler/geo_backend`, and first-pass source-level standard library module declarations live in `library/std/src`.

Recommended layout:

```text
library/
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

Current state: target support exists, including Linux and Windows x86-64 paths, NASM text emission for compatibility, and compiler-owned ELF64/PE64 object and executable writers. Executable builds do not use an external assembler, linker, or C runtime.

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

Current state: lexer and parser diagnostics carry token spans and are rendered with source paths, line/column locations, and underlines through module loading. Semantic diagnostics now preserve expression-level spans, top-level statement fallbacks, and originating module paths.

Recommended improvements:

- Stable diagnostic IDs such as `GEO0001`.
- Machine-readable JSON diagnostics.
- Multi-span diagnostics for imports, duplicate symbols, and borrow errors.
- Snapshot tests for important error rendering.
- Suggestions for common mistakes.

Runtime lifetime progress: `string_clone`, `string_from_byte`, and
`string_concat`, non-null `string_slice` results, path-based `read_file`
results, handle-based `file_read_to_string` results, and `read_line` results
now store the compiler-owned mapping header and can be released through
`std.string.string_free`.

Reason: good diagnostics are core compiler quality, especially for a new language.

## Language Improvements

### Syntax And Ergonomics

The compiler now has an AST-backed canonical formatter exposed through `geo fmt`.
It formats declarations, blocks, control flow, types, literals, calls, operators,
arrays, structs, and match expressions. The next formatter improvements are comment
preservation, source-map-aware edits, and configurable style checks.

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
- Expand the Linux ELF64 executable writer and Windows COFF writer beyond the current object subsets, and make both targets handle broader runtime imports, allocation-backed strings, and external symbols through compiled machine code. The direct helper set now includes first and last substring search, non-overlapping counting, decimal parsing, allocation-backed `string_concat`, `std.process.exit`, `std.mem.alloc`, path-based read/write/append/touch/truncate/remove file operations, handle open/read/write/flush/seek/close operations, and compiler-owned memory primitives; next prioritize richer metadata, richer process APIs, and allocation-backed substring/array results.
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
- Keep `examples/v1` in the direct executable-writer regression suite so self-hosting examples cannot silently regress to a non-native build path.
- Move generated `.exe` and `.asm` artifacts out of the repository root and into `target/`.
- Add native runtime coverage for directory enumeration, recursive directory mutation, and the remaining directory APIs. Basic `create_dir`/`remove_dir` mutation, `rename_file`, `copy_file`, and file timestamp queries now have direct two-target coverage.
- Add native runtime coverage for string comparison, substring, Unicode, formatting, and conversion APIs.
- Keep backend tests focused on executable behavior as well as instruction encodings; ordering predicates previously exposed a flag-preservation bug this way.
- Add native suffix matching and substring index/count operations next.
- Boundary-aware UTF-8 navigation and slicing are now native on both targets, including end clamping and malformed-input handling. Add formatting and conversion support next. Strict validity checking rejects malformed and out-of-range sequences.
- Native UTF-8 character extraction, first-codepoint search, and owned codepoint-to-string conversion now share compiler-owned logic on ELF64 and PE64; keep extending this family with richer formatting primitives.
- PE64 section placement now follows emitted text size, and the Windows array runtime smoke set exercises typed mutation, search, growth, copying, and cleanup end to end.
- Array search now compares full element widths on both ELF64 and PE64, with a typed `u16` execution fixture in CI.
- Treat read-only string runtime calls as shared borrows so scanner and parser code can inspect owned source text repeatedly.
- Extend the byte-array runtime to the remaining typed algorithms before using it for fully dynamic token buffers; full-width push/set/fill/resize, capacity growth, clone, clear, release, reverse, search, contains, count, indexed insertion, indexed removal, extension, and bounded copying are now native on both targets.
- Complete allocation lifetime semantics by extending the header-backed `alloc`/`free`/`realloc` contract to every compiler-owned allocation helper, with double-free and invalid-pointer diagnostics where the language can expose them.
- Array truncation, indexed insertion/removal, swap-based removal, and first/last pop are now native on both targets; finish generic copy/resize next so compiler-owned buffers can mutate without private helpers.
- Preserve pointee width through IR dereference/store lowering so `u8` buffers do not accidentally read or overwrite adjacent bytes.

## Tooling Improvements

- Add formatter tests for canonical syntax.
- Add `geo fmt --check`.
- Add `geo --version`.
- `geo dump-tokens`, `geo dump-ast`, and `geo dump-ir` are now available for compiler development; add `geo dump-asm` with stable machine-readable output next.
- Add CI for Windows and Linux.
- Add a small `xtask` crate for repeatable dev workflows.
- Add benchmark fixtures once backend behavior stabilizes.
- Execute a representative PE64 smoke set on Windows CI, including allocation-backed string cleanup and path-based and handle-based file reads, so the native Windows backend is validated beyond byte emission.

## Self-Hosting Improvements

The next self-hosting target should be meaningful but narrow:

1. A Geo lexer that tokenizes a subset of Geo source.
2. A Geo diagnostic formatter that renders file, line, column, and caret.
3. A Geo AST builder for a small expression grammar.
4. A Geo mini parser that uses the lexer and AST structs.
5. A Geo file echo tool that proves file IO and buffers.

Do not start with the full compiler rewrite. First make Geo capable of writing compiler-shaped components cleanly.
