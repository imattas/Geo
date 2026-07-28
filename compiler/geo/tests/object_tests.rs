use geo::lexer::lex;
use geo::lower::lower;
use geo::object::emit_elf64_relocatable;
use geo::parser::parse;
use geo::typecheck::check;

fn object_for(source: &str) -> Vec<u8> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    emit_elf64_relocatable(&ir)
}

#[test]
fn emits_elf64_relocatable_with_sections_and_symbols() {
    let object = object_for("fn main() -> int { return 42 }");

    assert_eq!(&object[0..4], b"\x7fELF");
    assert_eq!(read_u16(&object, 16), 1);
    assert!(contains_bytes(&object, b".text"));
    assert!(contains_bytes(&object, b".symtab"));
    assert!(contains_bytes(&object, b".strtab"));
    assert!(contains_bytes(&object, b".shstrtab"));
    assert!(contains_bytes(&object, b"main"));
}

#[test]
fn emits_relocation_for_runtime_call() {
    let object = object_for(
        r#"
            import std.io

            fn main() -> int {
                println("Geo")
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&object, b".rela.text"));
    assert!(contains_bytes(&object, b"println"));
    assert!(rela_text_payload_size(&object) > 0);
}

#[test]
fn emits_rodata_for_string_literals() {
    let object = object_for(
        r#"
            import std.io

            fn main() -> int {
                println("Geo")
                return 0
            }
        "#,
    );

    assert!(contains_bytes(&object, b".rodata"));
    assert!(contains_bytes(&object, b"Geo\0"));
    assert!(contains_bytes(&object, b"__geo_str_main_0"));
}

#[test]
fn emits_text_relocation_for_string_literal_address() {
    let object = object_for(
        r#"
            import std.io

            fn main() -> int {
                println("Geo")
                return 0
            }
        "#,
    );

    assert!(rela_text_payload_size(&object) >= 48);
    assert!(contains_bytes(
        section_payload(&object, 1),
        &[0x48, 0x8d, 0x05]
    ));
}

#[test]
fn emits_call_argument_register_for_string_literal_runtime_call() {
    let object = object_for(
        r#"
            import std.io

            fn main() -> int {
                println("Geo")
                return 0
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x8b, 0x7d]));
}

#[test]
fn emits_machine_code_for_function_parameter_register_spills() {
    let object = object_for(
        r#"
            fn add(a: int, b: int) -> int {
                return a + b
            }

            fn main() -> int {
                return add(40, 2)
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x89, 0x7d]));
    assert!(contains_bytes(text, &[0x48, 0x89, 0x75]));
    assert!(contains_bytes(text, &[0xe8]));
}

#[test]
fn emits_machine_code_for_stack_passed_function_arguments() {
    let object = object_for(
        r#"
            fn seventh(a: int, b: int, c: int, d: int, e: int, f: int, g: int) -> int {
                return g
            }

            fn main() -> int {
                return seventh(1, 2, 3, 4, 5, 6, 7)
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x50]));
    assert!(contains_bytes(text, &[0x48, 0x83, 0xc4, 0x10]));
    assert!(contains_bytes(text, &[0x48, 0x8b, 0x45, 0x10]));
}

#[test]
fn emits_relocation_for_bounds_check_runtime_call() {
    let object = object_for(
        r#"
            fn main() -> int {
                let values: [int] = [42]
                return values[0]
            }
        "#,
    );

    assert!(contains_bytes(&object, b"__geo_bounds_check"));
    assert!(rela_text_payload_size(&object) > 0);
}

#[test]
fn emits_machine_code_for_integer_addition() {
    let object = object_for("fn main() -> int { return 40 + 2 }");
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x03, 0x45]));
}

#[test]
fn emits_machine_code_for_local_loads_and_stores() {
    let object = object_for(
        r#"
            fn main() -> int {
                let base: int = 40
                return base + 2
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(count_bytes(text, &[0x48, 0x8b, 0x45]) >= 2);
    assert!(count_bytes(text, &[0x48, 0x89, 0x45]) >= 2);
}

#[test]
fn emits_machine_code_for_if_else_branches() {
    let object = object_for(
        r#"
            fn main() -> int {
                if 10 < 32 {
                    return 42
                } else {
                    return 1
                }
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x3b, 0x45]));
    assert!(contains_bytes(text, &[0x0f, 0x9c, 0xc0]));
    assert!(contains_bytes(text, &[0x0f, 0x84]));
    assert!(contains_bytes(text, &[0xe9]));
}

#[test]
fn emits_machine_code_for_while_loop_backedge() {
    let object = object_for(
        r#"
            fn main() -> int {
                var x: int = 0
                while x < 3 {
                    x = x + 1
                }
                return x
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x0f, 0x84]));
    assert!(contains_backward_jump(text));
}

#[test]
fn emits_machine_code_for_division_and_remainder() {
    let object = object_for("fn main() -> int { return 25 / 4 + 25 % 4 }");
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x99]));
    assert!(contains_bytes(text, &[0x48, 0xf7]));
    assert!(contains_bytes(text, &[0x48, 0x89, 0x55]));
}

#[test]
fn emits_machine_code_for_shift_operations() {
    let object = object_for("fn main() -> int { return 1 << 3 >> 1 }");
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x8b, 0x4d]));
    assert!(contains_bytes(text, &[0x48, 0xd3, 0xe0]));
    assert!(contains_bytes(text, &[0x48, 0xd3, 0xf8]));
}

#[test]
fn emits_machine_code_for_logical_and_bit_not_operations() {
    let object = object_for(
        r#"
            fn main() -> int {
                if true && false || true {
                    return ~0
                }
                return 0
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x23, 0x45]));
    assert!(contains_bytes(text, &[0x48, 0x0b, 0x45]));
    assert!(contains_bytes(text, &[0x48, 0xf7, 0xd0]));
}

#[test]
fn emits_machine_code_for_address_of_and_deref() {
    let object = object_for(
        r#"
            fn main() -> int {
                let x: int = 42
                unsafe {
                    let p: *int = &x
                    return *p
                }
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x8d, 0x45]));
    assert!(contains_bytes(text, &[0x48, 0x8b, 0x00]));
}

#[test]
fn emits_machine_code_for_pointer_store() {
    let object = object_for(
        r#"
            fn main() -> int {
                var x: int = 1
                unsafe {
                    let p: *int = &x
                    *p = 42
                }
                return x
            }
        "#,
    );
    let text = section_payload(&object, 1);

    assert!(contains_bytes(text, &[0x48, 0x8d, 0x45]));
    assert!(contains_bytes(text, &[0x4c, 0x89, 0x10]));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn count_bytes(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn contains_backward_jump(text: &[u8]) -> bool {
    text.windows(5)
        .any(|window| window[0] == 0xe9 && i32::from_le_bytes(window[1..5].try_into().unwrap()) < 0)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn section_payload(object: &[u8], section_index: usize) -> &[u8] {
    let section_header_offset = read_u64(object, 40) as usize;
    let section_header_size = read_u16(object, 58) as usize;
    let header = section_header_offset + section_index * section_header_size;
    let offset = read_u64(object, header + 24) as usize;
    let size = read_u64(object, header + 32) as usize;

    &object[offset..offset + size]
}

fn rela_text_payload_size(object: &[u8]) -> u64 {
    let section_header_offset = read_u64(object, 40) as usize;
    let section_header_size = read_u16(object, 58) as usize;
    let section_count = read_u16(object, 60) as usize;

    for index in 0..section_count {
        let header = section_header_offset + index * section_header_size;
        let section_type = u32::from_le_bytes(object[header + 4..header + 8].try_into().unwrap());
        if section_type == 4 {
            return read_u64(object, header + 32);
        }
    }
    0
}
