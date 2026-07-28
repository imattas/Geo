# Geo Native Handle File IO Design

## Goal

Make the declared `std.io` handle APIs direct compiler-owned runtime operations on Linux x86-64 and Windows x86-64.

## Architecture

The front end already lowers `file_open`, `file_open_write`, `file_open_append`, `file_write`, and `file_close` to stable symbols. The ELF64 writer emits `openat`, `write`, and `close` syscall helpers. The PE64 writer emits Win64 helpers around `CreateFileA`, `WriteFile`, and `CloseHandle`, reusing the existing import-table ABI. Open helpers return the native descriptor/handle or `-1`; write returns `0` only when the complete string is written and `1` otherwise; close returns `0` for compatibility with the standard runtime contract.

## Verification

Backend tests assert the runtime symbols and target operations. A Geo executable opens a file for writing, writes bytes, closes it, and is executed on Linux and Windows. A second append-mode example verifies that the native handle remains usable across the full open/write/close sequence.

## Scope boundary

Handle-based reading, seeking, truncation, metadata, and removal remain separate runtime slices. Unsupported direct programs continue using the existing assembly fallback until the direct backend covers their IR.
