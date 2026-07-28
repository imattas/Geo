# Geo v1 Phase 2 Language Basics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first v1 language-surface features: comments, string and char literals, fixed-width integer type names, unary operators, `break`, `continue`, and expression statements.

**Architecture:** This phase extends lexer, AST, parser, type checker, and lowering while preserving the existing stack backend for integer/bool programs. String and char literals become parsed and type-checked values; string runtime representation and backend data sections are deferred to Phase 3 so `geo check` can validate v1 syntax before full runtime emission lands.

**Tech Stack:** Rust 2021, Cargo, current Geo compiler modules.

## Global Constraints

- Existing v0.1 examples and tests must keep passing.
- Geo v1 supports line comments beginning with `//` and block comments using `/* ... */`.
- Geo v1 supports `char`, `usize`, fixed-width integer names, owned `string`, unary `-` and `!`, `break`, `continue`, and expression statements.
- String backend emission is deferred to the runtime/data-section phase; `check` must support strings now.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: Lexer Tokens for v1 Language Basics

**Files:**
- Modify: `src/token.rs`
- Modify: `src/lexer.rs`
- Modify: `tests/lexer_tests.rs`

**Interfaces:**
- Extends: `TokenKind` with `Char`, `String`, `Usize`, fixed-width integer type keywords, `StringLiteral(String)`, `CharLiteral(char)`, `Break`, `Continue`, and `Bang`.
- Extends: `lexer::lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>>`

- [ ] **Step 1: Add lexer tests**

Append tests for comments, strings, chars, fixed-width types, `break`, `continue`, and `!`.

- [ ] **Step 2: Implement token kinds and lexing**

Skip line/block comments. Lex string literals with `\n`, `\t`, `\"`, and `\\` escapes. Lex char literals with one character or a supported escape. Treat bare `!` as `Bang` and `!=` as `BangEqual`.

- [ ] **Step 3: Run lexer tests**

Run: `cargo test --test lexer_tests`

Expected: PASS.

---

### Task 2: AST, Parser, and Type Checker

**Files:**
- Modify: `src/ast.rs`
- Modify: `src/parser.rs`
- Modify: `src/typecheck.rs`
- Modify: `tests/parser_tests.rs`
- Modify: `tests/type_tests.rs`

**Interfaces:**
- Extends: `Type` with `Char`, `String`, `Usize`, `I8`, `I16`, `I32`, `I64`, `U8`, `U16`, `U32`, `U64`.
- Extends: `Expr` with `String(String)`, `Char(char)`, and `Unary { op: UnaryOp, expr: Box<Expr> }`.
- Adds: `UnaryOp::{Neg, Not}`.
- Extends: `Stmt` with `Break`, `Continue`, and `Expr(Expr)`.

- [ ] **Step 1: Add parser and type tests**

Add tests that parse and check:

```geo
fn main() -> int {
    let name: string = "Geo"
    let marker: char = 'G'
    let size: usize = 1
    return -1 + 2
}
```

Add tests that reject `!1` and accept `!true`.

- [ ] **Step 2: Implement AST and parser extensions**

Parse new type keywords, literals, unary expressions, expression statements, `break`, and `continue`.

- [ ] **Step 3: Implement type checking**

Type string and char literals. Unary `-` requires an integer-like operand and returns the same integer type. Unary `!` requires `bool` and returns `bool`. `break` and `continue` are valid only inside loops.

- [ ] **Step 4: Run parser and type tests**

Run: `cargo test --test parser_tests --test type_tests`

Expected: PASS.

---

### Task 3: Lowering Support for Unary and Loop Control

**Files:**
- Modify: `src/ir.rs`
- Modify: `src/lower.rs`
- Modify: `src/x86_64.rs`
- Modify: `tests/lower_tests.rs`

**Interfaces:**
- Existing IR stays sufficient for unary operations using `Const`, `Sub`, and `Cmp`.
- `break` lowers to a jump to the current loop end label.
- `continue` lowers to a jump to the current loop start label.
- Expression statements lower their expression and discard the result.

- [ ] **Step 1: Add lowering tests**

Add tests that `continue` and `break` lower to jumps and unary `!true` lowers to a comparison.

- [ ] **Step 2: Implement lowering**

Lower unary `-x` as `0 - x`; lower unary `!x` as `x == 0`; lower break/continue with a loop label stack.

- [ ] **Step 3: Run lowering tests**

Run: `cargo test --test lower_tests`

Expected: PASS.

---

## Plan Self-Review

- Spec coverage: This plan covers the Phase 2 v1 language basics from the spec.
- Placeholder scan: No placeholders remain.
- Type consistency: The new `Type`, `Expr`, `UnaryOp`, and `Stmt` variants are named consistently across lexer, parser, type checker, and lowering.
