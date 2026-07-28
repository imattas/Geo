# Geo Roadmap

Geo's v1 direction is a native systems language with a clean Rust/C++/C#/Java-influenced syntax, a from-scratch compiler pipeline, Linux and Windows support, and enough standard library/runtime support to write meaningful compiler components in Geo.

From-scratch means Geo owns lexing, parsing, semantic analysis, IR, target lowering, ABI handling, assembly/object emission, diagnostics, and runtime integration. LLVM, Cranelift, MLIR, GCCJIT, and C code generation are out of bounds for the compiler pipeline.

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
- Compiler-managed native runtime implementation located under `library/geo_runtime`.
- Diagnostic support crate located under `compiler/geo_diagnostics`.
- Source management crate located under `compiler/geo_source`.
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
- `cargo run --quiet -- emit-obj examples\object_backend.geo --target x86_64-linux -o target\workspace_object_backend_linux.o`
- `cargo run --quiet -- emit-obj examples\hello_world.geo --target x86_64-linux -o target\workspace_hello_world_linux.o`
- `cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\workspace_return_42_win.asm`

## Phase 1: Workspace Hardening

Goal: make the project structure scale before adding more language surface.

Deliverables:

- Expand `xtask` into the main developer workflow for repeatable commands.
- Add `compiler/geo_syntax` for source, tokens, lexer, parser, and AST.
- Move remaining source/syntax responsibilities out of `compiler/geo` into real compiler crates.
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
- Keep runtime metadata and link integration inside the custom compiler, not in a resolver crate.
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
- System V register and stack-passed parameter handling for compiler-owned ELF64 function bodies.
- Stack-passed argument support beyond Windows x64 register limits.
- Data sections for strings and globals.
- Direct ELF64 relocatable object writer.
- `geo emit-obj` CLI path for compiler-owned Linux ELF64 objects.
- `geo emit-obj` CLI path for compiler-owned Windows AMD64 COFF objects.
- PE64 writer support for runtime imports, local helper symbols, and external symbols.
- NASM backend retained as the stable fallback.

Acceptance criteria:

- Linux target emits valid System V calls.
- Windows target emits valid Windows x64 calls.
- Object writer tests cover sections, symbols, and relocations.
- CI emits a Linux object without invoking NASM.

Current PE64 progress: current-subset programs now use compiled Win64 machine code for entry, internal calls, `.rdata` references, bounds checks, `print`/`println`, `string_len`, `string_byte_at`, `string_compare`, `string_contains`, `string_starts_with`, `string_ends_with`, `string_eq`, `string_not_eq`, `string_less`, `string_less_or_equal`, `string_greater`, `string_greater_or_equal`, `string_is_empty`, `string_is_ascii`, and fixed-buffer `string_concat`.

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
