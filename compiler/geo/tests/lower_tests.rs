use geo::ir::{Instruction, ValueId};
use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::typecheck::check;

fn lower_source(source: &str) -> geo::ir::IrProgram {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    lower(&program)
}

#[test]
fn lowers_return_42() {
    let ir = lower_source("fn main() -> int { return 42 }");
    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::Const {
                dst: ValueId(0),
                value: 42
            },
            Instruction::Return { value: ValueId(0) },
        ]
    );
}

#[test]
fn lowers_function_tail_expression_to_return() {
    let ir = lower_source("fn main() -> int { 42 }");

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::Const {
                dst: ValueId(0),
                value: 42
            },
            Instruction::Return { value: ValueId(0) },
        ]
    );
}

#[test]
fn lowers_top_level_const_use_inline() {
    let ir = lower_source("const LIMIT: int = 42 fn main() -> int { return LIMIT }");

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 42, .. })));
}

#[test]
fn folds_top_level_const_arithmetic_dependencies() {
    let ir = lower_source(
        r#"
            const BASE: int = 40
            const LIMIT: int = BASE + 2

            fn main() -> int {
                return LIMIT
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 42, .. })));
    assert!(!ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
}

#[test]
fn lowers_enum_variant_to_discriminant() {
    let ir = lower_source(
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
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 2, .. })));
}

#[test]
fn lowers_enum_variant_to_explicit_discriminant() {
    let ir = lower_source(
        r#"
            enum Status {
                Ok = 0
                Warning = 7
                Error = 42
            }

            fn main() -> Status {
                Status.Error
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 42, .. })));
}

#[test]
fn lowers_implicit_enum_variant_after_explicit_discriminant_to_next_value() {
    let ir = lower_source(
        r#"
            enum Status {
                Ok = 5
                Warning
                Error
            }

            fn main() -> Status {
                Status.Error
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 7, .. })));
}

#[test]
fn lowers_match_expression_to_control_flow() {
    let ir = lower_source(
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
    );

    let function = &ir.functions[0];
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Cmp { .. })));
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::JumpIfZero { .. })));
    assert!(function.instructions.iter().any(
        |ins| matches!(ins, Instruction::Store { local, .. } if local.starts_with("__geo_match_"))
    ));
}

#[test]
fn lowers_if_expression_to_control_flow() {
    let ir = lower_source("fn main() -> int { return if true { 7 } else { 9 } }");

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::JumpIfZero { .. })));
    assert!(ir.functions[0].instructions.iter().any(
        |ins| matches!(ins, Instruction::Store { local, .. } if local.starts_with("__geo_if_"))
    ));
}

#[test]
fn lowers_block_expression_to_setup_statements_and_tail_value() {
    let ir = lower_source("fn main() -> int { return { let base: int = 40 base + 2 } }");

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "base")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
}

#[test]
fn lowers_addition() {
    let ir = lower_source("fn main() -> int { return 1 + 2 }");
    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::Const {
                dst: ValueId(0),
                value: 1
            },
            Instruction::Const {
                dst: ValueId(1),
                value: 2
            },
            Instruction::Add {
                dst: ValueId(2),
                left: ValueId(0),
                right: ValueId(1)
            },
            Instruction::Return { value: ValueId(2) },
        ]
    );
}

#[test]
fn lowers_remainder() {
    let ir = lower_source("fn main() -> int { return 10 % 4 }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Rem { .. })));
}

#[test]
fn lowers_compound_assignment_to_load_op_store() {
    let ir = lower_source("fn main() -> int { var x: int = 10 x += 5 return x }");
    let instructions = &ir.functions[0].instructions;
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Load { local, .. } if local == "x")));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
    assert!(
        instructions
            .iter()
            .filter(|ins| matches!(ins, Instruction::Store { local, .. } if local == "x"))
            .count()
            >= 2
    );
}

#[test]
fn lowers_boolean_logic() {
    let ir = lower_source("fn main() -> bool { return true || false && true }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::And { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Or { .. })));
}

#[test]
fn lowers_bitwise_ops() {
    let ir = lower_source("fn main() -> int { return 10 | 6 ^ 3 & 1 }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::BitAnd { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::BitXor { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::BitOr { .. })));
}

#[test]
fn lowers_shift_ops() {
    let ir = lower_source("fn main() -> int { return 1 << 3 >> 1 }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::ShiftLeft { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::ShiftRight { .. })));
}

#[test]
fn lowers_bitwise_not() {
    let ir = lower_source("fn main() -> int { return ~10 }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::BitNot { .. })));
}

#[test]
fn lowers_sizeof_type_to_constant() {
    let ir = lower_source(
        r#"
            struct Buffer {
                ptr: *u8
                len: usize
            }

            fn main() -> usize {
                return sizeof(Buffer)
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 16, .. })));
}

#[test]
fn lowers_alignof_type_to_constant() {
    let ir = lower_source(
        r#"
            struct Header {
                tag: u8
                next: *u8
            }

            fn main() -> usize {
                return alignof(Header)
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 8, .. })));
}

#[test]
fn lowers_offsetof_field_to_padded_constant() {
    let ir = lower_source(
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

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 8, .. })));
}

#[test]
fn lowers_raw_pointer_add_to_scaled_integer_add() {
    let ir = lower_source(
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

    let instructions = &ir.functions[0].instructions;
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 4, .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Mul { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
}

#[test]
fn lowers_raw_pointer_difference_to_scaled_integer_difference() {
    let ir = lower_source(
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

    let instructions = &ir.functions[0].instructions;
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Sub { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Div { .. })));
}

#[test]
fn lowers_raw_pointer_compound_assignment_to_scaled_store() {
    let ir = lower_source(
        r#"
            fn main() -> usize {
                unsafe {
                    var ptr: *u32 = null
                    ptr += 3
                    return ptr as usize
                }
            }
        "#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 4, .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Mul { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "ptr")));
}

#[test]
fn lowers_sizeof_struct_with_padding_to_constant() {
    let ir = lower_source(
        r#"
            struct Header {
                tag: u8
                next: *u8
            }

            fn main() -> usize {
                return sizeof(Header)
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 16, .. })));
}

#[test]
fn lowers_null_literal_to_zero_constant() {
    let ir = lower_source("fn main() -> int { let p: *u8 = null return 0 }");

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Const { value: 0, .. })));
}

#[test]
fn lowers_local_store_and_load() {
    let ir = lower_source("fn main() -> int { let x: int = 42 return x }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "x")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Load { local, .. } if local == "x")));
}

#[test]
fn lowers_function_call() {
    let source =
        "fn add(a: int, b: int) -> int { return a + b } fn main() -> int { return add(10, 32) }";
    let ir = lower_source(source);
    assert!(ir.functions.iter().any(|function| {
        function
            .instructions
            .iter()
            .any(|ins| matches!(ins, Instruction::Call { function, .. } if function == "add"))
    }));
}

#[test]
fn lowers_while_to_label_and_jump() {
    let ir = lower_source("fn main() -> int { var x: int = 0 while x < 1 { x = x + 1 } return x }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Label { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Jump { .. })));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::JumpIfZero { .. })));
}

#[test]
fn lowers_for_in_integer_range_loop_to_control_flow() {
    let ir = lower_source(
        r#"
            fn main() -> int {
                var total: int = 0
                for i in 0..4 {
                    total += i
                }
                return total
            }
        "#,
    );

    let function = &ir.functions[0];
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "i")));
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Cmp { .. })));
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::JumpIfZero { .. })));
}

#[test]
fn lowers_for_in_inclusive_integer_range_loop_to_less_equal_control_flow() {
    let ir = lower_source(
        r#"
            fn main() -> int {
                var total: int = 0
                for i in 0..=4 {
                    total += i
                }
                return total
            }
        "#,
    );

    let function = &ir.functions[0];
    assert!(function.instructions.iter().any(|ins| matches!(
        ins,
        Instruction::Cmp {
            op: geo::ir::CmpOp::LessEqual,
            ..
        }
    )));
}

#[test]
fn lowers_unconditional_loop_to_control_flow() {
    let ir = lower_source(
        r#"
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
        "#,
    );

    let function = &ir.functions[0];
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Label { name } if name.starts_with(".Lloop_"))));
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Jump { label } if label.starts_with(".Lloop_"))));
}

#[test]
fn lowers_unary_not_to_comparison() {
    let ir = lower_source("fn main() -> bool { return !true }");
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Cmp { .. })));
}

#[test]
fn lowers_break_and_continue_to_jumps() {
    let ir = lower_source(
        "fn main() -> int { var x: int = 0 while x < 10 { continue break } return x }",
    );
    let jump_count = ir.functions[0]
        .instructions
        .iter()
        .filter(|ins| matches!(ins, Instruction::Jump { .. }))
        .count();

    assert!(jump_count >= 3);
}

#[test]
fn lowers_string_literal_to_string_const() {
    let ir = lower_source("fn main() -> string { return \"Geo\" }");
    assert!(ir.functions[0].instructions.iter().any(|ins| {
        matches!(
        ins,
        Instruction::StringConst { label, value, .. }
            if label == "__geo_str_main_0" && value == "Geo"
        )
    }));
}

#[test]
fn lowers_unit_main_to_default_zero_return() {
    let ir = lower_source(
        r#"
            import std.io

            fn main() {
                println("Hello, world!")
            }
        "#,
    );

    assert!(matches!(
        ir.functions[0].instructions.last(),
        Some(Instruction::Return { .. })
    ));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Call { function, .. } if function == "println")));
}

#[test]
fn lowers_string_concat_operator_to_runtime_call() {
    let ir = lower_source(
        r#"
            fn greet(name: str) -> str {
                return "Hello, " + name
            }

            fn main() -> int {
                let message = greet("world")
                return 0
            }
        "#,
    );

    assert!(ir.functions[0].instructions.iter().any(|ins| {
        matches!(ins, Instruction::Call { function, .. } if function == "string_concat")
    }));
}

#[test]
fn lowers_string_index_to_runtime_bounds_checked_call() {
    let ir = lower_source(
        r#"
            fn main() -> char {
                let value: string = "Geo"
                return value[0]
            }
        "#,
    );

    assert!(ir.functions[0].instructions.iter().any(|ins| {
        matches!(ins, Instruction::Call { function, .. } if function == "__geo_string_get")
    }));
}

#[test]
fn lowers_struct_and_array_literals_to_scalar_slots() {
    let ir = lower_source(
        r#"
            struct Token {
                kind: int
                start: usize
            }

            fn main() -> int {
                let first: Token = Token { kind: 1 start: 0 }
                let pair: [Token] = [first]
                return pair[0].kind
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "first.kind")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "pair[0].kind")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Load { local, .. } if local == "pair[0].kind")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::BoundsCheck { len: 1, .. })));
}

#[test]
fn lowers_field_and_index_assignment_places_to_scalar_slots() {
    let ir = lower_source(
        r#"
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
        "#,
    );

    let function = &ir.functions[0];
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "token.kind")));
    assert!(function
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Store { local, .. } if local == "values[0]")));
}

#[test]
fn lowers_unsafe_address_of_and_deref() {
    let ir = lower_source(
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

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::AddressOf { local, .. } if local == "x")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Deref { .. })));
}

#[test]
fn lowers_unsafe_pointer_assignment() {
    let ir = lower_source(
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

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::StoreDeref { .. })));
}

#[test]
fn lowers_mutable_reference_assignment() {
    let ir = lower_source(
        r#"
            fn main() -> int {
                var x: int = 1
                let slot: &mut int = &mut x
                *slot = 42
                return x
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::StoreDeref { .. })));
}

#[test]
fn lowers_mutable_reference_compound_assignment_to_deref_op_store() {
    let ir = lower_source(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &mut int = &mut value
                *slot += 41
                return value
            }
        "#,
    );

    let instructions = &ir.functions[0].instructions;
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Deref { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Add { .. })));
    assert!(instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::StoreDeref { .. })));
}

#[test]
fn lowers_reference_borrow_and_safe_deref() {
    let ir = lower_source(
        r#"
            fn main() -> int {
                let x: int = 42
                let shared: &int = &x
                return *shared
            }
        "#,
    );

    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::AddressOf { local, .. } if local == "x")));
    assert!(ir.functions[0]
        .instructions
        .iter()
        .any(|ins| matches!(ins, Instruction::Deref { .. })));
}
