# Geo v1 Aggregate Scalar Slot Lowering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lower a meaningful v1 subset of local struct and array literals into scalar IR slots so compiler-shaped examples can compile past the current aggregate panic.

**Architecture:** This slice keeps the existing one-qword IR/backend model and represents local aggregate literals as deterministic backing locals such as `token.kind` and `tokens[0].kind`. Field and constant-index reads lower to `Load` from those backing locals. Dynamic indexing and first-class aggregate values remain explicit future work.

**Tech Stack:** Rust compiler crate, existing AST/typechecker/lowerer/IR/NASM backend, Cargo tests.

## Global Constraints

- Geo v1 supports structs with named fields.
- Geo v1 supports owned arrays `[T]` and array literals.
- Assembly output remains the production path for v1.
- The Rust compiler remains the authoritative compiler implementation for v1.
- No subagents are used for this implementation.

---

### Task 1: Add Scalar-Slot Lowering Tests

**Files:**
- Modify: `tests/lower_tests.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: `geo::lower::lower(&Program) -> IrProgram`
- Produces: test coverage for `Instruction::Store { local: "first.kind", .. }` and `Instruction::Load { local: "pair[0].kind", .. }`

- [ ] **Step 1: Write the failing lowerer test**

Add a test that lowers:

```geo
struct Token {
    kind: int
    start: usize
}

fn main() -> int {
    let first: Token = Token { kind: 1 start: 0 }
    let pair: [Token] = [first]
    return pair[0].kind
}
```

Assert the instruction stream stores `first.kind`, stores `pair[0].kind`, and loads `pair[0].kind`.

- [ ] **Step 2: Write the failing assembly smoke test**

Add a test that emits assembly for the same source and asserts the assembly contains stack moves for `pair[0].kind` and does not panic.

- [ ] **Step 3: Run focused tests to verify failure**

Run: `cargo test --test lower_tests lowers_struct_and_array_literals_to_scalar_slots -- --nocapture`

Expected: failure from `aggregate lowering requires the v1 runtime/data-section phase`.

### Task 2: Implement Aggregate Scalar Slots

**Files:**
- Modify: `src/lower.rs`

**Interfaces:**
- Consumes: `StructDecl` metadata from `Program`
- Produces: local scalar slots named by `aggregate_slot(base, components) -> String`

- [ ] **Step 1: Thread struct metadata through lowering**

Change `lower_function` to receive `&Program`, build a `HashMap<String, StructDecl>`, and store it in `LowerCtx`.

- [ ] **Step 2: Track local declared types**

When lowering `Stmt::Let`, record the declared type in `LowerCtx.locals`.

- [ ] **Step 3: Lower aggregate assignments**

For `Stmt::Let` or `Stmt::Assign` with `Expr::Struct` or `Expr::Array`, recursively store scalar fields/elements into backing locals instead of producing a first-class aggregate value.

- [ ] **Step 4: Lower field and constant-index reads**

For `Expr::Field` and `Expr::Index`, resolve a scalar aggregate place and emit `Instruction::Load` from the derived backing local. Only constant integer indices are supported in this slice.

- [ ] **Step 5: Preserve explicit failure for unsupported aggregate values**

Keep direct aggregate expressions outside local assignment and non-constant aggregate indexing as explicit panics with precise messages.

### Task 3: Verify

**Files:**
- Modify: `tests/lower_tests.rs`
- Modify: `tests/compile_tests.rs`
- Modify: `src/lower.rs`

**Interfaces:**
- Consumes: all tests added above
- Produces: passing regression suite

- [ ] **Step 1: Format**

Run: `cargo fmt`

- [ ] **Step 2: Run focused tests**

Run: `cargo test --test lower_tests lowers_struct_and_array_literals_to_scalar_slots`

Run: `cargo test --test compile_tests emits_assembly_for_scalar_slot_aggregates`

- [ ] **Step 3: Run full suite**

Run: `cargo test`

Expected: all tests pass.

## Self-Review

- Spec coverage: This plan advances structs, arrays, field access, indexing, and self-hosting-shaped examples enough to compile a useful subset.
- Known gap: It does not implement a full runtime memory representation for first-class aggregates or dynamic indexing.
- Placeholder scan: No placeholders remain.
- Type consistency: The plan uses existing `Instruction::Store`, `Instruction::Load`, `StructDecl`, `Type`, and `Expr` names.
