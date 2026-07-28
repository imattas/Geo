# Geo Clean Core Syntax Design

## Purpose

Geo keeps its current clean Rust-like structure, but simplifies the daily syntax for small scripts, large projects, and low-level systems code. The syntax should avoid unnecessary punctuation, avoid early Rust complexity, and make common programs read naturally.

## Core Direction

Geo source uses:

- `import std.io`
- `fn` for function declarations
- `name: type` for explicit types
- `-> type` for meaningful return values
- omitted return type for unit-returning functions
- `let` for immutable inferred or explicit locals
- `var` for mutable inferred or explicit locals
- optional semicolons
- braces for blocks
- explicit `return` when returning a value

The canonical hello world is:

```geo
import std.io

fn main() {
    println("Hello, world!")
}
```

Explicit exit-code programs remain supported:

```geo
import std.io

fn main() -> int {
    println("Hello, world!")
    return 0
}
```

## Function Rules

Functions with no return annotation return `unit`.

```geo
fn log(message: str) {
    println(message)
}
```

Functions with a return annotation must return a value on explicit return paths.

```geo
fn greet(name: str) -> str {
    return "Hello, " + name
}
```

`main()` may return `unit` or `int`.

- `fn main()` exits with code `0`.
- `fn main() -> int` exits with the returned integer.

## Local Bindings

`let` creates an immutable local. `var` creates a mutable local.

```geo
let message = greet("world")
let count: int = 3

var index = 0
index = index + 1
```

Type inference is local and conservative in this phase:

- literals infer obvious primitive types
- function calls infer their declared return type
- struct literals infer from the explicit struct constructor name
- array literals infer from homogeneous elements or from an explicit annotation

## Standard Library Naming

`string` remains accepted for compatibility, but `str` becomes the preferred spelling.

```geo
fn greet(name: str) -> str {
    return "Hello, " + name
}
```

`std.io.println` returns `unit`, not `int`. It may still trap or report runtime errors later through a separate error mechanism, but printing is statement-oriented by default.

## Statements And Expressions

Expression statements are valid for calls and other side-effecting expressions.

```geo
println(message)
```

Semicolons are accepted but not required:

```geo
println("with semicolon");
println("without semicolon")
```

## Conditionals And Structs

Existing braces remain:

```geo
fn classify(score: int) -> str {
    if score >= 90 {
        return "excellent"
    } else if score >= 60 {
        return "passing"
    } else {
        return "failing"
    }
}

struct User {
    name: str
    age: int
}
```

Struct literals stay simple:

```geo
let user = User {
    name: "Ian"
    age: 16
}
```

## Low-Level Compatibility

The clean syntax does not remove low-level features. Pointers, references, `unsafe`, `extern fn`, fixed-width integers, explicit target support, and raw runtime work remain part of Geo.

```geo
extern fn puts(message: *u8) -> int

fn main() -> int {
    unsafe {
        let p: *u8 = 0
    }
    return 0
}
```

## Compatibility

The old syntax remains valid where it does not conflict:

- `fn main() -> int`
- `let x: int = 1`
- `string`
- current imports
- current aggregate syntax

The compiler should prefer the new canonical style in examples and formatter behavior.

## Acceptance Criteria

- `import std.io;` and `import std.io` both parse.
- `fn main() { println("Hello, world!") }` checks, builds, and exits `0`.
- `println` returns `unit`.
- `fn main() -> int { println("Hello"); return 0 }` checks and builds.
- `let message = greet("world")` infers `str`.
- `var index = 0` creates an assignable mutable binding.
- assignment to `let` locals is rejected.
- `str` is accepted as an alias for `string`.
- existing v1 examples continue to check and emit assembly.
- direct native hello-world work can target the unit-returning `main` form.

## Non-Goals

- no generics syntax implementation in this syntax pass
- no lifetimes
- no expression-based implicit returns
- no overloading
- no package manager changes
- no removal of current syntax until compatibility tests exist
