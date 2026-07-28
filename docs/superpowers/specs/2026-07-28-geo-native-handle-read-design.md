# Geo Native Handle Read Design

## Goal

Implement the declared `std.io.file_read_to_string` operation directly in the ELF64 and PE64 executable writers.

## Architecture

The helper receives an already-open native descriptor or handle and returns a NUL-terminated allocation-backed Geo string. Linux uses `lseek` to determine the current file size, `mmap` for storage, rewinds with `lseek`, and reads with `read`. Windows uses `GetFileSize`, `VirtualAlloc`, and `ReadFile`. The helper does not close the caller-owned handle and returns a null pointer on failure.

## Verification

Backend tests assert the target operations. A Geo executable writes a file, closes it, reopens it, reads it through the handle API, closes the read handle, and returns the string length. The executable is built and run on Linux and Windows.

## Scope boundary

Seeking, partial-read loops, stream position preservation, and explicit deallocation remain future runtime work.
