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
- Compiler-owned native runtime emission located under `compiler/geo_backend`.
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
- Carry lexer and parser spans into source-aware diagnostics.

Acceptance criteria:

- Existing tests still pass.
- Root `cargo run -- check examples\return_42.geo` still works.
- Compiler phase tests live next to the crates they validate.

Progress: `compiler/geo_syntax` now owns the AST, token model, lexer, parser, and canonical formatter, `compiler/geo_ir` owns the typed machine-independent IR, `compiler/geo_semantic` owns resolution, type checking, borrow checking, and runtime symbol metadata, `compiler/geo_codegen` owns AST-to-IR lowering, `compiler/geo_backend` owns target definitions, x86-64 assembly, ELF/COFF object writers, and ELF/PE executable emission, and `compiler/geo_driver` owns CLI and compile orchestration. Lexer/parser token spans now flow through `compiler/geo_source` into rendered diagnostics. `compiler/geo` remains the compatibility/library shell and binary entry point.

Package directories are now accepted by the driver. The resolver discovers
`main.geo`, resolves relative imports from the package root, rejects import
cycles, and native builds can consume the merged multi-file program on Linux
and Windows.

Callable module visibility is now explicit. `pub fn` and `pub extern fn` are
exported across imports; private functions remain available to code in their
defining module but are excluded from importing scopes during type checking.
Modules that contain no visibility annotations retain implicit exports for
backward compatibility.

Public aliases, constants, structs, and enums now participate in the same
qualified import rewrite, while private qualified types and constants remain
unresolved at the import boundary.

Struct fields now support explicit `pub` visibility. Private imported fields are
rejected during type checking, while same-module access and legacy modules keep
working. Aggregate layout remains independent of visibility.

The semantic checker now builds per-function visibility environments for
unqualified imported structs, enums, and constants, and checks aliases before
type expansion so private aliases cannot disappear into their underlying type.

The compiler driver now exposes `dump-tokens`, `dump-ast`, and `dump-ir` for
inspecting each owned frontend and lowering stage directly from the CLI. Parsed
functions also retain their source span, expression spans, top-level statement
spans, and originating module path so semantic diagnostics can be rendered against
the correct file.

The syntax crate also owns the AST-backed canonical formatter used by `geo fmt`; it
formats the parsed language surface without delegating to an external formatter.

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

Progress: `int_to_string`, `usize_to_string`, and `bool_to_string` now lower to
native compiler-owned formatters on Linux and Windows, with direct execution
and PE64 build coverage.
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

Progress: the borrow checker now models temporary statement borrows separately
from borrows retained by reference locals, and records reference origins so
returning a local reference to stack data is rejected indirectly as well as
through a direct `&value` return.
Move state now uses definite-after-branch merging: a move on only one `if` path
does not poison later code, while moves on all paths remain rejected. Loop-body
moves are likewise not assumed definite after a possibly empty loop.
Semantic and borrow scopes now agree for nested blocks. Locals declared inside
conditionals, loops, `unsafe` blocks, and expression blocks do not leak, and
reference borrows owned by those locals are released at scope exit.
Nested lexical scopes may shadow outer locals, while same-scope duplicates are
still rejected. Reference reassignment updates ownership by releasing the old
origin and retaining the new one.
Reference origins now support chained escape diagnostics and dereference
reborrow targeting, keeping lifetime accounting attached to the actual pointee.
Path-dependent reference assignments retain an origin union across branches,
so later replacement releases every possible source borrow.

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
- NASM text emission remains available only for `geo emit-asm` compatibility and debugging; executable builds use compiler-owned ELF64/PE64 writers and fail explicitly when a program is outside their supported subset.

Acceptance criteria:

- Linux target emits valid System V calls.
- Windows target emits valid Windows x64 calls.
- Object writer tests cover sections, symbols, and relocations.
- CI emits Linux and Windows objects without invoking NASM, and executable builds do not resolve a C runtime.

Current PE64 progress: current-subset programs now use compiled Win64 machine code for entry, internal calls, `.rdata` references, bounds checks, `print`/`println`, string helpers, allocation-backed `string_concat`, `std.process.exit`, `std.mem.alloc`, and `std.io.read_file` using compiler-emitted Windows helpers and imports. The read path uses the Win64 ABI directly and returns a NUL-terminated buffer for Geo string operations.

Current ELF64 executable progress: current-subset Linux programs now use a compiler-owned `_start` wrapper, patched internal/data relocations, direct `string_len`, `string_byte_at`, `string_is_empty`, `string_is_ascii`, `string_find_byte`, `string_from_byte`, allocation-backed `string_clone`, `print`, `println`, `std.process.exit`, `std.mem.alloc`, `std.mem.alloc_copy`, `std.mem.mem_copy`, `std.mem.mem_move`, `std.mem.mem_zero`, `std.mem.mem_fill`, `std.mem.mem_find`, `std.mem.mem_compare`, `std.mem.mem_equal`, `std.mem.mem_is_zero`, `std.mem.mem_reverse`, `std.io.read_file`, `std.io.read_file_or`, `std.io.read_line`, `std.io.write_file`, `std.io.append_file`, `std.io.touch_file`, `std.io.remove_file`, `std.io.file_exists`, `std.io.file_is_file`, `std.io.file_is_dir`, `std.io.file_is_empty`, `std.io.file_size`, `std.io.file_open`, `std.io.file_open_write`, `std.io.file_open_append`, `std.io.file_write`, `std.io.file_flush`, `std.io.file_close`, `std.io.file_seek`, `std.io.file_read_to_string`, and allocation-backed `string_concat` using Linux syscalls. Broader standard-library runtime coverage and failure-path handling remain open.

The ELF64 and PE64 paths now also emit `std.io.truncate_file` natively.

The ELF64 and PE64 paths now also emit `std.fs.create_dir` and
`std.fs.remove_dir` through compiler-owned syscall and Win32 helper paths.

The ELF64 and PE64 paths now also emit `std.fs.rename_file` through compiler-owned
`rename` and `MoveFileA` paths.

The ELF64 and PE64 paths now also emit `std.fs.copy_file` through a compiler-owned
chunked Linux file-copy loop and the native Win32 `CopyFileA` API.

The ELF64 and PE64 paths now also emit file access, modification, and creation
timestamp queries; Windows `FILETIME` values are normalized to Unix seconds.

The ELF64 and PE64 paths now also emit `dir_entry_count` through native Linux
and Win32 directory enumeration APIs.

The ELF64 and PE64 paths now also emit `dir_entry_name` through native Linux
and Win32 directory enumeration APIs, returning an owned Geo string for the
selected non-dot entry.

The ELF64 and PE64 paths now also emit `dir_entry_path` by composing the
directory with the selected entry and releasing intermediate allocations.

The ELF64 and PE64 paths now also emit `process_id` through Linux `getpid` and
a Windows PEB read without adding a runtime-library dependency.

The ELF64 and PE64 paths now also emit `platform_path_separator` as a native
target constant.

The ELF64 and PE64 paths now also emit owned `platform_os`, `platform_arch`,
and `platform_newline` strings through compiler-owned allocation.

The ELF64 and PE64 paths now also emit `path_is_absolute` with POSIX,
Windows-root, and drive-letter checks.

The ELF64 and PE64 paths now also emit `path_file_name` by scanning both
separator styles and returning an owned basename through the native slice
allocator.

The ELF64 and PE64 paths now also emit `path_parent` by scanning both
separator styles and returning the owned prefix before the final separator.

The ELF64 and PE64 paths now also emit `path_extension`, including separator
resetting and dotfile handling, through the owned string-slice runtime.

The ELF64 and PE64 paths now also emit `path_stem`, preserving multi-dot
stems and dotfiles through the same owned slice ABI.

The ELF64 and PE64 paths now also emit `path_without_extension`, preserving
the full path prefix while removing only the final non-dotfile extension.

The ELF64 and PE64 paths now also emit `path_with_extension`, composing the
compiler-owned path and string helpers to replace, normalize, or remove a
file extension without a C runtime dependency.

Current PE64 executable progress: current-subset Windows programs now use compiler-emitted Win64 machine code for entry, internal calls, console IO, allocation, memory primitives, string byte access, byte search, string predicates, file reads, line input, `std.io.write_file`, `std.io.append_file`, `std.io.copy_file`, `std.io.touch_file`, `std.io.remove_file`, `std.io.file_exists`, `std.io.file_is_file`, `std.io.file_is_dir`, `std.io.file_is_empty`, `std.io.file_size`, `std.io.file_open`, `std.io.file_open_write`, `std.io.file_open_append`, `std.io.file_write`, `std.io.file_flush`, `std.io.file_close`, `std.io.file_seek`, and `std.io.file_read_to_string` through direct `CreateFileA`/`CopyFileA`/`WriteFile`/`CloseHandle`/`DeleteFileA`/`GetFileAttributesA`/`GetFileSize`/`SetFilePointerEx`/`FlushFileBuffers`/`VirtualAlloc`/`ReadFile` imports. Broader standard-library runtime coverage and failure-path handling remain open.

The PE64 layout now derives data-section placement from emitted text size, and
the native Windows array lifecycle, mutation, typed-composition, resize, and
runtime fixtures execute successfully.

Typed array search now compares complete element widths for `index_of`,
`last_index_of`, `contains`, and `count` on both native executable writers.

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
- Each example builds through the compiler-owned Linux ELF64 and Windows PE64 executable writers.
- Runtime-using examples link on supported hosts.
- The examples use the public standard library, not private runtime symbols.
- Native string comparison and ordering predicates are covered by direct ELF64 and PE64 executable tests.
- Native substring containment and prefix matching are covered by direct ELF64 and PE64 executable tests.
- Native suffix matching is covered by direct ELF64 and PE64 executable tests.
- Native first substring indexing is covered by direct ELF64 and PE64 executable tests.
- Native non-overlapping substring counting is covered by direct ELF64 and PE64 executable tests.
- Native last substring indexing is covered by direct ELF64 and PE64 executable tests.
- Native last byte search is covered by direct ELF64 and PE64 executable tests.
- Native byte slicing is covered by direct ELF64 and PE64 executable tests.
- Native UTF-8 codepoint counting is covered by direct ELF64 and PE64 executable tests.
- Native UTF-8 codepoint lookup is covered by direct ELF64 and PE64 executable tests.
- Native strict UTF-8 validity checking is covered by direct ELF64 and PE64 executable builds, including malformed and out-of-range sequences.
- Native UTF-8 navigation is covered by direct ELF64 and PE64 executable builds, including codepoint offsets, forward/backward boundaries, reverse index lookup, terminal offsets, and continuation-byte rejection.
- Native boundary-aware UTF-8 slicing is covered by direct ELF64 and PE64 builds, including end clamping, empty ranges, and malformed-input failure handling.
- Native UTF-8 character extraction and first-codepoint search are covered by direct ELF64 and PE64 builds, including multi-byte characters, misses, and out-of-range access.
- The v1 lexer example performs a real source scan and executes through both compiler-owned writers.
- The v1 parser example now scans and parses source text for a canonical
  function grammar with explicit cursor and error state through both
  compiler-owned writers, and materializes a small AST aggregate.
- A compiler-owned byte-array runtime path is covered by a direct executable fixture on ELF64 and PE64.
- A compiler-owned array lifecycle path is covered by direct ELF64 and PE64 builds, including reserve, clone, clear, and release.
- A compiler-owned array mutation path is covered by direct ELF64 and PE64 builds, including truncation, last/first pop, byte shifting, and invalid-input behavior.
- A compiler-owned indexed array mutation path is covered by direct ELF64 and PE64 builds, including insertion, removal, swap-based removal, bounds failures, and capacity failures.
- A compiler-owned array composition path is covered by direct ELF64 and PE64 builds, including payload extension and bounded indexed copying.
- Full-width typed `push`/`set`/`fill`, element-scaled `extend`/`resize`, and bounded `copy` are covered by direct ELF64 and PE64 execution fixtures.
- Byte-array resize growth, shrink, fill, capacity failure, and release are covered by direct ELF64 and PE64 execution.
- Direct `std.mem.alloc`/`free`/`realloc`/`alloc_copy` lifetime is covered by Linux execution and Windows PE64 compilation fixtures. `string_clone`, `string_from_byte`, `string_concat`, non-null `string_slice` results, path-based `read_file` results, handle-based `file_read_to_string` results, and `read_line` results now share the lifetime header through `string_free`.
- Native byte-array algorithms are covered by direct ELF64 and PE64 builds, including first/last access, fill, reverse, index search, last-index search, contains, and count.
- Fixed-size aggregate array copies now lower element-by-element, including
  arrays nested in copied structs; continue extending this representation to
  runtime-backed collection values and aggregate function arguments.
- The Linux ELF64 writer now preserves the initial process stack and implements
  native `std.process` argument accessors. The PE64 writer now provides
  quote-aware `arg_count`, `arg_exists`, owned `arg` token extraction, and
  `arg_or` fallback values now lower directly from `GetCommandLineA` and
  `VirtualAlloc`; expand escaped-quote edge cases next.

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

## Ownership Milestone

Status: in progress.

The borrow checker now models mutable reborrows as ownership transitions:
`&mut *view` suspends the parent reference, retains the child borrow, rejects
parent use during the child lifetime, and restores the parent when the child
scope ends. Continue extending this model to branch joins, nested reborrow
chains, and richer place expressions.
