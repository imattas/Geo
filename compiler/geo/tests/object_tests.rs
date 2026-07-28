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
