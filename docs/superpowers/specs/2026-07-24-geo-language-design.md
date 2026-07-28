# Geo Language Design

## Purpose

Geo is a small but real compiled systems programming language. The long-term goal is to compile `.geo` source files to native executables using Geo's own backend, without LLVM and without C code generation.

The initial implementation is intentionally narrow: a Rust compiler targeting x86-64 Linux, System V ABI, NASM Intel syntax, and linking through `gcc` or `clang`.

## Non-Goals for v0.1

Geo v0.1 will not include strings, arrays, structs, pointers, references, generics, traits, borrow checking, classes, closures, async, macros, modules, a package manager, garbage collection, advanced optimization, register allocation, `_start`, direct Linux syscalls, LLVM, or C codegen.

## Implementation Language

The compiler should be implemented in Rust.

Rust is the recommended implementation language because compiler internals benefit from algebraic data types, pattern matching, explicit ownership, strong error handling, and good test tooling. Go would be easier for a quick prototype, but Rust is the better fit for a long-term systems compiler with ASTs, typed IR, diagnostics, and a native backend.

## Target Platform

The initial target is:

- Architecture: x86-64
- Operating system: Linux
- ABI: System V AMD64 ABI
- Assembly syntax: NASM Intel syntax
- Object generation: `nasm -f elf64`
- Linking: `gcc` or `clang`

Geo will generate a normal `main` function first. It will not generate `_start` in v0.1.

## Compiler Pipeline

The v0.1 pipeline is:

```text
.geo source
-> lexer/tokenizer
-> parser
-> AST
-> type checker
-> IR
-> lowering from AST to IR
-> x86-64 backend
-> NASM assembly
-> object file
-> linked executable
```

Each stage should have tests and a clear data boundary. The compiler should expose enough debug output through tests to inspect tokens, AST shape, IR, and assembly.

## Source Language v0.1

Geo v0.1 supports:

- `.geo` source files
- `int`
- `bool`
- functions
- parameters
- `return`
- `let` bindings
- assignment
- arithmetic: `+`, `-`, `*`, `/`
- comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- `if` / `else`
- `while` loops
- local variables
- basic static type checking

The first milestone program is:

```geo
fn main() -> int {
    return 42
}
```

The next major target program is:

```geo
fn add(a: int, b: int) -> int {
    return a + b
}

fn main() -> int {
    let x: int = 10
    let y: int = 32
    return add(x, y)
}
```

## Syntax Decisions

Statements do not require semicolons in v0.1. Newlines are treated as whitespace, so statement boundaries are determined by grammar. This keeps examples concise but means the parser must know when expressions end based on surrounding syntax.

All functions must use explicit `return` statements in v0.1. Implicit tail returns are not supported.

Variables require explicit type annotations:

```geo
let x: int = 10
```

Assignment uses:

```geo
x = 20
```

More than six function parameters are rejected in v0.1 with a clear compiler error.

## Repository Structure

The planned repository structure is:

```text
Cargo.toml
src/
  main.rs
  cli.rs
  token.rs
  lexer.rs
  ast.rs
  parser.rs
  typecheck.rs
  ir.rs
  lower.rs
  x86_64.rs
  diagnostics.rs
  driver.rs

tests/
  lexer_tests.rs
  parser_tests.rs
  type_tests.rs
  lower_tests.rs
  compile_tests.rs

examples/
  return_42.geo
  arithmetic.geo
  variables.geo
  functions.geo
  if_else.geo
  while.geo
```

Responsibilities:

- `main.rs`: binary entry point.
- `cli.rs`: command-line parsing and command definitions.
- `token.rs`: token kinds, spans, and token structs.
- `lexer.rs`: source text to tokens.
- `ast.rs`: parsed source representation.
- `parser.rs`: tokens to AST.
- `typecheck.rs`: symbol tables and static type validation.
- `ir.rs`: intermediate representation types.
- `lower.rs`: checked AST to IR.
- `x86_64.rs`: IR to NASM assembly.
- `diagnostics.rs`: user-facing errors with spans.
- `driver.rs`: orchestration for check, emit-asm, build, and run.

## CLI Design

The compiler executable is named `geo`.

Commands:

```bash
geo check examples/return_42.geo
geo emit-asm examples/return_42.geo -o out.asm
geo build examples/return_42.geo -o out
geo run examples/return_42.geo
```

Useful options:

```bash
--target x86_64-linux
--keep-temps
--verbose
--linker gcc
--nasm nasm
```

Only `x86_64-linux` is valid in v0.1.

## Type Checking

The type checker validates:

- `main` exists.
- Function names are unique.
- Local variable names are unique within a function scope.
- Variables are declared before use.
- Return expression type matches function return type.
- Function call argument count matches parameter count.
- Function call argument types match parameter types.
- Arithmetic operands are `int`.
- Arithmetic results are `int`.
- Comparison operands are compatible.
- Comparison results are `bool`.
- `if` and `while` conditions are `bool`.
- Assignment target exists and assigned value type matches target type.

## Intermediate Representation

The initial IR uses virtual values and symbolic locals. It is not SSA and does not optimize.

Instruction set:

```text
Const dst, value
Add dst, left, right
Sub dst, left, right
Mul dst, left, right
Div dst, left, right
Load dst, local
Store local, value
Cmp dst, op, left, right
Jump label
JumpIfZero value, label
Label name
Call dst, function, args
Return value
```

The lowering pass converts typed AST expressions and statements into this IR. Backend stack layout is not part of IR.

## Backend Strategy

The x86-64 backend is deliberately simple:

- Generate NASM Intel syntax directly.
- Emit `global main`.
- Emit `section .text`.
- Use one assembly function per Geo function.
- Use a standard prologue and epilogue.
- Return values use `rax`.
- Expression codegen leaves results in `rax`.
- Use caller-saved scratch registers such as `r10` and `r11`.
- Store locals in stack slots relative to `rbp`.
- Avoid register allocation in v0.1.

Basic function shape:

```asm
global main
section .text

main:
    push rbp
    mov rbp, rsp
    ; optional stack frame reservation
    mov rax, 42
    mov rsp, rbp
    pop rbp
    ret
```

Object and link commands:

```bash
nasm -f elf64 out.asm -o out.o
gcc out.o -o out
```

## ABI Notes

System V AMD64 rules needed for v0.1:

- Integer return values use `rax`.
- Integer arguments 1 through 6 use `rdi`, `rsi`, `rdx`, `rcx`, `r8`, and `r9`.
- `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, and `r11` are caller-saved.
- `rbx`, `rbp`, and `r12` through `r15` are callee-saved.
- The stack must be 16-byte aligned before a `call`.
- After entering a function through `call`, the return address makes `rsp` misaligned by 8.
- After `push rbp`, choose a stack frame size that keeps `rsp` 16-byte aligned before nested calls.

Geo v0.1 should reject functions with more than six parameters instead of implementing stack-passed arguments.

## Error Handling

Compiler phases return `Result<T, Vec<Diagnostic>>`.

Diagnostics include:

- severity
- message
- source span
- optional notes

Example format:

```text
error: expected expression after return
 --> examples/bad.geo:2:12
  |
2 |     return
  |            expected expression here
```

Source errors should not panic. Panics are reserved for internal compiler bugs during early development.

## Testing Strategy

Testing should follow the compiler pipeline:

- Lexer tests validate token kinds and spans.
- Parser tests validate AST shape and syntax errors.
- Type tests validate accepted and rejected programs.
- Lowering tests validate IR snapshots.
- Compile tests validate generated assembly, NASM assembly, linking, and executable exit codes.

The first executable compile test should build and run `examples/return_42.geo` and assert exit code `42`.

## Phased Roadmap

### v0.0: Project Skeleton

Create the Rust CLI, repository layout, diagnostics shell, and example file.

Expected tests:

- CLI rejects missing file.
- CLI rejects non-`.geo` input.
- CLI reads valid `.geo` input.
- `geo --help` works.

### v0.0.1: Lexer

Implement tokenization for keywords, identifiers, integer literals, punctuation, and operators.

Expected tests:

- Tokenizes `fn main() -> int { return 42 }`.
- Distinguishes `=` from `==`.
- Distinguishes `<` from `<=` and `>` from `>=`.
- Tracks line and column spans.
- Rejects unknown characters.

### v0.0.2: Parser and AST

Parse functions, typed parameters, blocks, return statements, literals, identifiers, binary expressions, and calls.

Expected tests:

- Parses `main`.
- Parses typed parameters.
- Parses binary precedence.
- Parses function calls.
- Reports missing delimiters.

### v0.0.3: Type Checker

Validate functions, local bindings, return types, operators, calls, and conditions.

Expected tests:

- Accepts `return 42`.
- Rejects `return true` from `-> int`.
- Rejects unknown variables.
- Rejects wrong call arity.
- Rejects arithmetic with `bool`.

### v0.0.4: IR and Lowering

Lower typed AST to simple IR.

Expected tests:

- `return 42` lowers to `Const` and `Return`.
- `a + b` lowers to loads and add.
- `if / else` lowers to labels and jumps.
- `while` lowers to condition and loop labels.

### v0.0.5: First Native Executable

Generate NASM assembly for `return 42`, assemble with NASM, link with `gcc` or `clang`, and run.

Expected tests:

- `geo emit-asm examples/return_42.geo -o out.asm` writes valid assembly.
- NASM accepts the assembly.
- Linked executable exits with status `42`.

### v0.0.6: Locals and Arithmetic

Implement local stack slots, `let`, assignment, and arithmetic.

Expected tests:

- Local variables compile.
- Assignment updates a stack slot.
- `+`, `-`, `*`, and `/` compile.
- Division emits `cqo` and `idiv`.

### v0.0.7: Functions and Calls

Implement parameters and function calls using the first six System V integer argument registers.

Expected tests:

- `add(10, 32)` exits with `42`.
- Calls with wrong arity are rejected.
- Calls with wrong types are rejected.
- Nested call arguments compile.

### v0.1: Control Flow and Basic Language

Implement `bool`, comparisons, `if / else`, `while`, and full v0.1 static checks.

Expected tests:

- `if true { return 1 } else { return 2 }` compiles.
- Comparisons return `bool`.
- While loops compile and run.
- Non-bool `if` and `while` conditions are rejected.
- Assigning `bool` to `int` is rejected.

## Future Versions

v0.2:

- comments
- unary operators
- improved parser recovery
- stack-passed function arguments
- richer diagnostics

v0.3:

- string literals
- external function declarations
- basic libc calls

v0.4:

- arrays or slices
- pointers
- explicit memory operations

v0.5:

- structs
- multi-file compilation
- modules

v0.6 and later:

- register allocation
- optimization passes
- custom object writer
- `_start`
- direct Linux syscalls
- self-hosting experiments

## Risk List

The highest-risk areas are:

- Stack alignment around function calls.
- Correct signed division codegen with `cqo` and `idiv`.
- Expression evaluation without clobbering intermediate values.
- Parser ambiguity caused by semicolon-free syntax.
- Keeping AST, typed validation, IR, and backend responsibilities separate.
- Avoiding accidental scope creep before v0.1.

## First Implementation Task

The first implementation task is to create the Rust project skeleton and write failing CLI and lexer tests for:

```geo
fn main() -> int {
    return 42
}
```

The task is complete when `.geo` input can be read, tokenized with spans, and rejected cleanly when invalid. It should not emit assembly yet.
