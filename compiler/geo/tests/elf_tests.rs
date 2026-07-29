use geo::elf::emit_elf64_executable;
use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::typecheck::check;

fn executable_for(source: &str) -> Vec<u8> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    emit_elf64_executable(&lower(&program)).expect("program should fit direct ELF64 subset")
}

#[test]
fn emits_direct_elf64_executable_with_exit_entry() {
    let executable = executable_for("fn main() -> int { return 42 }");

    assert_eq!(&executable[0..4], b"\x7fELF");
    assert_eq!(executable[4], 2);
    assert_eq!(read_u64(&executable, 24), 0x401000);
    assert_eq!(read_u16(&executable, 18), 0x3e);
    assert_eq!(read_u16(&executable, 56), 1);
    assert!(contains_bytes(
        &executable,
        &[0x89, 0xc7, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]
    ));
}

#[test]
fn emits_direct_elf64_string_length_runtime_helper() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len("Geo") as int
            }
        "#,
    );

    assert!(contains_bytes(
        &executable,
        &[0x48, 0x89, 0xfe, 0x31, 0xc0, 0x0f, 0xb6, 0x0e, 0x85, 0xc9]
    ));
    assert!(contains_bytes(
        &executable,
        &[0x74, 0x08, 0x48, 0xff, 0xc6, 0x48, 0xff, 0xc0, 0xeb, 0xf1]
    ));
}

#[test]
fn emits_direct_elf64_console_runtime_syscall() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() {
                println("Geo")
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 1, 0, 0, 0]));
    assert!(contains_bytes(&executable, &[0xbf, 1, 0, 0, 0]));
    assert!(contains_bytes(&executable, &[0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xba, 1, 0, 0, 0]));
    assert!(contains_bytes(
        &executable,
        &[0x74, 0x07, 0x49, 0xff, 0xc0, 0xff, 0xc2, 0xeb, 0xf1]
    ));
    assert!(!contains_bytes(&executable, &[0x48, 0x89, 0xf2]));
    assert!(contains_bytes(
        &executable,
        &[0x48, 0x83, 0xec, 0x08, 0xc6, 0x04, 0x24, b'\n', 0xb8, 1, 0, 0, 0]
    ));
}

#[test]
fn emits_direct_elf64_string_concat_mmap_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() {
                let message: string = "Geo" + " compiler"
                println(message)
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0x48, 0x83, 0xec, 0x38]));
}

#[test]
fn emits_direct_elf64_process_exit_runtime() {
    let executable = executable_for(
        r#"
            import std.process

            fn main() -> int {
                return exit(42)
            }
        "#,
    );

    assert!(contains_bytes(
        &executable,
        &[0xb8, 60, 0, 0, 0, 0x0f, 0x05]
    ));
}

#[test]
fn emits_direct_elf64_memory_alloc_runtime() {
    let executable = executable_for(
        r#"
            import std.mem

            fn main() -> int {
                let memory: *u8 = alloc(1)
                if memory != null {
                    return 42
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_file_write_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                return write_file("/tmp/geo-elf-write-test", "Geo")
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 2, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 1, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 3, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_append_file_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                return append_file("/tmp/geo-elf-append-test", "Geo")
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 0x01, 0x01, 0, 0]));
    assert!(contains_bytes(&executable, &[0xb8, 1, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 3, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_touch_and_remove_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                let touched = touch_file("/tmp/geo-elf-touch-test")
                if touched != 0 {
                    return touched
                }
                return remove_file("/tmp/geo-elf-touch-test")
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 0x01, 0x01, 0, 0]));
    assert!(contains_bytes(
        &executable,
        &[0xb8, 87, 0, 0, 0, 0x0f, 0x05]
    ));
}

#[test]
fn emits_direct_elf64_handle_file_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                let handle = file_open_write("/tmp/geo-elf-handle-test")
                if handle < 0 {
                    return 1
                }
                let write_status = file_write(handle, "Geo")
                let close_status = file_close(handle)
                if write_status != 0 {
                    return write_status
                }
                return close_status
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 0x01, 0x01, 0, 0]));
    assert!(contains_bytes(&executable, &[0xb8, 1, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 3, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_handle_file_read_runtime() {
    let executable = executable_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                let handle = file_open("/tmp/geo-elf-handle-read-test")
                if handle < 0 {
                    return 1
                }
                let contents = file_read_to_string(handle)
                file_close(handle)
                return string_len(contents) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 8, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_file_exists_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                if file_exists("/tmp/geo-elf-file-exists") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(
        &executable,
        &[0xb8, 21, 0, 0, 0, 0x0f, 0x05]
    ));
}

#[test]
fn emits_direct_elf64_file_metadata_runtime() {
    let executable = executable_for(
        r#"
            import std.io

            fn main() -> int {
                if file_is_file("/tmp/geo-elf-file-metadata") {
                    return file_size("/tmp/geo-elf-file-metadata") as int
                }
                if file_is_dir("/tmp") && file_is_empty("/tmp/geo-elf-file-metadata") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 6, 1, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_string_runtime_helpers() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_byte_at("Geo", 1) != 101 {
                    return 1
                }
                if string_is_empty("Geo") || !string_is_empty("") {
                    return 2
                }
                if !string_is_ascii("Geo") || string_is_ascii("G\u{00e9}o") {
                    return 3
                }
                return string_find_byte("Geo", 111)
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x0f, 0xb6, 0x04, 0x37]));
}

#[test]
fn emits_direct_elf64_string_comparison_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_compare("Geo", "Geo") != 0 {
                    return 1
                }
                if !string_less("Geo", "Rust") {
                    return 2
                }
                if !string_greater_or_equal("Rust", "Rust") {
                    return 3
                }
                if !string_not_eq("Geo", "Rust") {
                    return 4
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x0f, 0xb6, 0x04, 0x0f]));
}

#[test]
fn emits_direct_elf64_string_matching_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if !string_contains("Geo compiler", "compiler") {
                    return 1
                }
                if string_contains("Geo", "Rust") {
                    return 2
                }
                if !string_starts_with("Geo compiler", "Geo") {
                    return 3
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x4c, 0x8d, 0x0c, 0x0f]));
}

#[test]
fn emits_direct_elf64_string_suffix_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if !string_ends_with("Geo compiler", "compiler") {
                    return 1
                }
                if string_ends_with("Geo", "Rust") {
                    return 2
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x4c, 0x39, 0xc1]));
}

#[test]
fn emits_direct_elf64_string_index_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_index_of("Geo compiler", "compiler") != 4 {
                    return 1
                }
                if string_index_of("Geo", "Rust") != -1 {
                    return 2
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x48, 0x89, 0xc8, 0xc3]));
}

#[test]
fn emits_direct_elf64_string_count_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_count("aaaa", "aa") != 2usize {
                    return 1
                }
                if string_count("Geo compiler", "o") != 2usize {
                    return 2
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x4c, 0x89, 0xd0, 0xc3]));
}

#[test]
fn emits_direct_elf64_string_last_index_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_last_index_of("compiler.geo.compiler.geo", ".geo") != 21 {
                    return 1
                }
                if string_last_index_of("aaaa", "aa") != 2 {
                    return 2
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x4c, 0x89, 0xd0, 0xc3]));
}

#[test]
fn emits_direct_elf64_string_last_find_byte_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_last_find_byte("banana", 97) != 5 {
                    return 1
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x49, 0x89, 0xca]));
}

#[test]
fn emits_direct_elf64_string_slice_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                let value: string = string_slice("compiler.geo", 0usize, 8usize)
                return string_len(value) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xf3, 0xa4]));
}

#[test]
fn emits_direct_elf64_string_utf8_length_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_utf8_len("λ😀") != 2 {
                    return 1
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(
        &executable,
        &[0x80, 0xe2, 0xc0, 0x80, 0xfa, 0x80]
    ));
}

#[test]
fn emits_direct_elf64_string_utf8_codepoint_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                if string_utf8_codepoint_at("λ😀", 1usize) != 128512 {
                    return 1
                }
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0x41, 0x0f, 0xb6, 0xc2]));
}

#[test]
fn emits_direct_elf64_file_read_runtime() {
    let executable = executable_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_file("/tmp/geo-read-file-test")) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 2, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0x31, 0xc0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xb8, 8, 0, 0, 0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_file_read_with_default_runtime() {
    let executable = executable_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_file_or("/tmp/geo-missing-file", "fallback")) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 2, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xe8]));
    assert!(contains_bytes(&executable, b"fallback\0"));
}

#[test]
fn emits_direct_elf64_read_line_runtime() {
    let executable = executable_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_line()) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0x31, 0xc0, 0x0f, 0x05]));
}

#[test]
fn emits_direct_elf64_memory_copy_and_zero_runtime() {
    let executable = executable_for(
        r#"
            import std.mem

            fn main() -> int {
                let memory: *u8 = alloc(8)
                mem_zero(memory, 8)
                return mem_copy(memory, memory, 8)
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xf3, 0xaa]));
}

#[test]
fn emits_direct_elf64_memory_move_runtime() {
    let executable = executable_for(
        r#"
            import std.mem

            fn main() -> int {
                let memory: *u8 = alloc(8)
                return mem_move(memory, memory, 8)
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xf3, 0xa4]));
}

#[test]
fn emits_direct_elf64_string_from_byte_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len(string_from_byte(65)) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xc6, 0x40, 0x01, 0x00]));
}

#[test]
fn emits_direct_elf64_string_clone_runtime() {
    let executable = executable_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len(string_clone("Geo")) as int
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xf3, 0xa4]));
}

#[test]
fn emits_direct_elf64_alloc_copy_runtime() {
    let executable = executable_for(
        r#"
            import std.mem

            fn main() -> int {
                let source: *u8 = alloc(8)
                let copy: *u8 = alloc_copy(source, 8)
                if copy != null {
                    return 42
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(&executable, &[0xb8, 9, 0, 0, 0, 0x0f, 0x05]));
    assert!(contains_bytes(&executable, &[0xf3, 0xa4]));
}

#[test]
fn emits_direct_elf64_mem_fill_runtime() {
    let executable = executable_for(
        r#"
            import std.mem
            fn main() -> int {
                let buffer: *u8 = alloc(8)
                return mem_fill(buffer, 8, 65)
            }
        "#,
    );
    assert!(contains_bytes(&executable, &[0xf3, 0xaa]));
}

#[test]
fn emits_direct_elf64_mem_find_runtime() {
    let executable = executable_for(
        r#"
            import std.mem
            fn main() -> int {
                let buffer: *u8 = alloc(8)
                return mem_find(buffer, 8, 65)
            }
        "#,
    );
    assert!(contains_bytes(&executable, &[0x38, 0x14, 0x07]));
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
