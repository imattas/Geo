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
  geo_layout/
    Cargo.toml
    src/
    tests/
library/
  geo_runtime/
    Cargo.toml
    src/
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
- `compiler/geo_layout`
- `src/bootstrap`
- `src/tools/xtask`

The compiler crate is the default workspace member, so root-level `cargo run -- ...` runs the `geo` compiler.

## Verified Commands

These commands were run successfully after the repository restructure:

```powershell
cargo fmt --check
cargo test --workspace --quiet
cargo run -p xtask --quiet -- layout
cargo run --quiet -- check examples\return_42.geo --target x86_64-linux
cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\workspace_return_42_win.asm
```

`git status --short` currently reports:

```text
fatal: not a git repository (or any of the parent directories): .git
```

That means this checkout is not currently initialized as a Git repository.

## Compiler Capabilities

The `geo` CLI currently exposes:

- `geo check`
- `geo emit-asm`
- `geo build`
- `geo run`
- `geo fmt`
- `geo test`

The compiler currently has modules for:

- AST
- borrow checking
- CLI
- diagnostics
- driver orchestration
- IR
- lexer
- lowering
- object support
- parser
- PE support
- name/module resolution
- runtime integration
- source loading
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

The exact implemented behavior is covered by the Rust test suite under `compiler/geo/tests` and the Geo examples under `examples`.

## Runtime Status

Current runtime layout:

- Compiler-managed native runtime implementation: `library/geo_runtime/geo_runtime.c`
- Source-level standard library modules: `library/std/src`

The compiler owns runtime metadata in `compiler/geo/src/runtime.rs`. No separate runtime Cargo crate remains in the workspace.

## Tooling Status

Current repository-level tooling:

- `src/bootstrap`: declares bootstrap stages for host compiler, native runtime, standard library, self-hosting examples, and distribution.
- `src/tools/xtask`: provides `layout`, `status`, and `verify` commands.
- `compiler/geo_layout`: validates that the expected compiler/library/src workspace shape exists.

## Documentation Status

Existing design/spec documents:

- `docs/superpowers/specs/2026-07-24-geo-language-design.md`
- `docs/superpowers/specs/2026-07-24-geo-v1-self-hosting-foundation-design.md`
- `docs/superpowers/specs/2026-07-28-geo-clean-core-syntax-design.md`

Existing implementation plans cover the original v0.1 path, v1 phases, clean syntax, runtime/stdlib scaffold, object writing, target ABI work, multi-file resolution, and self-hosting examples.

## Known Gaps

- No Git repository metadata in this checkout.
- No root-level `CONTRIBUTING.md`.
- Compiler internals are still mostly one crate.
- Runtime ABI is not yet documented as a stable contract.
- Formatter is minimal.
- Distribution/install layout is not defined.
- CI is not configured in this checkout.

## Current Priority

The next best technical move is Phase 1 from `ROADMAP.md`: continue splitting compiler internals into smaller crates before adding significantly more language surface.

That preserves momentum while reducing the risk of the compiler becoming difficult to change as v1 grows.
