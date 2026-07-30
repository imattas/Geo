use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::pe::emit_pe64_console;
use geo::typecheck::check;

fn pe_for(source: &str) -> Vec<u8> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    emit_pe64_console(&ir).expect("program should fit direct PE console subset")
}

#[test]
fn emits_direct_pe64_console_hello_world() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                println("Hello, world!")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"GetStdHandle"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(contains_bytes(&pe, b"Hello, world!\0"));
    assert!(contains_bytes(&pe, &[b'\n', 0]));
}

#[test]
fn emits_direct_pe64_println_as_compiled_helper_call() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                println("Hello, compiled PE!")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x55, 0x48, 0x89, 0xe5]));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x20, 0xe8]));
    assert!(contains_bytes(&pe, &[0x4c, 0x8b, 0x54, 0x24, 0x28]));
    assert!(contains_bytes(&pe, &[0x43, 0x80, 0x3c, 0x02, 0x00]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0xc0]));
    assert!(contains_bytes(&pe, &[0xeb, 0xf4]));
    assert!(contains_bytes(&pe, b"GetStdHandle"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(contains_bytes(&pe, b"Hello, compiled PE!\0"));
}

#[test]
fn emits_direct_pe64_constant_exit_program() {
    let pe = pe_for("fn main() -> int { return 42 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_internal_function_call_as_machine_code() {
    let pe = pe_for(
        r#"
            fn value() -> int {
                return 42
            }

            fn main() -> int {
                return value()
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x55, 0x48, 0x89, 0xe5]));
    assert!(contains_bytes(&pe, &[0xe8]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_bounds_check_as_compiled_runtime_helper() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let values: [int] = [42]
                return values[0]
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x55, 0x48, 0x89, 0xe5]));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x20, 0xe8]));
    assert!(contains_bytes(&pe, &[0x48, 0x39, 0xd1]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_prefixed_and_underscored_integer_literals() {
    let pe = pe_for("fn main() -> int { return 0xff + 0b1010 + 1_000 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_function_tail_expression_exit_program() {
    let pe = pe_for("fn main() -> int { 42 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_top_level_const_exit_program() {
    let pe = pe_for("const LIMIT: int = 42 fn main() -> int { return LIMIT }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_folded_top_level_const_arithmetic_dependencies() {
    let pe = pe_for(
        r#"
            const BASE: int = 40
            const LIMIT: int = BASE + 2

            fn main() -> int {
                return LIMIT
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_enum_variant_exit_program() {
    let pe = pe_for(
        "enum TokenKind { Eof Ident Number } fn main() -> TokenKind { return TokenKind.Number }",
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_explicit_enum_discriminant_exit_program() {
    let pe = pe_for(
        "enum Status { Ok = 0 Warning = 7 Error = 42 } fn main() -> Status { Status.Error }",
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_implicit_enum_discriminant_after_explicit_exit_program() {
    let pe = pe_for("enum Status { Ok = 5 Warning Error } fn main() -> Status { Status.Error }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_match_expression_exit_program() {
    let pe = pe_for("enum TokenKind { Eof Number } fn main() -> int { let kind: TokenKind = TokenKind.Number return match kind { TokenKind.Eof => 0 TokenKind.Number => 2 _ => 9 } }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_exhaustive_match_without_wildcard_exit_program() {
    let pe = pe_for("enum TokenKind { Eof Number } fn main() -> int { let kind: TokenKind = TokenKind.Number return match kind { TokenKind.Eof => 0 TokenKind.Number => 2 } }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_if_expression_exit_program() {
    let pe = pe_for("fn main() -> int { return if true { 7 } else { 9 } }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_block_expression_exit_program() {
    let pe = pe_for("fn main() -> int { return { let base: int = 40 base + 2 } }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_arithmetic_exit_program() {
    let pe = pe_for("fn main() -> int { return 6 * 7 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_remainder_exit_program() {
    let pe = pe_for("fn main() -> int { return 10 % 4 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_compound_assignment_exit_program() {
    let pe = pe_for("fn main() -> int { var x: int = 10 x += 5 x %= 4 return x }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_boolean_logic_exit_program() {
    let pe = pe_for("fn main() -> int { if true || false && false { return 7 } return 1 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_bitwise_exit_program() {
    let pe = pe_for("fn main() -> int { return 10 | 6 ^ 3 & 1 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_shift_exit_program() {
    let pe = pe_for("fn main() -> int { return 1 << 3 >> 1 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_bitwise_not_exit_program() {
    let pe = pe_for("fn main() -> int { return ~10 & 255 }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_integer_cast_exit_program() {
    let pe = pe_for("fn main() -> int { let x: i32 = 42 return x as int }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_pointer_to_usize_cast_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let ptr: *u8 = null
                let addr: usize = ptr as usize
                let zero: usize = 0
                if (addr == zero) {
                    return 42
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_usize_to_pointer_cast_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let addr: usize = 0
                unsafe {
                    let ptr: *u8 = addr as *u8
                    if ptr == null {
                        return 42
                    }
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_raw_pointer_add_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> usize {
                unsafe {
                    let ptr: *u32 = null
                    let next: *u32 = ptr + 2
                    return next as usize
                }
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_raw_pointer_difference_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                unsafe {
                    let first: *u32 = null
                    let last: *u32 = first + 3
                    return last - first
                }
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_raw_pointer_compound_assignment_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> usize {
                unsafe {
                    var ptr: *u32 = null
                    ptr += 3
                    ptr -= 1
                    return ptr as usize
                }
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_raw_pointer_ordering_comparison_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                unsafe {
                    let first: *u32 = null
                    let last: *u32 = first + 3
                    if first < last {
                        return 42
                    }
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_sizeof_exit_program() {
    let pe = pe_for("fn main() -> usize { return sizeof(*u8) }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_alignof_exit_program() {
    let pe = pe_for("fn main() -> usize { return alignof(*u8) }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_offsetof_exit_program() {
    let pe = pe_for(
        r#"
            struct Header {
                tag: u8
                next: *u8
            }

            fn main() -> usize {
                return offsetof(Header, next)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_comma_separated_struct_declaration_fields() {
    let pe = pe_for(
        r#"
            struct Token {
                kind: int,
                start: usize,
            }

            fn main() -> int {
                let token: Token = Token { kind: 42, start: 0, }
                return token.kind
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_struct_literal_field_shorthand() {
    let pe = pe_for(
        r#"
            struct Token {
                kind: int,
                start: usize,
            }

            fn main() -> int {
                let kind: int = 42
                let start: usize = 0
                let token: Token = Token { kind, start, }
                return token.kind
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_null_literal_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let p: *u8 = null
                if p == null {
                    return 42
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_null_comparison_with_pointer_on_right() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let p: *u8 = null
                if (null == p) {
                    return 42
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_local_arithmetic_exit_program() {
    let pe = pe_for("fn main() -> int { let x: int = 10 let y: int = 32 return x + y }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_mutable_reference_assignment_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                var x: int = 1
                let slot: &mut int = &mut x
                *slot = 42
                return x
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_mutable_reference_compound_assignment_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &mut int = &mut value
                *slot += 41
                return value
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_if_else_exit_program() {
    let pe = pe_for("fn main() -> int { if 10 < 32 { return 42 } else { return 1 } }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_multi_level_else_if_exit_program() {
    let pe = pe_for(
        r#"
            fn main() -> int {
                let score: int = 75
                if score >= 90 {
                    return 3
                } else if score >= 70 {
                    return 42
                } else if score >= 50 {
                    return 1
                } else {
                    return 0
                }
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_while_exit_program() {
    let pe = pe_for("fn main() -> int { var x: int = 0 while x < 42 { x = x + 1 } return x }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_for_range_exit_program() {
    let pe =
        pe_for("fn main() -> int { var total: int = 0 for i in 0..7 { total += i } return total }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_inclusive_for_range_exit_program() {
    let pe = pe_for(
        "fn main() -> int { var total: int = 0 for i in 0..=4 { total += i } return total }",
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_loop_exit_program() {
    let pe =
        pe_for("fn main() -> int { var x: int = 0 loop { x += 1 if x == 4 { break } } return x }");

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_user_function_call_exit_program() {
    let pe = pe_for(
        r#"
            fn add(a: int, b: int) -> int {
                return a + b
            }

            fn main() -> int {
                let x: int = 10
                let y: int = 32
                return add(x, y)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_user_function_call_inside_control_flow() {
    let pe = pe_for(
        r#"
            fn step(x: int) -> int {
                return x + 1
            }

            fn main() -> int {
                var x: int = 0
                while x < 42 {
                    x = step(x)
                }
                return x
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_trailing_commas_in_params_and_calls() {
    let pe = pe_for(
        r#"
            fn add(a: int, b: int,) -> int {
                return a + b
            }

            fn main() -> int {
                return add(40, 2,)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"KERNEL32.dll"));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_ordered_print_output() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                print("Geo")
                print(" ")
                println("compiler")
                println("v1")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"GetStdHandle"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"Geo\0"));
    assert!(contains_bytes(&pe, b" \0"));
    assert!(contains_bytes(&pe, b"compiler\0"));
    assert!(contains_bytes(&pe, b"v1\0"));
    assert!(contains_bytes(&pe, &[b'\n', 0]));
}

#[test]
fn emits_direct_pe64_decoded_string_escape_bytes() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                print("A\r\0B")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"A\r\0B"));
}

#[test]
fn emits_direct_pe64_decoded_hex_escape_bytes() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                print("A\x0d\x00B")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"A\r\0B"));
}

#[test]
fn emits_direct_pe64_decoded_unicode_escape_bytes() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                print("lambda: \u{03bb}")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"lambda: \xce\xbb"));
}

#[test]
fn emits_direct_pe64_raw_string_bytes() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                print(r"C:\temp\n.txt")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, br"C:\temp\n.txt"));
}

#[test]
fn emits_direct_pe64_hash_raw_string_quote_bytes() {
    let pe = pe_for(
        r##"
            import std.io

            fn main() {
                print(r#"quote: " and slash: \"#)
            }
        "##,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, br#"quote: " and slash: \"#));
}

#[test]
fn emits_direct_pe64_string_concat_output() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() {
                let message = "Geo " + "compiler"
                println(message)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"GetStdHandle"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo \0compiler\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x8a, 0x00, 0x84, 0xc0]));
    assert!(contains_bytes(&pe, &[0x41, 0x8a, 0x03, 0x84, 0xc0]));
    assert!(contains_bytes(&pe, &[0x41, 0x88, 0x01]));
    assert!(contains_bytes(&pe, &[0x48, 0x89, 0xc2, 0x49, 0x89, 0xc1]));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28]));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x38]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0x54, 0x24, 0x20]));
    assert!(contains_bytes(&pe, &[0x4c, 0x8b, 0x54, 0x24, 0x20]));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xc4, 0x28, 0xc3]));
    assert!(contains_bytes(
        &pe,
        &[0x41, 0xc6, 0x01, 0x00, 0x48, 0x83, 0xc4, 0x38, 0x48, 0x89, 0xd0, 0xc3]
    ));
}

#[test]
fn emits_direct_pe64_string_len_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len("compiler") as int
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler\0"));
    assert!(contains_bytes(
        &pe,
        &[0x48, 0x31, 0xc0, 0x80, 0x3c, 0x01, 0x00]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_memory_alloc_as_compiled_helper() {
    let pe = pe_for(
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

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28]));
}

#[test]
fn emits_direct_pe64_process_exit_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.process

            fn main() -> int {
                return exit(42)
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28]));
}

#[test]
fn emits_direct_pe64_file_read_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_file("C:\\geo-read-file-test")) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"GetFileSize"));
    assert!(contains_bytes(&pe, b"ReadFile"));
    assert!(contains_bytes(&pe, b"CloseHandle"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
}

#[test]
fn emits_direct_pe64_file_read_with_default_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_file_or("C:\\geo-missing-file", "fallback")) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"ReadFile"));
    assert!(contains_bytes(&pe, b"fallback\0"));
}

#[test]
fn emits_direct_pe64_file_write_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return write_file("C:\\geo-write-file-test", "Geo")
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"CloseHandle"));
}

#[test]
fn emits_direct_pe64_append_file_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                return append_file("C:\\geo-append-file-test", "Geo")
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"CloseHandle"));
}

#[test]
fn emits_direct_pe64_touch_and_remove_as_compiled_helpers() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                let touched = touch_file("C:\\geo-touch-file-test")
                if touched != 0 {
                    return touched
                }
                return remove_file("C:\\geo-touch-file-test")
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"CloseHandle"));
    assert!(contains_bytes(&pe, b"DeleteFileA"));
}

#[test]
fn emits_direct_pe64_handle_file_as_compiled_helpers() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                let handle = file_open_write("C:\\geo-handle-file-test")
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

    assert!(contains_bytes(&pe, b"CreateFileA"));
    assert!(contains_bytes(&pe, b"WriteFile"));
    assert!(contains_bytes(&pe, b"CloseHandle"));
}

#[test]
fn emits_direct_pe64_handle_file_read_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                let handle = file_open("C:\\geo-handle-read-test")
                if handle < 0 {
                    return 1
                }
                let contents = file_read_to_string(handle)
                file_close(handle)
                return string_len(contents) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"GetFileSize"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, b"ReadFile"));
}

#[test]
fn emits_direct_pe64_file_exists_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                if file_exists("C:\\geo-file-exists-test") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"GetFileAttributesA"));
}

#[test]
fn emits_direct_pe64_file_metadata_as_compiled_helpers() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                if file_is_file("C:\\geo-file-metadata") {
                    return file_size("C:\\geo-file-metadata") as int
                }
                if file_is_dir("C:\\") && file_is_empty("C:\\geo-file-metadata") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"GetFileAttributesA"));
    assert!(contains_bytes(&pe, b"GetFileSize"));
}

#[test]
fn emits_direct_pe64_read_line_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> int {
                return string_len(read_line()) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"GetStdHandle"));
    assert!(contains_bytes(&pe, b"ReadFile"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
}

#[test]
fn emits_direct_pe64_memory_copy_and_zero_as_compiled_helpers() {
    let pe = pe_for(
        r#"
            import std.mem

            fn main() -> int {
                let memory: *u8 = alloc(8)
                mem_zero(memory, 8)
                return mem_copy(memory, memory, 8)
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xca]));
}

#[test]
fn emits_direct_pe64_memory_move_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.mem

            fn main() -> int {
                let memory: *u8 = alloc(8)
                return mem_move(memory, memory, 8)
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xca]));
}

#[test]
fn emits_direct_pe64_string_from_byte_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len(string_from_byte(65)) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0xc6, 0x40, 0x01, 0x00]));
}

#[test]
fn emits_direct_pe64_string_from_utf8_codepoint_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len(string_from_utf8_codepoint(955)) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(
        &pe,
        &[0x48, 0x81, 0xf9, 0xff, 0x07, 0x00, 0x00]
    ));
}

#[test]
fn emits_direct_pe64_string_clone_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_len(string_clone("Geo")) as int
            }
        "#,
    );

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0x46, 0x8a, 0x0c, 0x1a, 0x47, 0x88]));
}

#[test]
fn emits_direct_pe64_string_slice_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                let value: string = string_slice("compiler.geo", 0usize, 8usize)
                return string_len(value) as int
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"compiler.geo\0"));
    assert!(contains_bytes(&pe, &[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]));
    assert!(contains_bytes(&pe, &[0x41, 0x8a, 0x02, 0x41, 0x88, 0x03]));
}

#[test]
fn emits_direct_pe64_string_utf8_length_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_utf8_len("λ😀") as int
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(
        &pe,
        &[0x41, 0x80, 0xe0, 0xc0, 0x41, 0x80, 0xf8, 0x80],
    ));
}

#[test]
fn emits_direct_pe64_string_utf8_codepoint_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_utf8_codepoint_at("λ😀", 1usize)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(
        &pe,
        &[0x49, 0x89, 0xc8, 0x45, 0x31, 0xc9, 0x45, 0x8a, 0x10],
    ));
}

#[test]
fn emits_direct_pe64_byte_array_runtime_helpers() {
    let pe = pe_for(
        r#"
            import std.array

            fn main() -> int {
                var items: *u8 = array_new(1usize, 2usize)
                var value: u8 = 7
                unsafe {
                    array_push(items, &value)
                    return array_len(items) as int
                }
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, &[0x41, 0xb8, 0x00, 0x30, 0x00, 0x00]));
}

#[test]
fn emits_pe64_array_push_with_windows_register_safe_copy_setup() {
    let pe = pe_for(
        r#"
            import std.array

            fn main() -> int {
                var items: *u8 = array_new(1usize, 2usize)
                var value: u8 = 7
                unsafe {
                    return array_push(items, &value)
                }
            }
        "#,
    );

    assert!(contains_bytes(
        &pe,
        &[
            0x4c, 0x8b, 0x41, 0x18, 0x4d, 0x89, 0xc1, 0x4c, 0x0f, 0xaf, 0xc0, 0x4c, 0x8b, 0x11,
            0x4d, 0x01, 0xd0,
        ]
    ));
}

#[test]
fn emits_direct_pe64_alloc_copy_as_compiled_helper() {
    let pe = pe_for(
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

    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0x46, 0x8a, 0x0c, 0x1a]));
}

#[test]
fn emits_direct_pe64_mem_fill_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.mem
            fn main() -> int {
                let buffer: *u8 = alloc(8)
                return mem_fill(buffer, 8, 65)
            }
        "#,
    );
    assert!(contains_bytes(&pe, &[0xf3, 0xaa]));
}

#[test]
fn emits_direct_pe64_mem_find_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.mem
            fn main() -> int {
                let buffer: *u8 = alloc(8)
                return mem_find(buffer, 8, 65)
            }
        "#,
    );
    assert!(contains_bytes(&pe, &[0x46, 0x38, 0x04, 0x09]));
}

#[test]
fn emits_direct_pe64_string_byte_at_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_byte_at("Geo", 1usize)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo\0"));
    assert!(contains_bytes(&pe, &[0x4d, 0x31, 0xc0, 0x49, 0x39, 0xd0]));
    assert!(contains_bytes(
        &pe,
        &[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_compare_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_compare("Geo", "Geo")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(&pe, &[0x44, 0x29, 0xc8, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_copy_file_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                return copy_file("source.txt", "destination.txt")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, b"CopyFileA"));
    assert!(contains_bytes(&pe, b"source.txt\0destination.txt\0"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_file_timestamps_as_compiled_helpers() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> usize {
                return file_modified_time("source.txt")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"GetFileAttributesExA"));
    assert!(contains_bytes(&pe, b"source.txt\0"));
}

#[test]
fn emits_direct_pe64_directory_count_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> usize {
                return dir_entry_count("target")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"FindFirstFileA"));
    assert!(contains_bytes(&pe, b"FindNextFileA"));
    assert!(contains_bytes(&pe, b"FindClose"));
}

#[test]
fn emits_direct_pe64_directory_name_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io
            import std.string

            fn main() -> usize {
                return string_len(dir_entry_name("target", 0))
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"FindFirstFileA"));
    assert!(contains_bytes(&pe, b"FindNextFileA"));
    assert!(contains_bytes(&pe, b"FindClose"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
}

#[test]
fn emits_direct_pe64_directory_path_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.io

            fn main() -> int {
                if file_is_file(dir_entry_path("target", 0)) {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"FindFirstFileA"));
    assert!(contains_bytes(&pe, b"FindNextFileA"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, b"VirtualFree"));
}

#[test]
fn emits_direct_pe64_process_id_without_runtime_import() {
    let pe = pe_for(
        r#"
            import std.process

            fn main() -> int {
                if process_id() > 0 {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(
        &pe,
        &[0x65, 0x48, 0x8b, 0x04, 0x25, 0x60, 0x00, 0x00, 0x00]
    ));
}

#[test]
fn emits_direct_pe64_process_argument_count_helpers() {
    let pe = pe_for(
        r#"
            import std.process

            fn main() -> int {
                if arg_exists(0) {
                    return arg_count()
                }
                return 0
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"GetCommandLineA"));
    assert!(contains_bytes(&pe, &[0x44, 0x8a, 0x12, 0x45, 0x84, 0xd2]));
}

#[test]
fn emits_direct_pe64_owned_process_argument_helpers() {
    let pe = pe_for(
        r#"
            import std.process

            fn main() -> int {
                let first: string = arg(0)
                let fallback: string = arg_or(9, "fallback")
                if first == fallback {
                    return 1
                }
                return 0
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"GetCommandLineA"));
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, &[0xf3, 0xa4, 0xc6, 0x07, 0x00]));
}

#[test]
fn emits_direct_pe64_platform_path_separator() {
    let pe = pe_for(
        r#"
            import std.platform

            fn main() -> int {
                let separator: char = platform_path_separator()
                if separator == '/' || separator == '\\' {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, &[0xb8, b'\\', 0x00, 0x00, 0x00, 0xc3]));
}

#[test]
fn emits_direct_pe64_path_file_name() {
    let pe = pe_for(
        r#"
        import std.platform
        import std.string
        fn main() -> int {
            let name = path_file_name("a/b\\only.txt")
            let parent = path_parent("a/b\\only.txt")
            let extension = path_extension("a/b\\only.txt")
            let stem = path_stem("a/b\\only.txt")
            let without_extension = path_without_extension("a/b\\only.txt")
            let with_extension = path_with_extension("a/b\\only.txt", ".o")
            string_free(name)
            string_free(parent)
            string_free(extension)
            string_free(stem)
            string_free(without_extension)
            string_free(with_extension)
            return 0
        }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(pe.windows(8).any(|window| window == b"VirtualA"));
}

#[test]
fn emits_direct_pe64_owned_platform_strings() {
    let pe = pe_for(
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let os = platform_os()
                let arch = platform_arch()
                let newline = platform_newline()
                string_free(os)
                string_free(arch)
                string_free(newline)
                return 0
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert!(contains_bytes(&pe, b"VirtualAlloc"));
    assert!(contains_bytes(&pe, b"VirtualFree"));
}

#[test]
fn emits_direct_pe64_absolute_path_predicate() {
    let pe = pe_for(
        r#"
            import std.platform

            fn main() -> int {
                if path_is_absolute("relative/path") {
                    return 1
                }
                return 0
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
}

#[test]
fn emits_direct_pe64_string_contains_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_contains("compiler.geo", ".geo") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo\0.geo\0"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xc8, 0x8a, 0x02]));
    assert!(contains_bytes(&pe, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_starts_with_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_starts_with("compiler.geo", "compiler") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo\0compiler\0"));
    assert!(contains_bytes(&pe, &[0x8a, 0x02, 0x84, 0xc0]));
    assert!(contains_bytes(&pe, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_ends_with_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_ends_with("compiler.geo", ".geo") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo\0.geo\0"));
    assert!(contains_bytes(
        &pe,
        &[0x4d, 0x31, 0xc0, 0x42, 0x80, 0x3c, 0x01, 0x00]
    ));
    assert!(contains_bytes(&pe, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_eq_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_eq("Geo", "Geo") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(&pe, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_not_eq_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_not_eq("Geo", "Rust") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo\0Rust\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(&pe, &[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_less_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_less("alpha", "beta") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"alpha\0beta\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(
        &pe,
        &[0x0f, 0x92, 0xc0, 0x0f, 0xb6, 0xc0, 0xc3]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_less_or_equal_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_less_or_equal("beta", "beta") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"beta\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(
        &pe,
        &[0x0f, 0x96, 0xc0, 0x0f, 0xb6, 0xc0, 0xc3]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_greater_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_greater("zeta", "omega") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"zeta\0omega\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(
        &pe,
        &[0x0f, 0x97, 0xc0, 0x0f, 0xb6, 0xc0, 0xc3]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_greater_or_equal_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_greater_or_equal("omega", "omega") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"omega\0"));
    assert!(contains_bytes(
        &pe,
        &[0x44, 0x0f, 0xb6, 0x01, 0x44, 0x0f, 0xb6, 0x0a]
    ));
    assert!(contains_bytes(
        &pe,
        &[0x0f, 0x93, 0xc0, 0x0f, 0xb6, 0xc0, 0xc3]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_empty_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_empty("") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(
        &pe,
        &[0x31, 0xc0, 0x0f, 0xb6, 0x01, 0x85, 0xc0, 0x0f, 0x94, 0xc0, 0xc3]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii("Geo_123") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo_123\0"));
    assert!(contains_bytes(&pe, &[0x44, 0x8a, 0x01, 0x45, 0x84, 0xc0]));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, 0x7f]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_digit_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_digit("12345") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"12345\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'0']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'9']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_hex_digit_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_hex_digit("0123456789abcdefABCDEF") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"0123456789abcdefABCDEF\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'0']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'f']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'F']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_alpha_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_alpha("GeoLang") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"GeoLang\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'A']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'Z']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'a']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'z']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_lower_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_lower("geolang") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"geolang\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'a']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'z']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_upper_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_upper("GEOLANG") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"GEOLANG\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'A']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'Z']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_alnum_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_alnum("Geo123") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo123\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'0']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'z']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_identifier_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_identifier("_geo123") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"_geo123\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'_']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'0']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'z']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_is_ascii_whitespace_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                if string_is_ascii_whitespace(" \t\n\r") {
                    return 0
                }
                return 1
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b" \t\n\r\0"));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b' ']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'\t']));
    assert!(contains_bytes(&pe, &[0x41, 0x80, 0xf8, b'\r']));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_find_byte_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_find_byte("Geo", 101)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"Geo\0"));
    assert!(contains_bytes(&pe, &[0x4d, 0x31, 0xc0]));
    assert!(contains_bytes(&pe, &[0x42, 0x8a, 0x04, 0x01]));
    assert!(contains_bytes(&pe, &[0x38, 0xd0]));
    assert!(contains_bytes(
        &pe,
        &[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_last_find_byte_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_last_find_byte("banana", 97)
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"banana\0"));
    assert!(contains_bytes(
        &pe,
        &[0x49, 0xc7, 0xc1, 0xff, 0xff, 0xff, 0xff]
    ));
    assert!(contains_bytes(&pe, &[0x42, 0x8a, 0x04, 0x01]));
    assert!(contains_bytes(&pe, &[0x38, 0xd0]));
    assert!(contains_bytes(&pe, &[0x4d, 0x89, 0xc1]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0xc8, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_index_of_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_index_of("compiler.geo", ".geo")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo\0.geo\0"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xc8, 0x4d, 0x31, 0xdb]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0xd8, 0xc3]));
    assert!(contains_bytes(
        &pe,
        &[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]
    ));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_last_index_of_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_last_index_of("compiler.geo.compiler.geo", ".geo")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo.compiler.geo\0.geo\0"));
    assert!(contains_bytes(
        &pe,
        &[0x49, 0x89, 0xc8, 0x48, 0x31, 0xc9, 0x49, 0xc7, 0xc3, 0xff, 0xff, 0xff, 0xff,]
    ));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xcb, 0x48, 0xff, 0xc1]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0xd8, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_count_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> usize {
                return string_count("compiler.geo.compiler.geo", ".geo")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b"compiler.geo.compiler.geo\0.geo\0"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xc8, 0x4d, 0x31, 0xc9]));
    assert!(contains_bytes(&pe, &[0x49, 0xff, 0xc1, 0x4d, 0x89, 0xd8]));
    assert!(contains_bytes(&pe, &[0x4c, 0x89, 0xc8, 0xc3]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

#[test]
fn emits_direct_pe64_string_parse_int_as_compiled_helper() {
    let pe = pe_for(
        r#"
            import std.string

            fn main() -> int {
                return string_parse_int(" -42")
            }
        "#,
    );

    assert_eq!(&pe[0..2], b"MZ");
    assert_eq!(&pe[0x80..0x84], b"PE\0\0");
    assert!(contains_bytes(&pe, &[0x48, 0x83, 0xec, 0x28, 0xe8]));
    assert!(contains_bytes(&pe, b" -42\0"));
    assert!(contains_bytes(&pe, &[0x49, 0x89, 0xc8, 0x49, 0x31, 0xc9]));
    assert!(contains_bytes(&pe, &[0x4d, 0x6b, 0xc9, 0x0a]));
    assert!(contains_bytes(&pe, &[0x4d, 0x01, 0xd9]));
    assert!(contains_bytes(&pe, &[0x4d, 0x0f, 0xaf, 0xca]));
    assert!(contains_bytes(&pe, b"ExitProcess"));
    assert!(!contains_bytes(&pe, b"WriteFile"));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
