# Geo Roadmap

Geo's v1 direction is a native systems language with a clean Rust/C++/C#/Java-influenced syntax, a from-scratch compiler pipeline, Linux and Windows support, and enough standard library/runtime support to write meaningful compiler components in Geo.

## Current North Star

Geo v1 should compile this style of program:

```geo
import std.io

fn main() {
    println("Hello, world!")
}
```

It should also support explicit exit codes:

```geo
import std.io

fn main() -> int {
    println("Hello, world!")
    return 0
}
```

The compiler remains written in Rust for v1. The self-hosting goal is that Geo can compile meaningful subsets of compiler-shaped Geo programs: lexer, parser, diagnostics, AST, buffers, file IO, and small CLI tools.

## Phase 0: Repository Foundation

Status: complete.

Deliverables:

- Rust-style workspace root.
- Compiler located under `compiler/geo`.
- Runtime crate located under `library/geo_runtime`.
- Layout validation crate located under `compiler/geo_layout`.
- Bootstrap model located under `src/bootstrap`.
- Workspace automation tool located under `src/tools/xtask`.
- Source-level standard library modules located under `library/std/src`.
- Root Cargo workspace with compiler as the default member.
- Existing examples and tests adjusted to the new layout.

Verification:

- `cargo fmt --check`
- `cargo test --workspace --quiet`
- `cargo run -p xtask --quiet -- layout`
- `cargo run --quiet -- check examples\return_42.geo --target x86_64-linux`
- `cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\workspace_return_42_win.asm`

## Phase 1: Workspace Hardening

Goal: make the project structure scale before adding more language surface.

Deliverables:

- Expand `xtask` into the main developer workflow for repeatable commands.
- Add `compiler/geo_syntax` for source, tokens, lexer, parser, and AST.
- Add `compiler/geo_diagnostics` for diagnostic data and rendering.
- Add `compiler/geo_driver` for check/build/run orchestration.
- Keep `compiler/geo` as the CLI binary crate.
- Add workspace-level developer docs.

Acceptance criteria:

- Existing tests still pass.
- Root `cargo run -- check examples\return_42.geo` still works.
- Compiler phase tests live next to the crates they validate.

## Phase 2: Clean Core Syntax

Goal: make the approved syntax canonical.

Deliverables:

- Parse `import std.io`.
- Parse unit-returning functions with omitted return type.
- Make `fn main()` exit with code `0`.
- Make `println` return `unit`.
- Add `unit` as an internal type.
- Add `var` for mutable locals.
- Reject assignment to immutable `let`.
- Accept `str` as the preferred alias for strings.
- Keep existing syntax compatible.

Acceptance criteria:

- `examples/hello_world.geo` uses the canonical `fn main()` form.
- `geo check examples/hello_world.geo` passes.
- `geo build examples/hello_world.geo` produces a native executable where host tools are available.
- Existing v1 examples still check or emit assembly.

## Phase 3: Standard Library Source Layout

Goal: introduce real Geo standard library modules.

Deliverables:

- Add `library/std/io.geo`.
- Add `library/std/mem.geo`.
- Add `library/std/process.geo`.
- Add `library/std/string.geo`.
- Add `library/std/array.geo`.
- Add import resolution for `std.*`.
- Keep platform-specific implementation hidden behind runtime symbols.

Acceptance criteria:

- `import std.io` resolves without local files.
- `println` lowers to a runtime call.
- Standard library module failures produce source-aware diagnostics.

## Phase 4: Runtime ABI

Goal: stabilize the boundary between generated Geo code and platform runtime code.

Deliverables:

- Define runtime symbols for print, println, allocation, free, realloc, exit, panic, file open/read/write/close.
- Add Linux and Windows implementations behind the same public ABI.
- Add runtime entry support for unit-returning `main`.
- Add trap path for bounds errors.
- Add tests that assert runtime symbol references in emitted assembly.

Acceptance criteria:

- Hello world links through the runtime.
- Explicit `main() -> int` still returns the requested exit code.
- File IO examples compile and link where host tools are available.

## Phase 5: Language Data Types

Goal: support the data model required by compiler-shaped programs.

Deliverables:

- Fixed-width integers and `usize`.
- `char`.
- String literals and owned `str`.
- Struct declarations and field access.
- Arrays and slices.
- Indexing with bounds checks.
- String concatenation through runtime helpers.

Acceptance criteria:

- `examples/v1/buffer.geo` checks and emits assembly.
- `examples/v1/ast.geo` checks and emits assembly.
- Bounds checks route through the runtime trap path.

## Phase 6: Modules, Imports, And Externs

Goal: support multi-file programs and platform boundaries.

Deliverables:

- Package root discovery.
- Import graph construction.
- Cycle detection.
- Module-private and exported symbol rules.
- `extern fn` declarations.
- Platform-aware runtime bindings.

Acceptance criteria:

- Multi-file examples compile from a package entry.
- Cyclic imports are rejected with a clear diagnostic.
- Extern calls are lowered with target ABI rules.

## Phase 7: Ownership Foundation

Goal: add enough safety for owned compiler data structures.

Deliverables:

- Move checking for owned strings, arrays, and structs.
- Immutable borrows.
- Mutable borrows.
- Escaping-borrow checks.
- `unsafe` blocks for raw pointer operations.
- Diagnostics for use-after-move and conflicting borrows.

Acceptance criteria:

- Safe v1 examples pass borrow checking.
- Unsafe operations outside `unsafe` are rejected.
- Borrow errors point to both the original borrow/move and the invalid use.

## Phase 8: Backend And Object Writers

Goal: make Linux and Windows first-class targets without depending on accidental host behavior.

Deliverables:

- ABI lowering layer for System V AMD64 and Windows x64.
- Stack-passed arguments beyond register limits.
- Data sections for strings and globals.
- Direct ELF64 relocatable object writer.
- PE64 writer support for runtime imports and external symbols.
- NASM backend retained as the stable fallback.

Acceptance criteria:

- Linux target emits valid System V calls.
- Windows target emits valid Windows x64 calls.
- Object writer tests cover sections, symbols, and relocations.

## Phase 9: Self-Hosting Foundation Examples

Goal: prove Geo can express real compiler components.

Deliverables:

- `examples/v1/buffer.geo`
- `examples/v1/lexer.geo`
- `examples/v1/diagnostics.geo`
- `examples/v1/ast.geo`
- `examples/v1/file_echo.geo`
- `examples/v1/mini_parser.geo`

Acceptance criteria:

- Each example checks.
- Each example emits Linux and Windows assembly.
- Runtime-using examples link on supported hosts.
- The examples use the public standard library, not private runtime symbols.

## Phase 10: Distribution

Goal: make Geo usable outside the development checkout.

Deliverables:

- `geo --version`.
- Installable binary layout.
- Standard library discovery from installed path.
- Runtime library packaging.
- Release smoke tests.
- Basic language reference docs.

Acceptance criteria:

- A fresh checkout can run one documented command to verify the compiler.
- An installed compiler can build hello world without repo-relative paths.
- Windows and Linux release artifacts are documented.
