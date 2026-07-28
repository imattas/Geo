# Geo

Geo is a native systems programming language project with a from-scratch compiler pipeline, a clean Rust/C++/C#/Java-influenced syntax direction, and first-class Linux x86-64 and Windows x86-64 goals.

Current canonical syntax direction:

```geo
import std.io

fn main() {
    println("Hello, world!")
}
```

## Repository Layout

```text
compiler/
  geo/              Rust compiler crate and CLI
  geo_diagnostics/  Diagnostic types and rendering
  geo_source/       Source file loading and module path mapping
library/
  geo_runtime/      Compiler-managed native runtime implementation
  std/              Source-level Geo standard library modules
src/
  bootstrap/        Bootstrap stage model
  tools/xtask/      Workspace automation tool
examples/           Geo source examples
docs/               Design specs and implementation plans
```

## Quick Checks

```powershell
cargo fmt --check
cargo test --workspace --quiet
cargo run -p xtask --quiet -- from-scratch
cargo run -p xtask --quiet -- layout
cargo run --quiet -- check examples\return_42.geo --target x86_64-linux
cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\return_42_windows.asm
```

## Workspace Tool

```powershell
cargo run -p xtask --quiet -- status
cargo run -p xtask --quiet -- layout
cargo run -p xtask --quiet -- from-scratch
cargo run -p xtask --quiet -- verify
```

## Project Docs

- `STATUS.md`: current implementation status.
- `ROADMAP.md`: v1 development roadmap.
- `IMPROVEMENTS.md`: concrete engineering improvements to consider.
- `CONTRIBUTING.md`: local verification and contribution notes.
