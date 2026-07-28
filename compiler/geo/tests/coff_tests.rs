use geo::lexer::lex;
use geo::lower::lower;
use geo::object::emit_coff_x64_relocatable;
use geo::parser::parse;
use geo::typecheck::check;

fn coff_for(source: &str) -> Vec<u8> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    emit_coff_x64_relocatable(&ir).expect("program should fit direct COFF subset")
}

#[test]
fn emits_win64_coff_relocatable_with_text_and_main_symbol() {
    let object = coff_for("fn main() -> int { return 42 }");

    assert_eq!(read_u16(&object, 0), 0x8664);
    assert_eq!(read_u16(&object, 2), 1);
    assert!(contains_bytes(&object, b".text"));
    assert!(contains_bytes(&object, b"main"));
    assert!(contains_bytes(
        &object,
        &[0x48, 0xc7, 0x45, 0xf8, 42, 0, 0, 0]
    ));
    assert!(contains_bytes(&object, &[0xc9, 0xc3]));
}

#[test]
fn emits_win64_coff_machine_code_for_stack_arithmetic() {
    let object = coff_for("fn main() -> int { return 40 + 2 }");

    assert_eq!(read_u16(&object, 0), 0x8664);
    assert!(contains_bytes(&object, b".text"));
    assert!(contains_bytes(&object, b"main"));
    assert!(contains_bytes(&object, &[0x48, 0x03, 0x45]));
    assert!(contains_bytes(&object, &[0xc9, 0xc3]));
}

#[test]
fn emits_win64_coff_string_data_and_text_relocation() {
    let object = coff_for(
        r#"
            fn main() -> int {
                let name: string = "Geo"
                return 0
            }
        "#,
    );

    assert_eq!(read_u16(&object, 2), 2);
    assert!(contains_bytes(&object, b".rdata"));
    assert!(contains_bytes(&object, b"Geo\0"));
    assert!(contains_bytes(&object, b"__geo_str_main_0"));
    assert!(text_relocation_count(&object) > 0);
}

#[test]
fn emits_win64_coff_internal_function_call_relocation() {
    let object = coff_for(
        r#"
            fn add(left: int, right: int) -> int {
                return left + right
            }

            fn main() -> int {
                return add(40, 2)
            }
        "#,
    );

    assert_eq!(read_u32(&object, 12), 2);
    assert!(contains_bytes(&object, b"add"));
    assert!(contains_bytes(&object, b"main"));
    assert!(contains_bytes(&object, &[0xe8, 0, 0, 0, 0]));
    assert!(text_relocation_count(&object) > 0);
}

#[test]
fn emits_win64_coff_calls_with_windows_argument_registers() {
    let object = coff_for(
        r#"
            fn add(left: int, right: int) -> int {
                return left + right
            }

            fn main() -> int {
                return add(40, 2)
            }
        "#,
    );

    assert!(contains_opcode_with_disp8(&object, &[0x48, 0x8b, 0x4d]));
    assert!(contains_opcode_with_disp8(&object, &[0x48, 0x8b, 0x55]));
    assert!(contains_bytes(
        &object,
        &[0x48, 0x83, 0xec, 0x20, 0xe8, 0, 0, 0, 0]
    ));
}

#[test]
fn emits_win64_coff_bounds_check_call_with_windows_shadow_space() {
    let object = coff_for(
        r#"
            fn main() -> int {
                let values: [int] = [42]
                return values[0]
            }
        "#,
    );

    assert!(contains_bytes(&object, b"__geo_bounds_check"));
    assert!(contains_opcode_with_disp8(&object, &[0x48, 0x8b, 0x4d]));
    assert!(contains_bytes(&object, &[0x48, 0xc7, 0xc2, 1, 0, 0, 0]));
    assert!(contains_bytes(
        &object,
        &[0x48, 0x83, 0xec, 0x20, 0xe8, 0, 0, 0, 0]
    ));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn contains_opcode_with_disp8(haystack: &[u8], opcode: &[u8]) -> bool {
    haystack
        .windows(opcode.len() + 1)
        .any(|window| window.starts_with(opcode))
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn text_relocation_count(object: &[u8]) -> u16 {
    read_u16(object, 20 + 32)
}
