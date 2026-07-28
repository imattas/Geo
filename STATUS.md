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
cargo run --quiet -- emit-obj examples\return_42.geo --target x86_64-linux -o target\workspace_return_42_linux.o
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
- direct Linux ELF64 relocatable object emission for the current object subset

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

External tools such as NASM and the platform linker are currently allowed as build-tool steps. They are not the compiler pipeline. `geo emit-obj --target x86_64-linux` now exercises a compiler-owned ELF64 relocatable writer for the current object subset; broader Linux linking and Windows COFF/PE object coverage remain roadmap work.

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
- Direct object emission is Linux ELF64-only today and covers a subset of lowered IR.

## Current Priority

The next best technical move is to expand compiler-owned object emission while continuing the Phase 1 split into real compiler crates.

That reduces dependence on external assemblers without letting the compiler internals become difficult to change as v1 grows.
