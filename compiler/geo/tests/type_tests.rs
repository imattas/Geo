use geo::lexer::lex;
use geo::parser::parse;
use geo::typecheck::check;

fn check_source(source: &str) -> Result<(), Vec<geo::diagnostics::Diagnostic>> {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program)
}

#[test]
fn accepts_return_42() {
    check_source("fn main() -> int { return 42 }").unwrap();
}

#[test]
fn accepts_function_tail_expression_return() {
    check_source("fn add(a: int, b: int) -> int { a + b } fn main() -> int { add(40, 2) }")
        .unwrap();
}

#[test]
fn rejects_function_tail_expression_type_mismatch() {
    let err = check_source("fn main() -> int { true }").unwrap_err();

    assert!(err[0].message.contains("tail expression type mismatch"));
}

#[test]
fn accepts_top_level_const_use() {
    check_source("const LIMIT: int = 42 fn main() -> int { return LIMIT }").unwrap();
}

#[test]
fn accepts_enum_variant_use() {
    check_source(
        r#"
            enum TokenKind {
                Eof
                Ident
                Number
            }

            fn main() -> TokenKind {
                return TokenKind.Number
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_enum_variant_with_explicit_discriminant() {
    check_source(
        r#"
            enum Status {
                Ok = 0
                Error = 42
            }

            fn main() -> Status {
                Status.Error
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_duplicate_enum_discriminant() {
    let err = check_source(
        r#"
            enum Status {
                Ok = 1
                Error = 1
            }

            fn main() -> int {
                0
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("duplicate discriminant")));
}

#[test]
fn rejects_duplicate_effective_enum_discriminant_after_explicit_value() {
    let err = check_source(
        r#"
            enum Status {
                Ok = 5
                Warning
                Error = 6
            }

            fn main() -> int {
                0
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("duplicate discriminant")));
}

#[test]
fn accepts_match_expression_on_enum() {
    check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    TokenKind.Eof => 0
                    TokenKind.Number => 2
                    _ => 9
                }
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_exhaustive_match_expression_on_enum_without_wildcard() {
    check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    TokenKind.Eof => 0
                    TokenKind.Number => 2
                }
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_non_exhaustive_match_expression_on_enum() {
    let err = check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    TokenKind.Eof => 0
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("non-exhaustive match expression")));
}

#[test]
fn rejects_non_exhaustive_match_expression_on_bool() {
    let err = check_source(
        r#"
            fn main() -> int {
                let flag: bool = true
                return match flag {
                    true => 1
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("non-exhaustive match expression")));
}

#[test]
fn rejects_duplicate_enum_match_arm() {
    let err = check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    TokenKind.Eof => 0
                    TokenKind.Eof => 1
                    TokenKind.Number => 2
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unreachable match arm")));
}

#[test]
fn rejects_duplicate_bool_match_arm() {
    let err = check_source(
        r#"
            fn main() -> int {
                let flag: bool = true
                return match flag {
                    true => 1
                    true => 2
                    false => 0
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unreachable match arm")));
}

#[test]
fn rejects_match_arm_after_wildcard() {
    let err = check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    _ => 9
                    TokenKind.Number => 2
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unreachable match arm")));
}

#[test]
fn rejects_match_arm_type_mismatch() {
    let err = check_source(
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn main() -> int {
                let kind: TokenKind = TokenKind.Number
                return match kind {
                    TokenKind.Eof => 0
                    _ => true
                }
            }
        "#,
    )
    .unwrap_err();

    assert!(err[0].message.contains("match arm type mismatch"));
}

#[test]
fn accepts_if_expression_with_matching_branches() {
    check_source("fn main() -> int { let value: int = if true { 1 } else { 2 } return value }")
        .unwrap();
}

#[test]
fn rejects_if_expression_branch_type_mismatch() {
    let err = check_source("fn main() -> int { return if true { 1 } else { false } }").unwrap_err();

    assert!(err[0]
        .message
        .contains("if expression branch type mismatch"));
}

#[test]
fn accepts_multi_level_else_if_chain() {
    check_source(
        r#"
            fn classify(score: int) -> int {
                if score >= 90 {
                    return 3
                } else if score >= 70 {
                    return 2
                } else if score >= 50 {
                    return 1
                } else {
                    return 0
                }
            }

            fn main() -> int {
                return classify(75)
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_block_expression_with_local_setup_and_tail_value() {
    check_source("fn main() -> int { return { let base: int = 40 base + 2 } }").unwrap();
}

#[test]
fn accepts_if_expression_branches_with_block_setup() {
    check_source(
        r#"
            fn main() -> int {
                let enabled: bool = true
                return if enabled {
                    let base: int = 40
                    base + 2
                } else {
                    let fallback: int = 7
                    fallback
                }
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_block_expression_tail_type_mismatch() {
    let err = check_source("fn main() -> int { return { let base: int = 40 true } }").unwrap_err();

    assert!(err[0].message.contains("return type mismatch"));
}

#[test]
fn rejects_unknown_enum_variant() {
    let err = check_source(
        r#"
            enum TokenKind {
                Eof
            }

            fn main() -> TokenKind {
                return TokenKind.Number
            }
        "#,
    )
    .unwrap_err();

    assert!(err[0].message.contains("unknown variant 'Number'"));
}

#[test]
fn rejects_const_type_mismatch() {
    let err = check_source("const LIMIT: int = true fn main() -> int { return 0 }").unwrap_err();
    assert!(err[0].message.contains("const initializer type mismatch"));
}

#[test]
fn rejects_circular_const_dependency() {
    let err = check_source(
        r#"
            const A: int = B + 1
            const B: int = A + 1

            fn main() -> int {
                return A
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("circular constant dependency")));
}

#[test]
fn rejects_duplicate_const_and_function_name() {
    let err = check_source("const main: int = 1 fn main() -> int { return main }").unwrap_err();
    assert!(err[0]
        .message
        .contains("duplicate function or constant 'main'"));
}

#[test]
fn rejects_wrong_return_type() {
    let err = check_source("fn main() -> int { return true }").unwrap_err();
    assert!(err[0].message.contains("return type mismatch"));
}

#[test]
fn rejects_unknown_variable() {
    let err = check_source("fn main() -> int { return x }").unwrap_err();
    assert!(err[0].message.contains("unknown variable 'x'"));
}

#[test]
fn rejects_wrong_call_arity() {
    let source =
        "fn add(a: int, b: int) -> int { return a + b } fn main() -> int { return add(1) }";
    let err = check_source(source).unwrap_err();
    assert!(err[0].message.contains("expected 2 arguments"));
}

#[test]
fn rejects_bool_arithmetic() {
    let err = check_source("fn main() -> int { return 1 + true }").unwrap_err();
    assert!(err[0].message.contains("arithmetic operands must be"));
}

#[test]
fn accepts_integer_remainder() {
    check_source("fn main() -> int { return 10 % 4 }").unwrap();
}

#[test]
fn accepts_prefixed_and_underscored_integer_literals() {
    check_source("fn main() -> int { return 0xff + 0b1010 + 0o755 + 1_000 }").unwrap();
}

#[test]
fn accepts_integer_literals_at_typed_integer_bounds() {
    check_source(
        r#"
            fn main() -> int {
                let min_i8: i8 = -128
                let max_i8: i8 = 127
                let max_u8: u8 = 255
                let max_i16: i16 = 32_767
                let max_u16: u16 = 65_535
                return max_u8 as int
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_typed_integer_literal_suffixes() {
    check_source("fn main() -> u8 { return 255u8 }").unwrap();
    check_source("fn main() -> usize { return 16usize }").unwrap();
}

#[test]
fn rejects_typed_integer_literal_suffix_out_of_range() {
    let err = check_source("fn main() -> u8 { return 256u8 }").unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 256 does not fit in type u8"));
}

#[test]
fn accepts_type_aliases_for_scalars_pointers_and_arrays() {
    check_source(
        r#"
            type Byte = u8
            type BytePtr = *Byte
            type Bytes = [Byte]

            fn main() -> int {
                let value: Byte = 255
                let ptr: BytePtr = null
                let values: Bytes = [1, 2, 3]
                return value as int + values[0] as int
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_integer_literal_outside_type_alias_range() {
    let err = check_source(
        "type Byte = u8 fn main() -> int { let value: Byte = 256 return value as int }",
    )
    .unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 256 does not fit in type u8"));
}

#[test]
fn rejects_duplicate_type_alias_name() {
    let err =
        check_source("type Byte = u8 type Byte = i8 fn main() -> int { return 0 }").unwrap_err();

    assert!(err[0].message.contains("duplicate type alias 'Byte'"));
}

#[test]
fn rejects_unknown_type_alias_target() {
    let err =
        check_source("type MissingAlias = Missing fn main() -> int { return 0 }").unwrap_err();

    assert!(err[0].message.contains("unknown type 'Missing'"));
}

#[test]
fn rejects_integer_literal_outside_annotated_local_type_range() {
    let err =
        check_source("fn main() -> int { let value: u8 = 256 return value as int }").unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 256 does not fit in type u8"));
}

#[test]
fn rejects_integer_literal_outside_const_type_range() {
    let err =
        check_source("const MASK: u8 = 0x1ff fn main() -> int { return MASK as int }").unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 511 does not fit in type u8"));
}

#[test]
fn rejects_integer_literal_outside_return_type_range() {
    let err = check_source("fn main() -> u8 { return 300 }").unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 300 does not fit in type u8"));
}

#[test]
fn rejects_integer_literal_outside_call_argument_type_range() {
    let err = check_source(
        "fn take(value: i8) -> int { return value as int } fn main() -> int { return take(128) }",
    )
    .unwrap_err();

    assert!(err[0]
        .message
        .contains("integer literal 128 does not fit in type i8"));
}

#[test]
fn rejects_bool_remainder() {
    let err = check_source("fn main() -> bool { return true % false }").unwrap_err();
    assert!(err[0].message.contains("arithmetic operands must be"));
}

#[test]
fn accepts_compound_assignment_to_mutable_integer() {
    check_source("fn main() -> int { var x: int = 10 x += 5 x %= 4 return x }").unwrap();
}

#[test]
fn accepts_bitwise_and_shift_compound_assignment_to_mutable_integer() {
    check_source(
        r#"
            fn main() -> int {
                var x: int = 3
                x &= 6
                x |= 8
                x ^= 1
                x <<= 2
                x >>= 1
                return x
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_compound_assignment_to_immutable_let() {
    let err = check_source("fn main() -> int { let x: int = 1 x += 2 return x }").unwrap_err();
    assert!(err[0]
        .message
        .contains("cannot assign to immutable local 'x'"));
}

#[test]
fn accepts_compound_assignment_through_mutable_reference() {
    check_source(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &mut int = &mut value
                *slot += 41
                return value
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_compound_assignment_through_shared_reference() {
    let err = check_source(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &int = &value
                *slot += 41
                return value
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0].message.contains("mutable reference"));
}

#[test]
fn accepts_compound_assignment_through_raw_pointer_in_unsafe() {
    check_source(
        r#"
            fn main() -> int {
                var value: int = 1
                unsafe {
                    let slot: *int = &value
                    *slot += 41
                }
                return value
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_compound_assignment_through_raw_pointer_outside_unsafe() {
    let err = check_source(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: *int = &value
                *slot += 41
                return value
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0].message.contains("requires unsafe"));
}

#[test]
fn accepts_boolean_logic() {
    check_source("fn main() -> bool { return true || false && true }").unwrap();
}

#[test]
fn rejects_integer_boolean_logic() {
    let err = check_source("fn main() -> bool { return 1 && 2 }").unwrap_err();
    assert!(err[0].message.contains("logical operands must be bool"));
}

#[test]
fn accepts_integer_bitwise_ops() {
    check_source("fn main() -> int { return 10 | 6 ^ 3 & 1 }").unwrap();
}

#[test]
fn rejects_bool_bitwise_ops() {
    let err = check_source("fn main() -> bool { return true & false }").unwrap_err();
    assert!(err[0]
        .message
        .contains("bitwise operands must be matching integer types"));
}

#[test]
fn accepts_integer_shift_ops() {
    check_source("fn main() -> int { return 1 << 3 >> 1 }").unwrap();
}

#[test]
fn rejects_bool_shift_ops() {
    let err = check_source("fn main() -> bool { return true << false }").unwrap_err();
    assert!(err[0]
        .message
        .contains("shift operands must be matching integer types"));
}

#[test]
fn accepts_integer_bitwise_not() {
    check_source("fn main() -> int { return ~10 }").unwrap();
}

#[test]
fn rejects_bool_bitwise_not() {
    let err = check_source("fn main() -> bool { return ~true }").unwrap_err();
    assert!(err[0]
        .message
        .contains("bitwise NOT operand must be an integer"));
}

#[test]
fn accepts_integer_casts() {
    check_source("fn main() -> i32 { let x: int = 42 return x as i32 }").unwrap();
}

#[test]
fn accepts_pointer_to_usize_cast() {
    check_source(
        r#"
            fn main() -> usize {
                let ptr: *u8 = null
                return ptr as usize
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_usize_to_pointer_cast_in_unsafe() {
    check_source(
        r#"
            fn main() -> int {
                let addr: usize = 0
                unsafe {
                    let ptr: *u8 = addr as *u8
                }
                return 0
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_usize_to_pointer_cast_outside_unsafe() {
    let err = check_source(
        r#"
            fn main() -> int {
                let addr: usize = 0
                let ptr: *u8 = addr as *u8
                return 0
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0]
        .message
        .contains("integer to pointer cast requires unsafe"));
}

#[test]
fn accepts_raw_pointer_add_and_sub_in_unsafe() {
    check_source(
        r#"
            fn main() -> usize {
                unsafe {
                    let ptr: *u32 = null
                    let next: *u32 = ptr + 2
                    let prev: *u32 = next - 1
                    return prev as usize
                }
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_raw_pointer_difference_in_unsafe() {
    check_source(
        r#"
            fn main() -> int {
                unsafe {
                    let first: *u32 = null
                    let last: *u32 = first + 3
                    return last - first
                }
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_raw_pointer_arithmetic_outside_unsafe() {
    let err = check_source(
        r#"
            fn main() -> int {
                let ptr: *u32 = null
                let next: *u32 = ptr + 1
                return 0
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0]
        .message
        .contains("raw pointer arithmetic requires unsafe"));
}

#[test]
fn accepts_raw_pointer_compound_assignment_in_unsafe() {
    check_source(
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
    )
    .unwrap();
}

#[test]
fn rejects_raw_pointer_compound_assignment_outside_unsafe() {
    let err = check_source(
        r#"
            fn main() -> int {
                var ptr: *u32 = null
                ptr += 1
                return 0
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0]
        .message
        .contains("raw pointer arithmetic requires unsafe"));
}

#[test]
fn accepts_raw_pointer_ordering_comparisons_in_unsafe() {
    check_source(
        r#"
            fn main() -> int {
                unsafe {
                    let first: *u32 = null
                    let last: *u32 = first + 3
                    if first < last && first <= last && last > first && last >= first {
                        return 42
                    }
                }
                return 1
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_raw_pointer_ordering_comparison_outside_unsafe() {
    let err = check_source(
        r#"
            fn main() -> int {
                let first: *u32 = null
                let last: *u32 = null
                if first < last {
                    return 42
                }
                return 1
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0]
        .message
        .contains("raw pointer comparison requires unsafe"));
}

#[test]
fn accepts_sizeof_as_usize() {
    check_source("fn main() -> usize { return sizeof(int) }").unwrap();
}

#[test]
fn rejects_sizeof_unknown_type() {
    let err = check_source("fn main() -> usize { return sizeof(Missing) }").unwrap_err();
    assert!(err[0].message.contains("unknown type 'Missing'"));
}

#[test]
fn accepts_alignof_as_usize() {
    check_source("fn main() -> usize { return alignof(int) }").unwrap();
}

#[test]
fn rejects_alignof_unknown_type() {
    let err = check_source("fn main() -> usize { return alignof(Missing) }").unwrap_err();
    assert!(err[0].message.contains("unknown type 'Missing'"));
}

#[test]
fn accepts_offsetof_as_usize() {
    check_source(
        r#"
            struct Header {
                tag: u8
                next: *u8
            }

            fn main() -> usize {
                return offsetof(Header, next)
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_offsetof_unknown_field() {
    let err = check_source(
        r#"
            struct Header {
                tag: u8
            }

            fn main() -> usize {
                return offsetof(Header, missing)
            }
        "#,
    )
    .unwrap_err();
    assert!(err[0].message.contains("unknown field 'missing'"));
}

#[test]
fn rejects_offsetof_non_struct_type() {
    let err = check_source("fn main() -> usize { return offsetof(int, value) }").unwrap_err();
    assert!(err[0].message.contains("offsetof requires a struct type"));
}

#[test]
fn accepts_null_for_raw_pointer_context() {
    check_source(
        r#"
            fn main() -> int {
                let p: *u8 = null
                if p == null {
                    return 42
                }
                return 1
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_null_comparison_with_pointer_on_right() {
    check_source(
        r#"
            fn main() -> int {
                let p: *u8 = null
                if (null == p) {
                    return 1
                }
                if (null != p) {
                    return 2
                }
                return 0
            }
        "#,
    )
    .unwrap();
}

#[test]
fn rejects_null_for_integer_context() {
    let err = check_source("fn main() -> int { let x: int = null return x }").unwrap_err();
    assert!(err[0]
        .message
        .contains("null requires raw pointer type context"));
}

#[test]
fn rejects_null_without_type_context() {
    let err = check_source("fn main() -> int { let p = null return 0 }").unwrap_err();
    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("raw pointer type context")));
}

#[test]
fn rejects_non_integer_cast_source() {
    let err = check_source("fn main() -> int { return true as int }").unwrap_err();
    assert!(err[0]
        .message
        .contains("casts require integer or raw pointer source and target types"));
}

#[test]
fn rejects_non_integer_cast_target() {
    let err = check_source("fn main() -> bool { return 1 as bool }").unwrap_err();
    assert!(err[0]
        .message
        .contains("casts require integer or raw pointer source and target types"));
}

#[test]
fn accepts_locals_assignment_if_and_while() {
    let source = r#"
        fn main() -> int {
            var x: int = 0
            while x < 42 {
                x = x + 1
            }
            if x == 42 {
                return x
            } else {
                return 0
            }
        }
    "#;
    check_source(source).unwrap();
}

#[test]
fn accepts_for_in_integer_range_loop() {
    let source = r#"
        fn main() -> int {
            var total: int = 0
            for i in 0..4 {
                total += i
            }
            return total
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_for_in_inclusive_integer_range_loop() {
    let source = r#"
        fn main() -> int {
            var total: int = 0
            for i in 0..=4 {
                total += i
            }
            return total
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_unconditional_loop_with_break() {
    let source = r#"
        fn main() -> int {
            var x: int = 0
            loop {
                x += 1
                if x == 4 {
                    break
                }
            }
            return x
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn rejects_non_integer_for_range_bounds() {
    let err = check_source("fn main() -> int { for i in false..true { return i } return 0 }")
        .unwrap_err();

    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("for range bounds must be matching integer types")));
}

#[test]
fn rejects_assignment_to_for_loop_variable() {
    let err = check_source("fn main() -> int { for i in 0..4 { i = 2 } return 0 }").unwrap_err();

    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot assign to immutable local 'i'")));
}

#[test]
fn rejects_assignment_type_mismatch() {
    let err = check_source("fn main() -> int { var x: int = 1 x = true return x }").unwrap_err();
    assert!(err[0].message.contains("assignment type mismatch"));
}

#[test]
fn rejects_assignment_to_immutable_let() {
    let err = check_source("fn main() -> int { let x: int = 1 x = 2 return x }").unwrap_err();
    assert!(err[0]
        .message
        .contains("cannot assign to immutable local 'x'"));
}

#[test]
fn rejects_non_bool_while_condition() {
    let err = check_source("fn main() -> int { while 1 { return 0 } return 1 }").unwrap_err();
    assert!(err[0].message.contains("while condition must be bool"));
}

#[test]
fn rejects_non_bool_if_condition() {
    let err = check_source("fn main() -> int { if 1 { return 0 } else { return 1 } }").unwrap_err();
    assert!(err[0].message.contains("if condition must be bool"));
}

#[test]
fn accepts_v1_literals_unary_and_loop_control() {
    let source = r#"
        fn main() -> int {
            let name: string = "Geo"
            let marker: char = 'G'
            let size: usize = 1
            while !false {
                continue
                break
            }
            return -1 + 2
        }
    "#;
    check_source(source).unwrap();
}

#[test]
fn rejects_wrong_unary_not_operand() {
    let err = check_source("fn main() -> int { return !1 }").unwrap_err();
    assert!(err[0].message.contains("unary '!' operand must be bool"));
}

#[test]
fn rejects_break_outside_loop() {
    let err = check_source("fn main() -> int { break return 1 }").unwrap_err();
    assert!(err[0].message.contains("break outside loop"));
}

#[test]
fn accepts_struct_array_field_and_index_types() {
    let source = r#"
        struct Token {
            kind: int
            start: usize
        }

        fn main() -> int {
            let tokens: [Token] = []
            let first: Token = Token { kind: 1 start: 0 }
            let pair: [Token] = [first]
            return pair[0].kind
        }
    "#;
    check_source(source).unwrap();
}

#[test]
fn accepts_comma_separated_struct_declaration_fields() {
    check_source(
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
    )
    .unwrap();
}

#[test]
fn accepts_struct_literal_field_shorthand() {
    check_source(
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
    )
    .unwrap();
}

#[test]
fn accepts_trailing_commas_in_params_calls_and_arrays() {
    check_source(
        r#"
            fn add(a: int, b: int,) -> int {
                return a + b
            }

            fn main() -> int {
                let values: [int] = [40, 2,]
                return add(values[0], values[1],)
            }
        "#,
    )
    .unwrap();
}

#[test]
fn accepts_field_and_index_assignment_places() {
    let source = r#"
        struct Token {
            kind: int
        }

        fn main() -> int {
            var token: Token = Token { kind: 1 }
            var values: [int] = [1]
            token.kind = 2
            values[0] += token.kind
            return values[0]
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn rejects_assignment_to_field_of_immutable_local() {
    let source = r#"
        struct Token {
            kind: int
        }

        fn main() -> int {
            let token: Token = Token { kind: 1 }
            token.kind = 2
            return token.kind
        }
    "#;

    let err = check_source(source).unwrap_err();
    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot assign to immutable local 'token'")));
}

#[test]
fn rejects_place_assignment_type_mismatch() {
    let source = r#"
        struct Token {
            kind: int
        }

        fn main() -> int {
            var token: Token = Token { kind: 1 }
            token.kind = true
            return token.kind
        }
    "#;

    let err = check_source(source).unwrap_err();
    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("place assignment type mismatch")));
}

#[test]
fn rejects_unknown_struct_field() {
    let source = r#"
        struct Token {
            kind: int
        }

        fn main() -> int {
            let first: Token = Token { kind: 1 }
            return first.start
        }
    "#;
    let err = check_source(source).unwrap_err();
    assert!(err[0].message.contains("unknown field 'start'"));
}

#[test]
fn rejects_mixed_array_literal_types() {
    let err = check_source("fn main() -> int { let xs: [int] = [1, true] return 0 }").unwrap_err();
    assert!(err[0].message.contains("array element type mismatch"));
}

#[test]
fn accepts_extern_declaration_and_call() {
    let source = r#"
        import std.io
        extern fn puts(message: *u8) -> int

        fn main() -> int {
            return puts(0)
        }
    "#;
    check_source(source).unwrap();
}

#[test]
fn rejects_duplicate_extern_and_function_name() {
    let source = r#"
        extern fn puts(message: *u8) -> int
        fn puts(message: *u8) -> int {
            return 0
        }
        fn main() -> int {
            return 0
        }
    "#;
    let err = check_source(source).unwrap_err();
    assert!(err[0].message.contains("duplicate function 'puts'"));
}

#[test]
fn accepts_imported_std_io_call() {
    let source = r#"
        import std.io

        fn main() {
            println("Geo")
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_clean_core_unit_main_inferred_let_var_str_and_concat() {
    let source = r#"
        import std.io

        fn greet(name: str) -> str {
            return "Hello, " + name
        }

        fn main() {
            let message = greet("world")
            var count = 0
            count = count + 1
            println(message)
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_imported_std_string_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            let len: usize = string_len("Geo")
            return 0
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_imported_std_platform_calls() {
    let source = r#"
        import std.platform

        fn main() -> int {
            let os: string = platform_os()
            let sep: char = platform_path_separator()
            let newline: string = platform_newline()
            return 0
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn rejects_unknown_std_import() {
    let source = r#"
        import std.net

        fn main() -> int {
            return 0
        }
    "#;

    let err = check_source(source).unwrap_err();
    assert!(err[0]
        .message
        .contains("unknown standard library module 'std.net'"));
}

#[test]
fn rejects_std_import_function_name_conflict() {
    let source = r#"
        import std.io

        fn println(value: string) -> int {
            return 0
        }

        fn main() -> int {
            return 0
        }
    "#;

    let err = check_source(source).unwrap_err();
    assert!(err[0].message.contains("duplicate function 'println'"));
}

#[test]
fn accepts_unsafe_address_of_and_deref() {
    let source = r#"
        fn main() -> int {
            let x: int = 42
            unsafe {
                let p: *int = &x
                return *p
            }
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_unsafe_pointer_assignment() {
    let source = r#"
        fn main() -> int {
            var x: int = 1
            unsafe {
                let p: *int = &x
                *p = 42
            }
            return x
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn accepts_mutable_reference_assignment() {
    let source = r#"
        fn main() -> int {
            var x: int = 1
            let slot: &mut int = &mut x
            *slot = 42
            return x
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn rejects_pointer_assignment_outside_unsafe() {
    let err = check_source("fn main() -> int { var x: int = 1 let p: *int = &x *p = 2 return x }")
        .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("requires unsafe")));
}

#[test]
fn rejects_assignment_through_shared_reference() {
    let err = check_source(
        r#"
            fn main() -> int {
                var x: int = 1
                let slot: &int = &x
                *slot = 42
                return x
            }
        "#,
    )
    .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("mutable reference")));
}

#[test]
fn rejects_pointer_assignment_type_mismatch() {
    let err = check_source(
        r#"
            fn main() -> int {
                var x: int = 1
                unsafe {
                    let p: *int = &x
                    *p = true
                }
                return x
            }
        "#,
    )
    .unwrap_err();

    assert!(err.iter().any(|diagnostic| diagnostic
        .message
        .contains("pointer assignment type mismatch")));
}

#[test]
fn accepts_references_and_safe_deref() {
    let source = r#"
        fn main() -> int {
            let x: int = 42
            let shared: &int = &x
            return *shared
        }
    "#;

    check_source(source).unwrap();
}

#[test]
fn rejects_raw_pointer_ops_outside_unsafe() {
    let err = check_source("fn main() -> int { let x: int = 42 let p: *int = &x return *p }")
        .unwrap_err();

    assert!(err
        .iter()
        .any(|diagnostic| diagnostic.message.contains("requires unsafe")));
}
