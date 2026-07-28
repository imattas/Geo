# Geo v1 Phase 3 Aggregates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add check-time support for structs, owned arrays, slices, field access, and indexing so compiler-shaped Geo programs can declare AST/data-buffer shapes before full runtime emission lands.

**Architecture:** This phase extends the front-end representation and type checker first. Aggregate values are parsed, resolved within a single source file, and type-checked. Lowering rejects aggregate runtime emission with clear diagnostics in later phases; existing scalar programs keep compiling.

**Tech Stack:** Rust 2021, Cargo, current Geo compiler modules.

## Global Constraints

- Existing v0.1 and Phase 2 tests must keep passing.
- Geo v1 supports structs with named fields.
- Geo v1 supports owned arrays `[T]` and slices `[]T`.
- Geo v1 supports array literals and indexing.
- Geo v1 supports field access on structs.
- Runtime representation, heap allocation, and backend data-section emission are deferred to the runtime phase.
- This workspace is not a Git repository, so commit steps are skipped until Git is initialized.

---

### Task 1: Tokens and AST for Aggregates

**Files:**
- Modify: `src/token.rs`
- Modify: `src/lexer.rs`
- Modify: `src/ast.rs`
- Modify: `tests/lexer_tests.rs`

**Interfaces:**
- Extends: `TokenKind` with `Struct`, `LeftBracket`, `RightBracket`, `Dot`.
- Extends: `Program` with `structs: Vec<StructDecl>`.
- Produces: `StructDecl { name: String, fields: Vec<Field> }`.
- Produces: `Field { name: String, ty: Type }`.
- Extends: `Type` with `Array(Box<Type>)`, `Slice(Box<Type>)`, and `Named(String)`.
- Extends: `Expr` with `Array(Vec<Expr>)`, `Field { base, name }`, and `Index { base, index }`.

- [ ] **Step 1: Add lexer tests for aggregate punctuation and `struct`**

Append a lexer test for `struct Token { values: [int] view: []int x.y a[0] }`.

- [ ] **Step 2: Implement token and AST additions**

Add the token and AST types exactly as listed in Interfaces.

- [ ] **Step 3: Run lexer tests**

Run: `cargo test --test lexer_tests`

Expected: PASS.

---

### Task 2: Parse Structs, Array/Slice Types, Field Access, and Indexing

**Files:**
- Modify: `src/parser.rs`
- Modify: `tests/parser_tests.rs`

**Interfaces:**
- Extends: `parser::parse(tokens: &[Token]) -> Result<Program, Vec<Diagnostic>>`.
- Parses top-level `struct` declarations before or between functions.
- Parses `[T]` as owned array type.
- Parses `[]T` as slice type.
- Parses `[expr, expr]` and `[]` as array literals.
- Parses postfix `.field` and `[index]`.

- [ ] **Step 1: Add parser tests**

Add a test that parses:

```geo
struct Token {
    kind: int
    start: usize
}

fn main() -> int {
    let tokens: [Token] = []
    let first: Token = Token { kind: 1 start: 0 }
    return first.kind
}
```

- [ ] **Step 2: Implement parser**

Parse top-level structs, aggregate types, struct literals, array literals, field access, and indexing.

- [ ] **Step 3: Run parser tests**

Run: `cargo test --test parser_tests`

Expected: PASS.

---

### Task 3: Type Check Aggregate Declarations and Expressions

**Files:**
- Modify: `src/typecheck.rs`
- Modify: `tests/type_tests.rs`

**Interfaces:**
- Struct names are unique.
- Struct field names are unique per struct.
- Named types must refer to declared structs.
- Struct literals require every field exactly once.
- Field access requires a struct value and known field.
- Array literals must contain one element type, or may infer from assignment for `[]`.
- Indexing `[T]` or `[]T` with an integer returns `T`.

- [ ] **Step 1: Add type tests**

Add tests for accepted struct/array programs, unknown field rejection, and mixed array literal rejection.

- [ ] **Step 2: Implement aggregate type checking**

Add a struct table. Thread optional expected types into initializer expression checks so `let xs: [int] = []` can type-check.

- [ ] **Step 3: Run type tests**

Run: `cargo test --test type_tests`

Expected: PASS.

---

### Task 4: Preserve Lowering Boundary

**Files:**
- Modify: `src/lower.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- `geo check` accepts aggregate programs.
- `geo emit-asm` for aggregate runtime values fails with a clear diagnostic until runtime lowering lands.
- Scalar programs continue to lower and emit.

- [ ] **Step 1: Add CLI check test**

Add an integration test that writes an aggregate source file in a temporary directory and runs `geo check`.

- [ ] **Step 2: Keep lowering panic-free where possible**

Do not lower aggregate runtime expressions yet. Keep scalar lowering intact. If an aggregate reaches lowering, panic message must clearly name the missing runtime phase.

- [ ] **Step 3: Run all tests**

Run: `cargo test`

Expected: PASS.

---

## Plan Self-Review

- Spec coverage: This plan covers Phase 3 front-end aggregate features from the v1 spec while deferring runtime representation as explicitly allowed by the design.
- Placeholder scan: No placeholders remain.
- Type consistency: `StructDecl`, `Field`, `Type::Array`, `Type::Slice`, `Type::Named`, `Expr::Array`, `Expr::Field`, and `Expr::Index` are named consistently.
