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
    assert!(contains_bytes(&object, &[0xb8, 42, 0, 0, 0, 0xc3]));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}
