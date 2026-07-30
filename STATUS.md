# Geo Status

Last updated: 2026-07-29.

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
  geo_syntax/
    Cargo.toml
    src/
    tests/
  geo_ir/
    Cargo.toml
    src/
    tests/
  geo_semantic/
    Cargo.toml
    src/
    tests/
  geo_codegen/
    Cargo.toml
    src/
    tests/
  geo_backend/
    Cargo.toml
    src/
    tests/
  geo_driver/
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
  runtime/
examples/
target/
```

Workspace members:

- `compiler/geo`
- `compiler/geo_syntax`
- `compiler/geo_ir`
- `compiler/geo_semantic`
- `compiler/geo_codegen`
- `compiler/geo_backend`
- `compiler/geo_driver`
- `compiler/geo_diagnostics`
- `compiler/geo_source`
- `src/bootstrap`
- `src/tools/xtask`

The direct native runtime now formats signed integers, `usize`, and booleans
as compiler-owned strings on both x86-64 backends. The Linux path was executed
under WSL; the Windows path was validated by PE64 emission.

`std.io.eprint` is also emitted directly to stderr on Linux and through the
Win32 standard-error handle on PE64.

The compiler crate is the default workspace member, so root-level `cargo run -- ...` runs the `geo` compiler.

Package directories are now valid compiler inputs: `main.geo` is discovered as
the package entry, relative imports are resolved from that directory, and the
directory can be checked, assembled, or built directly.

Imported callable declarations now have an explicit visibility boundary:
`pub fn` and `pub extern fn` are exported, while private functions remain
usable by their own module and are rejected from importing callers.
Modules without any visibility annotations retain legacy implicit exports for
source compatibility.

The same boundary now covers `pub const`, `pub type`, `pub struct`, and
`pub enum` declarations, including qualified imported type and constant use.
Struct fields now use the same explicit syntax: `pub value: T` exports a field,
while an unmarked field is private in explicitly annotated modules. Field
provenance is tracked through semantic checking, and legacy unannotated modules
continue to expose their fields for compatibility.
Semantic environments now apply that module boundary to unqualified imported
structs, enums, constants, and type aliases as well; declarations remain
available to their defining module without leaking into importing callers.

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
cargo run --quiet -- build examples\read_file_len_windows_exit.geo --target x86_64-windows -o target\read_file_len_windows_exit.exe
cargo run --quiet -- build examples\read_file_or_len_exit.geo --target x86_64-linux -o target\read_file_or_len_exit
cargo run --quiet -- build examples\read_file_or_len_windows_exit.geo --target x86_64-windows -o target\read_file_or_len_windows_exit.exe
cargo run --quiet -- build examples\read_line_len_exit.geo --target x86_64-linux -o target\read_line_len_exit
cargo run --quiet -- build examples\read_line_len_windows_exit.geo --target x86_64-windows -o target\read_line_len_windows_exit.exe
cargo run --quiet -- build examples\mem_runtime_exit.geo --target x86_64-linux -o target\mem_runtime_exit
cargo run --quiet -- build examples\mem_runtime_exit.geo --target x86_64-windows -o target\mem_runtime_exit.exe
cargo run --quiet -- build examples\string_from_byte_len_exit.geo --target x86_64-linux -o target\string_from_byte_len_exit
cargo run --quiet -- build examples\string_from_byte_len_exit.geo --target x86_64-windows -o target\string_from_byte_len_exit.exe
cargo run --quiet -- build examples\string_clone_len_exit.geo --target x86_64-linux -o target\string_clone_len_exit
cargo run --quiet -- build examples\string_clone_len_exit.geo --target x86_64-windows -o target\string_clone_len_exit.exe
cargo run --quiet -- build examples\alloc_copy_exit.geo --target x86_64-linux -o target\alloc_copy_exit
cargo run --quiet -- build examples\alloc_copy_exit.geo --target x86_64-windows -o target\alloc_copy_exit.exe
cargo run --quiet -- build examples\mem_fill_exit.geo --target x86_64-linux -o target\mem_fill_exit
cargo run --quiet -- build examples\mem_fill_exit.geo --target x86_64-windows -o target\mem_fill_exit.exe
cargo run --quiet -- build examples\mem_compare_exit.geo --target x86_64-linux -o target\mem_compare_exit
cargo run --quiet -- build examples\mem_compare_exit.geo --target x86_64-windows -o target\mem_compare_exit.exe
cargo run --quiet -- build examples\mem_predicates_exit.geo --target x86_64-linux -o target\mem_predicates_exit
cargo run --quiet -- build examples\mem_predicates_exit.geo --target x86_64-windows -o target\mem_predicates_exit.exe
cargo run --quiet -- build examples\mem_reorder_exit.geo --target x86_64-linux -o target\mem_reorder_exit
cargo run --quiet -- build examples\mem_reorder_exit.geo --target x86_64-windows -o target\mem_reorder_exit.exe
cargo run --quiet -- build examples\write_file_exit.geo --target x86_64-linux -o target\write_file_exit
cargo run --quiet -- build examples\write_file_exit.geo --target x86_64-windows -o target\write_file_exit.exe
cargo run --quiet -- build examples\append_file_exit.geo --target x86_64-linux -o target\append_file_exit
cargo run --quiet -- build examples\append_file_exit.geo --target x86_64-windows -o target\append_file_exit.exe
cargo run --quiet -- build examples\touch_remove_file_exit.geo --target x86_64-linux -o target\touch_remove_file_exit
cargo run --quiet -- build examples\touch_remove_file_exit.geo --target x86_64-windows -o target\touch_remove_file_exit.exe
cargo run --quiet -- build examples\file_handle_exit.geo --target x86_64-linux -o target\file_handle_exit
cargo run --quiet -- build examples\file_handle_exit.geo --target x86_64-windows -o target\file_handle_exit.exe
cargo run --quiet -- build examples\file_append_handle_exit.geo --target x86_64-linux -o target\file_append_handle_exit
cargo run --quiet -- build examples\file_append_handle_exit.geo --target x86_64-windows -o target\file_append_handle_exit.exe
cargo run --quiet -- build examples\file_read_handle_len_exit.geo --target x86_64-linux -o target\file_read_handle_len_exit
cargo run --quiet -- build examples\file_read_handle_len_exit.geo --target x86_64-windows -o target\file_read_handle_len_exit.exe
cargo test --locked -p geo --test compile_tests v1_examples_build_with_compiler_owned_executable_writers
cargo run --locked --quiet -- build examples\array_runtime_exit.geo --target x86_64-windows -o target\array_runtime_exit.exe
cargo run --locked --quiet -- build examples\array_resize_exit.geo --target x86_64-windows -o target\array_resize_exit.exe
cargo run --locked --quiet -- build examples\array_typed_composition_exit.geo --target x86_64-windows -o target\array_typed_composition_exit.exe
cargo run --locked --quiet -- build examples\array_mutation_exit.geo --target x86_64-windows -o target\array_mutation_exit.exe
cargo run --locked --quiet -- build examples\array_lifecycle_exit.geo --target x86_64-windows -o target\array_lifecycle_exit.exe
cargo run --locked --quiet -- build examples\array_typed_search_exit.geo --target x86_64-windows -o target\array_typed_search_exit.exe
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
- `geo dump-tokens`
- `geo dump-ast`
- `geo dump-ir`
- `geo test`
- `geo --version`

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

The developer introspection commands print the compiler-owned token stream,
parsed AST, and lowered IR without invoking an external compiler framework.

Semantic type and borrow diagnostics now render the originating module's file,
line, and top-level statement range across imported modules. The next diagnostic
refinement is to carry the narrower expression span.

The borrow checker now distinguishes statement-scoped temporary borrows from
borrows retained by named reference locals. Calls such as `inspect(&value)` no
longer keep a borrow alive after the call, while `let view: &T = &value` retains
the borrow and indirect returns through `view` are rejected as escaping.
Control-flow analysis now treats a move as definite only when every `if` path
moves the value; moves inside `while` and `for` bodies are not propagated past
loops that may execute zero times. Borrow and reference-origin state is merged
conservatively across conditional branches.
Type and borrow analysis now restore lexical locals after `if`, loop, `unsafe`,
and expression-block scopes. Retained borrows owned by inner reference locals
are released at scope exit, while moves of outer values still propagate.
Nested scopes now support intentional shadowing while same-scope duplicate
locals remain errors. Reassigning a reference releases its previous source
borrow before retaining the new one.
Reference-chain diagnostics now trace through intermediate references to the
root source value, and dereference reborrows target the pointee rather than the
reference variable itself.
Branch merges now preserve all possible reference origins. Reassigning a
reference after path-dependent assignments releases every source borrow rather
than leaving one branch's origin live.

The PE64 writer relocates `.rdata` and `.idata` after emitted text grows, and
the native Windows array smoke set covers typed mutation, search, growth,
copying, and cleanup end to end.

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
- native object and executable writers preserve full signed 64-bit integer constants instead of truncating literals to 32-bit immediates
- direct Linux ELF64 executable emission for the current System V subset, including a compiler-owned `_start` exit wrapper, internal/data relocations, `string_len`, `print`, `println`, `std.process.exit`, `std.mem.alloc`, `std.mem.free`, `std.io.read_file`, `std.io.write_file`, and allocation-backed `string_concat` through Linux syscalls
- direct PE64 executable emission now wraps compiled Win64 machine code for `main`, internal calls, `.rdata` references, bounds-check calls, `string_len`, `string_byte_at`, `string_find_byte`, `string_last_find_byte`, `string_index_of`, `string_last_index_of`, `string_count`, `string_parse_int`, `string_compare`, `string_contains`, `string_starts_with`, `string_ends_with`, `string_eq`, `string_not_eq`, `string_less`, `string_less_or_equal`, `string_greater`, `string_greater_or_equal`, `string_is_empty`, `string_is_ascii`, `string_is_ascii_digit`, `string_is_ascii_hex_digit`, `string_is_ascii_alpha`, `string_is_ascii_lower`, `string_is_ascii_upper`, `string_is_ascii_alnum`, `string_is_ascii_identifier`, `string_is_ascii_whitespace`, allocation-backed `string_concat`, `std.process.exit`, `std.mem.alloc`, and simple `print`/`println` string console output before calling `ExitProcess`
- direct PE64 executable emission also includes compiler-owned `std.io.read_file` using `CreateFileA`, `GetFileSize`, `VirtualAlloc`, `ReadFile`, and `CloseHandle`, with a NUL-terminated result for Geo string helpers
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.io.read_file_or`, returning the caller-provided default string when the file cannot be opened or read
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.io.read_line` with bounded native input buffers and newline termination
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_copy`, `std.mem.mem_move`, and `std.mem.mem_zero` buffer primitives
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.string.string_from_byte`
- direct ELF64 and PE64 executable emission also includes compiler-owned allocation-backed `std.string.string_clone`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.alloc_copy`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_fill`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_find`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_compare`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_equal` and `std.mem.mem_is_zero`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.mem.mem_reverse`
- direct PE64 executable emission also includes compiler-owned `std.io.write_file`
- direct ELF64 executable emission also includes compiler-owned `std.io.append_file`, `std.io.touch_file`, and `std.io.remove_file` through `openat`, `write`, `close`, and `unlink` syscalls
- direct PE64 executable emission also includes compiler-owned `std.io.append_file`, `std.io.touch_file`, and `std.io.remove_file` through `CreateFileA`, `WriteFile`, `CloseHandle`, and `DeleteFileA`
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.io.file_open`, `std.io.file_open_write`, `std.io.file_open_append`, `std.io.file_write`, and `std.io.file_close` through native descriptors/handles
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.io.file_read_to_string` through native handle reads and allocation-backed NUL-terminated strings
- direct ELF64 and PE64 executable emission also includes compiler-owned `std.io.file_exists` through native filesystem metadata checks
- the six `examples/v1` compiler-shaped programs now build through direct ELF64 and PE64 executable writers; aggregate indexing is guarded by the compiler-owned bounds helper

The exact implemented behavior is covered by the Rust test suite under `compiler/geo/tests` and the Geo examples under `examples`.

## Runtime Status

Current runtime layout:

- Compiler-owned native runtime implementation: `compiler/geo_backend/src/elf.rs` and `compiler/geo_backend/src/pe.rs`
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

The compiler owns executable emission for the supported subset: Linux builds write ELF64 images directly and Windows builds write PE64 images directly. `geo build` no longer invokes NASM, a platform linker, or a C runtime; unsupported programs receive an explicit native-backend diagnostic. NASM text emission remains available only through `geo emit-asm` as a compatibility/debugging output. `geo emit-obj --target x86_64-linux` exercises a compiler-owned ELF64 relocatable writer, and the Windows object path emits compiler-owned AMD64 COFF relocatables. Broader language and runtime coverage remain roadmap work.
- The direct native paths now also provide compiler-owned `std.io.read_line` helpers with bounded buffers and newline termination, plus `std.mem.alloc_copy` for native buffer duplication.

## Documentation Status

Existing design/spec documents:

- `docs/superpowers/specs/2026-07-24-geo-language-design.md`
- `docs/superpowers/specs/2026-07-24-geo-v1-self-hosting-foundation-design.md`
- `docs/superpowers/specs/2026-07-28-geo-clean-core-syntax-design.md`
- `docs/runtime/ABI.md`

Existing implementation plans cover the original v0.1 path, v1 phases, clean syntax, runtime/stdlib scaffold, object writing, target ABI work, multi-file resolution, and self-hosting examples.

## Known Gaps

- Compiler internals are now split across syntax, IR, semantic, lowering, backend, and driver crates; `compiler/geo` is the compatibility/library shell and binary entry point.
- Runtime ABI is documented in `docs/runtime/ABI.md`; versioning and compatibility guarantees remain open until the ABI is frozen.
- Lexer and parser diagnostics now carry token spans and are attached to source paths, lines, columns, and underlines during module loading. Semantic diagnostics now preserve expression-level spans, top-level statement fallbacks, and originating module paths.
- `geo fmt` now parses the program and emits canonical indentation, declaration,
  statement, type, and expression layout. Comment preservation and configurable
  style options remain open.
- Distribution/install layout is not defined.
- Direct object emission now covers scalarized struct and fixed-array ABI calls and returns; full runtime linking from compiler-owned objects and broad Windows COFF coverage remain open.
- Direct path-based file operations cover append, touch, remove, read, write, existence checks, file/directory classification, empty checks, and file size on Linux and Windows.
- Direct native `create_dir` and `remove_dir` support now uses Linux `mkdir`/`rmdir` syscalls and Win32 `CreateDirectoryA`/`RemoveDirectoryA`, with cross-platform mutation fixtures.
- Direct Linux `create_dir_all` now uses a compiler-emitted bounded `mkdir -p` helper that creates each POSIX prefix and accepts existing directories; Windows recursive directory support remains open.
- Direct native `rename_file` support now uses Linux `rename` and Win32 `MoveFileA`, with cross-platform source/destination cleanup fixtures.
- Direct native `copy_file` support now uses a compiler-emitted chunked Linux
  `open`/`read`/`write` path and Win32 `CopyFileA`, with ELF64, PE64, and CI
  execution fixtures.
- Direct native file timestamp queries now return Unix seconds on both targets:
  Linux reads `stat` fields, while PE64 converts Win32 `FILETIME` values.
- Direct native `dir_entry_count` now traverses Linux `getdents64` records and
  Win32 `FindFirstFileA`/`FindNextFileA` results, with cleanup fixtures on both
  executable writers.
- Direct native `dir_entry_name` now skips dot entries, returns an owned Geo
  string, and uses Linux `getdents64` or Win32 `FindFirstFileA`/`FindNextFileA`
  enumeration with cleanup fixtures on both executable writers.
- Direct native `dir_entry_path` now composes an owned directory-plus-entry
  path and releases intermediate allocations on both executable writers.
- Direct native `process_id` now uses Linux `getpid` and a Windows PEB read,
  keeping the executable writers free of a C runtime or process-ID import.
- Direct native `platform_path_separator` now returns the target separator
  constant from both executable writers.
- Direct native `platform_os`, `platform_arch`, and `platform_newline` now
  return owned strings on both executable writers.
- Direct native `path_is_absolute` now recognizes POSIX roots, Windows roots,
  and drive-letter roots on both executable writers.
- Direct native `path_file_name` now scans both slash styles and returns an
  owned basename through the compiler-owned string-slice runtime on both
  executable writers.
- Direct native `path_parent` now scans both slash styles and returns an
  owned parent path, including an owned empty string for paths without a
  separator, on both executable writers.
- Direct native `path_extension` now returns the final filename extension,
  ignores separator-directory dots, and treats dotfiles as extensionless on
  both executable writers.
- Direct native `path_stem` now preserves multi-dot stems, resets at both
  separator styles, and preserves dotfiles on both executable writers.
- Direct native `path_without_extension` now removes only the final
  non-dotfile extension while preserving directory prefixes and dotfiles.
- Direct native `path_with_extension` now replaces the final extension,
  normalizes optional leading dots, removes extensions when requested, and
  releases all compiler-owned temporary strings on ELF64 and PE64.
- Direct native `truncate_file` support now uses the Linux `truncate` syscall and Win32 `CreateFileA`/`SetFilePointerEx`/`SetEndOfFile` paths, with Linux execution and Windows PE64 execution coverage.
- Direct native `file_seek` support now uses Linux `lseek` and Win32 `SetFilePointerEx`, with compiler-owned Linux and PE64 fixtures that rewrite a file at an offset.
- Direct native `file_flush` support now uses Linux `fsync` and Win32 `FlushFileBuffers`, with compiler-owned Linux and PE64 durability fixtures.
- Direct Linux and Windows string runtime coverage includes byte access, empty checks, ASCII validation, and byte search.
- Direct Linux and Windows string runtime coverage also includes lexical comparison, equality, inequality, and ordering predicates.
- Direct Linux and Windows string runtime coverage also includes substring containment and prefix matching.
- Direct Linux and Windows string runtime coverage also includes suffix matching.
- Direct Linux and Windows string runtime coverage also includes first substring indexing.
- Direct Linux and Windows string runtime coverage also includes non-overlapping substring counting.
- Direct Linux and Windows string runtime coverage also includes last substring indexing.
- Direct Linux and Windows string runtime coverage also includes last byte search.
- Direct Linux and Windows string runtime coverage also includes allocation-backed byte slicing.
- Direct Linux and Windows string runtime coverage also includes UTF-8 codepoint counting for valid UTF-8.
- Direct Linux and Windows string runtime coverage also includes UTF-8 codepoint lookup for valid UTF-8.
- Direct Linux and Windows string runtime coverage now includes strict UTF-8 validity checking for overlong encodings, surrogate ranges, truncation, and out-of-range four-byte sequences.
- Direct Linux and Windows string runtime coverage now includes native UTF-8 codepoint-index to byte-offset conversion, next/previous boundary navigation, byte-offset to codepoint-index lookup, and boundary validation.
- Direct Linux and Windows string runtime coverage now includes boundary-aware UTF-8 slicing with end clamping and invalid-input handling.
- Direct Linux and Windows string runtime coverage now includes native UTF-8 character extraction and first-codepoint search, with multi-byte and out-of-range regression cases.
- Direct Linux and Windows string runtime coverage now includes owned UTF-8 codepoint-to-string conversion for ASCII, multi-byte, four-byte, and invalid surrogate inputs.
- Direct Linux and Windows array runtime coverage now includes byte-element allocation, length/capacity, indexed read/write, and push within the initial capacity.
- Direct Linux and Windows array lifecycle coverage now includes capacity growth, payload-preserving clone, clear, and native release.
- Direct Linux and Windows array mutation coverage now includes truncation, last-element pop, first-element pop with byte shifting, and invalid empty/growth handling.
- Direct Linux and Windows array mutation coverage now includes indexed insertion/removal, swap-based removal, bounds failures, and capacity failures.
- Direct Linux and Windows array buffer coverage now includes native extend and bounded indexed copy operations.
- A two-byte-element direct fixture now exercises full-width push/set, element-scaled extend, and bounded copy on ELF64, with matching PE64 compilation coverage.
- Direct Linux and Windows array runtime coverage now includes byte-array resize growth, shrink, fill, capacity failure, and release.
- Direct Linux and Windows byte-array algorithm coverage now includes first/last element access, fill, reverse, index search, last-index search, contains, and count.
- Typed pointer dereferences and stores now use the pointee width for byte-oriented memory instead of always reading or writing a full machine word.
- `examples/v1/lexer.geo` now scans a source string with token boundaries, byte classification, mutable state, and public standard-library APIs.
- `examples/v1/mini_parser.geo` now scans and parses source text for a
  canonical `fn main() -> int { return 42 }` grammar with cursor state and
  explicit error paths, materializes a `FunctionNode` AST aggregate, stores
  token spans in a compiler-owned dynamic buffer, and executes through both
  native writers.
- Dynamic indexing into fixed-size aggregate locals now lowers through
  compiler-emitted bounds checks and branch-selected scalar slots, including
  `tokens[index].field` reads, writes, and compound assignments on ELF64 and
  PE64. `examples/v1/dynamic_array.geo` is the direct execution fixture.
- Struct arguments are now flattened into their scalar field slots at the
  compiler-owned native ABI boundary, including nested structs; direct ELF64
  and PE64 fixtures pass a `Token` by value and read it inside a callee.
- Explicit fixed-array types now use `[T; N]`; array literals are checked
  against `N`, and fixed-array parameters flatten into ordered native ABI
  slots on ELF64 and PE64.
- Aggregate returns now use a compiler-owned hidden return buffer in the IR
  and native ABI. Structs and fixed arrays can be returned by value, with
  direct ELF64 and PE64 execution fixtures covering caller reloads and callee
  writes.
- Direct handle file operations currently cover open/read-mode selection, truncate-write, append, write, flush, read-to-string, seek, and close; richer truncation controls and metadata remain open.
- Direct allocation lifetime coverage now includes compiler-owned `alloc`/`alloc_copy` headers, Linux `munmap`, Windows `VirtualFree`, payload-preserving `realloc`, and two-platform `alloc`/`free`/`realloc`/`alloc_copy` fixtures. `string_clone`, `string_from_byte`, `string_concat`, non-null `string_slice` results, path-based `read_file` results, handle-based `file_read_to_string` results, and `read_line` results now use the same header and have direct free fixtures.
- Windows-host PE64 execution validation now covers hello, allocation-backed strings, file reads, UTF-8 conversion, and byte/typed array runtime and mutation fixtures, including push, set, extend, copy, and resize. Broader PE64 runtime coverage and richer failure-path validation remain open.
- The self-hosting parser now exercises `std.array`, `std.mem`, raw pointer
  field encoding, token span storage, token retrieval, and AST construction in
  one native Geo program.

## Current Priority

The next best technical move is richer standard-library/runtime coverage and
continued expansion of the real lexer, parser, and diagnostics flows. The v1
parser now executes through both native writers, and PE64 execution has a
Windows-host smoke gate for strings, file paths, arrays, and process arguments.

That gives the native backends the source-text operations needed for compiler-shaped Geo programs while keeping the compiler implementation independent of external toolchains.

## Ownership Checking

- Mutable references can be reborrowed through `&mut *reference`.
- A retained child mutable reborrow suspends its parent reference for the
  child's lifetime and rejects parent use while the child is active.
- Temporary mutable reborrows restore the parent at statement end.
- Nested-scope cleanup restores suspended parent borrows after child bindings
  are released, with regression coverage for both accepted and rejected cases.
- Fixed-size array values can now be copied during lowering, including arrays
  nested inside copied structs, by expanding each element into the existing
  scalar-slot IR instead of aborting.
- Native ELF64 entry now preserves the initial Linux process stack and exposes
  compiler-owned `std.process.arg_count`, `arg`, `arg_exists`, and `arg_or`
  accessors directly from `argc`/`argv` without a C runtime.
- Native PE64 now imports `GetCommandLineA` and provides quote-aware
  `std.process.arg_count` and `arg_exists` helpers without a C runtime. It
  also extracts owned `arg` tokens with compiler-emitted `VirtualAlloc` and
  supports `arg_or` fallbacks.
