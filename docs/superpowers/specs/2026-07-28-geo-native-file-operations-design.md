# Geo Native File Operations Design

## Goal

Extend the compiler-owned native runtime subset with append, touch, and remove file operations on Linux x86-64 and Windows x86-64.

## Architecture

The front end already exposes these functions through `std.io` and lowers calls to stable runtime symbols. The ELF64 executable writer will provide syscall-backed helpers for `append_file`, `touch_file`, and `remove_file`. The PE64 executable writer will provide Win64 helpers using `CreateFileA`, `WriteFile`, `CloseHandle`, and `DeleteFileA`. No C runtime symbol or compiler-local resolver is introduced by this feature.

Each helper follows the existing runtime ABI for its target and returns `0` on success and `1` on failure. Append writes the complete NUL-terminated Geo string at the end of the file, touch creates or opens a file without changing its contents, and remove deletes the named file.

## Verification

- Backend unit tests prove the new runtime symbols are recognized and the expected syscall/import structures are emitted.
- Linux and Windows Geo examples build through the direct ELF64 and PE64 writers.
- Native smoke tests create, append to, and remove a file, then verify the resulting bytes and exit status on both supported hosts where execution is available.

## Scope boundary

Handle-based file APIs, truncation, metadata, and elimination of the remaining NASM/C-runtime fallback are separate milestones. This change makes the direct compiler-owned runtime larger without claiming those later milestones are complete.
