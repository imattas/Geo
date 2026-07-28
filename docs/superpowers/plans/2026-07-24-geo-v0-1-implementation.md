# Geo v0.1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Geo compiler milestone set: a Rust CLI that reads `.geo` files, checks them, lowers them to IR, emits x86-64 Linux NASM assembly, and compiles the v0.1 language subset to native executables.

**Architecture:** The compiler is a staged pipeline: lexer, parser, AST, type checker, IR lowering, x86-64 NASM backend, and driver commands. Each stage exposes typed Rust data structures and focused tests before later stages consume it.

**Tech Stack:** Rust, Cargo, `clap`, `thiserror` or custom diagnostics, NASM, `gcc` or `clang`, x86-64 Linux System V ABI.

## Global Constraints

- Source files use the `.geo` extension.
- The compiler executable is named `geo`.
- The compiler is implemented in Rust.
- The initial target is x86-64 Linux using the System V ABI.
- Assembly output uses NASM Intel syntax.
- Object files are produced with `nasm -f elf64`.
- Executables are linked with `gcc` or `clang`.
- Geo v0.1 does not use LLVM.
- Geo v0.1 does not use C code generation.
- Geo v0.1 uses a simple stack-based backend.
- Expression codegen leaves results in `rax`.
- Return values use `rax`.
- The first six integer arguments use `rdi`, `rsi`, `rdx`, `rcx`, `r8`, and `r9`.
- Functions with more than six parameters are rejected in v0.1.
- Geo v0.1 does not include strings, arrays, structs, pointers, references, generics, traits, borrow checking, classes, closures, async, macros, modules, a package manager, garbage collection, advanced optimization, or register allocation.

---

## File Structure

- Create `Cargo.toml`: Rust package manifest for the `geo` compiler binary.
- Create `src/main.rs`: binary entry point.
- Create `src/cli.rs`: command-line parser and command enum.
- Create `src/token.rs`: token kinds, spans, and token records.
- Create `src/lexer.rs`: source-to-token lexer.
- Create `src/diagnostics.rs`: diagnostic structs and rendering helpers.
- Create `src/ast.rs`: AST node definitions.
- Create `src/parser.rs`: token-to-AST parser.
- Create `src/typecheck.rs`: symbol tables and type validation.
- Create `src/ir.rs`: IR value, instruction, function, and program definitions.
- Create `src/lower.rs`: AST-to-IR lowering.
- Create `src/x86_64.rs`: IR-to-NASM backend.
- Create `src/driver.rs`: orchestration for `check`, `emit-asm`, `build`, and `run`.
- Create `examples/return_42.geo`: first executable program.
- Create `examples/functions.geo`: function-call milestone program.
- Create `tests/lexer_tests.rs`: lexer tests.
- Create `tests/parser_tests.rs`: parser tests.
- Create `tests/type_tests.rs`: type checker tests.
- Create `tests/lower_tests.rs`: IR lowering tests.
- Create `tests/compile_tests.rs`: assembly, link, and run tests.

---

### Task 1: Rust Project Skeleton and CLI

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/cli.rs`
- Create: `src/diagnostics.rs`
- Create: `src/driver.rs`
- Create: `examples/return_42.geo`

**Interfaces:**
- Produces: `cli::Cli`, `cli::Command`, `driver::run_cli(cli::Cli) -> Result<(), Vec<diagnostics::Diagnostic>>`
- Produces: `diagnostics::Diagnostic::error(message: impl Into<String>) -> Diagnostic`

- [ ] **Step 1: Write the first example program**

Create `examples/return_42.geo`:

```geo
fn main() -> int {
    return 42
}
```

- [ ] **Step 2: Create the Cargo package**

Create `Cargo.toml`:

```toml
[package]
name = "geo"
version = "0.0.0"
edition = "2021"

[[bin]]
name = "geo"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 3: Add CLI types**

Create `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "geo")]
#[command(about = "The Geo programming language compiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Check {
        input: PathBuf,
    },
    EmitAsm {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Build {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value = "nasm")]
        nasm: String,
        #[arg(long, default_value = "gcc")]
        linker: String,
        #[arg(long)]
        keep_temps: bool,
    },
    Run {
        input: PathBuf,
        #[arg(long, default_value = "nasm")]
        nasm: String,
        #[arg(long, default_value = "gcc")]
        linker: String,
    },
}
```

- [ ] **Step 4: Add diagnostics shell**

Create `src/diagnostics.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
```

- [ ] **Step 5: Add driver file validation**

Create `src/driver.rs`:

```rust
use crate::cli::{Cli, Command};
use crate::diagnostics::Diagnostic;
use std::fs;
use std::path::Path;

pub fn run_cli(cli: Cli) -> Result<(), Vec<Diagnostic>> {
    match cli.command {
        Command::Check { input } => {
            read_geo_source(&input)?;
            Ok(())
        }
        Command::EmitAsm { input, output: _ } => {
            read_geo_source(&input)?;
            Err(vec![Diagnostic::error("emit-asm is not implemented yet")])
        }
        Command::Build { input, .. } => {
            read_geo_source(&input)?;
            Err(vec![Diagnostic::error("build is not implemented yet")])
        }
        Command::Run { input, .. } => {
            read_geo_source(&input)?;
            Err(vec![Diagnostic::error("run is not implemented yet")])
        }
    }
}

pub fn read_geo_source(path: &Path) -> Result<String, Vec<Diagnostic>> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("geo") {
        return Err(vec![Diagnostic::error("Geo source files must use the .geo extension")]);
    }

    fs::read_to_string(path)
        .map_err(|err| vec![Diagnostic::error(format!("failed to read source file: {err}"))])
}
```

- [ ] **Step 6: Add binary entry point**

Create `src/main.rs`:

```rust
mod cli;
mod diagnostics;
mod driver;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(diagnostics) = driver::run_cli(cli) {
        for diagnostic in diagnostics {
            eprintln!("error: {}", diagnostic.message);
        }
        std::process::exit(1);
    }
}
```

- [ ] **Step 7: Verify CLI skeleton**

Run:

```bash
cargo run -- check examples/return_42.geo
```

Expected: command exits successfully.

Run:

```bash
cargo run -- check examples/return_42.txt
```

Expected: command exits with an error mentioning `.geo`.

- [ ] **Step 8: Commit**

Run:

```bash
git add Cargo.toml src examples
git commit -m "chore: create geo compiler skeleton"
```

If the workspace is not a git repository, initialize git before the commit:

```bash
git init
git add Cargo.toml src examples
git commit -m "chore: create geo compiler skeleton"
```

---

### Task 2: Lexer

**Files:**
- Create: `src/token.rs`
- Create: `src/lexer.rs`
- Create: `tests/lexer_tests.rs`
- Modify: `src/main.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `token::Span { line: usize, column: usize, offset: usize, len: usize }`
- Produces: `token::Token { kind: TokenKind, span: Span }`
- Produces: `lexer::lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>>`
- Consumes: `diagnostics::Diagnostic`

- [ ] **Step 1: Add token definitions**

Create `src/token.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Fn,
    Return,
    Let,
    If,
    Else,
    While,
    True,
    False,
    Int,
    Bool,
    Ident(String),
    IntLiteral(i64),
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Colon,
    Comma,
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Arrow,
    Eof,
}
```

- [ ] **Step 2: Write failing lexer tests**

Create `tests/lexer_tests.rs`:

```rust
use geo::lexer::lex;
use geo::token::TokenKind;

#[test]
fn tokenizes_return_42() {
    let tokens = lex("fn main() -> int {\n    return 42\n}\n").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Ident("main".to_string()),
            TokenKind::LeftParen,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::Int,
            TokenKind::LeftBrace,
            TokenKind::Return,
            TokenKind::IntLiteral(42),
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn distinguishes_single_and_double_character_operators() {
    let tokens = lex("= == != < <= > >= ->").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Equal,
            TokenKind::EqualEqual,
            TokenKind::BangEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Arrow,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tracks_line_and_column() {
    let tokens = lex("fn\n  main").unwrap();
    assert_eq!(tokens[1].span.line, 2);
    assert_eq!(tokens[1].span.column, 3);
}

#[test]
fn rejects_unknown_characters() {
    let err = lex("@").unwrap_err();
    assert!(err[0].message.contains("unexpected character '@'"));
}
```

- [ ] **Step 3: Expose library modules**

Create `src/lib.rs`:

```rust
pub mod diagnostics;
pub mod lexer;
pub mod token;
```

Modify `src/main.rs`:

```rust
mod cli;
mod driver;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(diagnostics) = driver::run_cli(cli) {
        for diagnostic in diagnostics {
            eprintln!("error: {}", diagnostic.message);
        }
        std::process::exit(1);
    }
}
```

- [ ] **Step 4: Implement lexer**

Create `src/lexer.rs`:

```rust
use crate::diagnostics::Diagnostic;
use crate::token::{Span, Token, TokenKind};

pub fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer::new(source);
    lexer.lex_all()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    index: usize,
    offset: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            index: 0,
            offset: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex_all(&mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance_newline();
                }
                '0'..='9' => self.lex_number(),
                'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
                ':' => self.single(TokenKind::Colon),
                ',' => self.single(TokenKind::Comma),
                '+' => self.single(TokenKind::Plus),
                '*' => self.single(TokenKind::Star),
                '/' => self.single(TokenKind::Slash),
                '-' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('>') {
                        self.push(TokenKind::Arrow, span, 2);
                    } else {
                        self.push(TokenKind::Minus, span, 1);
                    }
                }
                '=' => self.two_char(TokenKind::Equal, '=', TokenKind::EqualEqual),
                '!' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('=') {
                        self.push(TokenKind::BangEqual, span, 2);
                    } else {
                        self.diagnostics.push(Diagnostic::error("unexpected character '!'"));
                    }
                }
                '<' => self.two_char(TokenKind::Less, '=', TokenKind::LessEqual),
                '>' => self.two_char(TokenKind::Greater, '=', TokenKind::GreaterEqual),
                other => {
                    self.diagnostics
                        .push(Diagnostic::error(format!("unexpected character '{other}'")));
                    self.advance();
                }
            }
        }

        let eof_span = self.start_span();
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: eof_span,
        });

        if self.diagnostics.is_empty() {
            Ok(std::mem::take(&mut self.tokens))
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        self.offset += ch.len_utf8();
        self.column += 1;
        Some(ch)
    }

    fn advance_newline(&mut self) {
        self.index += 1;
        self.offset += 1;
        self.line += 1;
        self.column = 1;
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn start_span(&self) -> Span {
        Span {
            line: self.line,
            column: self.column,
            offset: self.offset,
            len: 1,
        }
    }

    fn push(&mut self, kind: TokenKind, mut span: Span, len: usize) {
        span.len = len;
        self.tokens.push(Token { kind, span });
    }

    fn single(&mut self, kind: TokenKind) {
        let span = self.start_span();
        self.advance();
        self.push(kind, span, 1);
    }

    fn two_char(&mut self, single: TokenKind, second: char, double: TokenKind) {
        let span = self.start_span();
        self.advance();
        if self.match_char(second) {
            self.push(double, span, 2);
        } else {
            self.push(single, span, 1);
        }
    }

    fn lex_number(&mut self) {
        let span = self.start_span();
        let start = self.offset;
        while matches!(self.peek(), Some('0'..='9')) {
            self.advance();
        }
        let text = &self.source[start..self.offset];
        let value = text.parse::<i64>().expect("lexer collected only digits");
        self.push(TokenKind::IntLiteral(value), span, text.len());
    }

    fn lex_identifier(&mut self) {
        let span = self.start_span();
        let start = self.offset;
        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.advance();
        }
        let text = &self.source[start..self.offset];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "return" => TokenKind::Return,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "int" => TokenKind::Int,
            "bool" => TokenKind::Bool,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.push(kind, span, text.len());
    }
}
```

- [ ] **Step 5: Wire check command through lexer**

Modify `src/driver.rs`:

```rust
use crate::cli::{Cli, Command};
use geo::diagnostics::Diagnostic;
use geo::lexer::lex;
use std::fs;
use std::path::Path;

pub fn run_cli(cli: Cli) -> Result<(), Vec<Diagnostic>> {
    match cli.command {
        Command::Check { input } => {
            let source = read_geo_source(&input)?;
            lex(&source)?;
            Ok(())
        }
        Command::EmitAsm { input, output: _ } => {
            let source = read_geo_source(&input)?;
            lex(&source)?;
            Err(vec![Diagnostic::error("emit-asm is not implemented yet")])
        }
        Command::Build { input, .. } => {
            let source = read_geo_source(&input)?;
            lex(&source)?;
            Err(vec![Diagnostic::error("build is not implemented yet")])
        }
        Command::Run { input, .. } => {
            let source = read_geo_source(&input)?;
            lex(&source)?;
            Err(vec![Diagnostic::error("run is not implemented yet")])
        }
    }
}

pub fn read_geo_source(path: &Path) -> Result<String, Vec<Diagnostic>> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("geo") {
        return Err(vec![Diagnostic::error("Geo source files must use the .geo extension")]);
    }

    fs::read_to_string(path)
        .map_err(|err| vec![Diagnostic::error(format!("failed to read source file: {err}"))])
}
```

- [ ] **Step 6: Run lexer tests**

Run:

```bash
cargo test --test lexer_tests
```

Expected: all lexer tests pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add src tests
git commit -m "feat: add geo lexer"
```

---

### Task 3: Parser and AST for Functions and Returns

**Files:**
- Create: `src/ast.rs`
- Create: `src/parser.rs`
- Create: `tests/parser_tests.rs`
- Modify: `src/lib.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `parser::parse(tokens: &[Token]) -> Result<ast::Program, Vec<Diagnostic>>`
- Produces: `ast::Program`, `ast::Function`, `ast::Stmt`, `ast::Expr`, `ast::Type`
- Consumes: `lexer::lex`

- [ ] **Step 1: Define AST types**

Create `src/ast.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Return(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Var(String),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
```

- [ ] **Step 2: Write parser tests**

Create `tests/parser_tests.rs`:

```rust
use geo::ast::{BinaryOp, Expr, Stmt, Type};
use geo::lexer::lex;
use geo::parser::parse;

#[test]
fn parses_return_42() {
    let tokens = lex("fn main() -> int { return 42 }").unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].name, "main");
    assert_eq!(program.functions[0].return_type, Type::Int);
    assert_eq!(program.functions[0].body, vec![Stmt::Return(Expr::Int(42))]);
}

#[test]
fn parses_function_with_parameters_and_call() {
    let source = "fn main() -> int { return add(10, 32) }";
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    match &program.functions[0].body[0] {
        Stmt::Return(Expr::Call { name, args }) => {
            assert_eq!(name, "add");
            assert_eq!(args, &vec![Expr::Int(10), Expr::Int(32)]);
        }
    }
}

#[test]
fn parses_binary_precedence() {
    let tokens = lex("fn main() -> int { return 1 + 2 * 3 }").unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(
        program.functions[0].body[0],
        Stmt::Return(Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Int(1)),
            right: Box::new(Expr::Binary {
                op: BinaryOp::Mul,
                left: Box::new(Expr::Int(2)),
                right: Box::new(Expr::Int(3)),
            }),
        })
    );
}

#[test]
fn reports_missing_right_brace() {
    let tokens = lex("fn main() -> int { return 42").unwrap();
    let err = parse(&tokens).unwrap_err();
    assert!(err[0].message.contains("expected '}'"));
}
```

- [ ] **Step 3: Implement parser**

Create `src/parser.rs` with a recursive descent parser:

```rust
use crate::ast::*;
use crate::diagnostics::Diagnostic;
use crate::token::{Token, TokenKind};

pub fn parse(tokens: &[Token]) -> Result<Program, Vec<Diagnostic>> {
    Parser::new(tokens).parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, current: 0 }
    }

    fn parse_program(&mut self) -> Result<Program, Vec<Diagnostic>> {
        let mut functions = Vec::new();
        while !self.at(&TokenKind::Eof) {
            functions.push(self.parse_function()?);
        }
        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, Vec<Diagnostic>> {
        self.expect(&TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LeftParen, "expected '('")?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::RightParen, "expected ')'")?;
        self.expect(&TokenKind::Arrow, "expected '->'")?;
        let return_type = self.parse_type()?;
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut body = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            body.push(self.parse_stmt()?);
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(Function {
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Vec<Diagnostic>> {
        let mut params = Vec::new();
        if self.at(&TokenKind::RightParen) {
            return Ok(params);
        }

        loop {
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Colon, "expected ':'")?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            if !self.matches(&TokenKind::Comma) {
                break;
            }
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<Type, Vec<Diagnostic>> {
        if self.matches(&TokenKind::Int) {
            Ok(Type::Int)
        } else if self.matches(&TokenKind::Bool) {
            Ok(Type::Bool)
        } else {
            Err(vec![Diagnostic::error("expected type")])
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Vec<Diagnostic>> {
        self.expect(&TokenKind::Return, "expected statement")?;
        Ok(Stmt::Return(self.parse_expr()?))
    }

    fn parse_expr(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else if self.matches(&TokenKind::BangEqual) {
                BinaryOp::NotEqual
            } else if self.matches(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.matches(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.matches(&TokenKind::Greater) {
                BinaryOp::Greater
            } else if self.matches(&TokenKind::GreaterEqual) {
                BinaryOp::GreaterEqual
            } else {
                break;
            };
            let right = self.parse_term()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                BinaryOp::Add
            } else if self.matches(&TokenKind::Minus) {
                BinaryOp::Sub
            } else {
                break;
            };
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_primary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.matches(&TokenKind::Slash) {
                BinaryOp::Div
            } else {
                break;
            };
            let right = self.parse_primary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        match self.advance().kind.clone() {
            TokenKind::IntLiteral(value) => Ok(Expr::Int(value)),
            TokenKind::True => Ok(Expr::Bool(true)),
            TokenKind::False => Ok(Expr::Bool(false)),
            TokenKind::Ident(name) => {
                if self.matches(&TokenKind::LeftParen) {
                    let mut args = Vec::new();
                    if !self.at(&TokenKind::RightParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.matches(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(&TokenKind::RightParen, "expected ')'")?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Var(name))
                }
            }
            _ => Err(vec![Diagnostic::error("expected expression")]),
        }
    }

    fn expect_ident(&mut self) -> Result<String, Vec<Diagnostic>> {
        match self.advance().kind.clone() {
            TokenKind::Ident(name) => Ok(name),
            _ => Err(vec![Diagnostic::error("expected identifier")]),
        }
    }

    fn expect(&mut self, kind: &TokenKind, message: &str) -> Result<(), Vec<Diagnostic>> {
        if self.matches(kind) {
            Ok(())
        } else {
            Err(vec![Diagnostic::error(message)])
        }
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn advance(&mut self) -> &'a Token {
        let token = self.peek();
        if !self.at(&TokenKind::Eof) {
            self.current += 1;
        }
        token
    }

    fn peek(&self) -> &'a Token {
        &self.tokens[self.current]
    }
}
```

- [ ] **Step 4: Export parser and AST modules**

Modify `src/lib.rs`:

```rust
pub mod ast;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod token;
```

- [ ] **Step 5: Wire check command through parser**

Modify the `Check` branch in `src/driver.rs`:

```rust
Command::Check { input } => {
    let source = read_geo_source(&input)?;
    let tokens = lex(&source)?;
    geo::parser::parse(&tokens)?;
    Ok(())
}
```

- [ ] **Step 6: Run parser tests**

Run:

```bash
cargo test --test parser_tests
```

Expected: all parser tests pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add src tests
git commit -m "feat: parse functions and expressions"
```

---

### Task 4: Type Checker for First Programs

**Files:**
- Create: `src/typecheck.rs`
- Create: `tests/type_tests.rs`
- Modify: `src/lib.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `typecheck::check(program: &Program) -> Result<(), Vec<Diagnostic>>`
- Consumes: `ast::Program`

- [ ] **Step 1: Write type checker tests**

Create `tests/type_tests.rs`:

```rust
use geo::lexer::lex;
use geo::parser::parse;
use geo::typecheck::check;

fn check_source(source: &str) -> Result<(), Vec<geo::diagnostics::Diagnostic>> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program)
}

#[test]
fn accepts_return_42() {
    check_source("fn main() -> int { return 42 }").unwrap();
}

#[test]
fn rejects_wrong_return_type() {
    let err = check_source("fn main() -> int { return true }").unwrap_err();
    assert!(err[0].message.contains("return type mismatch"));
}

#[test]
fn rejects_unknown_variable() {
    let err = check_source("fn main() -> int { return x }").unwrap_err();
    assert!(err[0].message.contains("unknown variable 'x'"));
}

#[test]
fn rejects_wrong_call_arity() {
    let source = "fn add(a: int, b: int) -> int { return a + b } fn main() -> int { return add(1) }";
    let err = check_source(source).unwrap_err();
    assert!(err[0].message.contains("expected 2 arguments"));
}

#[test]
fn rejects_bool_arithmetic() {
    let err = check_source("fn main() -> int { return 1 + true }").unwrap_err();
    assert!(err[0].message.contains("arithmetic operands must be int"));
}
```

- [ ] **Step 2: Implement type checker**

Create `src/typecheck.rs`:

```rust
use crate::ast::{BinaryOp, Expr, Function, Program, Type};
use crate::diagnostics::Diagnostic;
use std::collections::HashMap;

pub fn check(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut functions = HashMap::new();
    let mut diagnostics = Vec::new();

    for function in &program.functions {
        if functions.insert(function.name.as_str(), function).is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function '{}'",
                function.name
            )));
        }
        if function.params.len() > 6 {
            diagnostics.push(Diagnostic::error(format!(
                "function '{}' has more than 6 parameters",
                function.name
            )));
        }
    }

    if !functions.contains_key("main") {
        diagnostics.push(Diagnostic::error("missing main function"));
    }

    for function in &program.functions {
        check_function(function, &functions, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_function<'a>(
    function: &'a Function,
    functions: &HashMap<&'a str, &'a Function>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut locals = HashMap::new();
    for param in &function.params {
        locals.insert(param.name.as_str(), param.ty.clone());
    }

    for stmt in &function.body {
        match stmt {
            crate::ast::Stmt::Return(expr) => {
                let actual = expr_type(expr, &locals, functions, diagnostics);
                if actual != Some(function.return_type.clone()) {
                    diagnostics.push(Diagnostic::error("return type mismatch"));
                }
            }
        }
    }
}

fn expr_type<'a>(
    expr: &'a Expr,
    locals: &HashMap<&'a str, Type>,
    functions: &HashMap<&'a str, &'a Function>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Type> {
    match expr {
        Expr::Int(_) => Some(Type::Int),
        Expr::Bool(_) => Some(Type::Bool),
        Expr::Var(name) => locals.get(name.as_str()).cloned().or_else(|| {
            diagnostics.push(Diagnostic::error(format!("unknown variable '{name}'")));
            None
        }),
        Expr::Binary { op, left, right } => {
            let left_ty = expr_type(left, locals, functions, diagnostics);
            let right_ty = expr_type(right, locals, functions, diagnostics);
            match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                    if left_ty == Some(Type::Int) && right_ty == Some(Type::Int) {
                        Some(Type::Int)
                    } else {
                        diagnostics.push(Diagnostic::error("arithmetic operands must be int"));
                        None
                    }
                }
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => {
                    if left_ty == Some(Type::Int) && right_ty == Some(Type::Int) {
                        Some(Type::Bool)
                    } else {
                        diagnostics.push(Diagnostic::error("comparison operands must be int"));
                        None
                    }
                }
            }
        }
        Expr::Call { name, args } => {
            let Some(function) = functions.get(name.as_str()) else {
                diagnostics.push(Diagnostic::error(format!("unknown function '{name}'")));
                return None;
            };
            if args.len() != function.params.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "function '{name}' expected {} arguments but got {}",
                    function.params.len(),
                    args.len()
                )));
                return None;
            }
            for (arg, param) in args.iter().zip(function.params.iter()) {
                let arg_ty = expr_type(arg, locals, functions, diagnostics);
                if arg_ty != Some(param.ty.clone()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "argument '{}' type mismatch",
                        param.name
                    )));
                }
            }
            Some(function.return_type.clone())
        }
    }
}
```

- [ ] **Step 3: Export type checker**

Modify `src/lib.rs`:

```rust
pub mod ast;
pub mod diagnostics;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod typecheck;
```

- [ ] **Step 4: Wire check command through type checker**

Modify the `Check` branch in `src/driver.rs`:

```rust
Command::Check { input } => {
    let source = read_geo_source(&input)?;
    let tokens = lex(&source)?;
    let program = geo::parser::parse(&tokens)?;
    geo::typecheck::check(&program)?;
    Ok(())
}
```

- [ ] **Step 5: Run type checker tests**

Run:

```bash
cargo test --test type_tests
```

Expected: all type checker tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add src tests
git commit -m "feat: add basic type checking"
```

---

### Task 5: IR and Lowering for Return Expressions

**Files:**
- Create: `src/ir.rs`
- Create: `src/lower.rs`
- Create: `tests/lower_tests.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `lower::lower(program: &Program) -> ir::IrProgram`
- Produces: `ir::Instruction`, `ir::IrFunction`, `ir::IrProgram`
- Consumes: checked `ast::Program`

- [ ] **Step 1: Define IR**

Create `src/ir.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Const { dst: ValueId, value: i64 },
    Add { dst: ValueId, left: ValueId, right: ValueId },
    Sub { dst: ValueId, left: ValueId, right: ValueId },
    Mul { dst: ValueId, left: ValueId, right: ValueId },
    Div { dst: ValueId, left: ValueId, right: ValueId },
    Return { value: ValueId },
}
```

- [ ] **Step 2: Write lowering tests**

Create `tests/lower_tests.rs`:

```rust
use geo::ir::{Instruction, ValueId};
use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::typecheck::check;

fn lower_source(source: &str) -> geo::ir::IrProgram {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    lower(&program)
}

#[test]
fn lowers_return_42() {
    let ir = lower_source("fn main() -> int { return 42 }");
    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::Const {
                dst: ValueId(0),
                value: 42
            },
            Instruction::Return { value: ValueId(0) },
        ]
    );
}

#[test]
fn lowers_addition() {
    let ir = lower_source("fn main() -> int { return 1 + 2 }");
    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::Const {
                dst: ValueId(0),
                value: 1
            },
            Instruction::Const {
                dst: ValueId(1),
                value: 2
            },
            Instruction::Add {
                dst: ValueId(2),
                left: ValueId(0),
                right: ValueId(1)
            },
            Instruction::Return { value: ValueId(2) },
        ]
    );
}
```

- [ ] **Step 3: Implement lowering**

Create `src/lower.rs`:

```rust
use crate::ast::{BinaryOp, Expr, Program, Stmt};
use crate::ir::{Instruction, IrFunction, IrProgram, ValueId};

pub fn lower(program: &Program) -> IrProgram {
    IrProgram {
        functions: program.functions.iter().map(lower_function).collect(),
    }
}

fn lower_function(function: &crate::ast::Function) -> IrFunction {
    let mut ctx = LowerCtx {
        next_value: 0,
        instructions: Vec::new(),
    };

    for stmt in &function.body {
        match stmt {
            Stmt::Return(expr) => {
                let value = ctx.lower_expr(expr);
                ctx.instructions.push(Instruction::Return { value });
            }
        }
    }

    IrFunction {
        name: function.name.clone(),
        params: function.params.iter().map(|param| param.name.clone()).collect(),
        instructions: ctx.instructions,
    }
}

struct LowerCtx {
    next_value: usize,
    instructions: Vec<Instruction>,
}

impl LowerCtx {
    fn fresh(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    fn lower_expr(&mut self, expr: &Expr) -> ValueId {
        match expr {
            Expr::Int(value) => {
                let dst = self.fresh();
                self.instructions.push(Instruction::Const { dst, value: *value });
                dst
            }
            Expr::Binary { op, left, right } => {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let dst = self.fresh();
                let instruction = match op {
                    BinaryOp::Add => Instruction::Add { dst, left, right },
                    BinaryOp::Sub => Instruction::Sub { dst, left, right },
                    BinaryOp::Mul => Instruction::Mul { dst, left, right },
                    BinaryOp::Div => Instruction::Div { dst, left, right },
                    _ => panic!("comparison lowering is not part of this task"),
                };
                self.instructions.push(instruction);
                dst
            }
            _ => panic!("expression lowering is not part of this task"),
        }
    }
}
```

- [ ] **Step 4: Export IR and lowering modules**

Modify `src/lib.rs`:

```rust
pub mod ast;
pub mod diagnostics;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
pub mod token;
pub mod typecheck;
```

- [ ] **Step 5: Run lowering tests**

Run:

```bash
cargo test --test lower_tests
```

Expected: all lowering tests pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add src tests
git commit -m "feat: lower expressions to ir"
```

---

### Task 6: x86-64 Assembly for `return 42`

**Files:**
- Create: `src/x86_64.rs`
- Create: `tests/compile_tests.rs`
- Modify: `src/lib.rs`
- Modify: `src/driver.rs`

**Interfaces:**
- Produces: `x86_64::emit_nasm(program: &IrProgram) -> String`
- Consumes: `ir::IrProgram`
- Consumes: `lower::lower`

- [ ] **Step 1: Write assembly emission test**

Create `tests/compile_tests.rs`:

```rust
use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::typecheck::check;
use geo::x86_64::emit_nasm;

#[test]
fn emits_assembly_for_return_42() {
    let tokens = lex("fn main() -> int { return 42 }").unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    let asm = emit_nasm(&ir);

    assert!(asm.contains("global main"));
    assert!(asm.contains("main:"));
    assert!(asm.contains("mov rax, 42"));
    assert!(asm.contains("ret"));
}
```

- [ ] **Step 2: Implement basic assembly backend**

Create `src/x86_64.rs`:

```rust
use crate::ir::{Instruction, IrProgram, ValueId};
use std::collections::HashMap;

pub fn emit_nasm(program: &IrProgram) -> String {
    let mut out = String::new();
    out.push_str("global main\n");
    out.push_str("section .text\n\n");

    for function in &program.functions {
        out.push_str(&format!("{}:\n", function.name));
        out.push_str("    push rbp\n");
        out.push_str("    mov rbp, rsp\n");

        let mut values = HashMap::<ValueId, i64>::new();
        for instruction in &function.instructions {
            match instruction {
                Instruction::Const { dst, value } => {
                    values.insert(*dst, *value);
                }
                Instruction::Return { value } => {
                    let immediate = values
                        .get(value)
                        .copied()
                        .expect("return_42 backend only supports constant returns");
                    out.push_str(&format!("    mov rax, {immediate}\n"));
                    out.push_str("    mov rsp, rbp\n");
                    out.push_str("    pop rbp\n");
                    out.push_str("    ret\n");
                }
                _ => panic!("only constant return codegen is part of this task"),
            }
        }
        out.push('\n');
    }

    out
}
```

- [ ] **Step 3: Export backend module**

Modify `src/lib.rs`:

```rust
pub mod ast;
pub mod diagnostics;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
pub mod token;
pub mod typecheck;
pub mod x86_64;
```

- [ ] **Step 4: Wire `emit-asm`**

Modify the `EmitAsm` branch in `src/driver.rs`:

```rust
Command::EmitAsm { input, output } => {
    let source = read_geo_source(&input)?;
    let tokens = lex(&source)?;
    let program = geo::parser::parse(&tokens)?;
    geo::typecheck::check(&program)?;
    let ir = geo::lower::lower(&program);
    let asm = geo::x86_64::emit_nasm(&ir);
    std::fs::write(&output, asm).map_err(|err| {
        vec![Diagnostic::error(format!(
            "failed to write assembly file: {err}"
        ))]
    })?;
    Ok(())
}
```

- [ ] **Step 5: Run assembly emission test**

Run:

```bash
cargo test --test compile_tests
```

Expected: assembly emission test passes.

- [ ] **Step 6: Manually verify NASM and linker**

Run:

```bash
cargo run -- emit-asm examples/return_42.geo -o out.asm
nasm -f elf64 out.asm -o out.o
gcc out.o -o out
./out
echo $?
```

Expected: exit code is `42`.

- [ ] **Step 7: Commit**

Run:

```bash
git add src tests
git commit -m "feat: emit nasm for return constant"
```

---

### Task 7: Locals, Assignment, Calls, and Control Flow Plan Expansion

**Files:**
- Modify: `src/ast.rs`
- Modify: `src/parser.rs`
- Modify: `src/typecheck.rs`
- Modify: `src/ir.rs`
- Modify: `src/lower.rs`
- Modify: `src/x86_64.rs`
- Modify: `tests/parser_tests.rs`
- Modify: `tests/type_tests.rs`
- Modify: `tests/lower_tests.rs`
- Modify: `tests/compile_tests.rs`
- Create: `examples/functions.geo`
- Create: `examples/variables.geo`
- Create: `examples/if_else.geo`
- Create: `examples/while.geo`

**Interfaces:**
- Extends: `ast::Stmt` with `Let`, `Assign`, `If`, and `While`
- Extends: `ir::Instruction` with `Load`, `Store`, `Cmp`, `Jump`, `JumpIfZero`, `Label`, and `Call`
- Extends: `x86_64::emit_nasm` to allocate stack slots and generate function calls

- [ ] **Step 1: Create milestone examples**

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

Create `examples/variables.geo`:

```geo
fn main() -> int {
    let x: int = 10
    let y: int = 32
    return x + y
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

- [ ] **Step 2: Split this task before implementation**

Before implementing this task, create a follow-up plan file named `docs/superpowers/plans/2026-07-24-geo-v0-1-control-flow-expansion.md`.

The follow-up plan must split this large task into these smaller executable tasks:

- Parser support for `let`, assignment, `if`, `else`, and `while`.
- Type checking for locals, assignment, and control-flow conditions.
- IR support for locals, labels, jumps, comparisons, and calls.
- Stack-slot backend for locals and arithmetic.
- Function-call backend using System V argument registers.
- Control-flow backend for `if / else` and `while`.
- End-to-end compile tests for all v0.1 examples.

- [ ] **Step 3: Commit examples and follow-up plan**

Run:

```bash
git add examples docs/superpowers/plans/2026-07-24-geo-v0-1-control-flow-expansion.md
git commit -m "docs: plan geo v0.1 expansion"
```

---

## Plan Self-Review

- Spec coverage: This plan covers skeleton, CLI, lexer, parser, type checker, IR, lowering, initial NASM backend, assembly/linking strategy, ABI constraints, and the first native executable. The broad v0.1 expansion is intentionally split into a required follow-up plan before implementation because locals, calls, and control flow are too large for one reviewable task.
- Placeholder scan: No task uses placeholder markers or unspecified test instructions.
- Type consistency: The interfaces in later tasks consume the exact module and function names introduced earlier.
- Scope control: Strings, arrays, structs, pointers, references, generics, borrow checking, modules, `_start`, syscalls, optimization, and register allocation remain outside v0.1.
