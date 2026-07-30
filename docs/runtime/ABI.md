# Geo Runtime ABI

Geo executable writers own the runtime ABI. Generated code calls compiler-defined
symbols, and the ELF64 and PE64 backends emit the corresponding native helpers
directly. No C runtime, external assembler, or platform linker is required for
the supported executable subset.

## Scalar Convention

- Integer, boolean, character, pointer, string, slice, reference, and handle
  values occupy one machine-word IR slot.
- Boolean results are normalized to `0` or `1`.
- `usize` and pointers are target-word-sized values.
- Strings are NUL-terminated pointers to compiler-owned byte storage when a
  runtime function returns an owned string.
- Allocation-backed strings and arrays carry a compiler-owned header so native
  `free` and `realloc` can recover the allocation size and validate ownership.

## Function Calls

Normal scalar calls use the target ABI:

- Linux x86-64 uses System V argument registers `rdi`, `rsi`, `rdx`, `rcx`,
  `r8`, and `r9`.
- Windows x86-64 uses `rcx`, `rdx`, `r8`, and `r9`, with 32 bytes of shadow
  space reserved by callers.
- Additional arguments are passed on the stack.

Structs and fixed arrays are flattened recursively into scalar slots at the
compiler-owned ABI boundary. Aggregate returns use a hidden first parameter,
`__geo_return_ptr`, pointing at caller-owned storage. The callee writes each
scalar leaf to that buffer, and the caller reloads the leaves into its local
slots. This convention is represented explicitly by `CallAggregate` and
`ReturnAggregate` IR instructions.

## Entry And Traps

- Linux executables enter through compiler-emitted `_start`, preserve the
  initial process stack for argument access, call `main`, and exit through the
  `exit` syscall.
- Windows executables enter through compiler-emitted PE64 startup code and
  terminate through `ExitProcess`.
- Bounds failures call the compiler-owned `__geo_bounds_check` trap helper.

## Compatibility

Runtime symbol names are source-level implementation details until a stable
ABI version is published. New helpers must be added to the semantic runtime
metadata and implemented in both native writers before they are exposed through
`library/std`.
