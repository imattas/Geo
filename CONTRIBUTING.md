# Contributing To Geo

Geo is organized as a Rust workspace with the compiler in `compiler/geo` and runtime support in `library/geo_runtime`.

## Local Verification

Run these before submitting changes:

```powershell
cargo fmt --check
cargo test --workspace --locked
cargo run --quiet -- check examples\return_42.geo --target x86_64-linux
cargo run --quiet -- emit-asm examples\return_42.geo --target x86_64-windows -o target\ci-return-42-windows.asm
```

Use `cargo test --workspace --quiet` during local iteration when lockfile checking is not relevant.

## Development Guidelines

- Keep compiler phase boundaries clear: parse, resolve, type check, borrow check, lower, emit.
- Add focused tests near the compiler boundary being changed.
- Prefer source-level Geo examples for user-visible language behavior.
- Keep generated binaries, assembly, objects, and scratch files under `target/`.
- Update `STATUS.md` when the implemented baseline changes.
- Update `ROADMAP.md` when priorities or phase definitions change.

## Repository Areas

- `compiler/geo`: current compiler crate and CLI.
- `library/geo_runtime`: runtime path helper and C runtime shim.
- `examples`: Geo source examples and acceptance-style inputs.
- `docs/superpowers`: design specs and implementation plans.
- `.github`: CI and contribution templates.

