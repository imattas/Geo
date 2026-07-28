# Geo v1 Self-Hosting Foundation Design

## Purpose

Geo v1 is a complete self-hosting foundation release. The compiler may still be written in Rust, but Geo itself must be capable of implementing meaningful compiler components: lexers, parsers, AST data structures, diagnostics, buffers, file readers, collections, and CLI tools.

v1 also makes Geo a practical systems language target instead of only a small v0.1 compiler experiment. It supports Linux x86-64 and Windows x86-64 as first-class targets, includes a small standard library and runtime, and introduces backend boundaries for direct object emission and lower-level runtime control.

## Current Baseline

Geo v0.1 currently supports:

- single-file `.geo` programs
- `int` and `bool`
- functions and parameters
- explicit `return`
- `let` bindings and assignment
- arithmetic and comparisons
- `if` / `else`
- `while`
- static type checking
- IR lowering
- x86-64 NASM assembly emission
- `check`, `emit-asm`, `build`, and `run` commands

v1 builds from that baseline without replacing the Rust implementation language for the compiler.

## v1 Goals

Geo v1 includes three coordinated tracks:

1. **Language and type system:** strings, arrays, slices, structs, modules, externs, pointers, unsafe blocks, and an ownership/borrowing foundation.
2. **Runtime and standard library:** printing, allocation, file IO, process basics, string helpers, array helpers, panic/trap handling, and platform abstraction.
3. **Backend and platform targets:** Linux x86-64 and Windows x86-64 support, target abstraction, richer ABI handling, runtime entry support, object-writer interfaces, and assembly fallback.

## Non-Goals for v1

Geo v1 will not include:

- generics
- traits or interfaces
- closures
- async
- macros
- full borrow-checker parity with Rust
- advanced optimization
- production-grade register allocation
- garbage collection
- package registry
- IDE integration
- full compiler rewrite in Geo

The self-hosting objective for v1 is readiness and compiler-shaped examples, not a complete Geo compiler implemented in Geo.

## Language Surface

Geo v1 supports:

- `int`, retained as the default signed integer type for simple examples
- `bool`
- `char`
- `usize`
- fixed-width integers: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`
- owned `string`
- owned arrays: `[T]`
- slices: `[]T`
- structs with named fields
- pointers and references
- modules and imports
- line comments beginning with `//`
- block comments using `/* ... */`
- unary operators: `-`, `!`, address-of, and dereference
- `break` and `continue`
- expression statements for calls
- `extern fn`
- `unsafe` blocks

## Syntax Direction

Geo keeps explicit function signatures:

```geo
fn main() -> int {
    return 0
}
```

Structs use named fields:

```geo
struct Token {
    kind: int
    start: usize
    len: usize
}
```

Owned arrays and strings are first-class values:

```geo
let names: [string] = []
let title: string = "Geo"
```

Modules use explicit imports:

```geo
import std.io
import lexer.token
```

Externs are explicit and platform-aware through target-specific linkage metadata:

```geo
extern fn puts(message: *u8) -> int
```

## Ownership and Borrowing

Geo v1 uses a modest ownership checker:

- Values have one owner by default.
- Assignment and function calls move owned strings, arrays, and structs unless borrowed.
- Immutable borrows allow many readers.
- Mutable borrows allow one writer.
- No borrow may outlive its owner.
- Raw pointers are allowed only inside `unsafe` blocks.
- Safe indexing of strings and arrays is bounds-checked.
- Unsafe operations are rejected outside `unsafe`.

This checker is intentionally smaller than Rust's. It should prevent obvious use-after-move, conflicting mutable borrows, and escaping borrows, but it does not need non-lexical lifetime sophistication in v1.

## Runtime and Standard Library

Geo v1 ships a small `std` implemented over a platform runtime layer.

Core modules:

- `std.io`: `print`, `println`, `eprint`, file open/read/write/close
- `std.mem`: `alloc`, `free`, `realloc`, `copy`, `zero`
- `std.process`: `args`, `exit`, environment basics
- `std.string`: length, clone, compare, concat, substring
- `std.array`: length, push, pop, indexing helpers
- `std.platform`: low-level Linux and Windows bindings hidden behind stable APIs

Runtime responsibilities:

- program entry and argument normalization
- heap allocator wrapper
- panic/trap path for bounds errors
- platform-specific file and console IO
- optional direct syscall backend on Linux
- WinAPI-backed runtime on Windows
- ABI adapters for calls between Geo code and runtime functions

The runtime may begin as Rust or C shim code linked beside generated Geo output. Its public ABI must stay simple enough to rewrite pieces in Geo later.

## Backend and Platform Targets

Geo v1 supports:

- Linux x86-64, System V ABI
- Windows x86-64, Windows x64 ABI
- NASM assembly output as the stable baseline
- platform linker integration
- stack-passed arguments beyond ABI register limits
- data sections for strings and globals
- runtime symbol references
- a structured scratch-register allocator
- optional `_start` or runtime entry support
- an object-writer interface
- a direct ELF64 relocatable object writer prototype for Linux with sections, symbols, and relocations sufficient for integer-returning functions and runtime calls

Assembly output remains the production path for v1. Direct object writing is introduced through interfaces and a Linux ELF64 prototype, not required as the only build path.

## Compiler Architecture

The v1 compiler keeps clear pipeline stages:

```text
.geo sources
-> source manager
-> lexer
-> parser
-> AST
-> name resolver
-> type checker
-> ownership checker
-> typed HIR
-> IR
-> target lowering
-> backend
-> assembly or object
-> linker
```

New compiler modules:

- `source`: multi-file source loading and span mapping
- `resolve`: module, import, type, function, and symbol resolution
- `hir`: typed high-level representation after name and type resolution
- `borrow`: ownership and borrowing validation
- `target`: target triples, ABI rules, object format, symbol rules, and linker commands
- `runtime`: runtime library discovery and link integration
- `object`: direct object writer interfaces and prototypes

Existing parser, type checker, lowering, and backend modules may be split as they grow.

## Modules and Multi-File Compilation

Geo v1 supports multiple source files in a package directory. A module maps to a `.geo` file or directory module entry. Imports are explicit and acyclic in v1.

The compiler resolves:

- current module symbols
- imported module symbols
- standard library modules
- extern declarations

Circular imports are rejected with a diagnostic.

## Diagnostics

v1 diagnostics must include:

- severity
- primary message
- file path
- line and column
- source excerpt
- caret underline
- optional notes

Diagnostics should be stable enough for tests to assert important message fragments and locations.

## CLI

Geo v1 CLI commands:

```bash
geo check path/to/main.geo
geo emit-asm path/to/main.geo -o out.asm --target x86_64-linux
geo build path/to/main.geo -o out --target x86_64-linux
geo run path/to/main.geo --target x86_64-linux
geo fmt path/to/main.geo
geo test path/to/package
```

Supported targets:

```text
x86_64-linux
x86_64-windows
```

The default target is the host target when supported.

## Self-Hosting Milestones

v1 includes compiler-shaped Geo examples:

- `examples/v1/buffer.geo`: grows an owned byte or string buffer
- `examples/v1/lexer.geo`: tokenizes a small source string or file
- `examples/v1/diagnostics.geo`: formats a source error with line and column
- `examples/v1/ast.geo`: builds simple AST structs and arrays
- `examples/v1/file_echo.geo`: reads a file and writes it back
- `examples/v1/mini_parser.geo`: parses a tiny expression grammar

Acceptance criteria:

- These examples compile for Linux and Windows targets.
- Examples using runtime features link against the v1 runtime.
- The ownership checker accepts natural safe versions of these programs.
- Unsafe code is limited to runtime and platform boundaries.
- The Rust compiler remains the authoritative compiler implementation for v1.

## Testing Strategy

Tests are organized by compiler boundary:

- lexer tests for comments, strings, new literals, and spans
- parser tests for structs, modules, arrays, strings, unsafe, externs, and imports
- resolver tests for multi-file symbols and import errors
- type tests for new primitive types, structs, arrays, strings, calls, and externs
- borrow tests for moves, immutable borrows, mutable borrows, escaping borrows, and unsafe restrictions
- lowering tests for HIR-to-IR and runtime calls
- backend tests for Linux and Windows assembly shape
- ABI tests for register and stack-passed arguments
- object writer tests for section/symbol/relocation structures
- runtime tests for IO, allocation, strings, arrays, and process behavior
- acceptance tests for `examples/v1`

Native execution tests run only when the host and required tools can execute the target. Cross-target tests must still validate emitted assembly or object structure.

## Implementation Phases

### Phase 1: Compiler Restructure

Introduce source management, target abstraction, richer diagnostics, and package/module-aware CLI paths.

### Phase 2: Language Basics

Add comments, string literals, char literals, fixed-width integers, unary operators, `break`, `continue`, and expression statements.

### Phase 3: Structs, Arrays, Strings, and Slices

Implement AST, type checking, runtime representation, indexing, bounds checks, and backend support.

### Phase 4: Modules, Imports, and Externs

Implement multi-file loading, name resolution, import validation, standard library lookup, and external function declarations.

### Phase 5: Ownership and Borrowing

Add move checking for owned values, immutable and mutable borrow validation, escaping-borrow rejection, and unsafe operation gating.

### Phase 6: Runtime and Standard Library

Build the platform runtime and `std` modules for IO, memory, process, string, array, and platform support.

### Phase 7: Windows and Linux Backends

Generalize ABI lowering and backend emission for Linux System V and Windows x64, including stack-passed arguments and platform link commands.

### Phase 8: Object Writer and Runtime Entry

Introduce object writer interfaces, a Linux ELF64 relocatable object writer prototype, optional `_start` or runtime entry paths, and direct Linux syscall experiments behind flags.

### Phase 9: Self-Hosting Examples

Implement and accept the compiler-shaped v1 examples that demonstrate realistic future self-hosting.

## Risks

- The v1 scope is large and must be implemented as separate plans with verification gates.
- Ownership checking can sprawl; v1 must keep it lexical and modest.
- Windows ABI and linking are different enough to require target-specific tests early.
- Strings and arrays require runtime support before many examples become useful.
- Direct object writing can consume time without improving user-facing language features; assembly fallback must remain available.
- Self-hosting examples should drive practical capability, not cosmetic demos.

## Completion Definition

Geo v1 is complete when:

- `geo check`, `emit-asm`, `build`, and `run` support Linux and Windows targets where tools are available.
- The v1 language surface is parsed, type-checked, ownership-checked, lowered, and emitted.
- The standard library/runtime supports the core IO, memory, process, string, and array APIs.
- `examples/v1` compiler-shaped programs compile for both targets.
- The test suite covers each compiler boundary and platform-specific backend behavior.
- Unsafe code is required only at runtime/platform boundaries in the v1 examples.
