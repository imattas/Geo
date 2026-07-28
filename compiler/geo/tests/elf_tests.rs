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
