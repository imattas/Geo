# Geo Status

Last updated: 2026-07-28.

## Repository Layout

Current layout:

```text
Cargo.toml
Cargo.lock
compiler/
  geo/
    Cargo.toml
    src/
    tests/
  geo_diagnostics/
    Cargo.toml
    src/
    tests/
  geo_source/
    Cargo.toml
    src/
    tests/
library/
  geo_runtime/
    geo_runtime.c
  std/
    src/
    src/platform/
src/
  bootstrap/
    Cargo.toml
    src/
    tests/
  tools/
    xtask/
      Cargo.toml
      src/
      tests/
docs/
  superpowers/
examples/
target/
```

Workspace members:

- `compiler/geo`
- `compiler/geo_diagnostics`
- `compiler/geo_source`
- `src/bootstrap`
- `src/tools/xtask`

The compiler crate is the default workspace member, so root-level `cargo run -- ...` runs the `geo` compiler.

## Verified Commands

These commands were run successfully after the repository restructure:

```powershell
cargo fmt --check
cargo test --workspace --quiet
cargo run -p xtask --quiet -- from-scratch
cargo run -p xtask --quiet -- layout
cargo run --quiet -- check examples\return_42.geo --target x86_64-linux
cargo run --quiet -- emit-obj examples\object_backend.geo --target x86_64-linux -o target\workspace_object_backend_linux.o
cargo run --quiet -- emit-obj examples\hello_world.geo --target x86_64-linux -o target\workspace_hello_world_linux.o
cargo run --quiet -- emit-obj examples\coff_backend.geo --target x86_64-windows -o target\workspace_coff_backend_windows.obj
cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\workspace_return_42_win.asm
```

`git status --short` currently reports:

```text
clean
```

The repository is initialized and tracks `origin/main` at `https://github.com/imattas/Geo`.

## Compiler Capabilities

The `geo` CLI currently exposes:

- `geo check`
- `geo emit-asm`
- `geo emit-obj`
- `geo build`
- `geo run`
- `geo fmt`
- `geo test`

The compiler currently has modules for:

- AST
- borrow checking
- CLI
- diagnostics through `compiler/geo_diagnostics`
- driver orchestration
- IR
- lexer
- lowering
- object support
- parser
- PE support
- name/module resolution
- runtime integration
- source loading through `compiler/geo_source`
- target handling
- tokens
- type checking
- x86-64 assembly emission

## Language Baseline

Current implemented surface includes a substantial v1-facing subset:

- `.geo` source files
- functions
- typed parameters
- explicit return values
- integer and boolean basics
- local bindings and assignment
- arithmetic and comparisons
- control flow
- structs/enums and aggregate examples
- pointers/references examples
- modules/import examples
- runtime-backed examples
- Linux and Windows target-aware assembly paths
- direct Linux ELF64 relocatable object emission for constants, stack locals, System V register and stack-passed function parameters, integer addition/subtraction/multiplication/division/remainder, shifts, logical/bitwise operations, comparisons, labels, conditional/unconditional jumps, address-of, dereference, pointer stores, bounds-check runtime argument setup, string data, calls, symbols, and relocations in the current object subset
- direct Windows AMD64 COFF relocatable object emission for stack code, `.rdata` strings, function/data symbols, internal function calls, Windows x64 register arguments, call shadow space, and text relocations in the current object subset
- direct PE64 executable emission now wraps compiled Win64 machine code for `main`, internal calls, `.rdata` references, bounds-check calls, `string_len`, `string_byte_at`, `string_compare`, `string_contains`, `string_starts_with`, `string_ends_with`, `string_eq`, `string_not_eq`, `string_less`, `string_less_or_equal`, `string_greater`, `string_greater_or_equal`, `string_concat`, and simple `print`/`println` string console output before calling `ExitProcess`

The exact implemented behavior is covered by the Rust test suite under `compiler/geo/tests` and the Geo examples under `examples`.

## Runtime Status

Current runtime layout:

- Compiler-managed native runtime implementation: `library/geo_runtime/geo_runtime.c`
- Source-level standard library modules: `library/std/src`

The compiler owns runtime metadata in `compiler/geo/src/runtime.rs`. No separate runtime Cargo crate remains in the workspace.

## Tooling Status

Current repository-level tooling:

- `src/bootstrap`: declares bootstrap stages for host compiler, native runtime, standard library, self-hosting examples, and distribution.
- `src/tools/xtask`: provides `from-scratch`, `layout`, `status`, and `verify` commands.
- `compiler/geo_diagnostics`: owns diagnostic data structures and rendering.
- `compiler/geo_source`: owns source file loading, source locations, and module path mapping.

## From-Scratch Policy

The compiler pipeline is implemented in this repository. The active workspace has no LLVM, Cranelift, MLIR, GCCJIT, or similar compiler backend framework dependency.

External tools such as NASM and the platform linker are currently allowed as build-tool steps. They are not the compiler pipeline. `geo emit-obj --target x86_64-linux` now exercises a compiler-owned ELF64 relocatable writer with stack-slot machine code, System V register and stack-passed parameter handling, bounds-check runtime ABI setup, branch patching, `.rodata` strings, and text relocations for the current object subset. `geo emit-obj --target x86_64-windows` now emits a compiler-owned AMD64 COFF relocatable for the current object subset, including stack code, `.rdata` strings, function/data symbols, internal function calls, Windows x64 argument registers, call shadow space, and text relocations. The direct PE64 path now uses compiled Win64 machine code for current-subset programs, patches internal and data relocations, includes local bounds-check, string length, string byte access, string comparison, string containment, string prefix/suffix checks, equality and ordering string wrappers, string concatenation, and console print helpers, and falls back to a constrained static-output path only when compiled helper resolution is not yet available. Broader Linux linking and broader Windows COFF/PE object coverage remain roadmap work.

## Documentation Status

Existing design/spec documents:

- `docs/superpowers/specs/2026-07-24-geo-language-design.md`
- `docs/superpowers/specs/2026-07-24-geo-v1-self-hosting-foundation-design.md`
- `docs/superpowers/specs/2026-07-28-geo-clean-core-syntax-design.md`

Existing implementation plans cover the original v0.1 path, v1 phases, clean syntax, runtime/stdlib scaffold, object writing, target ABI work, multi-file resolution, and self-hosting examples.

## Known Gaps

- Compiler internals are still mostly one crate.
- Runtime ABI is not yet documented as a stable contract.
- Formatter is minimal.
- Distribution/install layout is not defined.
- Direct object emission does not yet cover aggregate layout, full runtime linking from compiler-owned objects, or broad Windows COFF objects beyond the current object subset.

## Current Priority

The next best technical move is to expand compiler-owned object emission while continuing the Phase 1 split into real compiler crates.

That reduces dependence on external assemblers without letting the compiler internals become difficult to change as v1 grows.
