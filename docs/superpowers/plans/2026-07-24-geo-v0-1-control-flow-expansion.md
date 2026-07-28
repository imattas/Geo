# Geo v0.1 Control Flow Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the first Geo compiler from constant returns and simple expressions to the full v0.1 language subset: locals, assignment, function calls, comparisons, `if / else`, and `while`.

**Architecture:** This plan builds on `docs/superpowers/plans/2026-07-24-geo-v0-1-implementation.md`. It extends the AST first, then the type checker, then IR, then the x86-64 NASM backend, preserving one testable compiler boundary at a time.

**Tech Stack:** Rust, Cargo, NASM, `gcc` or `clang`, x86-64 Linux System V ABI.

## Global Constraints

- Source files use the `.geo` extension.
- The compiler executable is named `geo`.
- The compiler is implemented in Rust.
- The initial target is x86-64 Linux using the System V ABI.
- Assembly output uses NASM Intel syntax.
- Object files are produced with `nasm -f elf64`.
- Executables are linked with `gcc` or `clang`.
- Geo v0.1 uses a simple stack-based backend.
- Expression codegen leaves results in `rax`.
- Return values use `rax`.
- The first six integer arguments use `rdi`, `rsi`, `rdx`, `rcx`, `r8`, and `r9`.
- Functions with more than six parameters are rejected in v0.1.
- Geo v0.1 does not include strings, arrays, structs, pointers, references, generics, traits, borrow checking, classes, closures, async, macros, modules, a package manager, garbage collection, advanced optimization, or register allocation.

---

## File Structure

- Modify `src/ast.rs`: add `Let`, `Assign`, `If`, and `While` statements.
- Modify `src/parser.rs`: parse new statements and block bodies.
- Modify `src/typecheck.rs`: validate locals, assignments, branch conditions, and loop conditions.
- Modify `src/ir.rs`: add locals, comparisons, labels, jumps, and calls.
- Modify `src/lower.rs`: lower locals, calls, branches, and loops.
- Modify `src/x86_64.rs`: emit stack slots, calls, comparisons, labels, and jumps.
- Modify `src/driver.rs`: wire `build` and `run`.
- Modify `tests/parser_tests.rs`: add statement parsing tests.
- Modify `tests/type_tests.rs`: add local/control-flow validation tests.
- Modify `tests/lower_tests.rs`: add IR tests for locals and jumps.
- Modify `tests/compile_tests.rs`: add end-to-end compile/run tests.
- Create `examples/arithmetic.geo`
- Create `examples/variables.geo`
- Create `examples/functions.geo`
- Create `examples/if_else.geo`
- Create `examples/while.geo`

---

### Task 1: Parser Support for v0.1 Statements

**Files:**
- Modify: `src/ast.rs`
- Modify: `src/parser.rs`
- Modify: `tests/parser_tests.rs`

**Interfaces:**
- Extends: `ast::Stmt`
- Produces: parsed `let`, assignment, `if / else`, and `while` AST nodes.

- [ ] **Step 1: Extend AST statements**

Update `src/ast.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Return(Expr),
    Let {
        name: String,
        ty: Type,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
}
```

- [ ] **Step 2: Add parser tests for new statements**

Append to `tests/parser_tests.rs`:

```rust
#[test]
fn parses_let_assignment_if_and_while() {
    let source = r#"
        fn main() -> int {
            let x: int = 0
            while x < 42 {
                x = x + 1
            }
            if x == 42 {
                return x
            } else {
                return 0
            }
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    assert_eq!(program.functions[0].body.len(), 3);
}
```

- [ ] **Step 3: Implement statement parsing**

Update `parse_stmt` in `src/parser.rs` to dispatch by leading token:

```rust
fn parse_stmt(&mut self) -> Result<Stmt, Vec<Diagnostic>> {
    if self.matches(&TokenKind::Return) {
        return Ok(Stmt::Return(self.parse_expr()?));
    }

    if self.matches(&TokenKind::Let) {
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon, "expected ':'")?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Equal, "expected '='")?;
        let value = self.parse_expr()?;
        return Ok(Stmt::Let { name, ty, value });
    }

    if self.matches(&TokenKind::If) {
        let condition = self.parse_expr()?;
        let then_body = self.parse_block()?;
        self.expect(&TokenKind::Else, "expected 'else'")?;
        let else_body = self.parse_block()?;
        return Ok(Stmt::If {
            condition,
            then_body,
            else_body,
        });
    }

    if self.matches(&TokenKind::While) {
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        return Ok(Stmt::While { condition, body });
    }

    let name = self.expect_ident()?;
    self.expect(&TokenKind::Equal, "expected '='")?;
    let value = self.parse_expr()?;
    Ok(Stmt::Assign { name, value })
}
```

Add `parse_block`:

```rust
fn parse_block(&mut self) -> Result<Vec<Stmt>, Vec<Diagnostic>> {
    self.expect(&TokenKind::LeftBrace, "expected '{'")?;
    let mut body = Vec::new();
    while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
        body.push(self.parse_stmt()?);
    }
    self.expect(&TokenKind::RightBrace, "expected '}'")?;
    Ok(body)
}
```

Refactor `parse_function` to call `parse_block` for the function body after reading the return type.

- [ ] **Step 4: Run parser tests**

Run:

```bash
cargo test --test parser_tests
```

Expected: all parser tests pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/ast.rs src/parser.rs tests/parser_tests.rs
git commit -m "feat: parse geo v0.1 statements"
```

---

### Task 2: Type Checking for Locals and Control Flow

**Files:**
- Modify: `src/typecheck.rs`
- Modify: `tests/type_tests.rs`

**Interfaces:**
- Extends: `typecheck::check(program: &Program) -> Result<(), Vec<Diagnostic>>`
- Consumes: new `ast::Stmt` variants.

- [ ] **Step 1: Add type tests**

Append to `tests/type_tests.rs`:

```rust
#[test]
fn accepts_locals_assignment_if_and_while() {
    let source = r#"
        fn main() -> int {
            let x: int = 0
            while x < 42 {
                x = x + 1
            }
            if x == 42 {
                return x
            } else {
                return 0
            }
        }
    "#;
    check_source(source).unwrap();
}

#[test]
fn rejects_assignment_type_mismatch() {
    let err = check_source("fn main() -> int { let x: int = 1 x = true return x }").unwrap_err();
    assert!(err[0].message.contains("assignment type mismatch"));
}

#[test]
fn rejects_non_bool_while_condition() {
    let err = check_source("fn main() -> int { while 1 { return 0 } return 1 }").unwrap_err();
    assert!(err[0].message.contains("while condition must be bool"));
}

#[test]
fn rejects_non_bool_if_condition() {
    let err = check_source("fn main() -> int { if 1 { return 0 } else { return 1 } }").unwrap_err();
    assert!(err[0].message.contains("if condition must be bool"));
}
```

- [ ] **Step 2: Implement statement checking**

Refactor `check_function` in `src/typecheck.rs` to call:

```rust
fn check_stmts<'a>(
    stmts: &'a [crate::ast::Stmt],
    return_type: &Type,
    locals: &mut HashMap<&'a str, Type>,
    functions: &HashMap<&'a str, &'a Function>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            crate::ast::Stmt::Return(expr) => {
                let actual = expr_type(expr, locals, functions, diagnostics);
                if actual != Some(return_type.clone()) {
                    diagnostics.push(Diagnostic::error("return type mismatch"));
                }
            }
            crate::ast::Stmt::Let { name, ty, value } => {
                let actual = expr_type(value, locals, functions, diagnostics);
                if actual != Some(ty.clone()) {
                    diagnostics.push(Diagnostic::error("let initializer type mismatch"));
                }
                if locals.insert(name.as_str(), ty.clone()).is_some() {
                    diagnostics.push(Diagnostic::error(format!("duplicate local '{name}'")));
                }
            }
            crate::ast::Stmt::Assign { name, value } => {
                let expected = locals.get(name.as_str()).cloned();
                let actual = expr_type(value, locals, functions, diagnostics);
                if expected.is_none() {
                    diagnostics.push(Diagnostic::error(format!("unknown variable '{name}'")));
                } else if expected != actual {
                    diagnostics.push(Diagnostic::error("assignment type mismatch"));
                }
            }
            crate::ast::Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                if expr_type(condition, locals, functions, diagnostics) != Some(Type::Bool) {
                    diagnostics.push(Diagnostic::error("if condition must be bool"));
                }
                check_stmts(then_body, return_type, locals, functions, diagnostics);
                check_stmts(else_body, return_type, locals, functions, diagnostics);
            }
            crate::ast::Stmt::While { condition, body } => {
                if expr_type(condition, locals, functions, diagnostics) != Some(Type::Bool) {
                    diagnostics.push(Diagnostic::error("while condition must be bool"));
                }
                check_stmts(body, return_type, locals, functions, diagnostics);
            }
        }
    }
}
```

- [ ] **Step 3: Run type tests**

Run:

```bash
cargo test --test type_tests
```

Expected: all type tests pass.

- [ ] **Step 4: Commit**

Run:

```bash
git add src/typecheck.rs tests/type_tests.rs
git commit -m "feat: typecheck locals and control flow"
```

---

### Task 3: Full v0.1 IR

**Files:**
- Modify: `src/ir.rs`
- Modify: `src/lower.rs`
- Modify: `tests/lower_tests.rs`

**Interfaces:**
- Extends: `ir::Instruction`
- Produces: IR for locals, comparisons, labels, jumps, and calls.

- [ ] **Step 1: Extend IR**

Update `src/ir.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Const { dst: ValueId, value: i64 },
    Add { dst: ValueId, left: ValueId, right: ValueId },
    Sub { dst: ValueId, left: ValueId, right: ValueId },
    Mul { dst: ValueId, left: ValueId, right: ValueId },
    Div { dst: ValueId, left: ValueId, right: ValueId },
    Load { dst: ValueId, local: String },
    Store { local: String, value: ValueId },
    Cmp { dst: ValueId, op: CmpOp, left: ValueId, right: ValueId },
    Jump { label: String },
    JumpIfZero { value: ValueId, label: String },
    Label { name: String },
    Call { dst: ValueId, function: String, args: Vec<ValueId> },
    Return { value: ValueId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
```

- [ ] **Step 2: Add lowering tests**

Append to `tests/lower_tests.rs`:

```rust
#[test]
fn lowers_local_store_and_load() {
    let ir = lower_source("fn main() -> int { let x: int = 42 return x }");
    assert!(ir.functions[0].instructions.iter().any(|ins| {
        matches!(ins, geo::ir::Instruction::Store { local, .. } if local == "x")
    }));
    assert!(ir.functions[0].instructions.iter().any(|ins| {
        matches!(ins, geo::ir::Instruction::Load { local, .. } if local == "x")
    }));
}

#[test]
fn lowers_function_call() {
    let source = "fn add(a: int, b: int) -> int { return a + b } fn main() -> int { return add(10, 32) }";
    let ir = lower_source(source);
    assert!(ir.functions.iter().any(|function| {
        function.instructions.iter().any(|ins| {
            matches!(ins, geo::ir::Instruction::Call { function, .. } if function == "add")
        })
    }));
}

#[test]
fn lowers_while_to_label_and_jump() {
    let ir = lower_source("fn main() -> int { let x: int = 0 while x < 1 { x = x + 1 } return x }");
    assert!(ir.functions[0].instructions.iter().any(|ins| matches!(ins, geo::ir::Instruction::Label { .. })));
    assert!(ir.functions[0].instructions.iter().any(|ins| matches!(ins, geo::ir::Instruction::Jump { .. })));
    assert!(ir.functions[0].instructions.iter().any(|ins| matches!(ins, geo::ir::Instruction::JumpIfZero { .. })));
}
```

- [ ] **Step 3: Implement lowering extensions**

Update `src/lower.rs` so:

- `Expr::Var(name)` emits `Load`.
- `Expr::Call` lowers args left-to-right and emits `Call`.
- comparison binary ops emit `Cmp`.
- `Stmt::Let` lowers initializer and emits `Store`.
- `Stmt::Assign` lowers value and emits `Store`.
- `Stmt::If` emits condition, `JumpIfZero`, then body, `Jump`, else label, else body, end label.
- `Stmt::While` emits start label, condition, `JumpIfZero`, body, `Jump`, end label.

Use a label generator:

```rust
fn fresh_label(&mut self, prefix: &str) -> String {
    let label = format!(".L{prefix}_{}", self.next_label);
    self.next_label += 1;
    label
}
```

- [ ] **Step 4: Run lowering tests**

Run:

```bash
cargo test --test lower_tests
```

Expected: all lowering tests pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/ir.rs src/lower.rs tests/lower_tests.rs
git commit -m "feat: lower geo v0.1 to ir"
```

---

### Task 4: Stack-Based x86-64 Backend for v0.1

**Files:**
- Modify: `src/x86_64.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Extends: `x86_64::emit_nasm(program: &IrProgram) -> String`
- Consumes: full v0.1 IR.

- [ ] **Step 1: Add assembly shape tests**

Append to `tests/compile_tests.rs`:

```rust
#[test]
fn emits_calls_and_stack_locals() {
    let source = r#"
        fn add(a: int, b: int) -> int {
            return a + b
        }

        fn main() -> int {
            let x: int = 10
            let y: int = 32
            return add(x, y)
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    let asm = emit_nasm(&ir);

    assert!(asm.contains("add:"));
    assert!(asm.contains("call add"));
    assert!(asm.contains("mov rdi"));
    assert!(asm.contains("mov rsi"));
}
```

- [ ] **Step 2: Implement backend storage model**

In `src/x86_64.rs`, build a per-function layout:

```rust
struct FunctionLayout {
    value_slots: std::collections::HashMap<ValueId, i32>,
    local_slots: std::collections::HashMap<String, i32>,
    frame_size: i32,
}
```

Assign each virtual value and local a distinct 8-byte stack slot. Round `frame_size` up to a multiple of 16 after `push rbp`.

- [ ] **Step 3: Implement instruction emission**

Emit these instruction patterns:

```asm
; Const
mov qword [rbp - OFFSET], IMM

; Load
mov rax, [rbp - LOCAL_OFFSET]
mov [rbp - DST_OFFSET], rax

; Store
mov rax, [rbp - VALUE_OFFSET]
mov [rbp - LOCAL_OFFSET], rax

; Add
mov rax, [rbp - LEFT_OFFSET]
mov r10, [rbp - RIGHT_OFFSET]
add rax, r10
mov [rbp - DST_OFFSET], rax

; Sub
mov rax, [rbp - LEFT_OFFSET]
mov r10, [rbp - RIGHT_OFFSET]
sub rax, r10
mov [rbp - DST_OFFSET], rax

; Mul
mov rax, [rbp - LEFT_OFFSET]
mov r10, [rbp - RIGHT_OFFSET]
imul rax, r10
mov [rbp - DST_OFFSET], rax

; Div
mov rax, [rbp - LEFT_OFFSET]
cqo
mov r10, [rbp - RIGHT_OFFSET]
idiv r10
mov [rbp - DST_OFFSET], rax

; Cmp
mov rax, [rbp - LEFT_OFFSET]
cmp rax, [rbp - RIGHT_OFFSET]
setcc al
movzx rax, al
mov [rbp - DST_OFFSET], rax

; Jump
jmp LABEL

; JumpIfZero
mov rax, [rbp - VALUE_OFFSET]
cmp rax, 0
je LABEL

; Label
LABEL:

; Call
mov rdi, [rbp - ARG0_OFFSET]
mov rsi, [rbp - ARG1_OFFSET]
call FUNCTION
mov [rbp - DST_OFFSET], rax

; Return
mov rax, [rbp - VALUE_OFFSET]
mov rsp, rbp
pop rbp
ret
```

- [ ] **Step 4: Run assembly shape tests**

Run:

```bash
cargo test --test compile_tests
```

Expected: all compile tests pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/x86_64.rs tests/compile_tests.rs
git commit -m "feat: emit stack based x86_64"
```

---

### Task 5: Build and Run Driver

**Files:**
- Modify: `src/driver.rs`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Extends: `geo build`
- Extends: `geo run`

- [ ] **Step 1: Implement build command**

In `src/driver.rs`, implement `Build` by:

- Reading the `.geo` file.
- Lexing, parsing, type checking, lowering, and emitting assembly.
- Writing a temporary `.asm` file.
- Running `nasm -f elf64 temp.asm -o temp.o`.
- Running `gcc temp.o -o output`.
- Removing temporary files unless `--keep-temps` is set.

Use `std::process::Command`.

- [ ] **Step 2: Implement run command**

In `src/driver.rs`, implement `Run` by:

- Building to a temporary executable.
- Running the executable.
- Exiting the compiler process with the executable's exit code.

- [ ] **Step 3: Add end-to-end tests**

Append to `tests/compile_tests.rs`:

```rust
#[test]
fn compiles_and_runs_return_42_example() {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", "examples/return_42.geo"])
        .status()
        .expect("failed to run geo");

    assert_eq!(status.code(), Some(42));
}
```

- [ ] **Step 4: Run end-to-end tests on Linux with NASM installed**

Run:

```bash
cargo test --test compile_tests
```

Expected: all compile tests pass and `return_42.geo` exits with `42`.

- [ ] **Step 5: Commit**

Run:

```bash
git add src/driver.rs tests/compile_tests.rs
git commit -m "feat: build and run geo programs"
```

---

### Task 6: v0.1 Examples and Acceptance Tests

**Files:**
- Create: `examples/arithmetic.geo`
- Create: `examples/variables.geo`
- Create: `examples/functions.geo`
- Create: `examples/if_else.geo`
- Create: `examples/while.geo`
- Modify: `tests/compile_tests.rs`

**Interfaces:**
- Consumes: complete v0.1 compiler.

- [ ] **Step 1: Create examples**

Create `examples/arithmetic.geo`:

```geo
fn main() -> int {
    return 6 * 7
}
```

Create `examples/variables.geo`:

```geo
fn main() -> int {
    let x: int = 10
    let y: int = 32
    return x + y
}
```

Create `examples/functions.geo`:

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

Create `examples/if_else.geo`:

```geo
fn main() -> int {
    if 10 < 32 {
        return 42
    } else {
        return 1
    }
}
```

Create `examples/while.geo`:

```geo
fn main() -> int {
    let x: int = 0
    while x < 42 {
        x = x + 1
    }
    return x
}
```

- [ ] **Step 2: Add acceptance test helper**

Append to `tests/compile_tests.rs`:

```rust
fn assert_geo_exit(path: &str, expected: i32) {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path])
        .status()
        .expect("failed to run geo");

    assert_eq!(status.code(), Some(expected));
}
```

- [ ] **Step 3: Add v0.1 acceptance tests**

Append to `tests/compile_tests.rs`:

```rust
#[test]
fn compiles_v0_1_examples() {
    assert_geo_exit("examples/arithmetic.geo", 42);
    assert_geo_exit("examples/variables.geo", 42);
    assert_geo_exit("examples/functions.geo", 42);
    assert_geo_exit("examples/if_else.geo", 42);
    assert_geo_exit("examples/while.geo", 42);
}
```

- [ ] **Step 4: Run all tests**

Run:

```bash
cargo test
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add examples tests/compile_tests.rs
git commit -m "test: add geo v0.1 acceptance examples"
```

---

## Plan Self-Review

- Spec coverage: This plan covers the v0.1 features intentionally deferred from the first milestone plan: locals, assignment, calls, comparisons, branches, and loops.
- Placeholder scan: The plan gives concrete files, tests, commands, expected outcomes, and backend instruction patterns.
- Type consistency: `Stmt`, `Instruction`, `CmpOp`, and backend interfaces match the base implementation plan.
- Scope control: No strings, arrays, structs, pointers, modules, optimization, register allocation, `_start`, or syscall work is included.
