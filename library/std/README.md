# Geo Standard Library

This directory is the source-level standard library package for Geo code.

The compiler-managed native runtime in `library/geo_runtime` owns platform ABI
glue for generated programs. Modules in `library/std/src` define the public Geo
APIs that user code should import.

The first standard library modules are intentionally small:

- `std.io`
- `std.mem`
- `std.process`
- `std.string`
- `std.array`
- `std.fs`
- `std.platform`
