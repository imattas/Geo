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
library/
  geo_runtime/      Runtime crate and C runtime shim
examples/           Geo source examples
docs/               Design specs and implementation plans
```

## Quick Checks

```powershell
cargo fmt --check
cargo test --workspace --quiet
cargo run --quiet -- check examples\return_42.geo --target x86_64-linux
cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\return_42_windows.asm
```

## Project Docs

- `STATUS.md`: current implementation status.
- `ROADMAP.md`: v1 development roadmap.
- `IMPROVEMENTS.md`: concrete engineering improvements to consider.
- `CONTRIBUTING.md`: local verification and contribution notes.

