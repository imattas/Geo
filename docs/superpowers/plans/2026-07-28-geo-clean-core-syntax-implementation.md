# Geo Clean Core Syntax Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved clean Geo syntax and semantics so `fn main() { println("Hello, world!") }` checks, builds, runs, and exits `0`.

**Architecture:** Extend the existing AST/parser/typechecker/lowerer rather than replacing the compiler pipeline. Unit-returning functions lower to an integer process status of `0` when no explicit integer return is present. Standard library printing becomes statement-oriented by returning `unit`.

**Tech Stack:** Rust 2021, current Geo lexer/parser/typechecker/borrow/lower/x86_64 pipeline, compiler-managed native runtime for existing build path.

## Global Constraints

- Preserve existing v1 syntax compatibility.
- No subagents.
- Semicolons are optional.
- `str` aliases `string`.
- `let` is immutable; `var` is mutable.
- Functions without `-> type` return `unit`.
- `main()` returning `unit` exits `0`.
- `println` returns `unit`.

---

### Task 1: AST And Lexer Tokens

**Files:**
- Modify: `src/ast.rs`
- Modify: `src/token.rs`
- Modify: `src/lexer.rs`
- Test: `tests/lexer_tests.rs`

**Interfaces:**
- Produces: `Type::Unit`
- Produces: `Stmt::Return(Option<Expr>)`
- Produces: `Stmt::Let { name: String, ty: Option<Type>, mutable: bool, value: Expr }`
- Produces: `TokenKind::Var`, `TokenKind::Str`, `TokenKind::Semicolon`

- [ ] Add AST variants for unit, optional return expression, optional local type, and local mutability.
- [ ] Add lexer tokens for `var`, `str`, and `;`.
- [ ] Add lexer tests for `str`, `var`, and optional semicolon tokenization.
- [ ] Run `cargo test --test lexer_tests`.

### Task 2: Parser Clean Syntax

**Files:**
- Modify: `src/parser.rs`
- Test: `tests/parser_tests.rs`

**Interfaces:**
- Consumes: AST/token changes from Task 1.
- Produces: parsing for omitted function return type, inferred `let`, `var`, optional semicolons, optional `else`, and `else if`.

- [ ] Parse `fn main() { ... }` as `return_type: Type::Unit`.
- [ ] Parse `let name = expr`, `let name: type = expr`, `var name = expr`, and `var name: type = expr`.
- [ ] Accept semicolons after imports, fields, statements, array/struct elements where safe.
- [ ] Parse `str` as `Type::String`.
- [ ] Parse `if` without `else`; parse `else if` as an else-body containing one nested `Stmt::If`.
- [ ] Add parser tests for canonical hello world, inferred locals, mutable vars, and `else if`.
- [ ] Run `cargo test --test parser_tests`.

### Task 3: Typechecker Semantics

**Files:**
- Modify: `src/typecheck.rs`
- Modify: `src/runtime.rs`
- Test: `tests/type_tests.rs`
- Test: `tests/runtime_tests.rs`

**Interfaces:**
- Consumes: parser AST.
- Produces: local type inference, immutable assignment rejection, unit-return functions, `println -> unit`, string concatenation via `+`.

- [ ] Change `std.io.print`, `println`, and `eprint` metadata return type to `Type::Unit`.
- [ ] Allow `return` without value only in unit functions.
- [ ] Allow omitted return in unit functions.
- [ ] Infer local types when annotation is omitted.
- [ ] Track local mutability and reject assignment to `let`.
- [ ] Allow `string + string -> string`.
- [ ] Add type tests for canonical hello, explicit int main with println statement, inferred `let`, mutable `var`, immutable assignment rejection, `str`, and string concat.
- [ ] Run `cargo test --test type_tests --test runtime_tests`.

### Task 4: Borrow And Lowering

**Files:**
- Modify: `src/borrow.rs`
- Modify: `src/lower.rs`
- Modify: `src/x86_64.rs`
- Test: `tests/lower_tests.rs`
- Test: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: checked AST.
- Produces: unit functions lower to default `0`; `println` expression statements are valid; string `+` lowers to `string_concat`.

- [ ] Update borrow checker for optional return expressions and optional local types.
- [ ] Lower unit returns and functions with no explicit return to integer `0` for process ABI compatibility.
- [ ] Lower string concatenation to `string_concat`.
- [ ] Ensure x86 emitter emits a default epilogue if a function has no explicit return after lowering.
- [ ] Add lower and compile tests for canonical hello and string concat.
- [ ] Run `cargo test --test lower_tests --test compile_tests`.

### Task 5: Examples And Binary

**Files:**
- Modify: `examples/hello_world.geo`
- Test: full suite

**Interfaces:**
- Consumes: complete clean syntax pipeline.
- Produces: `hello_world.exe` built from canonical Geo syntax.

- [ ] Rewrite `examples/hello_world.geo` to canonical unit-returning syntax.
- [ ] Build `hello_world.exe` with the current Windows build path.
- [ ] Run `.\hello_world.exe` and verify it prints `Hello, world!`.
- [ ] Run `cargo test`.

## Self-Review

- Spec coverage: Tasks cover parser syntax, std metadata, type inference, local mutability, unit main, string alias, optional semicolons, and hello-world build.
- Known remaining backend gap: direct PE64/no-MSVC-linker is not in this syntax plan and needs a separate backend implementation plan.
