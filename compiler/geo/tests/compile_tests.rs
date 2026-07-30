use geo::driver::{compile_to_asm, CompileConfig};
use geo::lexer::lex;
use geo::lower::lower;
use geo::parser::parse;
use geo::target::Target;
use geo::typecheck::check;
use geo::x86_64::emit_nasm;

fn workspace_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn asm_for(source: &str) -> String {
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    check(&program).unwrap();
    let ir = lower(&program);
    emit_nasm(&ir)
}

#[test]
fn emits_assembly_for_return_42() {
    let asm = asm_for("fn main() -> int { return 42 }");

    assert!(asm.contains("global main"));
    assert!(asm.contains("main:"));
    assert!(asm.contains("mov rax, 42\n    mov [rbp - 8], rax"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_full_width_integer_constant() {
    let asm = asm_for("fn main() -> int { return 4294967297 }");

    assert!(asm.contains("mov rax, 4294967297\n    mov [rbp - 8], rax"));
}

#[test]
fn lowers_dynamic_fixed_array_struct_places() {
    let asm = asm_for(
        r#"
            struct Token {
                kind: int
                value: int
            }

            fn main() -> int {
                var index: usize = 1usize
                var tokens: [Token] = [
                    Token { kind: 1 value: 10 },
                    Token { kind: 2 value: 20 },
                ]
                tokens[index].value = 42
                tokens[index].kind += 3
                if tokens[index].value != 42 {
                    return 1
                }
                return tokens[index].kind
            }
        "#,
    );

    assert!(asm.matches("call __geo_bounds_check").count() >= 4);
    assert!(asm.contains(".Ldynamic_store_next_"));
    assert!(asm.contains(".Ldynamic_load_next_"));
}

#[test]
fn lowers_struct_arguments_as_flattened_abi_values() {
    let asm = asm_for(include_str!("../../../examples/v1/aggregate_argument.geo"));

    assert!(asm.contains("read_value:"));
    assert!(asm.contains("mov [rbp - "));
    assert!(asm.contains("call read_value"));
}

#[test]
fn lowers_fixed_array_arguments_as_flattened_abi_values() {
    let asm = asm_for(include_str!("../../../examples/v1/array_argument.geo"));

    assert!(asm.contains("read_value:"));
    assert!(asm.contains("call read_value"));
}

#[test]
fn lowers_struct_returns_through_hidden_return_buffer() {
    let asm = asm_for(include_str!("../../../examples/v1/aggregate_return.geo"));

    assert!(asm.contains("make_pair:"));
    assert!(asm.contains("call make_pair"));
    assert!(asm.contains("mov [rax], r10"));
    assert!(asm.contains("mov [rax - 8], r10"));
}

#[test]
fn lowers_fixed_array_returns_through_hidden_return_buffer() {
    let asm = asm_for(include_str!("../../../examples/v1/array_return.geo"));

    assert!(asm.contains("make_values:"));
    assert!(asm.contains("call make_values"));
    assert!(asm.contains("mov [rax - 8], r10"));
}

#[test]
fn emits_assembly_for_function_tail_expression_return() {
    let asm = asm_for("fn main() -> int { 42 }");

    assert!(asm.contains("global main"));
    assert!(asm.contains("main:"));
    assert!(asm.contains("mov rax, 42\n    mov [rbp - 8], rax"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_top_level_const_use() {
    let asm = asm_for("const LIMIT: int = 42 fn main() -> int { return LIMIT }");

    assert!(asm.contains("mov rax, 42"));
    assert!(asm.contains("mov [rbp - "));
}

#[test]
fn folds_top_level_const_arithmetic_dependencies_in_assembly() {
    let asm = asm_for(
        r#"
            const BASE: int = 40
            const LIMIT: int = BASE + 2

            fn main() -> int {
                return LIMIT
            }
        "#,
    );

    assert!(asm.contains(", 42"));
    assert!(!asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_enum_variant() {
    let asm = asm_for(
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

    assert!(asm.contains(", 2"));
}

#[test]
fn emits_assembly_for_enum_variant_explicit_discriminant() {
    let asm = asm_for(
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

    assert!(asm.contains(", 42"));
}

#[test]
fn emits_assembly_for_implicit_enum_variant_after_explicit_discriminant() {
    let asm = asm_for(
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

    assert!(asm.contains(", 7"));
}

#[test]
fn emits_assembly_for_match_expression() {
    let asm = asm_for(
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

    assert!(asm.contains(".Lmatch_arm_"));
    assert!(asm.contains(".Lmatch_end_"));
    assert!(asm.contains("setne al"));
}

#[test]
fn emits_assembly_for_if_expression() {
    let asm = asm_for("fn main() -> int { return if true { 7 } else { 9 } }");

    assert!(asm.contains(".Lif_else_"));
    assert!(asm.contains(".Lif_end_"));
    assert!(asm.contains("je .Lif_else_"));
}

#[test]
fn emits_assembly_for_multi_level_else_if_chain() {
    let asm = asm_for(
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

    assert!(asm.contains(".Lelse_"));
    assert!(asm.contains(".Lendif_"));
    assert!(asm.contains(", 42"));
}

#[test]
fn emits_assembly_for_block_expression() {
    let asm = asm_for("fn main() -> int { return { let base: int = 40 base + 2 } }");

    assert!(asm.contains(", 40"));
    assert!(asm.contains("add rax, r10"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_remainder() {
    let asm = asm_for("fn main() -> int { return 10 % 4 }");

    assert!(asm.contains("idiv r10"));
    assert!(asm.contains("mov [rbp - 24], rdx"));
}

#[test]
fn emits_assembly_for_prefixed_and_underscored_integer_literals() {
    let asm = asm_for("fn main() -> int { return 0xff + 0b1010 + 0o755 + 1_000 }");

    assert!(asm.contains(", 255"));
    assert!(asm.contains(", 10"));
    assert!(asm.contains(", 493"));
    assert!(asm.contains(", 1000"));
}

#[test]
fn emits_assembly_for_typed_integer_literal_suffixes() {
    let asm = asm_for("fn main() -> int { return 255u8 as int + 16usize as int }");

    assert!(asm.contains(", 255"));
    assert!(asm.contains(", 16"));
    assert!(asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_type_aliases() {
    let asm = asm_for(
        r#"
            type Byte = u8
            type Word = int

            fn main() -> Word {
                let value: Byte = 42
                return value as Word
            }
        "#,
    );

    assert!(asm.contains(", 42"));
}

#[test]
fn emits_assembly_for_boolean_logic() {
    let asm = asm_for("fn main() -> bool { return true || false && true }");

    assert!(asm.contains("and rax, r10"));
    assert!(asm.contains("or rax, r10"));
}

#[test]
fn emits_assembly_for_bitwise_ops() {
    let asm = asm_for("fn main() -> int { return 10 | 6 ^ 3 & 1 }");

    assert!(asm.contains("and rax, r10"));
    assert!(asm.contains("xor rax, r10"));
    assert!(asm.contains("or rax, r10"));
}

#[test]
fn emits_assembly_for_shift_ops() {
    let asm = asm_for("fn main() -> int { return 1 << 3 >> 1 }");

    assert!(asm.contains("shl rax, cl"));
    assert!(asm.contains("sar rax, cl"));
}

#[test]
fn emits_assembly_for_bitwise_and_shift_compound_assignments() {
    let asm = asm_for(
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
    );

    assert!(asm.contains("and rax, r10"));
    assert!(asm.contains("or rax, r10"));
    assert!(asm.contains("xor rax, r10"));
    assert!(asm.contains("shl rax, cl"));
    assert!(asm.contains("sar rax, cl"));
}

#[test]
fn emits_assembly_for_for_in_integer_range_loop() {
    let asm = asm_for(
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

    assert!(asm.contains("cmp rax, [rbp - "));
    assert!(asm.contains("setl al"));
    assert!(asm.contains("je .Lendfor_"));
    assert!(asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_for_in_inclusive_integer_range_loop() {
    let asm = asm_for(
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

    assert!(asm.contains("cmp rax, [rbp - "));
    assert!(asm.contains("setle al"));
    assert!(asm.contains("je .Lendfor_"));
    assert!(asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_mutable_reference_compound_assignment() {
    let asm = asm_for(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &mut int = &mut value
                *slot += 41
                return value
            }
        "#,
    );

    assert!(asm.contains("add rax, r10"));
    assert!(asm.contains("mov [rax], r10"));
}

#[test]
fn emits_assembly_for_unconditional_loop() {
    let asm = asm_for(
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

    assert!(asm.contains(".Lloop_"));
    assert!(asm.contains("jmp .Lloop_"));
    assert!(asm.contains(".Lendloop_"));
}

#[test]
fn emits_assembly_for_bitwise_not() {
    let asm = asm_for("fn main() -> int { return ~10 }");

    assert!(asm.contains("not rax"));
}

#[test]
fn emits_assembly_for_pointer_to_usize_cast() {
    let asm = asm_for(
        r#"
            fn main() -> usize {
                let ptr: *u8 = null
                return ptr as usize
            }
        "#,
    );

    assert!(asm.contains(", 0"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_usize_to_pointer_cast_in_unsafe() {
    let asm = asm_for(
        r#"
            fn main() -> int {
                let addr: usize = 0
                unsafe {
                    let ptr: *u8 = addr as *u8
                }
                return 0
            }
        "#,
    );

    assert!(asm.contains(", 0"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_raw_pointer_arithmetic() {
    let asm = asm_for(
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

    assert!(asm.contains("imul rax, r10"));
    assert!(asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_raw_pointer_compound_assignment() {
    let asm = asm_for(
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

    assert!(asm.contains("imul rax, r10"));
    assert!(asm.contains("add rax, r10"));
}

#[test]
fn emits_assembly_for_raw_pointer_ordering_comparison() {
    let asm = asm_for(
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

    assert!(asm.contains("setl al"));
}

#[test]
fn emits_assembly_for_sizeof_type() {
    let asm = asm_for(
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

    assert!(asm.contains(", 16"));
}

#[test]
fn emits_assembly_for_alignof_type() {
    let asm = asm_for(
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

    assert!(asm.contains(", 8"));
}

#[test]
fn emits_assembly_for_offsetof_field() {
    let asm = asm_for(
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

    assert!(asm.contains(", 8"));
}

#[test]
fn emits_assembly_for_null_literal() {
    let asm = asm_for(
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

    assert!(asm.contains(", 0"));
    assert!(asm.contains("sete al"));
}

#[test]
fn emits_assembly_for_null_comparison_with_pointer_on_right() {
    let asm = asm_for(
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

    assert!(asm.contains("sete al"));
    assert!(asm.contains(", 42"));
}

#[test]
fn emits_calls_and_stack_locals() {
    let source = r#"
        fn add(a: int, b: int) -> int {
            return a + b
        }

        fn main() -> int {
            let x: int = 10
            let y: int = 32
            return add(x, y)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("add:"));
    assert!(asm.contains("call add"));
    assert!(asm.contains("mov rdi"));
    assert!(asm.contains("mov rsi"));
}

#[test]
fn compiles_a_package_directory_through_its_main_entry() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let asm = compile_to_asm(&workspace_path("examples/modules"), &config).unwrap();

    assert!(asm.contains("add:"));
    assert!(asm.contains("call add"));
}

#[test]
fn rejects_calls_to_private_imported_functions() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let diagnostics = compile_to_asm(&workspace_path("examples/modules_private"), &config)
        .expect_err("private imported functions must not be callable");

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("unknown function 'math.hidden'")));
}

#[test]
fn rejects_unqualified_calls_to_private_imported_functions() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let diagnostics = compile_to_asm(
        &workspace_path("examples/modules_private_unqualified"),
        &config,
    )
    .expect_err("private imported functions must not enter the caller scope");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown function 'hidden'")));
}

#[test]
fn rejects_private_imported_types() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let diagnostics = compile_to_asm(&workspace_path("examples/modules_private_types"), &config)
        .expect_err("private imported types must not be visible");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown type 'types.Secret'")));
}

#[test]
fn rejects_private_imported_struct_fields() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let diagnostics = compile_to_asm(&workspace_path("examples/modules_private_fields"), &config)
        .expect_err("private imported fields must not be readable");

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("field 'secret' on struct 'Data' is private")));
}

#[test]
fn rejects_unqualified_private_imported_data() {
    let config = CompileConfig {
        target: Target::parse("x86_64-linux").unwrap(),
        runtime_entry: false,
    };
    let diagnostics = compile_to_asm(
        &workspace_path("examples/modules_private_data_unqualified"),
        &config,
    )
    .expect_err("private imported data must not enter the caller scope");

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown type 'Secret'")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unknown variable 'HIDDEN'")));
}

#[test]
fn emits_data_for_string_literal_calls() {
    let source = r#"
        import std.io
        import std.string

        fn main() -> int {
            println("Geo")
            return 0
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("section .data"));
    assert!(asm.contains("extern println"));
    assert!(asm.contains("__geo_str_main_0: db 71, 101, 111, 0"));
    assert!(asm.contains("lea rax, [rel __geo_str_main_0]"));
    assert!(asm.contains("call println"));
}

#[test]
fn emits_assembly_for_scalar_slot_aggregates() {
    let source = r#"
        struct Token {
            kind: int
            start: usize
        }

        fn main() -> int {
            let first: Token = Token { kind: 1 start: 0 }
            let pair: [Token] = [first]
            return pair[0].kind
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("main:"));
    assert!(asm.contains("call __geo_bounds_check"));
    assert!(asm.contains("mov rax, 1"));
    assert!(asm.contains("mov [rbp - "));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_comma_separated_struct_declaration_fields() {
    let asm = asm_for(
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

    assert!(asm.contains("main:"));
    assert!(asm.contains(", 42"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_struct_literal_field_shorthand() {
    let asm = asm_for(
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

    assert!(asm.contains("main:"));
    assert!(asm.contains(", 42"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_trailing_commas_in_params_calls_and_arrays() {
    let asm = asm_for(
        r#"
            fn add(a: int, b: int,) -> int {
                return a + b
            }

            fn main() -> int {
                let values: [int] = [40, 2,]
                return add(values[0], values[1],)
            }
        "#,
    );

    assert!(asm.contains("call add"));
    assert!(asm.contains(", 40"));
    assert!(asm.contains(", 2"));
}

#[test]
fn emits_assembly_for_field_and_index_assignment_places() {
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
    let asm = asm_for(source);

    assert!(asm.contains("    mov rax, 2"));
    assert!(asm.contains("    mov [rbp - "));
    assert!(asm.contains("    add rax, r10"));
    assert!(asm.contains("ret"));
}

#[test]
fn emits_assembly_for_unsafe_pointer_local_access() {
    let source = r#"
        fn main() -> int {
            let x: int = 42
            unsafe {
                let p: *int = &x
                return *p
            }
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("    lea rax, [rbp - "));
    assert!(asm.contains("    mov rax, [rax]"));
}

#[test]
fn emits_assembly_for_unsafe_pointer_assignment() {
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
    let asm = asm_for(source);

    assert!(asm.contains("    mov rax, [rbp - "));
    assert!(asm.contains("    mov r10, [rbp - "));
    assert!(asm.contains("    mov [rax], r10"));
}

#[test]
fn emits_assembly_for_mutable_reference_assignment() {
    let source = r#"
        fn main() -> int {
            var x: int = 1
            let slot: &mut int = &mut x
            *slot = 42
            return x
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("    mov rax, [rbp - "));
    assert!(asm.contains("    mov r10, [rbp - "));
    assert!(asm.contains("    mov [rax], r10"));
}

#[test]
fn emits_runtime_symbols_for_libc_conflicting_std_calls() {
    let source = r#"
        import std.mem
        import std.process

        fn main() -> int {
            let ptr: *u8 = alloc(8)
            let zeroed: *u8 = alloc_zeroed(8)
            let array: *u8 = alloc_array(2, 4)
            let copied: *u8 = alloc_copy(ptr, 8)
            let grown: *u8 = realloc_array(array, 2, 8)
            unsafe {
                mem_equal(ptr, ptr, 8)
                mem_is_zero(zeroed, 8)
                mem_is_zero(ptr, 8)
                mem_count(ptr, 8, 65)
                mem_contains(ptr, 8, 65)
                mem_all(ptr, 8, 65)
                mem_any(ptr, 8, 65)
                mem_leading_count(ptr, 8, 65)
                mem_trailing_count(ptr, 8, 65)
                mem_trimmed_len(ptr, 8, 65)
                mem_last_find(ptr, 8, 65)
                mem_find_pattern(ptr, 8, ptr, 2)
                mem_last_find_pattern(ptr, 8, ptr, 2)
                mem_count_pattern(ptr, 8, ptr, 2)
                mem_split_count(ptr, 8, 65)
                mem_split_count_pattern(ptr, 8, ptr, 2)
                mem_split_field_start(ptr, 8, 65, 0)
                mem_split_field_len(ptr, 8, 65, 0)
                mem_split_field_start_pattern(ptr, 8, ptr, 2, 0)
                mem_split_field_len_pattern(ptr, 8, ptr, 2, 0)
                mem_line_count(ptr, 8)
                mem_line_start(ptr, 8, 0)
                mem_line_len(ptr, 8, 0)
                mem_line_index_at(ptr, 8, 0)
                mem_column_at(ptr, 8, 0)
                mem_offset_at_line_column(ptr, 8, 0, 0)
                mem_hash(ptr, 8)
                mem_hash_seed(ptr, 8, 12345)
                mem_starts_with(ptr, 8, ptr, 4)
                mem_ends_with(ptr, 8, ptr + 4, 4)
                mem_replace_byte(ptr, 8, 65, 66)
                mem_replace_pattern(ptr, 8, ptr, 2, ptr + 2, 2)
                mem_xor_byte(ptr, 8, 255)
                mem_repeat_pattern(ptr, 8, ptr, 2)
                mem_rotate_left(ptr, 8, 3)
                mem_rotate_right(ptr, 8, 3)
                return mem_fill(ptr, 8, 65) + mem_copy(ptr + 4, ptr, 4) + mem_move(ptr + 1, ptr, 4) + mem_swap(ptr, ptr + 4, 4) + mem_reverse(ptr, 8) + mem_find(ptr, 8, 65) + mem_compare(ptr, ptr, 8) + free(grown) + free(copied) + free(zeroed) + free(ptr) + exit(0)
            }
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call alloc_zeroed"));
    assert!(asm.contains("call alloc_array"));
    assert!(asm.contains("call alloc_copy"));
    assert!(asm.contains("call realloc_array"));
    assert!(asm.contains("call mem_fill"));
    assert!(asm.contains("call mem_copy"));
    assert!(asm.contains("call mem_move"));
    assert!(asm.contains("call mem_swap"));
    assert!(asm.contains("call mem_reverse"));
    assert!(asm.contains("call mem_replace_byte"));
    assert!(asm.contains("call mem_replace_pattern"));
    assert!(asm.contains("call mem_xor_byte"));
    assert!(asm.contains("call mem_repeat_pattern"));
    assert!(asm.contains("call mem_rotate_left"));
    assert!(asm.contains("call mem_rotate_right"));
    assert!(asm.contains("call mem_find"));
    assert!(asm.contains("call mem_last_find"));
    assert!(asm.contains("call mem_find_pattern"));
    assert!(asm.contains("call mem_last_find_pattern"));
    assert!(asm.contains("call mem_count_pattern"));
    assert!(asm.contains("call mem_split_count"));
    assert!(asm.contains("call mem_split_count_pattern"));
    assert!(asm.contains("call mem_split_field_start"));
    assert!(asm.contains("call mem_split_field_len"));
    assert!(asm.contains("call mem_split_field_start_pattern"));
    assert!(asm.contains("call mem_split_field_len_pattern"));
    assert!(asm.contains("call mem_line_count"));
    assert!(asm.contains("call mem_line_start"));
    assert!(asm.contains("call mem_line_len"));
    assert!(asm.contains("call mem_line_index_at"));
    assert!(asm.contains("call mem_column_at"));
    assert!(asm.contains("call mem_offset_at_line_column"));
    assert!(asm.contains("call mem_hash"));
    assert!(asm.contains("call mem_hash_seed"));
    assert!(asm.contains("call mem_count"));
    assert!(asm.contains("call mem_contains"));
    assert!(asm.contains("call mem_all"));
    assert!(asm.contains("call mem_any"));
    assert!(asm.contains("call mem_leading_count"));
    assert!(asm.contains("call mem_trailing_count"));
    assert!(asm.contains("call mem_trimmed_len"));
    assert!(asm.contains("call mem_starts_with"));
    assert!(asm.contains("call mem_ends_with"));
    assert!(asm.contains("call mem_compare"));
    assert!(asm.contains("call mem_equal"));
    assert!(asm.contains("call mem_is_zero"));
    assert!(asm.contains("call free_geo"));
    assert!(asm.contains("call exit_geo"));
}

#[test]
fn emits_mem_alignment_runtime_calls() {
    let source = r#"
        import std.mem

        fn main() -> bool {
            return align_up(17usize, 8usize) == 24usize && align_down(17usize, 8usize) == 16usize && is_aligned(16usize, 8usize)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call align_up"));
    assert!(asm.contains("call align_down"));
    assert!(asm.contains("call is_aligned"));
}

#[test]
fn emits_process_arg_exists_runtime_call() {
    let source = r#"
        import std.process

        fn main() -> bool {
            return arg_exists(0)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call arg_exists"));
}

#[test]
fn emits_process_arg_or_runtime_call() {
    let source = r#"
        import std.process
        import std.string

        fn main() -> usize {
            let value: string = arg_or(1, "default")
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call arg_or"));
}

#[test]
fn emits_process_env_exists_runtime_call() {
    let source = r#"
        import std.process

        fn main() -> bool {
            return env_exists("GEO_TEST_ENV")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call env_exists"));
}

#[test]
fn emits_process_env_get_or_runtime_call() {
    let source = r#"
        import std.process
        import std.string

        fn main() -> usize {
            let value: string = env_get_or("GEO_TEST_ENV", "default")
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call env_get_or"));
}

#[test]
fn emits_process_env_mutation_runtime_calls() {
    let source = r#"
        import std.process

        fn main() -> int {
            return env_set("GEO_TEST_ENV", "value") + env_remove("GEO_TEST_ENV")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call env_set"));
    assert!(asm.contains("call env_remove"));
}

#[test]
fn emits_process_env_iteration_runtime_calls() {
    let source = r#"
        import std.process
        import std.string

        fn main() -> usize {
            let count: usize = env_count()
            let name: string = env_name(0)
            let value: string = env_value(0)
            return count + string_len(name) + string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call env_count"));
    assert!(asm.contains("call env_name"));
    assert!(asm.contains("call env_value"));
}

#[test]
fn emits_process_current_exe_runtime_call() {
    let source = r#"
        import std.process
        import std.string

        fn main() -> usize {
            let exe: string = current_exe()
            return string_len(exe)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call current_exe"));
}

#[test]
fn emits_process_id_runtime_call() {
    let source = r#"
        import std.process

        fn main() -> usize {
            return process_id()
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call process_id"));
}

#[test]
fn emits_process_run_command_runtime_call() {
    let source = r#"
        import std.process

        fn main() -> int {
            return run_command("exit 7")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call run_command"));
}

#[test]
fn emits_platform_runtime_calls() {
    let source = r#"
        import std.platform
        import std.string

        fn main() -> int {
            let os: string = platform_os()
            let arch: string = platform_arch()
            let sep: char = platform_path_separator()
            let newline: string = platform_newline()
            let temp: string = temp_dir()
            let home: string = home_dir()
            let user: string = user_name()
            let cpus: usize = cpu_count()
            let joined: string = path_join(temp, "geo")
            let file: string = path_file_name(joined)
            let parent: string = path_parent(joined)
            let ext: string = path_extension("main.geo")
            let stem: string = path_stem("main.geo")
            let absolute: bool = path_is_absolute(joined)
            let without_ext: string = path_without_extension("main.geo")
            let with_ext: string = path_with_extension("main.geo", "asm")
            let normalized: string = path_normalize("src/./lib/../main.geo")
            let unix_path: string = path_to_unix("src\\main.geo")
            let windows_path: string = path_to_windows("src/main.geo")
            let absolute_path: string = path_absolute("src/./main.geo")
            let cwd: string = current_dir()
            string_len(file) + string_len(parent) + string_len(ext) + string_len(stem) + string_len(without_ext) + string_len(with_ext) + string_len(normalized) + string_len(unix_path) + string_len(windows_path) + string_len(absolute_path) + string_len(home) + string_len(user) + string_len(arch) + cpus
            if absolute {
                string_len(joined)
            }
            change_dir(cwd)
            return 0
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call platform_os"));
    assert!(asm.contains("call platform_arch"));
    assert!(asm.contains("call platform_path_separator"));
    assert!(asm.contains("call platform_newline"));
    assert!(asm.contains("call temp_dir"));
    assert!(asm.contains("call home_dir"));
    assert!(asm.contains("call user_name"));
    assert!(asm.contains("call cpu_count"));
    assert!(asm.contains("call path_join"));
    assert!(asm.contains("call path_file_name"));
    assert!(asm.contains("call path_parent"));
    assert!(asm.contains("call path_extension"));
    assert!(asm.contains("call path_stem"));
    assert!(asm.contains("call path_is_absolute"));
    assert!(asm.contains("call path_without_extension"));
    assert!(asm.contains("call path_with_extension"));
    assert!(asm.contains("call path_normalize"));
    assert!(asm.contains("call path_to_unix"));
    assert!(asm.contains("call path_to_windows"));
    assert!(asm.contains("call path_absolute"));
    assert!(asm.contains("call current_dir"));
    assert!(asm.contains("call change_dir"));
}

#[test]
fn emits_random_runtime_calls() {
    let source = r#"
        import std.random

        fn main() -> usize {
            random_seed(123)
            let flag: bool = random_bool()
            let signed: int = random_int_range(-5, 5)
            if flag {
                return random_usize() + random_range(10) + random_range_inclusive(10) + signed as usize
            }
            return random_usize() + random_range(10) + random_range_inclusive(10)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call random_seed"));
    assert!(asm.contains("call random_usize"));
    assert!(asm.contains("call random_range"));
    assert!(asm.contains("call random_range_inclusive"));
    assert!(asm.contains("call random_bool"));
    assert!(asm.contains("call random_int_range"));
}

#[test]
fn emits_hash_runtime_calls() {
    let source = r#"
        import std.hash
        import std.mem

        fn main() -> usize {
            let ptr: *u8 = alloc(3)
            let left: usize = hash_string("geo")
            let right: usize = hash_usize(42)
            let bytes: usize = hash_bytes(ptr, 3usize)
            let seeded: usize = hash_bytes_seed(ptr, 3usize, left)
            free(ptr)
            return hash_combine(hash_combine(left, right), hash_combine(bytes, seeded))
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call hash_string"));
    assert!(asm.contains("call hash_usize"));
    assert!(asm.contains("call hash_bytes"));
    assert!(asm.contains("call hash_bytes_seed"));
    assert!(asm.contains("call hash_combine"));
}

#[test]
fn emits_array_runtime_calls() {
    let source = r#"
        import std.array

        fn main() -> usize {
            let items: *u8 = array_new(1, 2)
            let value: u8 = 7
            unsafe {
                array_clone(items)
                array_is_empty(items)
                array_reserve(items, 4usize)
                array_push(items, &value)
                array_set(items, 0usize, &value)
                array_fill(items, &value)
                array_extend(items, items)
                array_copy(items, 0usize, items, 0usize, 1usize)
                array_resize(items, 1usize, &value)
                array_get(items, 0usize)
                array_first(items)
                array_last(items)
                array_index_of(items, &value)
                array_last_index_of(items, &value)
                array_contains(items, &value)
                array_count(items, &value)
                array_insert(items, 0usize, &value)
                array_swap(items, 0usize, 1usize)
                array_reverse(items)
                array_remove(items, 0usize)
                array_swap_remove(items, 0usize)
                array_pop_first(items)
                array_truncate(items, 0usize)
                array_clear(items)
            }
            return array_len(items) + array_capacity(items)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call array_new"));
    assert!(asm.contains("call array_clone"));
    assert!(asm.contains("call array_is_empty"));
    assert!(asm.contains("call array_reserve"));
    assert!(asm.contains("call array_push"));
    assert!(asm.contains("call array_set"));
    assert!(asm.contains("call array_fill"));
    assert!(asm.contains("call array_extend"));
    assert!(asm.contains("call array_copy"));
    assert!(asm.contains("call array_resize"));
    assert!(asm.contains("call array_get"));
    assert!(asm.contains("call array_first"));
    assert!(asm.contains("call array_last"));
    assert!(asm.contains("call array_index_of"));
    assert!(asm.contains("call array_last_index_of"));
    assert!(asm.contains("call array_contains"));
    assert!(asm.contains("call array_count"));
    assert!(asm.contains("call array_insert"));
    assert!(asm.contains("call array_swap"));
    assert!(asm.contains("call array_reverse"));
    assert!(asm.contains("call array_remove"));
    assert!(asm.contains("call array_swap_remove"));
    assert!(asm.contains("call array_pop_first"));
    assert!(asm.contains("call array_truncate"));
    assert!(asm.contains("call array_clear"));
    assert!(asm.contains("call array_len"));
    assert!(asm.contains("call array_capacity"));
}

#[test]
fn emits_time_runtime_calls() {
    let source = r#"
        import std.time

        fn main() -> usize {
            let secs: usize = unix_time_secs()
            let millis: usize = unix_time_millis()
            let micros: usize = unix_time_micros()
            let nanos: usize = unix_time_nanos()
            let monotonic: usize = monotonic_millis()
            let monotonic_us: usize = monotonic_micros()
            let monotonic_ns: usize = monotonic_nanos()
            sleep_millis(0)
            return secs + millis + micros + nanos + monotonic + monotonic_us + monotonic_ns
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call unix_time_secs"));
    assert!(asm.contains("call unix_time_millis"));
    assert!(asm.contains("call unix_time_micros"));
    assert!(asm.contains("call unix_time_nanos"));
    assert!(asm.contains("call monotonic_millis"));
    assert!(asm.contains("call monotonic_micros"));
    assert!(asm.contains("call monotonic_nanos"));
    assert!(asm.contains("call sleep_millis"));
}

#[test]
fn emits_math_integer_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> int {
            return int_abs(-7) + int_abs_diff(-7, 3) as int + int_min(9, 4) + int_max(9, 4) + int_clamp(12, 0, 10) + int_div_floor(-7, 3) + int_div_ceil(7, 3) + int_div_euclid(-7, 3) + int_rem_floor(-7, 3) + int_rem_euclid(-7, 3) + int_checked_add(20, 22) + int_checked_sub(50, 8) + int_checked_mul(6, 7) + int_checked_div(84, 2) + int_checked_rem(85, 43) + int_checked_neg(-42) + int_checked_abs(-42) + int_saturating_add(20, 22) + int_saturating_sub(50, 8) + int_saturating_mul(6, 7) + int_saturating_abs(-42) + int_saturating_neg(-42) + int_wrapping_add(20, 22) + int_wrapping_sub(50, 8) + int_wrapping_mul(6, 7) + int_wrapping_neg(-42) + int_wrapping_abs(-42)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_abs"));
    assert!(asm.contains("call int_abs_diff"));
    assert!(asm.contains("call int_min"));
    assert!(asm.contains("call int_max"));
    assert!(asm.contains("call int_clamp"));
    assert!(asm.contains("call int_div_floor"));
    assert!(asm.contains("call int_div_ceil"));
    assert!(asm.contains("call int_div_euclid"));
    assert!(asm.contains("call int_rem_floor"));
    assert!(asm.contains("call int_rem_euclid"));
    assert!(asm.contains("call int_checked_add"));
    assert!(asm.contains("call int_checked_sub"));
    assert!(asm.contains("call int_checked_mul"));
    assert!(asm.contains("call int_checked_div"));
    assert!(asm.contains("call int_checked_rem"));
    assert!(asm.contains("call int_checked_neg"));
    assert!(asm.contains("call int_checked_abs"));
    assert!(asm.contains("call int_saturating_add"));
    assert!(asm.contains("call int_saturating_sub"));
    assert!(asm.contains("call int_saturating_mul"));
    assert!(asm.contains("call int_saturating_abs"));
    assert!(asm.contains("call int_saturating_neg"));
    assert!(asm.contains("call int_wrapping_add"));
    assert!(asm.contains("call int_wrapping_sub"));
    assert!(asm.contains("call int_wrapping_mul"));
    assert!(asm.contains("call int_wrapping_neg"));
    assert!(asm.contains("call int_wrapping_abs"));
}

#[test]
fn emits_math_usize_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> usize {
            return usize_min(9, 4) + usize_max(9, 4) + usize_clamp(12, 0, 10) + usize_abs_diff(3, 7) + usize_checked_add(20, 22) + usize_checked_sub(50, 8) + usize_checked_mul(6, 7) + usize_checked_div(84, 2) + usize_checked_rem(85, 43) + usize_saturating_add(20, 22) + usize_saturating_sub(50, 8) + usize_saturating_mul(6, 7) + usize_wrapping_add(20, 22) + usize_wrapping_sub(50, 8) + usize_wrapping_mul(6, 7)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call usize_min"));
    assert!(asm.contains("call usize_max"));
    assert!(asm.contains("call usize_clamp"));
    assert!(asm.contains("call usize_abs_diff"));
    assert!(asm.contains("call usize_checked_add"));
    assert!(asm.contains("call usize_checked_sub"));
    assert!(asm.contains("call usize_checked_mul"));
    assert!(asm.contains("call usize_checked_div"));
    assert!(asm.contains("call usize_checked_rem"));
    assert!(asm.contains("call usize_saturating_add"));
    assert!(asm.contains("call usize_saturating_sub"));
    assert!(asm.contains("call usize_saturating_mul"));
    assert!(asm.contains("call usize_wrapping_add"));
    assert!(asm.contains("call usize_wrapping_sub"));
    assert!(asm.contains("call usize_wrapping_mul"));
}

#[test]
fn emits_math_power_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> int {
            return int_pow(-2, 3) + int_checked_pow(-2, 3) + int_saturating_pow(-2, 3) + int_wrapping_pow(-2, 3) + usize_pow(2, 4) as int + usize_checked_pow(2, 4) as int + usize_saturating_pow(2, 4) as int + usize_wrapping_pow(2, 4) as int
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_pow"));
    assert!(asm.contains("call int_checked_pow"));
    assert!(asm.contains("call int_saturating_pow"));
    assert!(asm.contains("call int_wrapping_pow"));
    assert!(asm.contains("call usize_pow"));
    assert!(asm.contains("call usize_checked_pow"));
    assert!(asm.contains("call usize_saturating_pow"));
    assert!(asm.contains("call usize_wrapping_pow"));
}

#[test]
fn emits_math_gcd_lcm_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> int {
            return int_gcd(-54, 24) + int_lcm(-6, 8) + usize_gcd(54, 24) as int + usize_lcm(6, 8) as int
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_gcd"));
    assert!(asm.contains("call int_lcm"));
    assert!(asm.contains("call usize_gcd"));
    assert!(asm.contains("call usize_lcm"));
}

#[test]
fn emits_math_parity_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> int {
            if int_is_even(-4) && int_is_odd(-3) && int_is_power_of_two(16) && int_prev_power_of_two(31) == 16 && int_next_power_of_two(17) == 32 && int_checked_next_power_of_two(17) == 32 && int_saturating_next_power_of_two(17) == 32 && int_align_up(13, 8) == 16 && int_align_down(15, 8) == 8 && int_align_up_saturating(17, 8) == 24 && usize_is_even(10) && usize_is_odd(11) && usize_is_power_of_two(16) && usize_next_power_of_two(17) == 32 && usize_checked_next_power_of_two(17) == 32 && usize_saturating_next_power_of_two(17) == 32 && usize_prev_power_of_two(31) == 16 && usize_align_up(13, 8) == 16 && usize_align_down(15, 8) == 8 && usize_align_up_saturating(17, 8) == 24 && usize_div_ceil(17, 8) == 3 {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_is_even"));
    assert!(asm.contains("call int_is_odd"));
    assert!(asm.contains("call int_is_power_of_two"));
    assert!(asm.contains("call int_prev_power_of_two"));
    assert!(asm.contains("call int_next_power_of_two"));
    assert!(asm.contains("call int_checked_next_power_of_two"));
    assert!(asm.contains("call int_saturating_next_power_of_two"));
    assert!(asm.contains("call int_align_up"));
    assert!(asm.contains("call int_align_down"));
    assert!(asm.contains("call int_align_up_saturating"));
    assert!(asm.contains("call usize_is_even"));
    assert!(asm.contains("call usize_is_odd"));
    assert!(asm.contains("call usize_is_power_of_two"));
    assert!(asm.contains("call usize_next_power_of_two"));
    assert!(asm.contains("call usize_checked_next_power_of_two"));
    assert!(asm.contains("call usize_saturating_next_power_of_two"));
    assert!(asm.contains("call usize_prev_power_of_two"));
    assert!(asm.contains("call usize_align_up"));
    assert!(asm.contains("call usize_align_down"));
    assert!(asm.contains("call usize_align_up_saturating"));
    assert!(asm.contains("call usize_div_ceil"));
}

#[test]
fn emits_math_sign_runtime_calls() {
    let source = r#"
        import std.math

        fn main() -> int {
            if int_is_positive(7) && int_is_negative(-7) {
                return int_signum(9) + int_signum(-9) + int_signum(0)
            }
            return 99
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_is_positive"));
    assert!(asm.contains("call int_is_negative"));
    assert!(asm.contains("call int_signum"));
}

#[test]
fn emits_bits_runtime_calls() {
    let source = r#"
        import std.bits

        fn main() -> usize {
            if !usize_bit_is_set(10, 1) {
                return 0
            }
            if !int_bit_is_set(-1, 63) {
                return 0
            }
            if !int_parity(13) {
                return 0
            }
            if usize_parity(15) {
                return 0
            }
            if !int_bits_contains_all(14, 6) || int_bits_contains_all(8, 6) {
                return 0
            }
            if !int_bits_disjoint(8, 6) || int_bits_disjoint(14, 6) {
                return 0
            }
            if !usize_bits_contains_all(14, 6) || usize_bits_contains_all(8, 6) {
                return 0
            }
            if !usize_bits_disjoint(8, 6) || usize_bits_disjoint(14, 6) {
                return 0
            }
            return int_popcount(-1) + int_count_ones(-1) + int_count_zeros(13) + int_leading_zeros(1) + int_leading_ones(-1) + int_trailing_zeros(8) + int_trailing_ones(-1) + int_bit_width(-1) + int_lowest_one(40) as usize + int_highest_one(40) as usize + int_clear_lowest_one(40) as usize + int_clear_highest_one(40) as usize + int_fill_ones_below(40) as usize + int_fill_ones_above(40) as usize + int_bit_set(8, 1) as usize + int_low_mask(4) as usize + int_range_mask(4, 3) as usize + int_sign_extend(128, 8) as usize + int_extract_bits(0x5a, 1, 4) as usize + int_insert_bits(0, 13, 1, 4) as usize + int_byte_at(0x0102030405060708, 0) as usize + int_with_byte(0x0102030405060708, 0, 255) as usize + int_bit_clear(10, 1) as usize + int_bit_toggle(8, 1) as usize + int_reverse_bits(1) as usize + int_swap_bytes(0x0102030405060708) as usize + int_from_be(0x0102030405060708) as usize + int_from_le(0x0102030405060708) as usize + int_to_be(0x0102030405060708) as usize + int_to_le(0x0102030405060708) as usize + int_rotate_left(1, 3) as usize + int_rotate_right(8, 3) as usize + int_checked_shl(1, 3) as usize + int_checked_shr(8, 3) as usize + int_wrapping_shl(1, 65) as usize + int_wrapping_shr(8, 65) as usize + int_arithmetic_shr(-8, 1) as usize + usize_popcount(13) + usize_count_ones(13) + usize_count_zeros(13) + usize_leading_zeros(1) + usize_leading_ones(usize_low_mask(64)) + usize_trailing_zeros(8) + usize_trailing_ones(usize_low_mask(64)) + usize_reverse_bits(1) + usize_swap_bytes(0x0102030405060708) + usize_from_be(0x0102030405060708) + usize_from_le(0x0102030405060708) + usize_to_be(0x0102030405060708) + usize_to_le(0x0102030405060708) + usize_bit_width(8) + usize_lowest_one(40) + usize_highest_one(40) + usize_clear_lowest_one(40) + usize_clear_highest_one(40) + usize_fill_ones_below(40) + usize_fill_ones_above(40) + usize_bit_set(8, 1) + usize_low_mask(4) + usize_range_mask(4, 3) + usize_extract_bits(0x5a, 1, 4) + usize_insert_bits(0, 13, 1, 4) + usize_byte_at(0x0102030405060708, 0) as usize + usize_with_byte(0x0102030405060708, 0, 255) + usize_bit_clear(10, 1) + usize_bit_toggle(8, 1) + usize_rotate_left(1, 3) + usize_rotate_right(8, 3) + usize_checked_shl(1, 3) + usize_checked_shr(8, 3) + usize_wrapping_shl(1, 65) + usize_wrapping_shr(8, 65)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call usize_bit_is_set"));
    assert!(asm.contains("call int_bit_is_set"));
    assert!(asm.contains("call int_bits_contains_all"));
    assert!(asm.contains("call int_bits_disjoint"));
    assert!(asm.contains("call usize_bits_contains_all"));
    assert!(asm.contains("call usize_bits_disjoint"));
    assert!(asm.contains("call int_parity"));
    assert!(asm.contains("call usize_parity"));
    assert!(asm.contains("call int_popcount"));
    assert!(asm.contains("call int_count_ones"));
    assert!(asm.contains("call int_count_zeros"));
    assert!(asm.contains("call int_leading_zeros"));
    assert!(asm.contains("call int_leading_ones"));
    assert!(asm.contains("call int_trailing_zeros"));
    assert!(asm.contains("call int_trailing_ones"));
    assert!(asm.contains("call int_bit_width"));
    assert!(asm.contains("call int_lowest_one"));
    assert!(asm.contains("call int_highest_one"));
    assert!(asm.contains("call int_clear_lowest_one"));
    assert!(asm.contains("call int_clear_highest_one"));
    assert!(asm.contains("call int_fill_ones_below"));
    assert!(asm.contains("call int_fill_ones_above"));
    assert!(asm.contains("call int_bit_set"));
    assert!(asm.contains("call int_low_mask"));
    assert!(asm.contains("call int_range_mask"));
    assert!(asm.contains("call int_sign_extend"));
    assert!(asm.contains("call int_extract_bits"));
    assert!(asm.contains("call int_insert_bits"));
    assert!(asm.contains("call int_byte_at"));
    assert!(asm.contains("call int_with_byte"));
    assert!(asm.contains("call int_bit_clear"));
    assert!(asm.contains("call int_bit_toggle"));
    assert!(asm.contains("call int_reverse_bits"));
    assert!(asm.contains("call int_swap_bytes"));
    assert!(asm.contains("call int_from_be"));
    assert!(asm.contains("call int_from_le"));
    assert!(asm.contains("call int_to_be"));
    assert!(asm.contains("call int_to_le"));
    assert!(asm.contains("call int_rotate_left"));
    assert!(asm.contains("call int_rotate_right"));
    assert!(asm.contains("call int_checked_shl"));
    assert!(asm.contains("call int_checked_shr"));
    assert!(asm.contains("call int_wrapping_shl"));
    assert!(asm.contains("call int_wrapping_shr"));
    assert!(asm.contains("call int_arithmetic_shr"));
    assert!(asm.contains("call usize_popcount"));
    assert!(asm.contains("call usize_count_ones"));
    assert!(asm.contains("call usize_count_zeros"));
    assert!(asm.contains("call usize_leading_zeros"));
    assert!(asm.contains("call usize_leading_ones"));
    assert!(asm.contains("call usize_trailing_zeros"));
    assert!(asm.contains("call usize_trailing_ones"));
    assert!(asm.contains("call usize_reverse_bits"));
    assert!(asm.contains("call usize_swap_bytes"));
    assert!(asm.contains("call usize_from_be"));
    assert!(asm.contains("call usize_from_le"));
    assert!(asm.contains("call usize_to_be"));
    assert!(asm.contains("call usize_to_le"));
    assert!(asm.contains("call usize_bit_width"));
    assert!(asm.contains("call usize_lowest_one"));
    assert!(asm.contains("call usize_highest_one"));
    assert!(asm.contains("call usize_clear_lowest_one"));
    assert!(asm.contains("call usize_clear_highest_one"));
    assert!(asm.contains("call usize_fill_ones_below"));
    assert!(asm.contains("call usize_fill_ones_above"));
    assert!(asm.contains("call usize_bit_set"));
    assert!(asm.contains("call usize_low_mask"));
    assert!(asm.contains("call usize_range_mask"));
    assert!(asm.contains("call usize_extract_bits"));
    assert!(asm.contains("call usize_insert_bits"));
    assert!(asm.contains("call usize_byte_at"));
    assert!(asm.contains("call usize_with_byte"));
    assert!(asm.contains("call usize_bit_clear"));
    assert!(asm.contains("call usize_bit_toggle"));
    assert!(asm.contains("call usize_rotate_left"));
    assert!(asm.contains("call usize_rotate_right"));
    assert!(asm.contains("call usize_checked_shl"));
    assert!(asm.contains("call usize_checked_shr"));
    assert!(asm.contains("call usize_wrapping_shl"));
    assert!(asm.contains("call usize_wrapping_shr"));
}

#[test]
fn emits_string_index_runtime_call() {
    let source = r#"
        fn main() -> char {
            let value: string = "Geo"
            return value[0]
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call __geo_string_get"));
}

#[test]
fn emits_string_byte_at_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            return string_byte_at("Geo", 1usize)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_byte_at"));
}

#[test]
fn emits_string_from_byte_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_from_byte(65)
            let unicode: string = string_from_utf8_codepoint(955)
            return string_len(value) + string_len(unicode)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_from_byte"));
    assert!(asm.contains("call string_from_utf8_codepoint"));
}

#[test]
fn emits_string_find_byte_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_utf8_contains_codepoint("Geo", 101) && string_utf8_starts_with_codepoint("Geo", 71) {
                return string_find_byte("Geo", 101) + string_utf8_find_codepoint("Geo", 101)
            }
            return 0
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_find_byte"));
    assert!(asm.contains("call string_utf8_find_codepoint"));
    assert!(asm.contains("call string_utf8_contains_codepoint"));
    assert!(asm.contains("call string_utf8_starts_with_codepoint"));
}

#[test]
fn emits_string_last_find_byte_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_utf8_ends_with_codepoint("banana", 97) {
                return string_last_find_byte("banana", 97) + string_utf8_last_find_codepoint("banana", 97)
            }
            return 0
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_last_find_byte"));
    assert!(asm.contains("call string_utf8_last_find_codepoint"));
    assert!(asm.contains("call string_utf8_ends_with_codepoint"));
}

#[test]
fn emits_read_line_runtime_call() {
    let source = r#"
        import std.io
        import std.string

        fn main() -> usize {
            let line: string = read_line()
            return string_len(line)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call read_line"));
}

#[test]
fn emits_file_handle_open_runtime_calls() {
    let source = r#"
        import std.io

        fn main() -> int {
            let write_handle: int = file_open_write("out.txt")
            let append_handle: int = file_open_append("out.txt")
            return file_close(write_handle) + file_close(append_handle)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_open_write"));
    assert!(asm.contains("call file_open_append"));
    assert!(asm.contains("call file_close"));
}

#[test]
fn emits_read_write_file_runtime_calls() {
    let source = r#"
        import std.io
        import std.string

        fn main() -> int {
            let data: string = read_file("input.txt")
            let fallback: string = read_file_or("missing.txt", "default")
            return write_file("output.txt", data) + string_len(fallback) as int
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call read_file"));
    assert!(asm.contains("call read_file_or"));
    assert!(asm.contains("call write_file"));
}

#[test]
fn emits_append_file_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> int {
            return append_file("output.txt", "next")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call append_file"));
}

#[test]
fn emits_touch_file_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> int {
            return touch_file("output.txt")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call touch_file"));
}

#[test]
fn emits_truncate_file_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> int {
            return truncate_file("output.txt", 3)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call truncate_file"));
}

#[test]
fn emits_file_exists_and_remove_file_runtime_calls() {
    let source = r#"
        import std.io

        fn main() -> int {
            if file_exists("output.txt") && file_is_file("output.txt") && !file_is_empty("output.txt") {
                return remove_file("output.txt")
            } else {
                return 0
            }
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_exists"));
    assert!(asm.contains("call file_is_file"));
    assert!(asm.contains("call file_is_empty"));
    assert!(asm.contains("call remove_file"));
}

#[test]
fn emits_file_size_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> usize {
            return file_size("input.txt")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_size"));
}

#[test]
fn emits_file_modified_time_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> usize {
            return file_modified_time("input.txt")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_modified_time"));
}

#[test]
fn emits_file_accessed_time_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> usize {
            return file_accessed_time("input.txt")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_accessed_time"));
}

#[test]
fn emits_file_created_time_runtime_call() {
    let source = r#"
        import std.io

        fn main() -> usize {
            return file_created_time("input.txt")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call file_created_time"));
}

#[test]
fn emits_copy_and_rename_file_runtime_calls() {
    let source = r#"
        import std.io

        fn main() -> int {
            let copied: int = copy_file("input.txt", "copy.txt")
            let copied_dir: int = copy_dir_all("input-dir", "copy-dir")
            let renamed: int = rename_file("copy.txt", "renamed.txt")
            return copied + copied_dir + renamed
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call copy_file"));
    assert!(asm.contains("call copy_dir_all"));
    assert!(asm.contains("call rename_file"));
}

#[test]
fn emits_directory_runtime_calls() {
    let source = r#"
        import std.io
        import std.string

        fn main() -> int {
            create_dir_all("build/cache")
            let count: usize = dir_entry_count("build")
            let name: string = dir_entry_name("build", 0)
            let child: string = dir_entry_path("build", 0)
            if dir_exists("build") && file_is_dir("build") && count >= 0 && string_len(name) >= 0 && string_len(child) >= 0 {
                return remove_dir_all("build")
            } else {
                return 99
            }
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call create_dir_all"));
    assert!(asm.contains("call dir_entry_count"));
    assert!(asm.contains("call dir_entry_name"));
    assert!(asm.contains("call dir_entry_path"));
    assert!(asm.contains("call dir_exists"));
    assert!(asm.contains("call file_is_dir"));
    assert!(asm.contains("call remove_dir_all"));
}

#[test]
fn compiles_and_runs_return_42_example() {
    if !can_run_native_linux_examples() {
        return;
    }
    assert_geo_exit("examples/return_42.geo", 42);
}

#[test]
fn compiles_v0_1_examples() {
    if !can_run_native_linux_examples() {
        return;
    }
    assert_geo_exit("examples/arithmetic.geo", 42);
    assert_geo_exit("examples/variables.geo", 42);
    assert_geo_exit("examples/functions.geo", 42);
    assert_geo_exit("examples/if_else.geo", 42);
    assert_geo_exit("examples/while.geo", 42);
}

#[test]
fn native_run_prints_static_string_with_runtime_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-print-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.io

            fn main() {
                println("Geo")
            }
        "#,
    )
    .expect("failed to write print fixture");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .output()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Geo\n");
}

#[test]
fn native_run_traps_on_static_array_bounds_error_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-bounds-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn main() -> int {
                let xs: [int] = [1]
                return xs[1]
            }
        "#,
    )
    .expect("failed to write bounds fixture");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .output()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(output.status.code(), Some(101));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bounds check failed"));
}

#[test]
fn native_run_indexes_string_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-index-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn main() -> char {
                let value: string = "A"
                return value[0]
            }
        "#,
    )
    .expect("failed to write string index fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(65));
}

#[test]
fn native_run_reads_string_byte_without_trapping_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-byte-at-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_byte_at("Geo", 0usize) != 71 {
                    return 1
                }
                if string_byte_at("Geo", 2usize) != 111 {
                    return 2
                }
                if string_byte_at("Geo", 3usize) != -1 {
                    return 3
                }
                if string_byte_at("", 0usize) != -1 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_byte_at fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_builds_string_from_byte_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-from-byte-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let letter: string = string_from_byte(65)
                if string_compare(letter, "A") != 0 {
                    return 1
                }
                if string_byte_at(string_from_byte(255), 0usize) != 255 {
                    return 2
                }
                if string_len(string_from_byte(-1)) != 0 {
                    return 3
                }
                if string_len(string_from_byte(256)) != 0 {
                    return 4
                }
                let lambda: string = string_from_utf8_codepoint(955)
                if string_len(lambda) != 2usize {
                    return 5
                }
                if string_utf8_codepoint_at(lambda, 0usize) != 955 {
                    return 6
                }
                let face: string = string_from_utf8_codepoint(128512)
                if string_len(face) != 4usize {
                    return 7
                }
                if string_utf8_codepoint_at(face, 0usize) != 128512 {
                    return 8
                }
                let nul: string = string_from_utf8_codepoint(0)
                if string_len(nul) != 0usize {
                    return 9
                }
                if string_len(string_from_utf8_codepoint(-1)) != 0usize {
                    return 10
                }
                if string_len(string_from_utf8_codepoint(55296)) != 0usize {
                    return 11
                }
                if string_len(string_from_utf8_codepoint(1114112)) != 0usize {
                    return 12
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_from_byte fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_finds_string_byte_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-find-byte-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_find_byte("Geo", 71) != 0 {
                    return 1
                }
                if string_find_byte("banana", 97) != 1 {
                    return 2
                }
                if string_find_byte("Geo", 120) != -1 {
                    return 3
                }
                if string_find_byte("Geo", -1) != -1 {
                    return 4
                }
                if string_find_byte("Geo", 256) != -1 {
                    return 5
                }
                if string_utf8_find_codepoint("Geo", 71) != 0 {
                    return 6
                }
                if !string_utf8_starts_with_codepoint("Geo", 71) {
                    return 20
                }
                if string_utf8_starts_with_codepoint("Geo", 101) {
                    return 21
                }
                if !string_utf8_contains_codepoint("Geo", 71) {
                    return 15
                }
                if string_utf8_find_codepoint("banana", 97) != 1 {
                    return 7
                }
                if string_utf8_find_codepoint("Geo", 955) != -1 {
                    return 8
                }
                let lambda: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_find_codepoint(lambda, 955) != 0 {
                    return 9
                }
                let lambda_start: string = string_unescape_unicode("\\u{03bb}")
                if !string_utf8_starts_with_codepoint(lambda_start, 955) {
                    return 22
                }
                let mixed_lambda: string = string_unescape_unicode("\\u{03bb}")
                let mixed: string = string_concat("a", mixed_lambda)
                if string_utf8_find_codepoint(mixed, 955) != 1 {
                    return 10
                }
                let contains_lambda: string = string_unescape_unicode("\\u{03bb}")
                if !string_utf8_contains_codepoint(contains_lambda, 955) {
                    return 16
                }
                let contains_absent: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_contains_codepoint(contains_absent, 71) {
                    return 17
                }
                let contains_negative: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_contains_codepoint(contains_negative, -1) {
                    return 18
                }
                let negative: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_find_codepoint(negative, -1) != -1 {
                    return 11
                }
                let surrogate: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_find_codepoint(surrogate, 55296) != -1 {
                    return 12
                }
                let too_large: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_find_codepoint(too_large, 1114112) != -1 {
                    return 13
                }
                let invalid: string = string_from_byte(255)
                if string_utf8_find_codepoint(invalid, 255) != -1 {
                    return 14
                }
                let invalid_contains: string = string_from_byte(255)
                if string_utf8_contains_codepoint(invalid_contains, 255) {
                    return 19
                }
                let invalid_start: string = string_from_byte(255)
                if string_utf8_starts_with_codepoint(invalid_start, 255) {
                    return 23
                }
                if string_utf8_starts_with_codepoint("", 71) {
                    return 24
                }
                if string_utf8_starts_with_codepoint("Geo", -1) {
                    return 25
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_find_byte fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_finds_last_string_byte_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-last-find-byte-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_last_find_byte("Geo", 71) != 0 {
                    return 1
                }
                if string_last_find_byte("banana", 97) != 5 {
                    return 2
                }
                if string_last_find_byte("Geo", 120) != -1 {
                    return 3
                }
                if string_last_find_byte("Geo", -1) != -1 {
                    return 4
                }
                if string_last_find_byte("Geo", 256) != -1 {
                    return 5
                }
                if string_utf8_last_find_codepoint("Geo", 71) != 0 {
                    return 6
                }
                if string_utf8_ends_with_codepoint("Geo", 71) {
                    return 15
                }
                if !string_utf8_ends_with_codepoint("Geo", 111) {
                    return 16
                }
                if string_utf8_last_find_codepoint("banana", 97) != 5 {
                    return 7
                }
                if !string_utf8_ends_with_codepoint("banana", 97) {
                    return 17
                }
                let lambda_left: string = string_unescape_unicode("\\u{03bb}")
                let lambda_right: string = string_unescape_unicode("\\u{03bb}")
                let two: string = string_concat(lambda_left, lambda_right)
                if string_utf8_last_find_codepoint(two, 955) != 2 {
                    return 8
                }
                let lambda_end: string = string_unescape_unicode("\\u{03bb}")
                if !string_utf8_ends_with_codepoint(lambda_end, 955) {
                    return 18
                }
                let mixed_left: string = string_unescape_unicode("\\u{03bb}")
                let mixed_right: string = string_unescape_unicode("\\u{03bb}")
                let mixed_two: string = string_concat(mixed_left, mixed_right)
                let mixed: string = string_concat("a", mixed_two)
                if string_utf8_last_find_codepoint(mixed, 955) != 3 {
                    return 9
                }
                let absent: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_last_find_codepoint(absent, 71) != -1 {
                    return 10
                }
                let negative: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_last_find_codepoint(negative, -1) != -1 {
                    return 11
                }
                let surrogate: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_last_find_codepoint(surrogate, 55296) != -1 {
                    return 12
                }
                let too_large: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_last_find_codepoint(too_large, 1114112) != -1 {
                    return 13
                }
                let invalid: string = string_from_byte(255)
                if string_utf8_last_find_codepoint(invalid, 255) != -1 {
                    return 14
                }
                let invalid_end: string = string_from_byte(255)
                if string_utf8_ends_with_codepoint(invalid_end, 255) {
                    return 19
                }
                if string_utf8_ends_with_codepoint("", 71) {
                    return 20
                }
                if string_utf8_ends_with_codepoint("Geo", -1) {
                    return 21
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_last_find_byte fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_traps_on_string_bounds_error_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-bounds-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn main() -> char {
                let value: string = "A"
                return value[1]
            }
        "#,
    )
    .expect("failed to write string bounds fixture");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .output()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(output.status.code(), Some(101));
    assert!(String::from_utf8_lossy(&output.stderr).contains("bounds check failed"));
}

#[test]
fn native_run_reads_and_writes_file_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path = std::env::temp_dir().join(format!("geo-file-data-{}.txt", std::process::id()));
    std::fs::write(&data_path, "file io works").expect("failed to write file data fixture");
    let geo_path = std::env::temp_dir().join(format!("geo-file-io-{}.geo", std::process::id()));
    let data_path = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    let handle: int = file_open("{data_path}")
                    let data: string = file_read(handle)
                    file_write(1, data)
                    return file_close(handle)
                }}
            "#
        ),
    )
    .expect("failed to write file io fixture");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .output()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(data_path.replace("\\\\", "\\"));

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "file io works");
}

#[test]
fn native_run_writes_and_appends_file_handles_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path =
        std::env::temp_dir().join(format!("geo-file-handle-write-{}.txt", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    let geo_path =
        std::env::temp_dir().join(format!("geo-file-handle-write-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    let write_handle: int = file_open_write("{data}")
                    if write_handle < 0 {{
                        return 1
                    }}
                    if file_write(write_handle, "geo") != 0 {{
                        return 2
                    }}
                    if file_close(write_handle) != 0 {{
                        return 3
                    }}
                    let append_handle: int = file_open_append("{data}")
                    if append_handle < 0 {{
                        return 4
                    }}
                    if file_write(append_handle, "lang") != 0 {{
                        return 5
                    }}
                    if file_close(append_handle) != 0 {{
                        return 6
                    }}
                    if string_compare(read_file("{data}"), "geolang") != 0 {{
                        return 7
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write file handle write fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let written = std::fs::read_to_string(&data_path).unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert_eq!(status.code(), Some(0));
    assert_eq!(written, "geolang");
}

#[test]
fn native_run_reads_line_from_stdin_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-read-line-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.io

            fn main() -> int {
                let line: string = read_line()
                file_write(1, line)
                return 0
            }
        "#,
    )
    .expect("failed to write read_line fixture");

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run geo");
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().expect("child stdin should be piped");
        stdin
            .write_all(b"line from stdin\n")
            .expect("failed to write child stdin");
    }
    let output = child.wait_with_output().expect("failed to wait for geo");
    let _ = std::fs::remove_file(&path);

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "line from stdin\n");
}

#[test]
fn native_run_reads_and_writes_whole_files_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let input_path =
        std::env::temp_dir().join(format!("geo-whole-file-in-{}.txt", std::process::id()));
    let output_path =
        std::env::temp_dir().join(format!("geo-whole-file-out-{}.txt", std::process::id()));
    std::fs::write(&input_path, "whole file io works").expect("failed to write input fixture");
    let input = input_path.to_string_lossy().replace('\\', "\\\\");
    let output = output_path.to_string_lossy().replace('\\', "\\\\");
    let geo_path = std::env::temp_dir().join(format!("geo-whole-file-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    let data: string = read_file("{input}")
                    return write_file("{output}", data)
                }}
            "#
        ),
    )
    .expect("failed to write whole file fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let written = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);

    assert!(status.success());
    assert_eq!(written, "whole file io works");
}

#[test]
fn native_run_reads_file_with_default_and_checks_empty_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let empty_path =
        std::env::temp_dir().join(format!("geo-empty-file-{}.txt", std::process::id()));
    let data_path = std::env::temp_dir().join(format!("geo-data-file-{}.txt", std::process::id()));
    let missing_path =
        std::env::temp_dir().join(format!("geo-missing-file-{}.txt", std::process::id()));
    std::fs::write(&empty_path, "").expect("failed to write empty fixture");
    std::fs::write(&data_path, "geo").expect("failed to write data fixture");
    let empty = empty_path.to_string_lossy().replace('\\', "\\\\");
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    let missing = missing_path.to_string_lossy().replace('\\', "\\\\");
    let geo_path =
        std::env::temp_dir().join(format!("geo-read-file-or-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    if !file_is_empty("{empty}") {{
                        return 1
                    }}
                    if file_is_empty("{data}") {{
                        return 2
                    }}
                    if file_is_empty("{missing}") {{
                        return 3
                    }}
                    if string_compare(read_file_or("{missing}", "fallback"), "fallback") != 0 {{
                        return 4
                    }}
                    if string_compare(read_file_or("{data}", "fallback"), "geo") != 0 {{
                        return 5
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write read_file_or fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&empty_path);
    let _ = std::fs::remove_file(&data_path);
    let _ = std::fs::remove_file(&missing_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_appends_whole_files_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let output_path =
        std::env::temp_dir().join(format!("geo-append-file-out-{}.txt", std::process::id()));
    let output = output_path.to_string_lossy().replace('\\', "\\\\");
    let geo_path = std::env::temp_dir().join(format!("geo-append-file-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    if write_file("{output}", "geo") != 0 {{
                        return 1
                    }}
                    if append_file("{output}", "lang") != 0 {{
                        return 2
                    }}
                    if append_file("{output}", "!") != 0 {{
                        return 3
                    }}
                    let data: string = read_file("{output}")
                    if string_compare(data, "geolang!") != 0 {{
                        return 4
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write append file fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let written = std::fs::read_to_string(&output_path).unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&output_path);

    assert!(status.success());
    assert_eq!(written, "geolang!");
}

#[test]
fn native_run_checks_and_removes_file_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let target_path =
        std::env::temp_dir().join(format!("geo-remove-file-{}.txt", std::process::id()));
    let geo_path = std::env::temp_dir().join(format!("geo-remove-file-{}.geo", std::process::id()));
    let target = target_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    write_file("{target}", "data")
                    if file_exists("{target}") {{
                        return remove_file("{target}")
                    }} else {{
                        return 99
                    }}
                }}
            "#
        ),
    )
    .expect("failed to write remove file fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let still_exists = target_path.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&target_path);

    assert!(status.success());
    assert!(!still_exists);
}

#[test]
fn native_run_distinguishes_regular_files_and_directories_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let file_path = dir.join(format!("geo-is-file-{}.txt", std::process::id()));
    let dir_path = dir.join(format!("geo-is-file-dir-{}", std::process::id()));
    let geo_path = dir.join(format!("geo-is-file-{}.geo", std::process::id()));
    std::fs::write(&file_path, "data").expect("failed to write file_is_file fixture");
    std::fs::create_dir(&dir_path).expect("failed to create file_is_file directory fixture");
    let file = file_path.to_string_lossy().replace('\\', "\\\\");
    let directory = dir_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if !file_is_file("{file}") {{
                        return 1
                    }}
                    if file_is_file("{directory}") {{
                        return 2
                    }}
                    if !file_is_dir("{directory}") {{
                        return 3
                    }}
                    if file_is_dir("{file}") {{
                        return 4
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write file_is_file Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&file_path);
    let _ = std::fs::remove_dir(&dir_path);

    assert!(status.success());
}

#[test]
fn native_run_touches_file_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let target_path =
        std::env::temp_dir().join(format!("geo-touch-file-{}.txt", std::process::id()));
    let geo_path = std::env::temp_dir().join(format!("geo-touch-file-{}.geo", std::process::id()));
    let target = target_path.to_string_lossy().replace('\\', "\\\\");
    let _ = std::fs::remove_file(&target_path);
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if touch_file("{target}") != 0 {{
                        return 1
                    }}
                    if file_exists("{target}") {{
                        return remove_file("{target}")
                    }}
                    return 2
                }}
            "#
        ),
    )
    .expect("failed to write touch file fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let still_exists = target_path.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&target_path);

    assert!(status.success());
    assert!(!still_exists);
}

#[test]
fn native_run_truncates_file_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path =
        std::env::temp_dir().join(format!("geo-truncate-file-{}.txt", std::process::id()));
    std::fs::write(&data_path, "abcdef").expect("failed to write truncate file fixture");
    let geo_path =
        std::env::temp_dir().join(format!("geo-truncate-file-{}.geo", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if truncate_file("{data}", 3) != 0 {{
                        return 1
                    }}
                    if file_size("{data}") != 3 {{
                        return 2
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write truncate file Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let written = std::fs::read_to_string(&data_path).unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert!(status.success());
    assert_eq!(written, "abc");
}

#[test]
fn native_run_reports_file_size_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path = std::env::temp_dir().join(format!("geo-file-size-{}.txt", std::process::id()));
    std::fs::write(&data_path, "abcdef").expect("failed to write file size fixture");
    let geo_path = std::env::temp_dir().join(format!("geo-file-size-{}.geo", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> usize {{
                    return file_size("{data}")
                }}
            "#
        ),
    )
    .expect("failed to write file size Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert_eq!(status.code(), Some(6));
}

#[test]
fn native_run_reports_file_modified_time_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path =
        std::env::temp_dir().join(format!("geo-file-modified-{}.txt", std::process::id()));
    std::fs::write(&data_path, "modified").expect("failed to write modified time fixture");
    let geo_path =
        std::env::temp_dir().join(format!("geo-file-modified-{}.geo", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if file_modified_time("{data}") == 0 {{
                        return 1
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write modified time Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert!(status.success());
}

#[test]
fn native_run_reports_file_accessed_time_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path =
        std::env::temp_dir().join(format!("geo-file-accessed-{}.txt", std::process::id()));
    std::fs::write(&data_path, "accessed").expect("failed to write accessed time fixture");
    let geo_path =
        std::env::temp_dir().join(format!("geo-file-accessed-{}.geo", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if file_accessed_time("{data}") == 0 {{
                        return 1
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write accessed time Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert!(status.success());
}

#[test]
fn native_run_reports_file_created_time_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let data_path =
        std::env::temp_dir().join(format!("geo-file-created-{}.txt", std::process::id()));
    std::fs::write(&data_path, "created").expect("failed to write created time fixture");
    let geo_path =
        std::env::temp_dir().join(format!("geo-file-created-{}.geo", std::process::id()));
    let data = data_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if file_created_time("{data}") == 0 {{
                        return 1
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write created time Geo fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&data_path);

    assert!(status.success());
}

#[test]
fn native_run_copies_and_renames_file_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let source_path = dir.join(format!("geo-copy-src-{}.txt", std::process::id()));
    let copy_path = dir.join(format!("geo-copy-mid-{}.txt", std::process::id()));
    let renamed_path = dir.join(format!("geo-copy-renamed-{}.txt", std::process::id()));
    let geo_path = dir.join(format!("geo-copy-rename-{}.geo", std::process::id()));
    std::fs::write(&source_path, "copy rename works").expect("failed to write source fixture");
    let source = source_path.to_string_lossy().replace('\\', "\\\\");
    let copy = copy_path.to_string_lossy().replace('\\', "\\\\");
    let renamed = renamed_path.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    let copied: int = copy_file("{source}", "{copy}")
                    let renamed: int = rename_file("{copy}", "{renamed}")
                    return copied + renamed
                }}
            "#
        ),
    )
    .expect("failed to write copy rename fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let renamed_data = std::fs::read_to_string(&renamed_path).unwrap_or_default();
    let copy_still_exists = copy_path.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&copy_path);
    let _ = std::fs::remove_file(&renamed_path);

    assert!(status.success());
    assert_eq!(renamed_data, "copy rename works");
    assert!(!copy_still_exists);
}

#[test]
fn native_run_copies_directory_trees_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let source_dir = dir.join(format!("geo-copy-dir-src-{}", std::process::id()));
    let dest_dir = dir.join(format!("geo-copy-dir-dest-{}", std::process::id()));
    let nested_dir = source_dir.join("cache").join("objects");
    std::fs::create_dir_all(&nested_dir).expect("failed to create copy_dir_all fixture");
    std::fs::write(source_dir.join("root.txt"), "root").expect("failed to write root fixture");
    std::fs::write(nested_dir.join("stamp.txt"), "nested").expect("failed to write nested fixture");
    let geo_path = dir.join(format!("geo-copy-dir-{}.geo", std::process::id()));
    let source = source_dir.to_string_lossy().replace('\\', "\\\\");
    let dest = dest_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    if copy_dir_all("{source}", "{dest}") != 0 {{
                        return 1
                    }}
                    if !file_is_file("{dest}/root.txt") {{
                        return 2
                    }}
                    if string_compare(read_file("{dest}/cache/objects/stamp.txt"), "nested") != 0 {{
                        return 3
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write copy_dir_all fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let root_copy = dest_dir.join("root.txt").exists();
    let nested_copy =
        std::fs::read_to_string(dest_dir.join("cache").join("objects").join("stamp.txt"))
            .unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&source_dir);
    let _ = std::fs::remove_dir_all(&dest_dir);

    assert!(status.success());
    assert!(root_copy);
    assert_eq!(nested_copy, "nested");
}

#[test]
fn native_run_gets_and_changes_current_directory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let workspace = dir.join(format!("geo-cwd-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("failed to create cwd fixture directory");
    let marker = workspace.join("marker.txt");
    let geo_path = dir.join(format!("geo-cwd-{}.geo", std::process::id()));
    let workspace_arg = workspace.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.platform
                import std.string

                fn main() -> int {{
                    let before: string = current_dir()
                    change_dir("{workspace_arg}")
                    let after: string = current_dir()
                    if string_len(before) > 0 {{
                        return write_file("marker.txt", after)
                    }} else {{
                        return 99
                    }}
                }}
            "#
        ),
    )
    .expect("failed to write cwd fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let marker_data = std::fs::read_to_string(&marker).unwrap_or_default();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&workspace);

    assert!(status.success());
    assert!(!marker_data.is_empty());
}

#[test]
fn native_run_reports_temp_directory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-temp-dir-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.io
            import std.platform
            import std.string

            fn main() -> int {
                let temp: string = temp_dir()
                if string_len(temp) == 0 {
                    return 1
                }
                if !dir_exists(temp) {
                    return 2
                }
                return 0
            }
        "#,
    )
    .expect("failed to write temp_dir fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_home_directory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-home-dir-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.io
            import std.platform
            import std.string

            fn main() -> int {
                let home: string = home_dir()
                if string_len(home) == 0 {
                    return 1
                }
                if !dir_exists(home) {
                    return 2
                }
                return 0
            }
        "#,
    )
    .expect("failed to write home_dir fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_user_name_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-user-name-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                if string_len(user_name()) == 0 {
                    return 1
                }
                return 0
            }
        "#,
    )
    .expect("failed to write user_name fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_platform_arch_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-platform-arch-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                if string_len(platform_arch()) == 0 {
                    return 1
                }
                return 0
            }
        "#,
    )
    .expect("failed to write platform_arch fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_cpu_count_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-cpu-count-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform

            fn main() -> int {
                if cpu_count() == 0 {
                    return 1
                }
                return 0
            }
        "#,
    )
    .expect("failed to write cpu_count fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_joins_platform_paths_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-join-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let joined: string = path_join("geo", "compiler")
                if string_compare(joined, "geo/compiler") != 0 {
                    return 1
                }
                let preserved: string = path_join("geo/", "compiler")
                if string_compare(preserved, "geo/compiler") != 0 {
                    return 2
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_join fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_gets_platform_path_file_names_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-file-name-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let unix_name: string = path_file_name("src/runtime.geo")
                if string_compare(unix_name, "runtime.geo") != 0 {
                    return 1
                }
                let windows_name: string = path_file_name("src\\runtime.geo")
                if string_compare(windows_name, "runtime.geo") != 0 {
                    return 2
                }
                let trailing: string = path_file_name("src/")
                if string_len(trailing) != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_file_name fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_gets_platform_path_parents_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-parent-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let unix_parent: string = path_parent("src/runtime.geo")
                if string_compare(unix_parent, "src") != 0 {
                    return 1
                }
                let windows_parent: string = path_parent("src\\runtime.geo")
                if string_compare(windows_parent, "src") != 0 {
                    return 2
                }
                let missing: string = path_parent("runtime.geo")
                if string_len(missing) != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_parent fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_gets_platform_path_extensions_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-extension-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let source_ext: string = path_extension("src/main.geo")
                if string_compare(source_ext, "geo") != 0 {
                    return 1
                }
                let object_ext: string = path_extension("build\\main.obj")
                if string_compare(object_ext, "obj") != 0 {
                    return 2
                }
                let missing: string = path_extension("Makefile")
                if string_len(missing) != 0 {
                    return 3
                }
                let dotfile: string = path_extension(".gitignore")
                if string_len(dotfile) != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_extension fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_gets_platform_path_stems_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-stem-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let source_stem: string = path_stem("src/main.geo")
                if string_compare(source_stem, "main") != 0 {
                    return 1
                }
                let archive_stem: string = path_stem("pkg/archive.tar.gz")
                if string_compare(archive_stem, "archive.tar") != 0 {
                    return 2
                }
                let missing: string = path_stem("Makefile")
                if string_compare(missing, "Makefile") != 0 {
                    return 3
                }
                let dotfile: string = path_stem(".gitignore")
                if string_compare(dotfile, ".gitignore") != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_stem fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_absolute_platform_paths_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-absolute-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform

            fn main() -> int {
                if !path_is_absolute("/tmp/geo") {
                    return 1
                }
                if !path_is_absolute("C:\\Geo\\main.geo") {
                    return 2
                }
                if !path_is_absolute("\\\\server\\share\\main.geo") {
                    return 3
                }
                if path_is_absolute("src/main.geo") {
                    return 4
                }
                if path_is_absolute("") {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_is_absolute fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_removes_platform_path_extensions_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!(
        "geo-path-without-extension-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let source: string = path_without_extension("src/main.geo")
                if string_compare(source, "src/main") != 0 {
                    return 1
                }
                let archive: string = path_without_extension("pkg/archive.tar.gz")
                if string_compare(archive, "pkg/archive.tar") != 0 {
                    return 2
                }
                let missing: string = path_without_extension("Makefile")
                if string_compare(missing, "Makefile") != 0 {
                    return 3
                }
                let dotfile: string = path_without_extension(".gitignore")
                if string_compare(dotfile, ".gitignore") != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_without_extension fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_replaces_platform_path_extensions_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!(
        "geo-path-with-extension-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let asm: string = path_with_extension("src/main.geo", "asm")
                if string_compare(asm, "src/main.asm") != 0 {
                    return 1
                }
                let obj: string = path_with_extension("src/main", ".o")
                if string_compare(obj, "src/main.o") != 0 {
                    return 2
                }
                let archive: string = path_with_extension("pkg/archive.tar.gz", "zip")
                if string_compare(archive, "pkg/archive.tar.zip") != 0 {
                    return 3
                }
                let removed: string = path_with_extension("src/main.geo", "")
                if string_compare(removed, "src/main") != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_with_extension fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_normalizes_platform_paths_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-path-normalize-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let simple: string = path_normalize("src/./lib/../main.geo")
                if string_compare(simple, "src/main.geo") != 0 {
                    return 1
                }
                let collapsed: string = path_normalize("a//b///c")
                if string_compare(collapsed, "a/b/c") != 0 {
                    return 2
                }
                let parent: string = path_normalize("../src/./main.geo")
                if string_compare(parent, "../src/main.geo") != 0 {
                    return 3
                }
                let absolute: string = path_normalize("/tmp/../geo")
                if string_compare(absolute, "/geo") != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path_normalize fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_converts_platform_path_separators_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!(
        "geo-path-separator-conversion-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &geo_path,
        r#"
            import std.platform
            import std.string

            fn main() -> int {
                let unix_path: string = path_to_unix("src\\compiler\\main.geo")
                if string_compare(unix_path, "src/compiler/main.geo") != 0 {
                    return 1
                }
                let windows_path: string = path_to_windows("src/compiler/main.geo")
                if string_compare(windows_path, "src\\compiler\\main.geo") != 0 {
                    return 2
                }
                let mixed_unix: string = path_to_unix("src/compiler\\main.geo")
                if string_compare(mixed_unix, "src/compiler/main.geo") != 0 {
                    return 3
                }
                let mixed_windows: string = path_to_windows("src\\compiler/main.geo")
                if string_compare(mixed_windows, "src\\compiler\\main.geo") != 0 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write path separator conversion fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_absolutizes_platform_paths_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let workspace = dir.join(format!("geo-path-absolute-base-{}", std::process::id()));
    let nested = workspace.join("src");
    std::fs::create_dir_all(&nested).expect("failed to create path_absolute fixture directory");
    std::fs::write(nested.join("main.geo"), "data").expect("failed to write path_absolute fixture");
    let geo_path = dir.join(format!("geo-path-absolute-{}.geo", std::process::id()));
    let workspace_arg = workspace.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.platform
                import std.string

                fn main() -> int {{
                    change_dir("{workspace_arg}")
                    let absolute: string = path_absolute("src/./main.geo")
                    if !path_is_absolute(absolute) {{
                        return 1
                    }}
                    if !file_is_file(absolute) {{
                        return 2
                    }}
                    if string_len(path_absolute("")) != 0 {{
                        return 3
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write path_absolute fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&workspace);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reads_unix_time_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-unix-time-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.time

            fn main() -> int {
                let secs: usize = unix_time_secs()
                let millis: usize = unix_time_millis()
                let micros: usize = unix_time_micros()
                let nanos: usize = unix_time_nanos()
                if secs == 0 {
                    return 1
                }
                if millis < secs * 1000 {
                    return 2
                }
                if micros < millis * 1000 {
                    return 3
                }
                if nanos < micros * 1000 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write unix_time fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_sleeps_for_millis_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-sleep-millis-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.time

            fn main() -> int {
                let start: usize = unix_time_millis()
                let code: int = sleep_millis(20)
                let end: usize = unix_time_millis()
                if code != 0 {
                    return 1
                }
                if end < start + 10 {
                    return 2
                }
                return 0
            }
        "#,
    )
    .expect("failed to write sleep_millis fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reads_monotonic_time_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-monotonic-millis-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.time

            fn main() -> int {
                let start: usize = monotonic_millis()
                let start_us: usize = monotonic_micros()
                let start_ns: usize = monotonic_nanos()
                sleep_millis(20)
                let end: usize = monotonic_millis()
                let end_us: usize = monotonic_micros()
                let end_ns: usize = monotonic_nanos()
                if start == 0 {
                    return 1
                }
                if end < start + 10 {
                    return 2
                }
                if start_us == 0 || start_ns == 0 {
                    return 3
                }
                if end_us < start_us {
                    return 4
                }
                if end_ns < start_ns {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write monotonic_millis fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_math_integer_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-int-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                if int_abs_diff(-7, 3) != 10 {
                    return 64
                }
                if int_abs_diff(-9223372036854775808, 9223372036854775807) != 18446744073709551615 {
                    return 65
                }
                if int_div_floor(7, 3) != 2 {
                    return 1
                }
                if int_div_floor(-7, 3) != -3 {
                    return 2
                }
                if int_div_floor(7, -3) != -3 {
                    return 3
                }
                if int_div_floor(-7, -3) != 2 {
                    return 4
                }
                if int_div_floor(7, 0) != 0 {
                    return 5
                }
                if int_div_ceil(7, 3) != 3 {
                    return 6
                }
                if int_div_ceil(-7, 3) != -2 {
                    return 7
                }
                if int_div_ceil(7, -3) != -2 {
                    return 8
                }
                if int_div_ceil(-7, -3) != 3 {
                    return 9
                }
                if int_div_ceil(7, 0) != 0 {
                    return 10
                }
                if int_div_euclid(7, 3) != 2 {
                    return 11
                }
                if int_div_euclid(-7, 3) != -3 {
                    return 12
                }
                if int_div_euclid(7, -3) != -2 {
                    return 13
                }
                if int_div_euclid(-7, -3) != 3 {
                    return 14
                }
                if int_div_euclid(7, 0) != 0 {
                    return 15
                }
                if int_rem_floor(7, 3) != 1 {
                    return 16
                }
                if int_rem_floor(-7, 3) != 2 {
                    return 17
                }
                if int_rem_floor(7, -3) != -2 {
                    return 18
                }
                if int_rem_floor(-7, -3) != -1 {
                    return 19
                }
                if int_rem_floor(7, 0) != 0 {
                    return 20
                }
                if int_rem_euclid(7, 3) != 1 {
                    return 21
                }
                if int_rem_euclid(-7, 3) != 2 {
                    return 22
                }
                if int_rem_euclid(7, -3) != 1 {
                    return 23
                }
                if int_rem_euclid(-7, -3) != 2 {
                    return 24
                }
                if int_rem_euclid(7, 0) != 0 {
                    return 25
                }
                if int_checked_add(20, 22) != 42 {
                    return 26
                }
                if int_checked_add(9223372036854775807, 1) != 0 {
                    return 27
                }
                if int_checked_add(-9223372036854775808, -1) != 0 {
                    return 28
                }
                if int_checked_sub(50, 8) != 42 {
                    return 29
                }
                if int_checked_sub(-9223372036854775808, 1) != 0 {
                    return 30
                }
                if int_checked_sub(9223372036854775807, -1) != 0 {
                    return 31
                }
                if int_checked_mul(6, 7) != 42 {
                    return 32
                }
                if int_checked_mul(9223372036854775807, 2) != 0 {
                    return 33
                }
                if int_checked_mul(-9223372036854775808, -1) != 0 {
                    return 34
                }
                if int_checked_div(84, 2) != 42 {
                    return 35
                }
                if int_checked_div(84, 0) != 0 {
                    return 36
                }
                if int_checked_div(-9223372036854775808, -1) != 0 {
                    return 37
                }
                if int_checked_rem(85, 43) != 42 {
                    return 38
                }
                if int_checked_rem(84, 0) != 0 {
                    return 39
                }
                if int_checked_rem(-9223372036854775808, -1) != 0 {
                    return 40
                }
                if int_checked_neg(-42) != 42 {
                    return 41
                }
                if int_checked_neg(42) != -42 {
                    return 42
                }
                if int_checked_neg(-9223372036854775808) != 0 {
                    return 43
                }
                if int_checked_abs(-42) != 42 {
                    return 72
                }
                if int_checked_abs(42) != 42 {
                    return 73
                }
                if int_checked_abs(-9223372036854775808) != 0 {
                    return 74
                }
                if int_saturating_add(20, 22) != 42 {
                    return 44
                }
                if int_saturating_add(9223372036854775807, 1) != 9223372036854775807 {
                    return 45
                }
                if int_saturating_add(-9223372036854775808, -1) != -9223372036854775808 {
                    return 46
                }
                if int_saturating_sub(50, 8) != 42 {
                    return 47
                }
                if int_saturating_sub(9223372036854775807, -1) != 9223372036854775807 {
                    return 48
                }
                if int_saturating_sub(-9223372036854775808, 1) != -9223372036854775808 {
                    return 49
                }
                if int_saturating_mul(6, 7) != 42 {
                    return 50
                }
                if int_saturating_mul(9223372036854775807, 2) != 9223372036854775807 {
                    return 51
                }
                if int_saturating_mul(-9223372036854775808, 2) != -9223372036854775808 {
                    return 52
                }
                if int_saturating_abs(-42) != 42 {
                    return 66
                }
                if int_saturating_abs(42) != 42 {
                    return 67
                }
                if int_saturating_abs(-9223372036854775808) != 9223372036854775807 {
                    return 68
                }
                if int_saturating_neg(-42) != 42 {
                    return 75
                }
                if int_saturating_neg(42) != -42 {
                    return 76
                }
                if int_saturating_neg(-9223372036854775808) != 9223372036854775807 {
                    return 77
                }
                if int_wrapping_add(20, 22) != 42 {
                    return 53
                }
                if int_wrapping_add(9223372036854775807, 1) != -9223372036854775808 {
                    return 54
                }
                if int_wrapping_add(-9223372036854775808, -1) != 9223372036854775807 {
                    return 55
                }
                if int_wrapping_sub(50, 8) != 42 {
                    return 56
                }
                if int_wrapping_sub(-9223372036854775808, 1) != 9223372036854775807 {
                    return 57
                }
                if int_wrapping_sub(9223372036854775807, -1) != -9223372036854775808 {
                    return 58
                }
                if int_wrapping_mul(6, 7) != 42 {
                    return 59
                }
                if int_wrapping_mul(-9223372036854775808, 2) != 0 {
                    return 60
                }
                if int_wrapping_mul(9223372036854775807, 2) != -2 {
                    return 61
                }
                if int_wrapping_neg(-42) != 42 {
                    return 62
                }
                if int_wrapping_neg(-9223372036854775808) != -9223372036854775808 {
                    return 63
                }
                if int_wrapping_abs(-42) != 42 {
                    return 69
                }
                if int_wrapping_abs(42) != 42 {
                    return 70
                }
                if int_wrapping_abs(-9223372036854775808) != -9223372036854775808 {
                    return 71
                }
                return int_abs(-7) + int_min(9, 4) + int_max(9, 4) + int_clamp(12, 0, 10)
            }
        "#,
    )
    .expect("failed to write math integer fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(30));
}

#[test]
fn native_run_uses_math_usize_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-usize-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                if usize_checked_add(20, 22) != 42 {
                    return 1
                }
                if usize_checked_add(18446744073709551615, 1) != 0 {
                    return 2
                }
                if usize_checked_sub(50, 8) != 42 {
                    return 3
                }
                if usize_checked_sub(0, 1) != 0 {
                    return 4
                }
                if usize_checked_mul(6, 7) != 42 {
                    return 5
                }
                if usize_checked_mul(9223372036854775808, 2) != 0 {
                    return 6
                }
                if usize_checked_div(84, 2) != 42 {
                    return 7
                }
                if usize_checked_div(84, 0) != 0 {
                    return 8
                }
                if usize_checked_rem(85, 43) != 42 {
                    return 9
                }
                if usize_checked_rem(84, 0) != 0 {
                    return 10
                }
                if usize_saturating_add(20, 22) != 42 {
                    return 11
                }
                if usize_saturating_add(18446744073709551615, 1) != 18446744073709551615 {
                    return 12
                }
                if usize_saturating_sub(50, 8) != 42 {
                    return 13
                }
                if usize_saturating_sub(0, 1) != 0 {
                    return 14
                }
                if usize_saturating_mul(6, 7) != 42 {
                    return 15
                }
                if usize_saturating_mul(9223372036854775808, 2) != 18446744073709551615 {
                    return 16
                }
                if usize_wrapping_add(20, 22) != 42 {
                    return 17
                }
                if usize_wrapping_add(18446744073709551615, 1) != 0 {
                    return 18
                }
                if usize_wrapping_sub(50, 8) != 42 {
                    return 19
                }
                if usize_wrapping_sub(0, 1) != 18446744073709551615 {
                    return 20
                }
                if usize_wrapping_mul(6, 7) != 42 {
                    return 21
                }
                if usize_wrapping_mul(9223372036854775808, 2) != 0 {
                    return 22
                }
                if usize_abs_diff(3, 7) != 4 {
                    return 23
                }
                if usize_abs_diff(7, 3) != 4 {
                    return 24
                }
                let value: usize = usize_min(9, 4) + usize_max(9, 4) + usize_clamp(12, 0, 10)
                return value as int
            }
        "#,
    )
    .expect("failed to write math usize fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(23));
}

#[test]
fn native_run_uses_math_power_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-pow-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                if int_checked_pow(-2, 3) != -8 {
                    return 1
                }
                if int_checked_pow(3037000500, 2) != 0 {
                    return 2
                }
                if int_checked_pow(-9223372036854775808, 1) != -9223372036854775808 {
                    return 3
                }
                if int_checked_pow(-9223372036854775808, 2) != 0 {
                    return 4
                }
                if int_saturating_pow(-2, 3) != -8 {
                    return 7
                }
                if int_saturating_pow(3037000500, 2) != 9223372036854775807 {
                    return 9
                }
                if int_saturating_pow(-3037000500, 3) != -9223372036854775808 {
                    return 10
                }
                if int_wrapping_pow(-2, 3) != -8 {
                    return 13
                }
                if int_wrapping_pow(9223372036854775807, 2) != 1 {
                    return 14
                }
                if usize_checked_pow(2, 4) != 16 {
                    return 5
                }
                if usize_checked_pow(9223372036854775808, 2) != 0 {
                    return 6
                }
                if usize_saturating_pow(2, 4) != 16 {
                    return 11
                }
                if usize_saturating_pow(9223372036854775808, 2) != usize_wrapping_sub(0, 1) {
                    return 12
                }
                if usize_wrapping_pow(2, 4) != 16 {
                    return 15
                }
                if usize_wrapping_pow(9223372036854775808, 2) != 0 {
                    return 16
                }
                return int_pow(-2, 3) + usize_pow(2, 4) as int
            }
        "#,
    )
    .expect("failed to write math power fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(8));
}

#[test]
fn native_run_uses_math_gcd_lcm_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-gcd-lcm-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                return int_gcd(-54, 24) + int_lcm(-6, 8) + usize_gcd(54, 24) as int + usize_lcm(6, 8) as int
            }
        "#,
    )
    .expect("failed to write math gcd/lcm fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(84));
}

#[test]
fn native_run_uses_math_parity_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-parity-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                if !int_is_even(-4) {
                    return 1
                }
                if !int_is_odd(-3) {
                    return 2
                }
                if !usize_is_even(10) {
                    return 3
                }
                if !usize_is_odd(11) {
                    return 4
                }
                if int_is_even(-3) || int_is_odd(-4) || usize_is_even(11) || usize_is_odd(10) {
                    return 5
                }
                if !int_is_power_of_two(1) || !int_is_power_of_two(16) {
                    return 23
                }
                if int_is_power_of_two(0) || int_is_power_of_two(18) || int_is_power_of_two(-16) {
                    return 24
                }
                if int_prev_power_of_two(-1) != 0 {
                    return 25
                }
                if int_prev_power_of_two(0) != 0 {
                    return 26
                }
                if int_prev_power_of_two(1) != 1 || int_prev_power_of_two(16) != 16 {
                    return 27
                }
                if int_prev_power_of_two(31) != 16 {
                    return 28
                }
                if int_prev_power_of_two(9223372036854775807) != 4611686018427387904 {
                    return 29
                }
                if int_next_power_of_two(-1) != 0 {
                    return 177
                }
                if int_next_power_of_two(0) != 0 {
                    return 178
                }
                if int_next_power_of_two(1) != 1 || int_next_power_of_two(16) != 16 {
                    return 179
                }
                if int_next_power_of_two(17) != 32 {
                    return 180
                }
                if int_next_power_of_two(4611686018427387904) != 4611686018427387904 {
                    return 181
                }
                if int_next_power_of_two(4611686018427387905) != 0 {
                    return 182
                }
                if int_checked_next_power_of_two(-1) != 0 {
                    return 165
                }
                if int_checked_next_power_of_two(0) != 0 {
                    return 166
                }
                if int_checked_next_power_of_two(1) != 1 || int_checked_next_power_of_two(16) != 16 {
                    return 167
                }
                if int_checked_next_power_of_two(17) != 32 {
                    return 168
                }
                if int_checked_next_power_of_two(4611686018427387904) != 4611686018427387904 {
                    return 169
                }
                if int_checked_next_power_of_two(4611686018427387905) != 0 {
                    return 170
                }
                if int_saturating_next_power_of_two(-1) != 0 {
                    return 171
                }
                if int_saturating_next_power_of_two(0) != 0 {
                    return 172
                }
                if int_saturating_next_power_of_two(1) != 1 || int_saturating_next_power_of_two(16) != 16 {
                    return 173
                }
                if int_saturating_next_power_of_two(17) != 32 {
                    return 174
                }
                if int_saturating_next_power_of_two(4611686018427387904) != 4611686018427387904 {
                    return 175
                }
                if int_saturating_next_power_of_two(4611686018427387905) != 9223372036854775807 {
                    return 176
                }
                if int_align_up(0, 8) != 0 {
                    return 183
                }
                if int_align_up(13, 8) != 16 || int_align_up(16, 8) != 16 {
                    return 184
                }
                if int_align_up(13, 0) != 13 || int_align_up(13, 1) != 13 {
                    return 185
                }
                if int_align_down(0, 8) != 0 {
                    return 186
                }
                if int_align_down(15, 8) != 8 || int_align_down(16, 8) != 16 {
                    return 187
                }
                if int_align_down(13, 0) != 13 || int_align_down(13, 1) != 13 {
                    return 188
                }
                if int_align_up_saturating(0, 8) != 0 {
                    return 189
                }
                if int_align_up_saturating(17, 8) != 24 {
                    return 190
                }
                if int_align_up_saturating(9223372036854775806, 8) != 9223372036854775807 {
                    return 191
                }
                if int_align_up_saturating(13, 0) != 13 || int_align_up_saturating(13, 1) != 13 {
                    return 192
                }
                if !usize_is_power_of_two(1) || !usize_is_power_of_two(16) {
                    return 6
                }
                if usize_is_power_of_two(0) || usize_is_power_of_two(18) {
                    return 7
                }
                if usize_next_power_of_two(0) != 1 {
                    return 8
                }
                if usize_next_power_of_two(1) != 1 || usize_next_power_of_two(16) != 16 {
                    return 9
                }
                if usize_next_power_of_two(17) != 32 {
                    return 10
                }
                if usize_checked_next_power_of_two(0) != 1 {
                    return 78
                }
                if usize_checked_next_power_of_two(17) != 32 {
                    return 79
                }
                if usize_checked_next_power_of_two(9223372036854775808 + 1) != 0 {
                    return 80
                }
                if usize_saturating_next_power_of_two(0) != 1 {
                    return 81
                }
                if usize_saturating_next_power_of_two(17) != 32 {
                    return 82
                }
                if usize_saturating_next_power_of_two(9223372036854775808 + 1) != usize_wrapping_sub(0, 1) {
                    return 83
                }
                if usize_prev_power_of_two(0) != 0 {
                    return 11
                }
                if usize_prev_power_of_two(1) != 1 || usize_prev_power_of_two(16) != 16 {
                    return 12
                }
                if usize_prev_power_of_two(31) != 16 {
                    return 13
                }
                if usize_align_up(0, 8) != 0 {
                    return 14
                }
                if usize_align_up(13, 8) != 16 || usize_align_up(16, 8) != 16 {
                    return 15
                }
                if usize_align_up(13, 0) != 13 || usize_align_up(13, 1) != 13 {
                    return 16
                }
                if usize_align_down(0, 8) != 0 {
                    return 17
                }
                if usize_align_down(15, 8) != 8 || usize_align_down(16, 8) != 16 {
                    return 18
                }
                if usize_align_down(13, 0) != 13 || usize_align_down(13, 1) != 13 {
                    return 19
                }
                if usize_align_up_saturating(17, 8) != 24 {
                    return 23
                }
                if usize_align_up_saturating(17, 0) != 17 {
                    return 24
                }
                if usize_align_up_saturating(9223372036854775808, 9223372036854775807) <= 9223372036854775808 {
                    return 25
                }
                if usize_div_ceil(16, 8) != 2 {
                    return 20
                }
                if usize_div_ceil(17, 8) != 3 {
                    return 21
                }
                if usize_div_ceil(0, 8) != 0 || usize_div_ceil(13, 0) != 0 {
                    return 22
                }
                return 0
            }
        "#,
    )
    .expect("failed to write math parity fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_math_sign_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-math-sign-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.math

            fn main() -> int {
                if int_signum(42) != 1 {
                    return 1
                }
                if int_signum(-42) != -1 {
                    return 2
                }
                if int_signum(0) != 0 {
                    return 3
                }
                if !int_is_positive(42) || int_is_positive(0) || int_is_positive(-42) {
                    return 4
                }
                if !int_is_negative(-42) || int_is_negative(0) || int_is_negative(42) {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write math sign fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_bits_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-bits-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.bits

            fn main() -> int {
                if int_popcount(-1) != 64 {
                    return 9
                }
                if int_popcount(13) != 3 {
                    return 10
                }
                if int_count_ones(-1) != 64 {
                    return 165
                }
                if int_count_ones(13) != 3 {
                    return 166
                }
                if int_parity(0) {
                    return 155
                }
                if !int_parity(1) {
                    return 156
                }
                if int_parity(3) {
                    return 157
                }
                if !int_parity(13) {
                    return 158
                }
                if int_parity(-1) {
                    return 159
                }
                if int_count_zeros(0) != 64 {
                    return 34
                }
                if int_count_zeros(13) != 61 {
                    return 35
                }
                if int_count_zeros(-1) != 0 {
                    return 36
                }
                if int_leading_zeros(1) != 63 {
                    return 11
                }
                if int_leading_zeros(0) != 64 {
                    return 12
                }
                if int_leading_zeros(-1) != 0 {
                    return 13
                }
                if int_leading_ones(-1) != 64 {
                    return 169
                }
                if int_leading_ones(-2) != 63 {
                    return 170
                }
                if int_leading_ones(0) != 0 {
                    return 171
                }
                if int_trailing_zeros(8) != 3 {
                    return 14
                }
                if int_trailing_zeros(0) != 64 {
                    return 15
                }
                if int_trailing_zeros(-8) != 3 {
                    return 16
                }
                if int_trailing_ones(-1) != 64 {
                    return 172
                }
                if int_trailing_ones(7) != 3 {
                    return 173
                }
                if int_trailing_ones(0) != 0 {
                    return 174
                }
                if int_reverse_bits(1) != -9223372036854775808 {
                    return 17
                }
                if int_reverse_bits(-9223372036854775808) != 1 {
                    return 18
                }
                if int_swap_bytes(72623859790382856) != 578437695752307201 {
                    return 19
                }
                if int_swap_bytes(-1) != -1 {
                    return 20
                }
                if int_from_be(72623859790382856) != 578437695752307201 {
                    return 94
                }
                if int_to_be(72623859790382856) != 578437695752307201 {
                    return 95
                }
                if int_from_le(72623859790382856) != 72623859790382856 {
                    return 96
                }
                if int_to_le(72623859790382856) != 72623859790382856 {
                    return 97
                }
                if int_from_be(int_to_be(72623859790382856)) != 72623859790382856 {
                    return 98
                }
                if int_from_le(int_to_le(72623859790382856)) != 72623859790382856 {
                    return 99
                }
                if int_from_be(-1) != -1 {
                    return 100
                }
                if int_rotate_left(1, 3) != 8 {
                    return 21
                }
                if int_rotate_left(-9223372036854775808, 1) != 1 {
                    return 22
                }
                if int_rotate_right(8, 3) != 1 {
                    return 23
                }
                if int_rotate_right(1, 1) != -9223372036854775808 {
                    return 24
                }
                if int_checked_shl(1, 3) != 8 {
                    return 68
                }
                if int_checked_shl(1, 63) != -9223372036854775808 {
                    return 69
                }
                if int_checked_shl(1, 64) != 0 {
                    return 70
                }
                if int_checked_shr(8, 1) != 4 {
                    return 71
                }
                if int_checked_shr(-8, 1) != 9223372036854775804 {
                    return 72
                }
                if int_checked_shr(8, 64) != 0 {
                    return 73
                }
                if int_wrapping_shl(1, 64) != 1 {
                    return 74
                }
                if int_wrapping_shl(1, 65) != 2 {
                    return 75
                }
                if int_wrapping_shl(1, 63) != -9223372036854775808 {
                    return 76
                }
                if int_wrapping_shr(8, 64) != 8 {
                    return 77
                }
                if int_wrapping_shr(8, 65) != 4 {
                    return 78
                }
                if int_wrapping_shr(-8, 65) != 9223372036854775804 {
                    return 79
                }
                if int_arithmetic_shr(-8, 1) != -4 {
                    return 84
                }
                if int_arithmetic_shr(-1, 63) != -1 {
                    return 85
                }
                if int_arithmetic_shr(8, 1) != 4 {
                    return 86
                }
                if int_arithmetic_shr(-8, 65) != -4 {
                    return 87
                }
                if int_bit_width(0) != 0 {
                    return 28
                }
                if int_bit_width(8) != 4 {
                    return 29
                }
                if int_bit_width(-1) != 64 {
                    return 30
                }
                if int_lowest_one(40) != 8 {
                    return 181
                }
                if int_lowest_one(-8) != 8 {
                    return 182
                }
                if int_lowest_one(0) != 0 {
                    return 183
                }
                if int_highest_one(40) != 32 {
                    return 184
                }
                if int_highest_one(-1) != -9223372036854775808 {
                    return 185
                }
                if int_highest_one(0) != 0 {
                    return 186
                }
                if int_clear_lowest_one(40) != 32 {
                    return 192
                }
                if int_clear_lowest_one(0) != 0 {
                    return 193
                }
                if int_clear_highest_one(40) != 8 {
                    return 194
                }
                if int_clear_highest_one(-1) != 9223372036854775807 {
                    return 195
                }
                if int_clear_highest_one(0) != 0 {
                    return 196
                }
                if int_fill_ones_below(40) != 63 {
                    return 202
                }
                if int_fill_ones_below(0) != 0 {
                    return 203
                }
                if int_fill_ones_above(40) != -8 {
                    return 204
                }
                if int_fill_ones_above(0) != 0 {
                    return 205
                }
                if !int_bit_is_set(-1, 63) {
                    return 49
                }
                if int_bit_is_set(8, 1) {
                    return 50
                }
                if int_bit_is_set(-1, 64) {
                    return 51
                }
                if !int_bits_contains_all(14, 6) {
                    return 210
                }
                if int_bits_contains_all(8, 6) {
                    return 211
                }
                if !int_bits_contains_all(-1, 9223372036854775807) {
                    return 212
                }
                if !int_bits_disjoint(8, 6) {
                    return 213
                }
                if int_bits_disjoint(14, 6) {
                    return 214
                }
                if int_bit_set(8, 1) != 10 {
                    return 52
                }
                if int_bit_set(0, 63) != -9223372036854775808 {
                    return 53
                }
                if int_bit_set(1, 64) != 1 {
                    return 54
                }
                if int_low_mask(0) != 0 {
                    return 122
                }
                if int_low_mask(4) != 15 {
                    return 123
                }
                if int_low_mask(63) != 9223372036854775807 {
                    return 124
                }
                if int_low_mask(64) != -1 {
                    return 125
                }
                if int_range_mask(4, 3) != 112 {
                    return 126
                }
                if int_range_mask(60, 8) != -1152921504606846976 {
                    return 127
                }
                if int_range_mask(64, 1) != 0 {
                    return 128
                }
                if int_range_mask(8, 0) != 0 {
                    return 129
                }
                if int_sign_extend(127, 8) != 127 {
                    return 130
                }
                if int_sign_extend(128, 8) != -128 {
                    return 131
                }
                if int_sign_extend(255, 8) != -1 {
                    return 132
                }
                if int_sign_extend(2047, 12) != 2047 {
                    return 133
                }
                if int_sign_extend(2048, 12) != -2048 {
                    return 134
                }
                if int_sign_extend(1, 0) != 0 {
                    return 135
                }
                if int_sign_extend(-5, 64) != -5 {
                    return 136
                }
                if int_extract_bits(0x5a, 1, 4) != 13 {
                    return 137
                }
                if int_extract_bits(-1, 60, 8) != 15 {
                    return 138
                }
                if int_extract_bits(123, 64, 1) != 0 {
                    return 139
                }
                if int_extract_bits(123, 8, 0) != 0 {
                    return 140
                }
                if int_insert_bits(0, 13, 1, 4) != 26 {
                    return 145
                }
                if int_insert_bits(255, 0, 4, 4) != 15 {
                    return 146
                }
                if int_insert_bits(0, -1, 60, 8) != -1152921504606846976 {
                    return 147
                }
                if int_insert_bits(123, 7, 64, 1) != 123 {
                    return 148
                }
                if int_insert_bits(123, 7, 8, 0) != 123 {
                    return 149
                }
                if int_byte_at(0x0102030405060708, 0) != 8 {
                    return 107
                }
                if int_byte_at(0x0102030405060708, 7) != 1 {
                    return 108
                }
                if int_byte_at(-1, 7) != 255 {
                    return 109
                }
                if int_byte_at(0x0102030405060708, 8) != 0 {
                    return 110
                }
                if int_with_byte(0x0102030405060708, 0, 255) != 0x01020304050607ff {
                    return 111
                }
                if int_with_byte(0x0102030405060708, 7, 255) != -71494644084504824 {
                    return 112
                }
                if int_with_byte(0x0102030405060708, 8, 255) != 0x0102030405060708 {
                    return 113
                }
                if int_bit_clear(10, 1) != 8 {
                    return 55
                }
                if int_bit_clear(-1, 63) != 9223372036854775807 {
                    return 56
                }
                if int_bit_clear(1, 64) != 1 {
                    return 57
                }
                if int_bit_toggle(8, 1) != 10 {
                    return 58
                }
                if int_bit_toggle(10, 1) != 8 {
                    return 59
                }
                if int_bit_toggle(1, 63) != -9223372036854775807 {
                    return 60
                }
                if int_bit_toggle(1, 64) != 1 {
                    return 61
                }
                if usize_popcount(13) != 3 {
                    return 1
                }
                if usize_count_ones(13) != 3 {
                    return 167
                }
                if usize_count_ones(18446744073709551615) != 64 {
                    return 168
                }
                if usize_parity(0) {
                    return 160
                }
                if !usize_parity(1) {
                    return 161
                }
                if usize_parity(3) {
                    return 162
                }
                if !usize_parity(13) {
                    return 163
                }
                if usize_parity(18446744073709551615) {
                    return 164
                }
                if usize_count_zeros(0) != 64 {
                    return 31
                }
                if usize_count_zeros(13) != 61 {
                    return 32
                }
                if usize_count_zeros(18446744073709551615) != 0 {
                    return 33
                }
                if !usize_bit_is_set(10, 1) {
                    return 37
                }
                if usize_bit_is_set(10, 0) {
                    return 38
                }
                if usize_bit_is_set(1, 64) {
                    return 39
                }
                if !usize_bits_contains_all(14, 6) {
                    return 215
                }
                if usize_bits_contains_all(8, 6) {
                    return 216
                }
                if !usize_bits_contains_all(usize_low_mask(64), 9223372036854775807) {
                    return 217
                }
                if !usize_bits_disjoint(8, 6) {
                    return 218
                }
                if usize_bits_disjoint(14, 6) {
                    return 219
                }
                if usize_bit_set(8, 1) != 10 {
                    return 40
                }
                if usize_bit_set(10, 1) != 10 {
                    return 41
                }
                if usize_bit_set(1, 64) != 1 {
                    return 42
                }
                if usize_low_mask(0) != 0 {
                    return 114
                }
                if usize_low_mask(4) != 15 {
                    return 115
                }
                if usize_low_mask(64) != 18446744073709551615 {
                    return 116
                }
                if usize_low_mask(65) != 18446744073709551615 {
                    return 117
                }
                if usize_range_mask(4, 3) != 112 {
                    return 118
                }
                if usize_range_mask(60, 8) != 17293822569102704640 {
                    return 119
                }
                if usize_range_mask(64, 1) != 0 {
                    return 120
                }
                if usize_range_mask(8, 0) != 0 {
                    return 121
                }
                if usize_extract_bits(0x5a, 1, 4) != 13 {
                    return 141
                }
                if usize_extract_bits(18446744073709551615, 60, 8) != 15 {
                    return 142
                }
                if usize_extract_bits(123, 64, 1) != 0 {
                    return 143
                }
                if usize_extract_bits(123, 8, 0) != 0 {
                    return 144
                }
                if usize_insert_bits(0, 13, 1, 4) != 26 {
                    return 150
                }
                if usize_insert_bits(255, 0, 4, 4) != 15 {
                    return 151
                }
                if usize_insert_bits(0, 18446744073709551615, 60, 8) != 17293822569102704640 {
                    return 152
                }
                if usize_insert_bits(123, 7, 64, 1) != 123 {
                    return 153
                }
                if usize_insert_bits(123, 7, 8, 0) != 123 {
                    return 154
                }
                if usize_byte_at(0x0102030405060708, 0) != 8 {
                    return 101
                }
                if usize_byte_at(0x0102030405060708, 7) != 1 {
                    return 102
                }
                if usize_byte_at(0x0102030405060708, 8) != 0 {
                    return 103
                }
                if usize_with_byte(0x0102030405060708, 0, 255) != 0x01020304050607ff {
                    return 104
                }
                if usize_with_byte(0x0102030405060708, 7, 255) != 0xff02030405060708 {
                    return 105
                }
                if usize_with_byte(0x0102030405060708, 8, 255) != 0x0102030405060708 {
                    return 106
                }
                if usize_bit_clear(10, 1) != 8 {
                    return 43
                }
                if usize_bit_clear(8, 1) != 8 {
                    return 44
                }
                if usize_bit_clear(1, 64) != 1 {
                    return 45
                }
                if usize_bit_toggle(8, 1) != 10 {
                    return 46
                }
                if usize_bit_toggle(10, 1) != 8 {
                    return 47
                }
                if usize_bit_toggle(1, 64) != 1 {
                    return 48
                }
                if usize_trailing_zeros(8) != 3 {
                    return 2
                }
                if usize_trailing_zeros(0) != 64 {
                    return 3
                }
                if usize_leading_zeros(0) != 64 {
                    return 4
                }
                if usize_leading_ones(usize_low_mask(64)) != 64 {
                    return 175
                }
                if usize_leading_ones(9223372036854775808) != 1 {
                    return 176
                }
                if usize_leading_ones(0) != 0 {
                    return 177
                }
                if usize_trailing_ones(usize_low_mask(64)) != 64 {
                    return 178
                }
                if usize_trailing_ones(7) != 3 {
                    return 179
                }
                if usize_trailing_ones(0) != 0 {
                    return 180
                }
                if usize_reverse_bits(1) != 9223372036854775808 {
                    return 5
                }
                if usize_swap_bytes(72623859790382856) != 578437695752307201 {
                    return 6
                }
                if usize_from_be(72623859790382856) != 578437695752307201 {
                    return 88
                }
                if usize_to_be(72623859790382856) != 578437695752307201 {
                    return 89
                }
                if usize_from_le(72623859790382856) != 72623859790382856 {
                    return 90
                }
                if usize_to_le(72623859790382856) != 72623859790382856 {
                    return 91
                }
                if usize_from_be(usize_to_be(72623859790382856)) != 72623859790382856 {
                    return 92
                }
                if usize_from_le(usize_to_le(72623859790382856)) != 72623859790382856 {
                    return 93
                }
                if usize_bit_width(0) != 0 {
                    return 25
                }
                if usize_bit_width(1) != 1 {
                    return 26
                }
                if usize_bit_width(9223372036854775808) != 64 {
                    return 27
                }
                if usize_lowest_one(40) != 8 {
                    return 187
                }
                if usize_lowest_one(0) != 0 {
                    return 188
                }
                if usize_highest_one(40) != 32 {
                    return 189
                }
                if usize_highest_one(9223372036854775808) != 9223372036854775808 {
                    return 190
                }
                if usize_highest_one(0) != 0 {
                    return 191
                }
                if usize_clear_lowest_one(40) != 32 {
                    return 197
                }
                if usize_clear_lowest_one(0) != 0 {
                    return 198
                }
                if usize_clear_highest_one(40) != 8 {
                    return 199
                }
                if usize_clear_highest_one(9223372036854775808) != 0 {
                    return 200
                }
                if usize_clear_highest_one(0) != 0 {
                    return 201
                }
                if usize_fill_ones_below(40) != 63 {
                    return 206
                }
                if usize_fill_ones_below(0) != 0 {
                    return 207
                }
                if usize_fill_ones_above(40) != usize_low_mask(64) - 7 {
                    return 208
                }
                if usize_fill_ones_above(0) != 0 {
                    return 209
                }
                if usize_rotate_left(1, 3) != 8 {
                    return 7
                }
                if usize_rotate_right(8, 3) != 1 {
                    return 8
                }
                if usize_checked_shl(1, 3) != 8 {
                    return 62
                }
                if usize_checked_shl(1, 63) != 9223372036854775808 {
                    return 63
                }
                if usize_checked_shl(1, 64) != 0 {
                    return 64
                }
                if usize_checked_shr(8, 1) != 4 {
                    return 65
                }
                if usize_checked_shr(9223372036854775808, 63) != 1 {
                    return 66
                }
                if usize_checked_shr(8, 64) != 0 {
                    return 67
                }
                if usize_wrapping_shl(1, 64) != 1 {
                    return 80
                }
                if usize_wrapping_shl(1, 65) != 2 {
                    return 81
                }
                if usize_wrapping_shr(8, 64) != 8 {
                    return 82
                }
                if usize_wrapping_shr(8, 65) != 4 {
                    return 83
                }
                return 0
            }
        "#,
    )
    .expect("failed to write bits fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_random_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-random-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.random

            fn main() -> int {
                random_seed(123)
                let first: usize = random_usize()
                random_seed(123)
                let second: usize = random_usize()
                if first != second {
                    return 1
                }
                let bounded: usize = random_range(7)
                if bounded >= 7 {
                    return 2
                }
                if random_range(0) != 0 {
                    return 3
                }
                let inclusive: usize = random_range_inclusive(7)
                if inclusive > 7 {
                    return 4
                }
                if random_range_inclusive(0) != 0 {
                    return 5
                }
                let signed: int = random_int_range(-3, 3)
                if signed < -3 || signed >= 3 {
                    return 6
                }
                let collapsed: int = random_int_range(4, 4)
                if collapsed != 4 {
                    return 7
                }
                random_bool()
                return 0
            }
        "#,
    )
    .expect("failed to write random fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_hash_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let geo_path = dir.join(format!("geo-hash-{}.geo", std::process::id()));
    std::fs::write(
        &geo_path,
        r#"
            import std.hash

            fn main() -> int {
                let first: usize = hash_string("geo")
                let second: usize = hash_string("geo")
                let other: usize = hash_string("compiler")
                if first != second {
                    return 1
                }
                if first == other {
                    return 2
                }
                if hash_usize(42) == hash_usize(43) {
                    return 3
                }
                if hash_combine(first, hash_usize(1)) == hash_combine(first, hash_usize(2)) {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write hash fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_hashes_byte_buffers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-hash-bytes-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.hash
            import std.mem

            fn main() -> int {
                let left: *u8 = alloc(4)
                let right: *u8 = alloc(4)
                unsafe {
                    *(left + 0) = 71
                    *(left + 1) = 101
                    *(left + 2) = 111
                    *(left + 3) = 33
                    mem_copy(right, left, 4)
                    if hash_bytes(left, 4usize) != hash_bytes(right, 4usize) {
                        return 1
                    }
                    *(right + 3) = 63
                    if hash_bytes(left, 4usize) == hash_bytes(right, 4usize) {
                        return 2
                    }
                    if hash_bytes_seed(left, 4usize, 123usize) != hash_bytes_seed(left, 4usize, 123usize) {
                        return 3
                    }
                    if hash_bytes_seed(left, 4usize, 123usize) == hash_bytes_seed(left, 4usize, 456usize) {
                        return 4
                    }
                    if hash_bytes(left, 0usize) != hash_bytes(right, 0usize) {
                        return 5
                    }
                }
                free(left)
                free(right)
                return 0
            }
        "#,
    )
    .expect("failed to write hash_bytes fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_creates_checks_and_removes_directory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let target_dir = dir.join(format!("geo-created-dir-{}", std::process::id()));
    let geo_path = dir.join(format!("geo-created-dir-{}.geo", std::process::id()));
    let target = target_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    create_dir("{target}")
                    if dir_exists("{target}") {{
                        return remove_dir("{target}")
                    }} else {{
                        return 99
                    }}
                }}
            "#
        ),
    )
    .expect("failed to write directory fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let still_exists = target_dir.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&target_dir);

    assert!(status.success());
    assert!(!still_exists);
}

#[test]
fn native_run_creates_nested_directories_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let root_dir = dir.join(format!("geo-created-dir-all-{}", std::process::id()));
    let nested_dir = root_dir.join("cache").join("objects");
    let geo_path = dir.join(format!("geo-created-dir-all-{}.geo", std::process::id()));
    let nested = nested_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if create_dir_all("{nested}") != 0 {{
                        return 1
                    }}
                    if dir_exists("{nested}") {{
                        return 0
                    }}
                    return 2
                }}
            "#
        ),
    )
    .expect("failed to write create_dir_all fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let nested_exists = nested_dir.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&root_dir);

    assert!(status.success());
    assert!(nested_exists);
}

#[test]
fn native_run_counts_directory_entries_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let root_dir = dir.join(format!("geo-dir-entry-count-{}", std::process::id()));
    std::fs::create_dir_all(root_dir.join("nested"))
        .expect("failed to create dir_entry_count fixture directory");
    std::fs::write(root_dir.join("file.txt"), "data")
        .expect("failed to write dir_entry_count fixture file");
    let geo_path = dir.join(format!("geo-dir-entry-count-{}.geo", std::process::id()));
    let root = root_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if dir_entry_count("{root}") != 2 {{
                        return 1
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write dir_entry_count fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&root_dir);

    assert!(status.success());
}

#[test]
fn native_run_reads_directory_entry_name_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let root_dir = dir.join(format!("geo-dir-entry-name-{}", std::process::id()));
    std::fs::create_dir_all(&root_dir).expect("failed to create dir_entry_name fixture directory");
    std::fs::write(root_dir.join("only.txt"), "data")
        .expect("failed to write dir_entry_name fixture file");
    let geo_path = dir.join(format!("geo-dir-entry-name-{}.geo", std::process::id()));
    let root = root_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    if string_compare(dir_entry_name("{root}", 0), "only.txt") != 0 {{
                        return 1
                    }}
                    if string_len(dir_entry_name("{root}", 1)) != 0 {{
                        return 2
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write dir_entry_name fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&root_dir);

    assert!(status.success());
}

#[test]
fn native_run_reads_directory_entry_path_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let root_dir = dir.join(format!("geo-dir-entry-path-{}", std::process::id()));
    std::fs::create_dir_all(&root_dir).expect("failed to create dir_entry_path fixture directory");
    std::fs::write(root_dir.join("only.txt"), "data")
        .expect("failed to write dir_entry_path fixture file");
    let geo_path = dir.join(format!("geo-dir-entry-path-{}.geo", std::process::id()));
    let root = root_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io
                import std.string

                fn main() -> int {{
                    let child: string = dir_entry_path("{root}", 0)
                    if !file_is_file(child) {{
                        return 1
                    }}
                    if string_len(dir_entry_path("{root}", 1)) != 0 {{
                        return 2
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write dir_entry_path fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&root_dir);

    assert!(status.success());
}

#[test]
fn native_run_removes_nested_directories_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let dir = std::env::temp_dir();
    let root_dir = dir.join(format!("geo-remove-dir-all-{}", std::process::id()));
    let nested_dir = root_dir.join("cache").join("objects");
    std::fs::create_dir_all(&nested_dir).expect("failed to create remove_dir_all fixture");
    std::fs::write(nested_dir.join("stamp.txt"), "stamp")
        .expect("failed to write remove_dir_all fixture file");
    let geo_path = dir.join(format!("geo-remove-dir-all-{}.geo", std::process::id()));
    let root = root_dir.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(
        &geo_path,
        format!(
            r#"
                import std.io

                fn main() -> int {{
                    if remove_dir_all("{root}") != 0 {{
                        return 1
                    }}
                    if dir_exists("{root}") {{
                        return 2
                    }}
                    return 0
                }}
            "#
        ),
    )
    .expect("failed to write remove_dir_all fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", geo_path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let root_exists = root_dir.exists();
    let _ = std::fs::remove_file(&geo_path);
    let _ = std::fs::remove_dir_all(&root_dir);

    assert!(status.success());
    assert!(!root_exists);
}

#[test]
fn native_run_reports_process_arg_count_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-args-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process

            fn main() -> int {
                return arg_count()
            }
        "#,
    )
    .expect("failed to write process args fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(1));
}

#[test]
fn native_run_reports_current_exe_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-current-exe-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                let exe: string = current_exe()
                if string_len(exe) == 0 {
                    return 1
                }
                return 0
            }
        "#,
    )
    .expect("failed to write current_exe fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_process_id_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-process-id-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process

            fn main() -> int {
                if process_id() == 0 {
                    return 1
                }
                return 0
            }
        "#,
    )
    .expect("failed to write process_id fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_command_exit_status_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-run-command-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process

            fn main() -> int {
                return run_command("exit 7")
            }
        "#,
    )
    .expect("failed to write run_command fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(7));
}

#[test]
fn native_run_passes_program_args_after_separator_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-run-args-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                if arg_count() != 3 {
                    return 1
                }
                let first: string = arg(1)
                if string_compare(first, "alpha") != 0 {
                    return 2
                }
                let second: string = arg(2)
                if string_compare(second, "--flag") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write run args fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "run",
            path.to_string_lossy().as_ref(),
            "--",
            "alpha",
            "--flag",
        ])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_process_arg_exists_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-arg-exists-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process

            fn main() -> int {
                if !arg_exists(0) {
                    return 1
                }
                if arg_exists(1) {
                    return 2
                }
                if arg_exists(-1) {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write arg_exists fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_process_arg_or_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-arg-or-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                if string_compare(arg_or(1, "missing"), "alpha") != 0 {
                    return 1
                }
                if string_compare(arg_or(2, "fallback"), "fallback") != 0 {
                    return 2
                }
                if string_compare(arg_or(-1, "negative"), "negative") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write arg_or fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref(), "--", "alpha"])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reads_process_environment_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-env-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> usize {
                let value: string = env_get("GEO_TEST_ENV")
                return string_len(value)
            }
        "#,
    )
    .expect("failed to write env fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .env("GEO_TEST_ENV", "abcdef")
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(6));
}

#[test]
fn native_run_uses_process_env_get_or_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-env-get-or-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                if string_compare(env_get_or("GEO_TEST_ENV_GET_OR", "missing"), "geo") != 0 {
                    return 1
                }
                if string_compare(env_get_or("GEO_TEST_ENV_GET_OR_MISSING", "fallback"), "fallback") != 0 {
                    return 2
                }
                if string_compare(env_get_or("", "empty-name"), "empty-name") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write env_get_or fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .env("GEO_TEST_ENV_GET_OR", "geo")
        .env_remove("GEO_TEST_ENV_GET_OR_MISSING")
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reports_process_environment_exists_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-env-exists-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process

            fn main() -> int {
                if !env_exists("GEO_TEST_ENV_PRESENT") {
                    return 1
                }
                if env_exists("GEO_TEST_ENV_MISSING") {
                    return 2
                }
                if env_exists("") {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write env_exists fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .env("GEO_TEST_ENV_PRESENT", "")
        .env_remove("GEO_TEST_ENV_MISSING")
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_iterates_process_environment_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-env-iter-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                let count: usize = env_count()
                if count == 0 {
                    return 1
                }
                if string_len(env_name(0)) == 0 {
                    return 2
                }
                if string_len(env_name(count + 100)) != 0 {
                    return 3
                }
                let name: string = env_name(0)
                let value: string = env_value(0)
                if string_compare(env_get(name), value) != 0 {
                    return 4
                }
                if string_len(env_value(count + 100)) != 0 {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write env iteration fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .env("GEO_TEST_ENV_ITER", "present")
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_mutates_process_environment_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-env-mutate-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.process
            import std.string

            fn main() -> int {
                if env_set("GEO_TEST_ENV_MUTATE", "geo") != 0 {
                    return 1
                }
                let value: string = env_get("GEO_TEST_ENV_MUTATE")
                if string_compare(value, "geo") != 0 {
                    return 2
                }
                if env_remove("GEO_TEST_ENV_MUTATE") != 0 {
                    return 3
                }
                if env_exists("GEO_TEST_ENV_MUTATE") {
                    return 4
                }
                if env_set("", "bad") == 0 {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write env mutation fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .env_remove("GEO_TEST_ENV_MUTATE")
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_allocates_and_frees_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let ptr: *u8 = alloc(16)
                return free(ptr)
            }
        "#,
    )
    .expect("failed to write memory fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_allocates_zeroed_and_checked_arrays_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-mem-alloc-helpers-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let zeroed: *u8 = alloc_zeroed(4)
                let array: *u8 = alloc_array(2, 4)
                unsafe {
                    if zeroed == null {
                        return 1
                    }
                    if array == null {
                        return 2
                    }
                    if !mem_is_zero(zeroed, 4) {
                        return 3
                    }
                    if !mem_is_zero(array, 8) {
                        return 4
                    }
                    *(array + 0) = 65
                    *(array + 7) = 90
                    if mem_is_zero(array, 8) {
                        return 5
                    }
                    if alloc_array(9223372036854775808, 3) != null {
                        return 6
                    }
                    free(zeroed)
                    free(array)
                    return 0
                }
            }
        "#,
    )
    .expect("failed to write memory allocation helper fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_allocates_independent_memory_copies_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-alloc-copy-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let source: *u8 = alloc(4)
                unsafe {
                    *(source + 0) = 65
                    *(source + 1) = 66
                    *(source + 2) = 67
                    *(source + 3) = 68
                    let copied: *u8 = alloc_copy(source, 4)
                    if copied == null {
                        return 1
                    }
                    if !mem_equal(source, copied, 4) {
                        return 2
                    }
                    *(copied + 0) = 90
                    if mem_equal(source, copied, 4) {
                        return 3
                    }
                    if alloc_copy(null, 4) != null {
                        return 4
                    }
                    free(copied)
                    free(source)
                    return 0
                }
            }
        "#,
    )
    .expect("failed to write memory copy allocation fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reallocates_checked_arrays_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-mem-realloc-array-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let original: *u8 = alloc_array(1, 4)
                unsafe {
                    *(original + 0) = 65
                    *(original + 1) = 66
                    *(original + 2) = 67
                    *(original + 3) = 68
                    let grown: *u8 = realloc_array(original, 1, 8)
                    if grown == null {
                        return 1
                    }
                    if *(grown + 0) != 65 || *(grown + 3) != 68 {
                        return 2
                    }
                    *(grown + 7) = 90
                    if realloc_array(grown, 9223372036854775808, 3) != null {
                        return 3
                    }
                    free(grown)
                    return 0
                }
            }
        "#,
    )
    .expect("failed to write checked array reallocation fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_memory_alignment_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-align-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                if align_up(17usize, 8usize) != 24usize {
                    return 1
                }
                if align_down(17usize, 8usize) != 16usize {
                    return 2
                }
                if !is_aligned(16usize, 8usize) {
                    return 3
                }
                if is_aligned(18usize, 8usize) {
                    return 4
                }
                if align_up(17usize, 0usize) != 17usize {
                    return 5
                }
                if align_down(17usize, 0usize) != 17usize {
                    return 6
                }
                if is_aligned(16usize, 0usize) {
                    return 7
                }
                if align_up(18446744073709551615usize, 8usize) != 0usize {
                    return 8
                }
                return 0
            }
        "#,
    )
    .expect("failed to write memory alignment fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_compares_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-compare-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let left: *u8 = alloc(3)
                let right: *u8 = alloc(3)
                unsafe {
                    *(left + 0) = 65
                    *(left + 1) = 66
                    *(left + 2) = 67
                    *(right + 0) = 65
                    *(right + 1) = 66
                    *(right + 2) = 68
                }
                if mem_compare(left, right, 2) != 0 {
                    return 1
                }
                if mem_compare(left, right, 3) >= 0 {
                    return 2
                }
                if mem_compare(right, left, 3) <= 0 {
                    return 3
                }
                free(left)
                free(right)
                return 0
            }
        "#,
    )
    .expect("failed to write memory compare fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_memory_equality_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-equal-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let left: *u8 = alloc(3)
                let right: *u8 = alloc(3)
                unsafe {
                    *(left + 0) = 65
                    *(left + 1) = 66
                    *(left + 2) = 67
                    *(right + 0) = 65
                    *(right + 1) = 66
                    *(right + 2) = 68
                }
                if !mem_equal(left, right, 2) {
                    return 1
                }
                if mem_equal(left, right, 3) {
                    return 2
                }
                if !mem_equal(left, right, 0) {
                    return 3
                }
                free(left)
                free(right)
                return 0
            }
        "#,
    )
    .expect("failed to write memory equal fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_zeroed_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-is-zero-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(3)
                mem_fill(buffer, 3, 0)
                if !mem_is_zero(buffer, 3) {
                    return 1
                }
                unsafe {
                    *(buffer + 1) = 7
                }
                if mem_is_zero(buffer, 3) {
                    return 2
                }
                if !mem_is_zero(buffer, 0) {
                    return 3
                }
                free(buffer)
                return 0
            }
        "#,
    )
    .expect("failed to write memory zero check fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_swaps_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-swap-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let left: *u8 = alloc(3)
                let right: *u8 = alloc(3)
                let expected_left: *u8 = alloc(3)
                let expected_right: *u8 = alloc(3)
                unsafe {
                    *(left + 0) = 65
                    *(left + 1) = 66
                    *(left + 2) = 67
                    *(right + 0) = 88
                    *(right + 1) = 89
                    *(right + 2) = 90
                    *(expected_left + 0) = 88
                    *(expected_left + 1) = 89
                    *(expected_left + 2) = 90
                    *(expected_right + 0) = 65
                    *(expected_right + 1) = 66
                    *(expected_right + 2) = 67
                }
                if mem_swap(left, right, 3) != 0 {
                    return 1
                }
                if !mem_equal(left, expected_left, 3) {
                    return 2
                }
                if !mem_equal(right, expected_right, 3) {
                    return 3
                }
                if mem_swap(left, right, 0) != 0 {
                    return 4
                }
                free(left)
                free(right)
                free(expected_left)
                free(expected_right)
                return 0
            }
        "#,
    )
    .expect("failed to write memory swap fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reverses_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-reverse-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(5)
                let expected: *u8 = alloc(5)
                unsafe {
                    *(buffer + 0) = 1
                    *(buffer + 1) = 2
                    *(buffer + 2) = 3
                    *(buffer + 3) = 4
                    *(buffer + 4) = 5
                    *(expected + 0) = 5
                    *(expected + 1) = 4
                    *(expected + 2) = 3
                    *(expected + 3) = 2
                    *(expected + 4) = 1
                }
                if mem_reverse(buffer, 5) != 0 {
                    return 1
                }
                if !mem_equal(buffer, expected, 5) {
                    return 2
                }
                if mem_reverse(buffer, 0) != 0 {
                    return 3
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory reverse fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_fills_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-fill-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let left: *u8 = alloc(4)
                let right: *u8 = alloc(4)
                mem_fill(left, 4, 65)
                unsafe {
                    *(right + 0) = 65
                    *(right + 1) = 65
                    *(right + 2) = 65
                    *(right + 3) = 65
                }
                if mem_compare(left, right, 4) != 0 {
                    return 1
                }
                mem_fill(left, 2, 66)
                unsafe {
                    *(right + 0) = 66
                    *(right + 1) = 66
                }
                if mem_compare(left, right, 4) != 0 {
                    return 2
                }
                free(left)
                free(right)
                return 0
            }
        "#,
    )
    .expect("failed to write memory fill fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_replaces_memory_bytes_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-replace-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(5)
                let expected: *u8 = alloc(5)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 65
                    *(buffer + 3) = 67
                    *(buffer + 4) = 65
                    *(expected + 0) = 90
                    *(expected + 1) = 66
                    *(expected + 2) = 90
                    *(expected + 3) = 67
                    *(expected + 4) = 90
                }
                if mem_replace_byte(buffer, 5, 65, 90) != 3usize {
                    return 1
                }
                if !mem_equal(buffer, expected, 5) {
                    return 2
                }
                if mem_replace_byte(buffer, 5, 65, 88) != 0usize {
                    return 3
                }
                if mem_replace_byte(buffer, 0, 90, 65) != 0usize {
                    return 4
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory replace fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_replaces_memory_patterns_in_place_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-mem-replace-pattern-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(8)
                let pattern: *u8 = alloc(2)
                let replacement: *u8 = alloc(2)
                let short: *u8 = alloc(1)
                let expected: *u8 = alloc(8)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 65
                    *(buffer + 3) = 66
                    *(buffer + 4) = 67
                    *(buffer + 5) = 65
                    *(buffer + 6) = 66
                    *(buffer + 7) = 68
                    *(pattern + 0) = 65
                    *(pattern + 1) = 66
                    *(replacement + 0) = 88
                    *(replacement + 1) = 89
                    *(short + 0) = 90
                    *(expected + 0) = 88
                    *(expected + 1) = 89
                    *(expected + 2) = 88
                    *(expected + 3) = 89
                    *(expected + 4) = 67
                    *(expected + 5) = 88
                    *(expected + 6) = 89
                    *(expected + 7) = 68
                }
                if mem_replace_pattern(buffer, 8, pattern, 2, replacement, 2) != 3usize {
                    return 1
                }
                if !mem_equal(buffer, expected, 8) {
                    return 2
                }
                if mem_replace_pattern(buffer, 8, pattern, 2, replacement, 2) != 0usize {
                    return 3
                }
                if mem_replace_pattern(buffer, 8, pattern, 2, short, 1) != 0usize {
                    return 4
                }
                if mem_replace_pattern(buffer, 8, pattern, 0, replacement, 0) != 0usize {
                    return 5
                }
                if mem_replace_pattern(null, 8, pattern, 2, replacement, 2) != 0usize {
                    return 6
                }
                free(buffer)
                free(pattern)
                free(replacement)
                free(short)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory pattern replacement fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_xors_memory_bytes_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-xor-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(4)
                let expected: *u8 = alloc(4)
                unsafe {
                    *(buffer + 0) = 0
                    *(buffer + 1) = 15
                    *(buffer + 2) = 240
                    *(buffer + 3) = 255
                    *(expected + 0) = 255
                    *(expected + 1) = 240
                    *(expected + 2) = 15
                    *(expected + 3) = 0
                }
                if mem_xor_byte(buffer, 4, 255) != 0 {
                    return 1
                }
                if !mem_equal(buffer, expected, 4) {
                    return 2
                }
                if mem_xor_byte(buffer, 0, 255) != 0 {
                    return 3
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory xor fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_repeats_memory_pattern_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-repeat-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(7)
                let pattern: *u8 = alloc(3)
                let expected: *u8 = alloc(7)
                unsafe {
                    *(pattern + 0) = 65
                    *(pattern + 1) = 66
                    *(pattern + 2) = 67
                    *(expected + 0) = 65
                    *(expected + 1) = 66
                    *(expected + 2) = 67
                    *(expected + 3) = 65
                    *(expected + 4) = 66
                    *(expected + 5) = 67
                    *(expected + 6) = 65
                }
                if mem_repeat_pattern(buffer, 7, pattern, 3) != 0 {
                    return 1
                }
                if !mem_equal(buffer, expected, 7) {
                    return 2
                }
                if mem_repeat_pattern(buffer, 0, pattern, 3) != 0 {
                    return 3
                }
                free(buffer)
                free(pattern)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory repeat fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_rotates_memory_left_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-rotate-left-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(5)
                let expected: *u8 = alloc(5)
                unsafe {
                    *(buffer + 0) = 1
                    *(buffer + 1) = 2
                    *(buffer + 2) = 3
                    *(buffer + 3) = 4
                    *(buffer + 4) = 5
                    *(expected + 0) = 3
                    *(expected + 1) = 4
                    *(expected + 2) = 5
                    *(expected + 3) = 1
                    *(expected + 4) = 2
                }
                if mem_rotate_left(buffer, 5, 2) != 0 {
                    return 1
                }
                if !mem_equal(buffer, expected, 5) {
                    return 2
                }
                if mem_rotate_left(buffer, 5, 0) != 0 {
                    return 3
                }
                if mem_rotate_left(buffer, 0, 3) != 0 {
                    return 4
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory rotate-left fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_rotates_memory_right_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-mem-rotate-right-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(5)
                let expected: *u8 = alloc(5)
                unsafe {
                    *(buffer + 0) = 1
                    *(buffer + 1) = 2
                    *(buffer + 2) = 3
                    *(buffer + 3) = 4
                    *(buffer + 4) = 5
                    *(expected + 0) = 4
                    *(expected + 1) = 5
                    *(expected + 2) = 1
                    *(expected + 3) = 2
                    *(expected + 4) = 3
                }
                if mem_rotate_right(buffer, 5, 2) != 0 {
                    return 1
                }
                if !mem_equal(buffer, expected, 5) {
                    return 2
                }
                if mem_rotate_right(buffer, 5, 0) != 0 {
                    return 3
                }
                if mem_rotate_right(buffer, 0, 3) != 0 {
                    return 4
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory rotate-right fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_moves_overlapping_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-move-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(6)
                let expected: *u8 = alloc(6)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 68
                    *(buffer + 4) = 69
                    *(buffer + 5) = 70
                    *(expected + 0) = 65
                    *(expected + 1) = 65
                    *(expected + 2) = 66
                    *(expected + 3) = 67
                    *(expected + 4) = 68
                    *(expected + 5) = 70
                }
                if mem_move(buffer + 1, buffer, 4) != 0 {
                    return 1
                }
                if mem_compare(buffer, expected, 6) != 0 {
                    return 2
                }
                free(buffer)
                free(expected)
                return 0
            }
        "#,
    )
    .expect("failed to write memory move fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_copies_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-copy-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let source: *u8 = alloc(4)
                let target: *u8 = alloc(4)
                unsafe {
                    *(source + 0) = 71
                    *(source + 1) = 69
                    *(source + 2) = 79
                    *(source + 3) = 33
                    *(target + 0) = 0
                    *(target + 1) = 0
                    *(target + 2) = 0
                    *(target + 3) = 0
                }
                if mem_copy(target, source, 4) != 0 {
                    return 1
                }
                if mem_compare(source, target, 4) != 0 {
                    return 2
                }
                if mem_copy(target, source, 0) != 0 {
                    return 3
                }
                free(source)
                free(target)
                return 0
            }
        "#,
    )
    .expect("failed to write memory copy fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_finds_memory_bytes_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-find-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(5)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 66
                    *(buffer + 4) = 68
                }
                if mem_find(buffer, 5, 66) != 1 {
                    return 1
                }
                if mem_find(buffer + 2, 3, 66) != 1 {
                    return 2
                }
                if mem_find(buffer, 5, 90) != -1 {
                    return 3
                }
                if mem_find(buffer, 0, 65) != -1 {
                    return 4
                }
                free(buffer)
                return 0
            }
        "#,
    )
    .expect("failed to write memory find fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_finds_memory_patterns_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-mem-find-pattern-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(8)
                let pattern: *u8 = alloc(2)
                let missing: *u8 = alloc(2)
                let long: *u8 = alloc(9)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 65
                    *(buffer + 4) = 66
                    *(buffer + 5) = 67
                    *(buffer + 6) = 65
                    *(buffer + 7) = 66
                    *(pattern + 0) = 65
                    *(pattern + 1) = 66
                    *(missing + 0) = 90
                    *(missing + 1) = 90
                    *(long + 0) = 65
                }
                if mem_find_pattern(buffer, 8, pattern, 2) != 0 {
                    return 1
                }
                if mem_find_pattern(buffer + 1, 7, pattern, 2) != 2 {
                    return 2
                }
                if mem_last_find_pattern(buffer, 8, pattern, 2) != 6 {
                    return 3
                }
                if mem_count_pattern(buffer, 8, pattern, 2) != 3usize {
                    return 4
                }
                if mem_find_pattern(buffer, 8, missing, 2) != -1 {
                    return 5
                }
                if mem_last_find_pattern(buffer, 8, missing, 2) != -1 {
                    return 6
                }
                if mem_count_pattern(buffer, 8, missing, 2) != 0usize {
                    return 7
                }
                if mem_find_pattern(buffer, 8, pattern, 0) != 0 {
                    return 8
                }
                if mem_last_find_pattern(buffer, 8, pattern, 0) != 8 {
                    return 9
                }
                if mem_count_pattern(buffer, 8, pattern, 0) != 0usize {
                    return 10
                }
                if mem_find_pattern(buffer, 8, long, 9) != -1 {
                    return 11
                }
                if mem_find_pattern(null, 8, pattern, 2) != -1 {
                    return 12
                }
                if mem_find_pattern(buffer, 8, null, 2) != -1 {
                    return 13
                }
                free(buffer)
                free(pattern)
                free(missing)
                free(long)
                return 0
            }
        "#,
    )
    .expect("failed to write memory pattern search fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_counts_memory_splits_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-split-count-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let csv: *u8 = alloc(5)
                let repeated: *u8 = alloc(4)
                let pattern: *u8 = alloc(2)
                let missing: *u8 = alloc(2)
                unsafe {
                    *(csv + 0) = 65
                    *(csv + 1) = 44
                    *(csv + 2) = 44
                    *(csv + 3) = 66
                    *(csv + 4) = 44
                    *(repeated + 0) = 65
                    *(repeated + 1) = 65
                    *(repeated + 2) = 65
                    *(repeated + 3) = 65
                    *(pattern + 0) = 65
                    *(pattern + 1) = 65
                    *(missing + 0) = 90
                    *(missing + 1) = 90
                }
                if mem_split_count(csv, 5, 44) != 4usize {
                    return 1
                }
                if mem_split_count(csv, 5, 90) != 1usize {
                    return 2
                }
                if mem_split_count(csv, 0, 44) != 0usize {
                    return 3
                }
                if mem_split_count(null, 5, 44) != 0usize {
                    return 4
                }
                if mem_split_count_pattern(repeated, 4, pattern, 2) != 3usize {
                    return 5
                }
                if mem_split_count_pattern(repeated, 4, missing, 2) != 1usize {
                    return 6
                }
                if mem_split_count_pattern(repeated, 0, pattern, 2) != 0usize {
                    return 7
                }
                if mem_split_count_pattern(repeated, 4, pattern, 0) != 0usize {
                    return 8
                }
                if mem_split_count_pattern(null, 4, pattern, 2) != 0usize {
                    return 9
                }
                free(csv)
                free(repeated)
                free(pattern)
                free(missing)
                return 0
            }
        "#,
    )
    .expect("failed to write memory split count fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reads_memory_split_fields_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-mem-split-fields-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let csv: *u8 = alloc(5)
                let text: *u8 = alloc(8)
                let delimiter: *u8 = alloc(2)
                unsafe {
                    *(csv + 0) = 65
                    *(csv + 1) = 44
                    *(csv + 2) = 44
                    *(csv + 3) = 66
                    *(csv + 4) = 44
                    *(text + 0) = 65
                    *(text + 1) = 58
                    *(text + 2) = 58
                    *(text + 3) = 66
                    *(text + 4) = 67
                    *(text + 5) = 58
                    *(text + 6) = 58
                    *(text + 7) = 68
                    *(delimiter + 0) = 58
                    *(delimiter + 1) = 58
                }
                if mem_split_field_start(csv, 5, 44, 0usize) != 0 {
                    return 1
                }
                if mem_split_field_len(csv, 5, 44, 0usize) != 1usize {
                    return 2
                }
                if mem_split_field_start(csv, 5, 44, 1usize) != 2 {
                    return 3
                }
                if mem_split_field_len(csv, 5, 44, 1usize) != 0usize {
                    return 4
                }
                if mem_split_field_start(csv, 5, 44, 2usize) != 3 {
                    return 5
                }
                if mem_split_field_len(csv, 5, 44, 2usize) != 1usize {
                    return 6
                }
                if mem_split_field_start(csv, 5, 44, 3usize) != 5 {
                    return 7
                }
                if mem_split_field_len(csv, 5, 44, 3usize) != 0usize {
                    return 8
                }
                if mem_split_field_start(csv, 5, 44, 4usize) != -1 {
                    return 9
                }
                if mem_split_field_start_pattern(text, 8, delimiter, 2, 1usize) != 3 {
                    return 10
                }
                if mem_split_field_len_pattern(text, 8, delimiter, 2, 1usize) != 2usize {
                    return 11
                }
                if mem_split_field_start_pattern(text, 8, delimiter, 2, 2usize) != 7 {
                    return 12
                }
                if mem_split_field_len_pattern(text, 8, delimiter, 2, 2usize) != 1usize {
                    return 13
                }
                if mem_split_field_start_pattern(text, 8, delimiter, 0, 0usize) != -1 {
                    return 14
                }
                if mem_split_field_len_pattern(null, 8, delimiter, 2, 0usize) != 0usize {
                    return 15
                }
                free(csv)
                free(text)
                free(delimiter)
                return 0
            }
        "#,
    )
    .expect("failed to write memory split field fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_reads_memory_source_lines_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-lines-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let source: *u8 = alloc(12)
                let trailing: *u8 = alloc(2)
                unsafe {
                    *(source + 0) = 65
                    *(source + 1) = 10
                    *(source + 2) = 66
                    *(source + 3) = 13
                    *(source + 4) = 10
                    *(source + 5) = 10
                    *(source + 6) = 67
                    *(source + 7) = 68
                    *(source + 8) = 13
                    *(source + 9) = 10
                    *(source + 10) = 69
                    *(source + 11) = 70
                    *(trailing + 0) = 65
                    *(trailing + 1) = 10
                }
                if mem_line_count(source, 12) != 5usize {
                    return 1
                }
                if mem_line_start(source, 12, 0usize) != 0 {
                    return 2
                }
                if mem_line_len(source, 12, 0usize) != 1usize {
                    return 3
                }
                if mem_line_start(source, 12, 1usize) != 2 {
                    return 4
                }
                if mem_line_len(source, 12, 1usize) != 1usize {
                    return 5
                }
                if mem_line_start(source, 12, 2usize) != 5 {
                    return 6
                }
                if mem_line_len(source, 12, 2usize) != 0usize {
                    return 7
                }
                if mem_line_start(source, 12, 3usize) != 6 {
                    return 8
                }
                if mem_line_len(source, 12, 3usize) != 2usize {
                    return 9
                }
                if mem_line_start(source, 12, 4usize) != 10 {
                    return 10
                }
                if mem_line_len(source, 12, 4usize) != 2usize {
                    return 11
                }
                if mem_line_start(source, 12, 5usize) != -1 {
                    return 12
                }
                if mem_line_len(source, 0, 0usize) != 0usize {
                    return 13
                }
                if mem_line_count(trailing, 2) != 1usize {
                    return 14
                }
                if mem_line_count(null, 2) != 0usize {
                    return 15
                }
                if mem_line_index_at(source, 12, 0usize) != 0 {
                    return 16
                }
                if mem_column_at(source, 12, 0usize) != 0 {
                    return 17
                }
                if mem_line_index_at(source, 12, 3usize) != 1 {
                    return 18
                }
                if mem_column_at(source, 12, 3usize) != 1 {
                    return 19
                }
                if mem_line_index_at(source, 12, 5usize) != 2 {
                    return 20
                }
                if mem_column_at(source, 12, 5usize) != 0 {
                    return 21
                }
                if mem_line_index_at(source, 12, 8usize) != 3 {
                    return 22
                }
                if mem_column_at(source, 12, 8usize) != 2 {
                    return 23
                }
                if mem_line_index_at(source, 12, 12usize) != -1 {
                    return 24
                }
                if mem_column_at(null, 12, 0usize) != -1 {
                    return 25
                }
                if mem_offset_at_line_column(source, 12, 0usize, 0usize) != 0 {
                    return 26
                }
                if mem_offset_at_line_column(source, 12, 1usize, 1usize) != 3 {
                    return 27
                }
                if mem_offset_at_line_column(source, 12, 2usize, 0usize) != 5 {
                    return 28
                }
                if mem_offset_at_line_column(source, 12, 3usize, 2usize) != 8 {
                    return 29
                }
                if mem_offset_at_line_column(source, 12, 4usize, 2usize) != 12 {
                    return 30
                }
                if mem_offset_at_line_column(source, 12, 4usize, 3usize) != -1 {
                    return 31
                }
                if mem_offset_at_line_column(null, 12, 0usize, 0usize) != -1 {
                    return 32
                }
                free(source)
                free(trailing)
                return 0
            }
        "#,
    )
    .expect("failed to write memory line fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-helpers-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> usize {
                let joined: string = string_concat("Geo", "!")
                let part: string = substring(joined, 1, 2)
                let same: int = string_compare(part, "eo")
                return string_len(joined) + same
            }
        "#,
    )
    .expect("failed to write string helper fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(4));
}

#[test]
fn emits_string_search_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_contains("compiler.geo", ".geo") {
                if string_starts_with("compiler.geo", "compiler") {
                    if string_ends_with("compiler.geo", ".geo") {
                        if string_compare(string_before("compiler.geo", "."), "compiler") == 0 {
                            if string_compare(string_after("compiler.geo", "."), "geo") == 0 {
                                if string_compare(string_before_last("src/compiler.geo", "/"), "src") == 0 {
                                    if string_compare(string_after_last("src/compiler.geo", "/"), "compiler.geo") == 0 {
                                        if string_compare(string_strip_prefix("compiler.geo", "compiler."), "geo") == 0 {
                                            if string_compare(string_strip_suffix("compiler.geo", ".geo"), "compiler") == 0 {
                                                if string_compare(string_between("module[core].geo", "[", "]"), "core") == 0 {
                                                    if string_compare(string_between_last("module[core][parser].geo", "[", "]"), "parser") == 0 {
                                                        return 0
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_contains"));
    assert!(asm.contains("call string_starts_with"));
    assert!(asm.contains("call string_ends_with"));
    assert!(asm.contains("call string_before"));
    assert!(asm.contains("call string_after"));
    assert!(asm.contains("call string_before_last"));
    assert!(asm.contains("call string_after_last"));
    assert!(asm.contains("call string_strip_prefix"));
    assert!(asm.contains("call string_strip_suffix"));
    assert!(asm.contains("call string_between"));
    assert!(asm.contains("call string_between_last"));
}

#[test]
fn emits_string_compare_wrapper_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_eq("geo", "geo") && string_not_eq("geo", "rust") {
                if string_less("alpha", "beta") && string_less_or_equal("beta", "beta") {
                    if string_greater("zeta", "omega") && string_greater_or_equal("omega", "omega") {
                        return 0
                    }
                }
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_eq"));
    assert!(asm.contains("call string_not_eq"));
    assert!(asm.contains("call string_less"));
    assert!(asm.contains("call string_less_or_equal"));
    assert!(asm.contains("call string_greater"));
    assert!(asm.contains("call string_greater_or_equal"));
}

#[test]
fn emits_string_compare_ignore_case_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_compare_ignore_case("Geo", "geo") == 0 {
                if string_eq_ignore_case("Geo", "geo") && string_not_eq_ignore_case("Geo", "rust") {
                    if string_less_ignore_case("Alpha", "beta") && string_less_or_equal_ignore_case("BETA", "beta") {
                        if string_greater_ignore_case("Zeta", "omega") && string_greater_or_equal_ignore_case("OMEGA", "omega") {
                            return 0
                        }
                    }
                }
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_compare_ignore_case"));
    assert!(asm.contains("call string_eq_ignore_case"));
    assert!(asm.contains("call string_not_eq_ignore_case"));
    assert!(asm.contains("call string_less_ignore_case"));
    assert!(asm.contains("call string_less_or_equal_ignore_case"));
    assert!(asm.contains("call string_greater_ignore_case"));
    assert!(asm.contains("call string_greater_or_equal_ignore_case"));
}

#[test]
fn emits_string_is_empty_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_empty("") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_empty"));
}

#[test]
fn emits_string_is_ascii_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii("Geo_123") && string_is_utf8("Geo_123") && string_utf8_is_valid("Geo_123") && string_utf8_len("Geo_123") == 7 && string_len(string_utf8_char_at("Geo_123", 3usize)) == 1usize && string_utf8_codepoint_at("Geo_123", 3usize) == 95 && string_utf8_byte_offset("Geo_123", 3usize) == 3 && string_utf8_next_offset("Geo_123", 3usize) == 4 && string_utf8_prev_offset("Geo_123", 3usize) == 2 && string_utf8_index_at("Geo_123", 3usize) == 3 && string_utf8_is_boundary("Geo_123", 3usize) {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii"));
    assert!(asm.contains("call string_is_utf8"));
    assert!(asm.contains("call string_utf8_is_valid"));
    assert!(asm.contains("call string_utf8_len"));
    assert!(asm.contains("call string_utf8_char_at"));
    assert!(asm.contains("call string_utf8_codepoint_at"));
    assert!(asm.contains("call string_utf8_byte_offset"));
    assert!(asm.contains("call string_utf8_next_offset"));
    assert!(asm.contains("call string_utf8_prev_offset"));
    assert!(asm.contains("call string_utf8_index_at"));
    assert!(asm.contains("call string_utf8_is_boundary"));
}

#[test]
fn emits_string_is_ascii_digit_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_digit("12345") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_digit"));
}

#[test]
fn emits_string_is_ascii_hex_digit_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_hex_digit("0123456789abcdefABCDEF") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_hex_digit"));
}

#[test]
fn emits_string_is_ascii_alpha_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_alpha("GeoLang") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_alpha"));
}

#[test]
fn emits_string_is_ascii_lower_upper_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_lower("geo") {
                if string_is_ascii_upper("GEO") {
                    return 0
                }
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_lower"));
    assert!(asm.contains("call string_is_ascii_upper"));
}

#[test]
fn emits_string_is_ascii_alnum_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_alnum("Geo123") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_alnum"));
}

#[test]
fn emits_string_is_ascii_identifier_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_identifier("_geo123") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_identifier"));
}

#[test]
fn emits_string_is_ascii_whitespace_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            if string_is_ascii_whitespace(" \t\n") {
                return 0
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_is_ascii_whitespace"));
}

#[test]
fn emits_string_index_of_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            return string_index_of("compiler.geo", ".geo")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_index_of"));
}

#[test]
fn emits_string_last_index_of_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            return string_last_index_of("compiler.geo.compiler.geo", ".geo")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_last_index_of"));
}

#[test]
fn emits_string_count_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            return string_count("compiler.geo.compiler.geo", ".geo") + string_utf8_count_codepoint("banana", 97)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_count"));
    assert!(asm.contains("call string_utf8_count_codepoint"));
}

#[test]
fn emits_string_trim_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            let left: string = string_trim_start("  Geo")
            let right: string = string_trim_end("Geo  ")
            let both: string = string_trim("  Geo  ")
            let uncommented: string = string_strip_ascii_line_comment("Geo // comment")
            let blockless: string = string_strip_ascii_block_comment("Geo /* comment */ lang")
            let collapsed: string = string_collapse_ascii_whitespace(" Geo\t compiler \n source ")
            let codepoint: string = string_utf8_trim_start_codepoint("GGGeo", 71)
            let end_codepoint: string = string_utf8_trim_end_codepoint("GeoOO", 79)
            let both_codepoint: string = string_utf8_trim_codepoint("GGGeoGG", 71)
            return string_len(left) as int + string_len(right) as int + string_len(both) as int + string_len(uncommented) as int + string_len(blockless) as int + string_len(collapsed) as int + string_len(codepoint) as int + string_len(end_codepoint) as int + string_len(both_codepoint) as int
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_trim_start"));
    assert!(asm.contains("call string_trim_end"));
    assert!(asm.contains("call string_trim"));
    assert!(asm.contains("call string_strip_ascii_line_comment"));
    assert!(asm.contains("call string_strip_ascii_block_comment"));
    assert!(asm.contains("call string_collapse_ascii_whitespace"));
    assert!(asm.contains("call string_utf8_trim_start_codepoint"));
    assert!(asm.contains("call string_utf8_trim_end_codepoint"));
    assert!(asm.contains("call string_utf8_trim_codepoint"));
}

#[test]
fn emits_string_line_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let line: string = string_line_at("one\ntwo\n", 1usize)
            let indented: string = string_indent("one\ntwo", "> ")
            let prefixed: string = string_prefix_lines("one\ntwo\n", "> ")
            let suffixed: string = string_suffix_lines("one\ntwo\n", " <")
            let dedented: string = string_dedent("> one\n> two", "> ")
            let line_index: int = string_line_index_at("one\ntwo\n", 4usize)
            let column: int = string_column_at("one\ntwo\n", 4usize)
            let offset: int = string_offset_at_line_column("one\ntwo\n", 1usize, 1usize)
            return string_line_count("one\ntwo\n") + string_len(line) + string_len(indented) + string_len(prefixed) + string_len(suffixed) + string_len(dedented) + line_index as usize + column as usize + offset as usize
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_line_count"));
    assert!(asm.contains("call string_line_at"));
    assert!(asm.contains("call string_indent"));
    assert!(asm.contains("call string_prefix_lines"));
    assert!(asm.contains("call string_suffix_lines"));
    assert!(asm.contains("call string_dedent"));
    assert!(asm.contains("call string_line_index_at"));
    assert!(asm.contains("call string_column_at"));
    assert!(asm.contains("call string_offset_at_line_column"));
}

#[test]
fn emits_string_slice_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_slice("compiler.geo", 0usize, 8usize)
            let prefix: string = string_take("compiler.geo", 8usize)
            let suffix: string = string_drop("compiler.geo", 9usize)
            let tail: string = string_take_last("compiler.geo", 3usize)
            let without_tail: string = string_drop_last("compiler.geo", 4usize)
            let utf8: string = string_utf8_slice("compiler.geo", 0usize, 8usize)
            let taken_while: string = string_utf8_take_while_codepoint("GGGeo", 71)
            let taken_until: string = string_utf8_take_until_codepoint("Geo", 111)
            let through_codepoint: string = string_utf8_through_codepoint("Geo", 101)
            let through_last_codepoint: string = string_utf8_through_last_codepoint("a.b.c", 46)
            let between_codepoints: string = string_utf8_between_codepoints("[Geo]", 91, 93)
            let between_last_codepoints: string = string_utf8_between_last_codepoints("[one][two]", 91, 93)
            let between_outer_codepoints: string = string_utf8_between_outer_codepoints("[one][two]", 91, 93)
            let before_codepoint: string = string_utf8_before_codepoint("Geo", 111)
            let before_last_codepoint: string = string_utf8_before_last_codepoint("Geo.geo", 46)
            let dropped_until: string = string_utf8_drop_until_codepoint("Geo", 111)
            let after_codepoint: string = string_utf8_after_codepoint("Geo", 101)
            let after_last_codepoint: string = string_utf8_after_last_codepoint("Geo.geo", 46)
            let dropped_while: string = string_utf8_drop_while_codepoint("GGGeo", 71)
            let stripped: string = string_utf8_strip_prefix_codepoint("Geo", 71)
            let stripped_suffix: string = string_utf8_strip_suffix_codepoint("Geo", 111)
            return string_len(value) + string_len(prefix) + string_len(suffix) + string_len(tail) + string_len(without_tail) + string_len(utf8) + string_len(taken_while) + string_len(taken_until) + string_len(through_codepoint) + string_len(through_last_codepoint) + string_len(between_codepoints) + string_len(between_last_codepoints) + string_len(between_outer_codepoints) + string_len(before_codepoint) + string_len(before_last_codepoint) + string_len(dropped_until) + string_len(after_codepoint) + string_len(after_last_codepoint) + string_len(dropped_while) + string_len(stripped) + string_len(stripped_suffix)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_slice"));
    assert!(asm.contains("call string_utf8_slice"));
    assert!(asm.contains("call string_utf8_take_while_codepoint"));
    assert!(asm.contains("call string_utf8_take_until_codepoint"));
    assert!(asm.contains("call string_utf8_through_codepoint"));
    assert!(asm.contains("call string_utf8_through_last_codepoint"));
    assert!(asm.contains("call string_utf8_between_codepoints"));
    assert!(asm.contains("call string_utf8_between_last_codepoints"));
    assert!(asm.contains("call string_utf8_between_outer_codepoints"));
    assert!(asm.contains("call string_utf8_before_codepoint"));
    assert!(asm.contains("call string_utf8_before_last_codepoint"));
    assert!(asm.contains("call string_utf8_drop_until_codepoint"));
    assert!(asm.contains("call string_utf8_after_codepoint"));
    assert!(asm.contains("call string_utf8_after_last_codepoint"));
    assert!(asm.contains("call string_utf8_drop_while_codepoint"));
    assert!(asm.contains("call string_utf8_strip_prefix_codepoint"));
    assert!(asm.contains("call string_utf8_strip_suffix_codepoint"));
    assert!(asm.contains("call string_take"));
    assert!(asm.contains("call string_drop"));
    assert!(asm.contains("call string_take_last"));
    assert!(asm.contains("call string_drop_last"));
}

#[test]
fn emits_string_case_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            let lower: string = string_to_lower("GeoLANG")
            let upper: string = string_to_upper("GeoLANG")
            if ascii_is_digit(48) && ascii_is_hex_digit(70) && ascii_is_identifier_start(95) && ascii_is_identifier_continue(57) && unicode_is_ascii_digit(48) && unicode_is_ascii_hex_digit(70) && unicode_is_ascii_identifier_start(65) && unicode_is_ascii_identifier_continue(57) && ascii_is_alpha(65) && ascii_is_alnum(122) && unicode_is_ascii_alpha(65) && unicode_is_ascii_alnum(122) && ascii_is_whitespace(32) && unicode_is_ascii_whitespace(32) {
                return string_len(lower) as int + string_len(upper) as int + ascii_to_lower(65) + ascii_to_upper(97) + unicode_ascii_to_lower(65) + unicode_ascii_to_upper(97) + ascii_digit_value(57) + ascii_hex_value(70) + unicode_ascii_digit_value(57) + unicode_ascii_hex_value(70)
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_to_lower"));
    assert!(asm.contains("call string_to_upper"));
    assert!(asm.contains("call ascii_to_lower"));
    assert!(asm.contains("call ascii_to_upper"));
    assert!(asm.contains("call unicode_ascii_to_lower"));
    assert!(asm.contains("call unicode_ascii_to_upper"));
    assert!(asm.contains("call ascii_digit_value"));
    assert!(asm.contains("call ascii_hex_value"));
    assert!(asm.contains("call unicode_ascii_digit_value"));
    assert!(asm.contains("call unicode_ascii_hex_value"));
    assert!(asm.contains("call ascii_is_digit"));
    assert!(asm.contains("call ascii_is_hex_digit"));
    assert!(asm.contains("call unicode_is_ascii_digit"));
    assert!(asm.contains("call unicode_is_ascii_hex_digit"));
    assert!(asm.contains("call ascii_is_identifier_start"));
    assert!(asm.contains("call ascii_is_identifier_continue"));
    assert!(asm.contains("call unicode_is_ascii_identifier_start"));
    assert!(asm.contains("call unicode_is_ascii_identifier_continue"));
    assert!(asm.contains("call ascii_is_alpha"));
    assert!(asm.contains("call ascii_is_alnum"));
    assert!(asm.contains("call unicode_is_ascii_alpha"));
    assert!(asm.contains("call unicode_is_ascii_alnum"));
    assert!(asm.contains("call ascii_is_whitespace"));
    assert!(asm.contains("call unicode_is_ascii_whitespace"));
}

#[test]
fn emits_string_reverse_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_reverse("Geo")
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_reverse"));
}

#[test]
fn emits_string_replace_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_replace("src\\main.geo", "\\", "/")
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_replace"));
}

#[test]
fn emits_string_replace_all_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_replace_all("src\\main\\compiler.geo", "\\", "/")
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_replace_all"));
}

#[test]
fn emits_string_escape_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_escape("Geo\n\"lang\"")
            let ascii: string = string_escape_ascii("Geo\n")
            let decoded: string = string_unescape("Geo\\n\\\"lang\\\"")
            let decoded_hex: string = string_unescape_hex("\\x47\\x65\\x6f")
            let decoded_unicode: string = string_unescape_unicode("\\u{03bb}")
            return string_len(value) + string_len(ascii) + string_len(decoded) + string_len(decoded_hex) + string_len(decoded_unicode)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_escape"));
    assert!(asm.contains("call string_escape_ascii"));
    assert!(asm.contains("call string_unescape"));
    assert!(asm.contains("call string_unescape_hex"));
    assert!(asm.contains("call string_unescape_unicode"));
}

#[test]
fn emits_string_repeat_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = string_repeat("geo", 3usize)
            let left: string = string_pad_start("42", 5usize, "0")
            let right: string = string_pad_end("geo", 5usize, ".")
            return string_len(value) + string_len(left) + string_len(right)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_repeat"));
    assert!(asm.contains("call string_pad_start"));
    assert!(asm.contains("call string_pad_end"));
}

#[test]
fn emits_string_parse_int_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> int {
            return string_parse_int("-42")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_parse_int"));
}

#[test]
fn emits_string_parse_usize_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            return string_parse_usize("42")
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_parse_usize"));
}

#[test]
fn emits_string_try_parse_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> int {
            var parsed_int: int = 0
            var parsed_usize: usize = 0usize
            unsafe {
                if string_try_parse_int("-42", &parsed_int) && string_try_parse_usize("42", &parsed_usize) {
                    return parsed_int + parsed_usize as int
                }
            }
            return 1
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call string_try_parse_int"));
    assert!(asm.contains("call string_try_parse_usize"));
}

#[test]
fn emits_integer_to_string_runtime_calls() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let negative: string = int_to_string(-42)
            let size: string = usize_to_string(42usize)
            return string_len(negative) + string_len(size)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call int_to_string"));
    assert!(asm.contains("call usize_to_string"));
}

#[test]
fn emits_bool_to_string_runtime_call() {
    let source = r#"
        import std.string

        fn main() -> usize {
            let value: string = bool_to_string(true)
            return string_len(value)
        }
    "#;
    let asm = asm_for(source);

    assert!(asm.contains("call bool_to_string"));
}

#[test]
fn native_run_uses_string_search_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-search-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_contains("compiler.geo", ".geo") {
                    return 1
                }
                if string_contains("compiler.geo", ".rs") {
                    return 2
                }
                if !string_starts_with("compiler.geo", "compiler") {
                    return 3
                }
                if string_starts_with("compiler.geo", "runtime") {
                    return 4
                }
                if !string_ends_with("compiler.geo", ".geo") {
                    return 5
                }
                if string_ends_with("compiler.geo", ".rs") {
                    return 6
                }
                if string_compare(string_before("compiler.geo", "."), "compiler") != 0 {
                    return 7
                }
                if string_compare(string_before("compiler.geo", "/"), "compiler.geo") != 0 {
                    return 8
                }
                if string_compare(string_before("compiler.geo", ""), "") != 0 {
                    return 9
                }
                if string_compare(string_after("compiler.geo", "."), "geo") != 0 {
                    return 10
                }
                if string_compare(string_after("compiler.geo", "/"), "") != 0 {
                    return 11
                }
                if string_compare(string_after("compiler.geo", ""), "compiler.geo") != 0 {
                    return 12
                }
                if string_compare(string_before_last("src/compiler.geo", "/"), "src") != 0 {
                    return 13
                }
                if string_compare(string_before_last("pkg/src/compiler.geo", "/"), "pkg/src") != 0 {
                    return 14
                }
                if string_compare(string_before_last("compiler.geo", "/"), "compiler.geo") != 0 {
                    return 15
                }
                if string_compare(string_before_last("compiler.geo", ""), "") != 0 {
                    return 16
                }
                if string_compare(string_after_last("src/compiler.geo", "/"), "compiler.geo") != 0 {
                    return 17
                }
                if string_compare(string_after_last("pkg/src/compiler.geo", "/"), "compiler.geo") != 0 {
                    return 18
                }
                if string_compare(string_after_last("compiler.geo", "/"), "") != 0 {
                    return 19
                }
                if string_compare(string_after_last("compiler.geo", ""), "compiler.geo") != 0 {
                    return 20
                }
                if string_compare(string_strip_prefix("compiler.geo", "compiler."), "geo") != 0 {
                    return 21
                }
                if string_compare(string_strip_prefix("compiler.geo", "runtime."), "compiler.geo") != 0 {
                    return 22
                }
                if string_compare(string_strip_prefix("compiler.geo", ""), "compiler.geo") != 0 {
                    return 23
                }
                if string_compare(string_strip_suffix("compiler.geo", ".geo"), "compiler") != 0 {
                    return 24
                }
                if string_compare(string_strip_suffix("compiler.geo", ".rs"), "compiler.geo") != 0 {
                    return 25
                }
                if string_compare(string_strip_suffix("compiler.geo", ""), "compiler.geo") != 0 {
                    return 26
                }
                if string_compare(string_between("module[core].geo", "[", "]"), "core") != 0 {
                    return 27
                }
                if string_compare(string_between("module[core].geo", "{", "}"), "") != 0 {
                    return 28
                }
                if string_compare(string_between("module[core.geo", "[", "]"), "") != 0 {
                    return 29
                }
                if string_compare(string_between("module[core].geo", "", "]"), "") != 0 {
                    return 30
                }
                if string_compare(string_between("module[core].geo", "[", ""), "") != 0 {
                    return 31
                }
                if string_compare(string_between_last("module[core][parser].geo", "[", "]"), "parser") != 0 {
                    return 32
                }
                if string_compare(string_between_last("module[core][parser.geo", "[", "]"), "core") != 0 {
                    return 33
                }
                if string_compare(string_between_last("module[core].geo", "{", "}"), "") != 0 {
                    return 34
                }
                if string_compare(string_between_last("module[core].geo", "", "]"), "") != 0 {
                    return 35
                }
                if string_compare(string_between_last("module[core].geo", "[", ""), "") != 0 {
                    return 36
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string search fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_compare_wrappers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-compare-wrappers-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_eq("geo", "geo") || string_eq("geo", "Geo") {
                    return 1
                }
                if !string_not_eq("geo", "rust") || string_not_eq("geo", "geo") {
                    return 2
                }
                if !string_less("alpha", "beta") || string_less("beta", "alpha") {
                    return 3
                }
                if !string_less_or_equal("alpha", "beta") || !string_less_or_equal("beta", "beta") {
                    return 4
                }
                if !string_greater("zeta", "omega") || string_greater("alpha", "beta") {
                    return 5
                }
                if !string_greater_or_equal("zeta", "omega") || !string_greater_or_equal("omega", "omega") {
                    return 6
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string compare wrapper fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_compare_ignore_case_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-compare-ignore-case-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_compare_ignore_case("Geo", "geo") != 0 {
                    return 1
                }
                if string_compare_ignore_case("alpha", "BETA") >= 0 {
                    return 2
                }
                if string_compare_ignore_case("ZETA", "omega") <= 0 {
                    return 3
                }
                if !string_eq_ignore_case("Geo", "geo") || string_eq_ignore_case("Geo", "rust") {
                    return 4
                }
                if !string_not_eq_ignore_case("Geo", "rust") || string_not_eq_ignore_case("Geo", "geo") {
                    return 5
                }
                if !string_less_ignore_case("Alpha", "beta") || string_less_ignore_case("beta", "Alpha") {
                    return 6
                }
                if !string_less_or_equal_ignore_case("Alpha", "beta") || !string_less_or_equal_ignore_case("BETA", "beta") {
                    return 7
                }
                if !string_greater_ignore_case("Zeta", "omega") || string_greater_ignore_case("Alpha", "beta") {
                    return 8
                }
                if !string_greater_or_equal_ignore_case("Zeta", "omega") || !string_greater_or_equal_ignore_case("OMEGA", "omega") {
                    return 9
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string compare ignore case fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_empty_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-is-empty-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_empty("") {
                    return 1
                }
                if string_is_empty("geo") {
                    return 2
                }
                if string_is_empty(string_trim("   ")) == false {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_empty fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-is-ascii-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii("Geo_123") {
                    return 1
                }
                if !string_is_ascii("") {
                    return 2
                }
                let bytes: *u8 = alloc(3)
                unsafe {
                    *(bytes + 0) = 71
                    *(bytes + 1) = 195
                    *(bytes + 2) = 0
                }
                let text: string = bytes as string
                if string_is_ascii(text) {
                    return 3
                }
                if string_is_utf8(text) {
                    return 4
                }
                if string_utf8_is_valid(text) {
                    return 33
                }
                if string_utf8_len(text) != -1 {
                    return 5
                }
                if string_len(string_utf8_char_at(text, 0usize)) != 0usize {
                    return 6
                }
                if string_utf8_codepoint_at(text, 0usize) != -1 {
                    return 7
                }
                if string_utf8_byte_offset(text, 1usize) != -1 {
                    return 8
                }
                if string_utf8_next_offset(text, 0usize) != -1 {
                    return 9
                }
                if string_utf8_prev_offset(text, 1usize) != -1 {
                    return 10
                }
                if string_utf8_index_at(text, 1usize) != -1 {
                    return 11
                }
                if string_utf8_is_boundary(text, 1usize) {
                    return 12
                }
                free(bytes)
                if !string_is_utf8("Geo") {
                    return 13
                }
                if !string_utf8_is_valid("Geo") {
                    return 34
                }
                if string_utf8_len("Geo") != 3 {
                    return 14
                }
                let ascii_char: string = string_utf8_char_at("Geo", 1usize)
                if string_compare(ascii_char, "e") != 0 {
                    return 15
                }
                if string_utf8_codepoint_at("Geo", 1usize) != 101 {
                    return 16
                }
                if string_len(string_utf8_char_at("Geo", 3usize)) != 0usize {
                    return 17
                }
                if string_utf8_codepoint_at("Geo", 3usize) != -1 {
                    return 18
                }
                if string_utf8_byte_offset("Geo", 2usize) != 2 {
                    return 19
                }
                if string_utf8_byte_offset("Geo", 3usize) != 3 {
                    return 20
                }
                if string_utf8_byte_offset("Geo", 4usize) != -1 {
                    return 21
                }
                if string_utf8_next_offset("Geo", 0usize) != 1 {
                    return 22
                }
                if string_utf8_next_offset("Geo", 2usize) != 3 {
                    return 23
                }
                if string_utf8_next_offset("Geo", 3usize) != 3 {
                    return 24
                }
                if string_utf8_next_offset("Geo", 4usize) != -1 {
                    return 25
                }
                if string_utf8_prev_offset("Geo", 0usize) != 0 {
                    return 26
                }
                if string_utf8_prev_offset("Geo", 1usize) != 0 {
                    return 27
                }
                if string_utf8_prev_offset("Geo", 3usize) != 2 {
                    return 28
                }
                if string_utf8_prev_offset("Geo", 4usize) != -1 {
                    return 29
                }
                if string_utf8_index_at("Geo", 2usize) != 2 {
                    return 30
                }
                if string_utf8_index_at("Geo", 3usize) != 3 {
                    return 31
                }
                if string_utf8_index_at("Geo", 4usize) != -1 {
                    return 32
                }
                if !string_utf8_is_boundary("Geo", 0usize) {
                    return 33
                }
                if !string_utf8_is_boundary("Geo", 3usize) {
                    return 34
                }
                if string_utf8_is_boundary("Geo", 4usize) {
                    return 35
                }
                let lambda: string = string_unescape_unicode("\\u{03bb}")
                if !string_is_utf8(lambda) {
                    return 36
                }
                if string_len(lambda) != 2usize {
                    return 37
                }
                if string_utf8_len(lambda) != 1 {
                    return 38
                }
                let lambda_char: string = string_utf8_char_at(lambda, 0usize)
                if string_compare(lambda_char, lambda) != 0 {
                    return 39
                }
                if string_len(lambda_char) != 2usize {
                    return 40
                }
                if string_utf8_codepoint_at(lambda, 0usize) != 955 {
                    return 41
                }
                if string_len(string_utf8_char_at(lambda, 1usize)) != 0usize {
                    return 42
                }
                if string_utf8_codepoint_at(lambda, 1usize) != -1 {
                    return 43
                }
                if string_utf8_byte_offset(lambda, 0usize) != 0 {
                    return 44
                }
                if string_utf8_byte_offset(lambda, 1usize) != 2 {
                    return 45
                }
                if string_utf8_byte_offset(lambda, 2usize) != -1 {
                    return 46
                }
                if string_utf8_next_offset(lambda, 0usize) != 2 {
                    return 47
                }
                if string_utf8_next_offset(lambda, 1usize) != -1 {
                    return 48
                }
                if string_utf8_next_offset(lambda, 2usize) != 2 {
                    return 49
                }
                if string_utf8_prev_offset(lambda, 0usize) != 0 {
                    return 50
                }
                if string_utf8_prev_offset(lambda, 1usize) != -1 {
                    return 51
                }
                if string_utf8_prev_offset(lambda, 2usize) != 0 {
                    return 52
                }
                if string_utf8_index_at(lambda, 0usize) != 0 {
                    return 53
                }
                if string_utf8_index_at(lambda, 1usize) != -1 {
                    return 54
                }
                if string_utf8_index_at(lambda, 2usize) != 1 {
                    return 55
                }
                if !string_utf8_is_boundary(lambda, 0usize) {
                    return 56
                }
                if string_utf8_is_boundary(lambda, 1usize) {
                    return 57
                }
                if !string_utf8_is_boundary(lambda, 2usize) {
                    return 58
                }
                let face: string = string_unescape_unicode("\\u{1f600}")
                if string_utf8_codepoint_at(face, 0usize) != 128512 {
                    return 59
                }
                if string_utf8_len("") != 0 {
                    return 60
                }
                if string_len(string_utf8_char_at("", 0usize)) != 0usize {
                    return 61
                }
                if string_utf8_codepoint_at("", 0usize) != -1 {
                    return 62
                }
                if string_utf8_byte_offset("", 0usize) != 0 {
                    return 63
                }
                if string_utf8_byte_offset("", 1usize) != -1 {
                    return 64
                }
                if string_utf8_next_offset("", 0usize) != 0 {
                    return 65
                }
                if string_utf8_next_offset("", 1usize) != -1 {
                    return 66
                }
                if string_utf8_prev_offset("", 0usize) != 0 {
                    return 67
                }
                if string_utf8_prev_offset("", 1usize) != -1 {
                    return 68
                }
                if string_utf8_index_at("", 0usize) != 0 {
                    return 69
                }
                if string_utf8_index_at("", 1usize) != -1 {
                    return 70
                }
                if !string_utf8_is_boundary("", 0usize) {
                    return 71
                }
                if string_utf8_is_boundary("", 1usize) {
                    return 72
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_digit_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-digit-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_digit("12345") {
                    return 1
                }
                if string_is_ascii_digit("12a45") {
                    return 2
                }
                if string_is_ascii_digit("") {
                    return 3
                }
                if string_is_ascii_digit(" 123") {
                    return 4
                }
                if !string_is_ascii_hex_digit("0123456789abcdefABCDEF") {
                    return 5
                }
                if string_is_ascii_hex_digit("123g") {
                    return 6
                }
                if string_is_ascii_hex_digit("") {
                    return 7
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_digit fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_alpha_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-alpha-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_alpha("GeoLang") {
                    return 1
                }
                if string_is_ascii_alpha("Geo123") {
                    return 2
                }
                if string_is_ascii_alpha("") {
                    return 3
                }
                if string_is_ascii_alpha("Geo_Lang") {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_alpha fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_lower_upper_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-lower-upper-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_lower("geolang") {
                    return 1
                }
                if string_is_ascii_lower("GeoLang") {
                    return 2
                }
                if string_is_ascii_lower("") {
                    return 3
                }
                if string_is_ascii_lower("geo123") {
                    return 4
                }
                if !string_is_ascii_upper("GEOLANG") {
                    return 5
                }
                if string_is_ascii_upper("GeoLang") {
                    return 6
                }
                if string_is_ascii_upper("") {
                    return 7
                }
                if string_is_ascii_upper("GEO123") {
                    return 8
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_lower_upper fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_alnum_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-alnum-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_alnum("Geo123") {
                    return 1
                }
                if !string_is_ascii_alnum("Geo") {
                    return 2
                }
                if !string_is_ascii_alnum("123") {
                    return 3
                }
                if string_is_ascii_alnum("Geo_123") {
                    return 4
                }
                if string_is_ascii_alnum("") {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_alnum fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_identifier_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-identifier-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_identifier("Geo_123") {
                    return 1
                }
                if !string_is_ascii_identifier("_tmp") {
                    return 2
                }
                if string_is_ascii_identifier("123Geo") {
                    return 3
                }
                if string_is_ascii_identifier("Geo-123") {
                    return 4
                }
                if string_is_ascii_identifier("") {
                    return 5
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_identifier fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_is_ascii_whitespace_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-is-ascii-whitespace-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if !string_is_ascii_whitespace(" \t\n\r") {
                    return 1
                }
                if string_is_ascii_whitespace(" geo ") {
                    return 2
                }
                if string_is_ascii_whitespace("") {
                    return 3
                }
                if string_is_ascii_whitespace(" \t_") {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_is_ascii_whitespace fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_index_of_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-index-of-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let found: int = string_index_of("compiler.geo", ".geo")
                let missing: int = string_index_of("compiler.geo", ".rs")
                return found + missing
            }
        "#,
    )
    .expect("failed to write string_index_of fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(7));
}

#[test]
fn native_run_uses_string_last_index_of_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!(
        "geo-string-last-index-of-{}.geo",
        std::process::id()
    ));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_last_index_of("compiler.geo.compiler.geo", ".geo") != 21 {
                    return 1
                }
                if string_last_index_of("aaaa", "aa") != 2 {
                    return 2
                }
                if string_last_index_of("geo", "rs") != -1 {
                    return 3
                }
                if string_last_index_of("geo", "") != -1 {
                    return 4
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_last_index_of fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_count_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-count-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_count("compiler.geo.compiler.geo", ".geo") != 2usize {
                    return 1
                }
                if string_count("aaaa", "aa") != 2usize {
                    return 2
                }
                if string_count("geo", "rs") != 0usize {
                    return 3
                }
                if string_count("geo", "") != 0usize {
                    return 4
                }
                if string_utf8_count_codepoint("banana", 97) != 3usize {
                    return 5
                }
                if string_utf8_count_codepoint("Geo", 955) != 0usize {
                    return 6
                }
                let left: string = string_unescape_unicode("\\u{03bb}")
                let right: string = string_unescape_unicode("\\u{03bb}")
                let two: string = string_concat(left, right)
                if string_utf8_count_codepoint(two, 955) != 2usize {
                    return 7
                }
                let invalid_codepoint: string = string_unescape_unicode("\\u{03bb}")
                if string_utf8_count_codepoint(invalid_codepoint, 55296) != 0usize {
                    return 8
                }
                let invalid_utf8: string = string_from_byte(255)
                if string_utf8_count_codepoint(invalid_utf8, 255) != 0usize {
                    return 9
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_count fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_trim_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-trim-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let left: string = string_trim_start("  Geo")
                let right: string = string_trim_end("Geo  ")
                let both: string = string_trim("\t Geo \n")
                if string_compare(left, "Geo") != 0 {
                    return 1
                }
                if string_compare(right, "Geo") != 0 {
                    return 2
                }
                if string_compare(both, "Geo") != 0 {
                    return 3
                }
                let uncommented: string = string_strip_ascii_line_comment("Geo // comment")
                if string_compare(uncommented, "Geo ") != 0 {
                    return 40
                }
                let full_comment: string = string_strip_ascii_line_comment("// comment")
                if string_len(full_comment) != 0usize {
                    return 41
                }
                let no_comment: string = string_strip_ascii_line_comment("Geo")
                if string_compare(no_comment, "Geo") != 0 {
                    return 42
                }
                let slash_only: string = string_strip_ascii_line_comment("Geo / comment")
                if string_compare(slash_only, "Geo / comment") != 0 {
                    return 43
                }
                let url_like: string = string_strip_ascii_line_comment("http://geo")
                if string_compare(url_like, "http:") != 0 {
                    return 44
                }
                let blockless: string = string_strip_ascii_block_comment("Geo /* comment */ lang")
                if string_compare(blockless, "Geo  lang") != 0 {
                    return 45
                }
                let full_block: string = string_strip_ascii_block_comment("/* comment */")
                if string_len(full_block) != 0usize {
                    return 46
                }
                let no_block: string = string_strip_ascii_block_comment("Geo")
                if string_compare(no_block, "Geo") != 0 {
                    return 47
                }
                let open_only: string = string_strip_ascii_block_comment("Geo /* comment")
                if string_compare(open_only, "Geo /* comment") != 0 {
                    return 48
                }
                let close_before_open: string = string_strip_ascii_block_comment("*/ Geo /* comment */")
                if string_compare(close_before_open, "*/ Geo ") != 0 {
                    return 49
                }
                let collapsed: string = string_collapse_ascii_whitespace(" Geo\t compiler \n source ")
                if string_compare(collapsed, "Geo compiler source") != 0 {
                    return 50
                }
                let collapse_clean: string = string_collapse_ascii_whitespace("Geo compiler")
                if string_compare(collapse_clean, "Geo compiler") != 0 {
                    return 51
                }
                let collapse_all: string = string_collapse_ascii_whitespace(" \t\n\r")
                if string_len(collapse_all) != 0usize {
                    return 52
                }
                let collapse_empty: string = string_collapse_ascii_whitespace("")
                if string_len(collapse_empty) != 0usize {
                    return 53
                }
                let trimmed_ascii: string = string_utf8_trim_start_codepoint("GGGeo", 71)
                if string_compare(trimmed_ascii, "eo") != 0 {
                    return 4
                }
                let kept_ascii: string = string_utf8_trim_start_codepoint("Geo", 101)
                if string_compare(kept_ascii, "Geo") != 0 {
                    return 5
                }
                let left_lambda: string = string_unescape_unicode("\\u{03bb}")
                let right_lambda: string = string_unescape_unicode("\\u{03bb}")
                let lambdas: string = string_concat(left_lambda, right_lambda)
                let lambda_tail: string = string_utf8_trim_start_codepoint(lambdas, 955)
                if string_len(lambda_tail) != 0usize {
                    return 6
                }
                let keep_lambda: string = string_unescape_unicode("\\u{03bb}")
                let kept_lambda: string = string_utf8_trim_start_codepoint(keep_lambda, 71)
                if string_utf8_codepoint_at(kept_lambda, 0usize) != 955 {
                    return 7
                }
                let invalid_utf8: string = string_from_byte(255)
                let invalid_kept: string = string_utf8_trim_start_codepoint(invalid_utf8, 255)
                if string_len(invalid_kept) != 1usize {
                    return 8
                }
                let invalid_codepoint: string = string_utf8_trim_start_codepoint("Geo", -1)
                if string_compare(invalid_codepoint, "Geo") != 0 {
                    return 9
                }
                let empty: string = string_utf8_trim_start_codepoint("", 71)
                if string_len(empty) != 0usize {
                    return 10
                }
                let trimmed_end_ascii: string = string_utf8_trim_end_codepoint("GeoOO", 79)
                if string_compare(trimmed_end_ascii, "Ge") != 0 {
                    return 11
                }
                let kept_end_ascii: string = string_utf8_trim_end_codepoint("Geo", 101)
                if string_compare(kept_end_ascii, "Geo") != 0 {
                    return 12
                }
                let end_left_lambda: string = string_unescape_unicode("\\u{03bb}")
                let end_right_lambda: string = string_unescape_unicode("\\u{03bb}")
                let end_lambdas: string = string_concat(end_left_lambda, end_right_lambda)
                let end_lambda_tail: string = string_utf8_trim_end_codepoint(end_lambdas, 955)
                if string_len(end_lambda_tail) != 0usize {
                    return 13
                }
                let end_keep_lambda: string = string_unescape_unicode("\\u{03bb}")
                let end_kept_lambda: string = string_utf8_trim_end_codepoint(end_keep_lambda, 71)
                if string_utf8_codepoint_at(end_kept_lambda, 0usize) != 955 {
                    return 14
                }
                let end_invalid_utf8: string = string_from_byte(255)
                let end_invalid_kept: string = string_utf8_trim_end_codepoint(end_invalid_utf8, 255)
                if string_len(end_invalid_kept) != 1usize {
                    return 15
                }
                let end_invalid_codepoint: string = string_utf8_trim_end_codepoint("Geo", -1)
                if string_compare(end_invalid_codepoint, "Geo") != 0 {
                    return 16
                }
                let end_empty: string = string_utf8_trim_end_codepoint("", 71)
                if string_len(end_empty) != 0usize {
                    return 17
                }
                let trimmed_both_ascii: string = string_utf8_trim_codepoint("GGGeoGG", 71)
                if string_compare(trimmed_both_ascii, "eo") != 0 {
                    return 18
                }
                let kept_both_ascii: string = string_utf8_trim_codepoint("Geo", 101)
                if string_compare(kept_both_ascii, "Geo") != 0 {
                    return 19
                }
                let both_left_lambda: string = string_unescape_unicode("\\u{03bb}")
                let both_middle: string = string_concat(both_left_lambda, "Geo")
                let both_right_lambda: string = string_unescape_unicode("\\u{03bb}")
                let both_lambdas: string = string_concat(both_middle, both_right_lambda)
                let both_lambda_trimmed: string = string_utf8_trim_codepoint(both_lambdas, 955)
                if string_compare(both_lambda_trimmed, "Geo") != 0 {
                    return 20
                }
                let both_keep_lambda: string = string_unescape_unicode("\\u{03bb}")
                let both_kept_lambda: string = string_utf8_trim_codepoint(both_keep_lambda, 71)
                if string_utf8_codepoint_at(both_kept_lambda, 0usize) != 955 {
                    return 21
                }
                let both_invalid_utf8: string = string_from_byte(255)
                let both_invalid_kept: string = string_utf8_trim_codepoint(both_invalid_utf8, 255)
                if string_len(both_invalid_kept) != 1usize {
                    return 22
                }
                let both_invalid_codepoint: string = string_utf8_trim_codepoint("Geo", -1)
                if string_compare(both_invalid_codepoint, "Geo") != 0 {
                    return 23
                }
                let both_empty: string = string_utf8_trim_codepoint("", 71)
                if string_len(both_empty) != 0usize {
                    return 24
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string trim fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_line_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-lines-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                if string_line_count("") != 0usize {
                    return 1
                }
                if string_line_count("one") != 1usize {
                    return 2
                }
                if string_line_count("one\ntwo\n") != 2usize {
                    return 3
                }
                if string_compare(string_line_at("one\ntwo\n", 0usize), "one") != 0 {
                    return 4
                }
                if string_compare(string_line_at("one\r\ntwo", 0usize), "one") != 0 {
                    return 5
                }
                if string_compare(string_line_at("one\ntwo", 1usize), "two") != 0 {
                    return 6
                }
                if string_len(string_line_at("one\ntwo", 2usize)) != 0usize {
                    return 7
                }
                if string_line_index_at("one\ntwo\nthree", 4usize) != 1 {
                    return 8
                }
                if string_column_at("one\ntwo\nthree", 4usize) != 0 {
                    return 9
                }
                if string_column_at("one\ntwo\nthree", 6usize) != 2 {
                    return 10
                }
                if string_line_index_at("one\ntwo", 7usize) != -1 {
                    return 11
                }
                if string_offset_at_line_column("one\ntwo\nthree", 1usize, 2usize) != 6 {
                    return 12
                }
                if string_offset_at_line_column("one\ntwo\nthree", 2usize, 5usize) != 13 {
                    return 13
                }
                if string_offset_at_line_column("one\ntwo", 2usize, 0usize) != -1 {
                    return 14
                }
                let indented: string = string_indent("one\ntwo", "> ")
                if string_compare(indented, "> one\n> two") != 0 {
                    return 15
                }
                let indented_empty_line: string = string_indent("one\n\ntwo", "|")
                if string_compare(indented_empty_line, "|one\n|\n|two") != 0 {
                    return 16
                }
                let indented_empty: string = string_indent("", "> ")
                if string_len(indented_empty) != 0usize {
                    return 17
                }
                let indented_no_prefix: string = string_indent("one\ntwo", "")
                if string_compare(indented_no_prefix, "one\ntwo") != 0 {
                    return 18
                }
                let prefixed: string = string_prefix_lines("one\ntwo", "> ")
                if string_compare(prefixed, "> one\n> two") != 0 {
                    return 24
                }
                let prefixed_trailing: string = string_prefix_lines("one\ntwo\n", "> ")
                if string_compare(prefixed_trailing, "> one\n> two\n> ") != 0 {
                    return 25
                }
                let prefixed_empty_line: string = string_prefix_lines("one\n\ntwo", "|")
                if string_compare(prefixed_empty_line, "|one\n|\n|two") != 0 {
                    return 26
                }
                let prefixed_empty: string = string_prefix_lines("", "> ")
                if string_len(prefixed_empty) != 0usize {
                    return 27
                }
                let prefixed_no_prefix: string = string_prefix_lines("one\ntwo\n", "")
                if string_compare(prefixed_no_prefix, "one\ntwo\n") != 0 {
                    return 28
                }
                let suffixed: string = string_suffix_lines("one\ntwo", " <")
                if string_compare(suffixed, "one <\ntwo <") != 0 {
                    return 29
                }
                let suffixed_trailing: string = string_suffix_lines("one\ntwo\n", " <")
                if string_compare(suffixed_trailing, "one <\ntwo <\n") != 0 {
                    return 30
                }
                let suffixed_empty_line: string = string_suffix_lines("one\n\ntwo", "|")
                if string_compare(suffixed_empty_line, "one|\n|\ntwo|") != 0 {
                    return 31
                }
                let suffixed_empty: string = string_suffix_lines("", " <")
                if string_len(suffixed_empty) != 0usize {
                    return 32
                }
                let suffixed_no_suffix: string = string_suffix_lines("one\ntwo\n", "")
                if string_compare(suffixed_no_suffix, "one\ntwo\n") != 0 {
                    return 33
                }
                let dedented: string = string_dedent("> one\n> two", "> ")
                if string_compare(dedented, "one\ntwo") != 0 {
                    return 19
                }
                let dedented_partial: string = string_dedent("> one\ntwo", "> ")
                if string_compare(dedented_partial, "one\ntwo") != 0 {
                    return 20
                }
                let dedented_empty_line: string = string_dedent("|one\n|\n|two", "|")
                if string_compare(dedented_empty_line, "one\n\ntwo") != 0 {
                    return 21
                }
                let dedented_empty: string = string_dedent("", "> ")
                if string_len(dedented_empty) != 0usize {
                    return 22
                }
                let dedented_no_prefix: string = string_dedent("one\ntwo", "")
                if string_compare(dedented_no_prefix, "one\ntwo") != 0 {
                    return 23
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string line fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_slice_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-slice-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let prefix: string = string_slice("compiler.geo", 0usize, 8usize)
                if string_compare(prefix, "compiler") != 0 {
                    return 1
                }
                let suffix: string = string_slice("compiler.geo", 9usize, 64usize)
                if string_compare(suffix, "geo") != 0 {
                    return 2
                }
                let empty: string = string_slice("compiler.geo", 64usize, 4usize)
                if string_len(empty) != 0usize {
                    return 3
                }
                let zero: string = string_slice("compiler.geo", 3usize, 0usize)
                if string_len(zero) != 0usize {
                    return 4
                }
                let utf8_prefix: string = string_utf8_slice("compiler.geo", 0usize, 8usize)
                if string_compare(utf8_prefix, "compiler") != 0 {
                    return 5
                }
                let utf8_suffix: string = string_utf8_slice("compiler.geo", 9usize, 64usize)
                if string_compare(utf8_suffix, "geo") != 0 {
                    return 6
                }
                let lambda: string = string_unescape_unicode("\\u{03bb}")
                let lambda_slice: string = string_utf8_slice(lambda, 0usize, 1usize)
                if string_compare(lambda_slice, lambda) != 0 {
                    return 7
                }
                if string_len(lambda_slice) != 2usize {
                    return 8
                }
                let lambda_empty: string = string_utf8_slice(lambda, 1usize, 1usize)
                if string_len(lambda_empty) != 0usize {
                    return 9
                }
                let lambda_clamped: string = string_utf8_slice(lambda, 0usize, 64usize)
                if string_compare(lambda_clamped, lambda) != 0 {
                    return 10
                }
                let lambda_out: string = string_utf8_slice(lambda, 2usize, 3usize)
                if string_len(lambda_out) != 0usize {
                    return 11
                }
                let invalid: string = string_from_byte(255)
                let invalid_slice: string = string_utf8_slice(invalid, 0usize, 1usize)
                if string_len(invalid_slice) != 0usize {
                    return 12
                }
                let take_ascii: string = string_utf8_take_while_codepoint("GGGeo", 71)
                if string_compare(take_ascii, "GGG") != 0 {
                    return 43
                }
                let take_none: string = string_utf8_take_while_codepoint("Geo", 101)
                if string_len(take_none) != 0usize {
                    return 44
                }
                let take_left_lambda: string = string_unescape_unicode("\\u{03bb}")
                let take_right_lambda: string = string_unescape_unicode("\\u{03bb}")
                let take_lambdas: string = string_concat(take_left_lambda, take_right_lambda)
                let take_lambda_prefix: string = string_utf8_take_while_codepoint(take_lambdas, 955)
                if string_len(take_lambda_prefix) != 4usize {
                    return 45
                }
                let take_invalid: string = string_from_byte(255)
                let take_invalid_empty: string = string_utf8_take_while_codepoint(take_invalid, 255)
                if string_len(take_invalid_empty) != 0usize {
                    return 46
                }
                let take_invalid_codepoint: string = string_utf8_take_while_codepoint("Geo", -1)
                if string_len(take_invalid_codepoint) != 0usize {
                    return 47
                }
                let take_empty: string = string_utf8_take_while_codepoint("", 71)
                if string_len(take_empty) != 0usize {
                    return 48
                }
                let until_ascii: string = string_utf8_take_until_codepoint("Geo", 111)
                if string_compare(until_ascii, "Ge") != 0 {
                    return 55
                }
                let until_start: string = string_utf8_take_until_codepoint("Geo", 71)
                if string_len(until_start) != 0usize {
                    return 56
                }
                let until_absent: string = string_utf8_take_until_codepoint("Geo", 120)
                if string_compare(until_absent, "Geo") != 0 {
                    return 57
                }
                let until_left: string = string_unescape_unicode("\\u{03bb}")
                let until_text: string = string_concat("Geo", until_left)
                let until_unicode: string = string_utf8_take_until_codepoint(until_text, 955)
                if string_compare(until_unicode, "Geo") != 0 {
                    return 58
                }
                let until_invalid: string = string_from_byte(255)
                let until_invalid_empty: string = string_utf8_take_until_codepoint(until_invalid, 255)
                if string_len(until_invalid_empty) != 0usize {
                    return 59
                }
                let until_invalid_codepoint: string = string_utf8_take_until_codepoint("Geo", -1)
                if string_len(until_invalid_codepoint) != 0usize {
                    return 60
                }
                let until_empty: string = string_utf8_take_until_codepoint("", 71)
                if string_len(until_empty) != 0usize {
                    return 61
                }
                let through_ascii: string = string_utf8_through_codepoint("Geo", 101)
                if string_compare(through_ascii, "Ge") != 0 {
                    return 101
                }
                let through_start: string = string_utf8_through_codepoint("Geo", 71)
                if string_compare(through_start, "G") != 0 {
                    return 102
                }
                let through_absent: string = string_utf8_through_codepoint("Geo", 120)
                if string_compare(through_absent, "Geo") != 0 {
                    return 103
                }
                let through_left: string = string_unescape_unicode("\\u{03bb}")
                let through_text: string = string_concat("Geo", through_left)
                let through_unicode: string = string_utf8_through_codepoint(through_text, 955)
                if string_utf8_codepoint_at(through_unicode, 3usize) != 955 {
                    return 104
                }
                let through_invalid: string = string_from_byte(255)
                let through_invalid_empty: string = string_utf8_through_codepoint(through_invalid, 255)
                if string_len(through_invalid_empty) != 0usize {
                    return 105
                }
                let through_invalid_codepoint: string = string_utf8_through_codepoint("Geo", -1)
                if string_len(through_invalid_codepoint) != 0usize {
                    return 106
                }
                let through_empty: string = string_utf8_through_codepoint("", 71)
                if string_len(through_empty) != 0usize {
                    return 107
                }
                let through_last_ascii: string = string_utf8_through_last_codepoint("Geo.geo", 46)
                if string_compare(through_last_ascii, "Geo.") != 0 {
                    return 108
                }
                let through_last_repeated: string = string_utf8_through_last_codepoint("a.b.c", 46)
                if string_compare(through_last_repeated, "a.b.") != 0 {
                    return 109
                }
                let through_last_start: string = string_utf8_through_last_codepoint(".geo", 46)
                if string_compare(through_last_start, ".") != 0 {
                    return 110
                }
                let through_last_absent: string = string_utf8_through_last_codepoint("Geo", 46)
                if string_compare(through_last_absent, "Geo") != 0 {
                    return 111
                }
                let through_last_left: string = string_unescape_unicode("\\u{03bb}")
                let through_last_middle: string = string_concat("Geo", through_last_left)
                let through_last_text: string = string_concat(through_last_middle, "mod")
                let through_last_unicode: string = string_utf8_through_last_codepoint(through_last_text, 955)
                if string_utf8_codepoint_at(through_last_unicode, 3usize) != 955 {
                    return 112
                }
                let through_last_invalid: string = string_from_byte(255)
                let through_last_invalid_empty: string = string_utf8_through_last_codepoint(through_last_invalid, 255)
                if string_len(through_last_invalid_empty) != 0usize {
                    return 113
                }
                let through_last_invalid_codepoint: string = string_utf8_through_last_codepoint("Geo", -1)
                if string_len(through_last_invalid_codepoint) != 0usize {
                    return 114
                }
                let through_last_empty: string = string_utf8_through_last_codepoint("", 71)
                if string_len(through_last_empty) != 0usize {
                    return 115
                }
                let between_ascii: string = string_utf8_between_codepoints("[Geo]", 91, 93)
                if string_compare(between_ascii, "Geo") != 0 {
                    return 116
                }
                let between_empty: string = string_utf8_between_codepoints("[]", 91, 93)
                if string_len(between_empty) != 0usize {
                    return 117
                }
                let between_missing_start: string = string_utf8_between_codepoints("Geo]", 91, 93)
                if string_len(between_missing_start) != 0usize {
                    return 118
                }
                let between_missing_end: string = string_utf8_between_codepoints("[Geo", 91, 93)
                if string_len(between_missing_end) != 0usize {
                    return 119
                }
                let between_start_left: string = string_unescape_unicode("\\u{03bb}")
                let between_end_right: string = string_unescape_unicode("\\u{03bc}")
                let between_left_text: string = string_concat(between_start_left, "Geo")
                let between_unicode_text: string = string_concat(between_left_text, between_end_right)
                let between_unicode: string = string_utf8_between_codepoints(between_unicode_text, 955, 956)
                if string_compare(between_unicode, "Geo") != 0 {
                    return 120
                }
                let between_invalid: string = string_from_byte(255)
                let between_invalid_empty: string = string_utf8_between_codepoints(between_invalid, 91, 93)
                if string_len(between_invalid_empty) != 0usize {
                    return 121
                }
                let between_invalid_start: string = string_utf8_between_codepoints("[Geo]", -1, 93)
                if string_len(between_invalid_start) != 0usize {
                    return 122
                }
                let between_invalid_end: string = string_utf8_between_codepoints("[Geo]", 91, -1)
                if string_len(between_invalid_end) != 0usize {
                    return 123
                }
                let between_empty_input: string = string_utf8_between_codepoints("", 91, 93)
                if string_len(between_empty_input) != 0usize {
                    return 124
                }
                let between_last_ascii: string = string_utf8_between_last_codepoints("[one][two]", 91, 93)
                if string_compare(between_last_ascii, "two") != 0 {
                    return 125
                }
                let between_last_empty: string = string_utf8_between_last_codepoints("[one][]", 91, 93)
                if string_len(between_last_empty) != 0usize {
                    return 126
                }
                let between_last_missing_start: string = string_utf8_between_last_codepoints("Geo]", 91, 93)
                if string_len(between_last_missing_start) != 0usize {
                    return 127
                }
                let between_last_missing_end: string = string_utf8_between_last_codepoints("[one][two", 91, 93)
                if string_len(between_last_missing_end) != 0usize {
                    return 128
                }
                let between_last_start_left: string = string_unescape_unicode("\\u{03bb}")
                let between_last_end_right: string = string_unescape_unicode("\\u{03bc}")
                let between_last_first: string = string_concat(between_last_start_left, "one")
                let between_last_first_closed: string = string_concat(between_last_first, between_last_end_right)
                let between_last_second_open: string = string_concat(between_last_first_closed, between_last_start_left)
                let between_last_unicode_text: string = string_concat(between_last_second_open, "two")
                let between_last_unicode_full: string = string_concat(between_last_unicode_text, between_last_end_right)
                let between_last_unicode: string = string_utf8_between_last_codepoints(between_last_unicode_full, 955, 956)
                if string_compare(between_last_unicode, "two") != 0 {
                    return 129
                }
                let between_last_invalid: string = string_from_byte(255)
                let between_last_invalid_empty: string = string_utf8_between_last_codepoints(between_last_invalid, 91, 93)
                if string_len(between_last_invalid_empty) != 0usize {
                    return 130
                }
                let between_last_invalid_start: string = string_utf8_between_last_codepoints("[Geo]", -1, 93)
                if string_len(between_last_invalid_start) != 0usize {
                    return 131
                }
                let between_last_invalid_end: string = string_utf8_between_last_codepoints("[Geo]", 91, -1)
                if string_len(between_last_invalid_end) != 0usize {
                    return 132
                }
                let between_last_empty_input: string = string_utf8_between_last_codepoints("", 91, 93)
                if string_len(between_last_empty_input) != 0usize {
                    return 133
                }
                let between_outer_ascii: string = string_utf8_between_outer_codepoints("[one][two]", 91, 93)
                if string_compare(between_outer_ascii, "one][two") != 0 {
                    return 134
                }
                let between_outer_empty: string = string_utf8_between_outer_codepoints("[]", 91, 93)
                if string_len(between_outer_empty) != 0usize {
                    return 135
                }
                let between_outer_missing_start: string = string_utf8_between_outer_codepoints("Geo]", 91, 93)
                if string_len(between_outer_missing_start) != 0usize {
                    return 136
                }
                let between_outer_missing_end: string = string_utf8_between_outer_codepoints("[Geo", 91, 93)
                if string_len(between_outer_missing_end) != 0usize {
                    return 137
                }
                let between_outer_reversed: string = string_utf8_between_outer_codepoints("]Geo[", 91, 93)
                if string_len(between_outer_reversed) != 0usize {
                    return 138
                }
                let between_outer_start_left: string = string_unescape_unicode("\\u{03bb}")
                let between_outer_end_right: string = string_unescape_unicode("\\u{03bc}")
                let between_outer_left_text: string = string_concat(between_outer_start_left, "one")
                let between_outer_middle_text: string = string_concat(between_outer_left_text, between_outer_end_right)
                let between_outer_second_open: string = string_concat(between_outer_middle_text, between_outer_start_left)
                let between_outer_second_content: string = string_concat(between_outer_second_open, "two")
                let between_outer_unicode_full: string = string_concat(between_outer_second_content, between_outer_end_right)
                let between_outer_unicode: string = string_utf8_between_outer_codepoints(between_outer_unicode_full, 955, 956)
                let between_outer_expected_left: string = string_concat("one", between_outer_end_right)
                let between_outer_expected_middle: string = string_concat(between_outer_expected_left, between_outer_start_left)
                let between_outer_expected: string = string_concat(between_outer_expected_middle, "two")
                if string_compare(between_outer_unicode, between_outer_expected) != 0 {
                    return 139
                }
                let between_outer_invalid: string = string_from_byte(255)
                let between_outer_invalid_empty: string = string_utf8_between_outer_codepoints(between_outer_invalid, 91, 93)
                if string_len(between_outer_invalid_empty) != 0usize {
                    return 140
                }
                let between_outer_invalid_start: string = string_utf8_between_outer_codepoints("[Geo]", -1, 93)
                if string_len(between_outer_invalid_start) != 0usize {
                    return 141
                }
                let between_outer_invalid_end: string = string_utf8_between_outer_codepoints("[Geo]", 91, -1)
                if string_len(between_outer_invalid_end) != 0usize {
                    return 142
                }
                let between_outer_empty_input: string = string_utf8_between_outer_codepoints("", 91, 93)
                if string_len(between_outer_empty_input) != 0usize {
                    return 143
                }
                let before_ascii: string = string_utf8_before_codepoint("Geo", 111)
                if string_compare(before_ascii, "Ge") != 0 {
                    return 78
                }
                let before_start: string = string_utf8_before_codepoint("Geo", 71)
                if string_len(before_start) != 0usize {
                    return 79
                }
                let before_absent: string = string_utf8_before_codepoint("Geo", 120)
                if string_compare(before_absent, "Geo") != 0 {
                    return 80
                }
                let before_left: string = string_unescape_unicode("\\u{03bb}")
                let before_text: string = string_concat("Geo", before_left)
                let before_unicode: string = string_utf8_before_codepoint(before_text, 955)
                if string_compare(before_unicode, "Geo") != 0 {
                    return 81
                }
                let before_invalid: string = string_from_byte(255)
                let before_invalid_empty: string = string_utf8_before_codepoint(before_invalid, 255)
                if string_len(before_invalid_empty) != 0usize {
                    return 82
                }
                let before_invalid_codepoint: string = string_utf8_before_codepoint("Geo", -1)
                if string_len(before_invalid_codepoint) != 0usize {
                    return 83
                }
                let before_empty: string = string_utf8_before_codepoint("", 71)
                if string_len(before_empty) != 0usize {
                    return 84
                }
                let before_last_ascii: string = string_utf8_before_last_codepoint("Geo.geo", 46)
                if string_compare(before_last_ascii, "Geo") != 0 {
                    return 85
                }
                let before_last_repeated: string = string_utf8_before_last_codepoint("a.b.c", 46)
                if string_compare(before_last_repeated, "a.b") != 0 {
                    return 86
                }
                let before_last_start: string = string_utf8_before_last_codepoint(".geo", 46)
                if string_len(before_last_start) != 0usize {
                    return 87
                }
                let before_last_absent: string = string_utf8_before_last_codepoint("Geo", 46)
                if string_compare(before_last_absent, "Geo") != 0 {
                    return 88
                }
                let before_last_left: string = string_unescape_unicode("\\u{03bb}")
                let before_last_middle: string = string_concat("Geo", before_last_left)
                let before_last_text: string = string_concat(before_last_middle, "mod")
                let before_last_unicode: string = string_utf8_before_last_codepoint(before_last_text, 955)
                if string_compare(before_last_unicode, "Geo") != 0 {
                    return 89
                }
                let before_last_invalid: string = string_from_byte(255)
                let before_last_invalid_empty: string = string_utf8_before_last_codepoint(before_last_invalid, 255)
                if string_len(before_last_invalid_empty) != 0usize {
                    return 90
                }
                let before_last_invalid_codepoint: string = string_utf8_before_last_codepoint("Geo", -1)
                if string_len(before_last_invalid_codepoint) != 0usize {
                    return 91
                }
                let before_last_empty: string = string_utf8_before_last_codepoint("", 71)
                if string_len(before_last_empty) != 0usize {
                    return 92
                }
                let drop_until_ascii: string = string_utf8_drop_until_codepoint("Geo", 111)
                if string_compare(drop_until_ascii, "o") != 0 {
                    return 62
                }
                let drop_until_start: string = string_utf8_drop_until_codepoint("Geo", 71)
                if string_compare(drop_until_start, "Geo") != 0 {
                    return 63
                }
                let drop_until_absent: string = string_utf8_drop_until_codepoint("Geo", 120)
                if string_len(drop_until_absent) != 0usize {
                    return 64
                }
                let drop_until_left: string = string_unescape_unicode("\\u{03bb}")
                let drop_until_text: string = string_concat("Geo", drop_until_left)
                let drop_until_unicode: string = string_utf8_drop_until_codepoint(drop_until_text, 955)
                if string_utf8_codepoint_at(drop_until_unicode, 0usize) != 955 {
                    return 65
                }
                let drop_until_invalid: string = string_from_byte(255)
                let drop_until_invalid_empty: string = string_utf8_drop_until_codepoint(drop_until_invalid, 255)
                if string_len(drop_until_invalid_empty) != 0usize {
                    return 66
                }
                let drop_until_invalid_codepoint: string = string_utf8_drop_until_codepoint("Geo", -1)
                if string_len(drop_until_invalid_codepoint) != 0usize {
                    return 67
                }
                let drop_until_empty: string = string_utf8_drop_until_codepoint("", 71)
                if string_len(drop_until_empty) != 0usize {
                    return 68
                }
                let after_ascii: string = string_utf8_after_codepoint("Geo", 101)
                if string_compare(after_ascii, "o") != 0 {
                    return 69
                }
                let after_start: string = string_utf8_after_codepoint("Geo", 71)
                if string_compare(after_start, "eo") != 0 {
                    return 70
                }
                let after_end: string = string_utf8_after_codepoint("Geo", 111)
                if string_len(after_end) != 0usize {
                    return 71
                }
                let after_absent: string = string_utf8_after_codepoint("Geo", 120)
                if string_len(after_absent) != 0usize {
                    return 72
                }
                let after_left: string = string_unescape_unicode("\\u{03bb}")
                let after_text: string = string_concat("Geo", after_left)
                let after_unicode: string = string_utf8_after_codepoint(after_text, 955)
                if string_len(after_unicode) != 0usize {
                    return 73
                }
                let after_prefix_lambda: string = string_unescape_unicode("\\u{03bb}")
                let after_prefix_text: string = string_concat(after_prefix_lambda, "Geo")
                let after_unicode_tail: string = string_utf8_after_codepoint(after_prefix_text, 955)
                if string_compare(after_unicode_tail, "Geo") != 0 {
                    return 74
                }
                let after_invalid: string = string_from_byte(255)
                let after_invalid_empty: string = string_utf8_after_codepoint(after_invalid, 255)
                if string_len(after_invalid_empty) != 0usize {
                    return 75
                }
                let after_invalid_codepoint: string = string_utf8_after_codepoint("Geo", -1)
                if string_len(after_invalid_codepoint) != 0usize {
                    return 76
                }
                let after_empty: string = string_utf8_after_codepoint("", 71)
                if string_len(after_empty) != 0usize {
                    return 77
                }
                let after_last_ascii: string = string_utf8_after_last_codepoint("Geo.geo", 46)
                if string_compare(after_last_ascii, "geo") != 0 {
                    return 93
                }
                let after_last_repeated: string = string_utf8_after_last_codepoint("a.b.c", 46)
                if string_compare(after_last_repeated, "c") != 0 {
                    return 94
                }
                let after_last_end: string = string_utf8_after_last_codepoint("geo.", 46)
                if string_len(after_last_end) != 0usize {
                    return 95
                }
                let after_last_absent: string = string_utf8_after_last_codepoint("Geo", 46)
                if string_len(after_last_absent) != 0usize {
                    return 96
                }
                let after_last_left: string = string_unescape_unicode("\\u{03bb}")
                let after_last_middle: string = string_concat("Geo", after_last_left)
                let after_last_text: string = string_concat(after_last_middle, "mod")
                let after_last_unicode: string = string_utf8_after_last_codepoint(after_last_text, 955)
                if string_compare(after_last_unicode, "mod") != 0 {
                    return 97
                }
                let after_last_invalid: string = string_from_byte(255)
                let after_last_invalid_empty: string = string_utf8_after_last_codepoint(after_last_invalid, 255)
                if string_len(after_last_invalid_empty) != 0usize {
                    return 98
                }
                let after_last_invalid_codepoint: string = string_utf8_after_last_codepoint("Geo", -1)
                if string_len(after_last_invalid_codepoint) != 0usize {
                    return 99
                }
                let after_last_empty: string = string_utf8_after_last_codepoint("", 71)
                if string_len(after_last_empty) != 0usize {
                    return 100
                }
                let drop_ascii: string = string_utf8_drop_while_codepoint("GGGeo", 71)
                if string_compare(drop_ascii, "eo") != 0 {
                    return 49
                }
                let drop_none: string = string_utf8_drop_while_codepoint("Geo", 101)
                if string_compare(drop_none, "Geo") != 0 {
                    return 50
                }
                let drop_left_lambda: string = string_unescape_unicode("\\u{03bb}")
                let drop_right_lambda: string = string_unescape_unicode("\\u{03bb}")
                let drop_lambdas: string = string_concat(drop_left_lambda, drop_right_lambda)
                let drop_lambda_suffix: string = string_utf8_drop_while_codepoint(drop_lambdas, 955)
                if string_len(drop_lambda_suffix) != 0usize {
                    return 51
                }
                let drop_invalid: string = string_from_byte(255)
                let drop_invalid_kept: string = string_utf8_drop_while_codepoint(drop_invalid, 255)
                if string_len(drop_invalid_kept) != 1usize {
                    return 52
                }
                let drop_invalid_codepoint: string = string_utf8_drop_while_codepoint("Geo", -1)
                if string_compare(drop_invalid_codepoint, "Geo") != 0 {
                    return 53
                }
                let drop_empty: string = string_utf8_drop_while_codepoint("", 71)
                if string_len(drop_empty) != 0usize {
                    return 54
                }
                let stripped_ascii: string = string_utf8_strip_prefix_codepoint("Geo", 71)
                if string_compare(stripped_ascii, "eo") != 0 {
                    return 29
                }
                let not_stripped_ascii: string = string_utf8_strip_prefix_codepoint("Geo", 101)
                if string_compare(not_stripped_ascii, "Geo") != 0 {
                    return 30
                }
                let lambda_strip: string = string_unescape_unicode("\\u{03bb}")
                let lambda_tail: string = string_utf8_strip_prefix_codepoint(lambda_strip, 955)
                if string_len(lambda_tail) != 0usize {
                    return 31
                }
                let lambda_keep: string = string_unescape_unicode("\\u{03bb}")
                let lambda_kept: string = string_utf8_strip_prefix_codepoint(lambda_keep, 71)
                if string_utf8_codepoint_at(lambda_kept, 0usize) != 955 {
                    return 32
                }
                let invalid_prefix: string = string_from_byte(255)
                let invalid_kept: string = string_utf8_strip_prefix_codepoint(invalid_prefix, 255)
                if string_len(invalid_kept) != 1usize {
                    return 33
                }
                let invalid_codepoint_keep: string = string_utf8_strip_prefix_codepoint("Geo", -1)
                if string_compare(invalid_codepoint_keep, "Geo") != 0 {
                    return 34
                }
                let empty_keep: string = string_utf8_strip_prefix_codepoint("", 71)
                if string_len(empty_keep) != 0usize {
                    return 35
                }
                let suffix_stripped_ascii: string = string_utf8_strip_suffix_codepoint("Geo", 111)
                if string_compare(suffix_stripped_ascii, "Ge") != 0 {
                    return 36
                }
                let suffix_kept_ascii: string = string_utf8_strip_suffix_codepoint("Geo", 101)
                if string_compare(suffix_kept_ascii, "Geo") != 0 {
                    return 37
                }
                let lambda_suffix: string = string_unescape_unicode("\\u{03bb}")
                let lambda_suffix_tail: string = string_utf8_strip_suffix_codepoint(lambda_suffix, 955)
                if string_len(lambda_suffix_tail) != 0usize {
                    return 38
                }
                let lambda_suffix_keep: string = string_unescape_unicode("\\u{03bb}")
                let lambda_suffix_kept: string = string_utf8_strip_suffix_codepoint(lambda_suffix_keep, 71)
                if string_utf8_codepoint_at(lambda_suffix_kept, 0usize) != 955 {
                    return 39
                }
                let invalid_suffix: string = string_from_byte(255)
                let invalid_suffix_kept: string = string_utf8_strip_suffix_codepoint(invalid_suffix, 255)
                if string_len(invalid_suffix_kept) != 1usize {
                    return 40
                }
                let invalid_suffix_codepoint_keep: string = string_utf8_strip_suffix_codepoint("Geo", -1)
                if string_compare(invalid_suffix_codepoint_keep, "Geo") != 0 {
                    return 41
                }
                let empty_suffix_keep: string = string_utf8_strip_suffix_codepoint("", 71)
                if string_len(empty_suffix_keep) != 0usize {
                    return 42
                }
                let taken: string = string_take("compiler.geo", 8usize)
                if string_compare(taken, "compiler") != 0 {
                    return 13
                }
                let taken_all: string = string_take("compiler.geo", 64usize)
                if string_compare(taken_all, "compiler.geo") != 0 {
                    return 14
                }
                let taken_zero: string = string_take("compiler.geo", 0usize)
                if string_len(taken_zero) != 0usize {
                    return 15
                }
                let dropped: string = string_drop("compiler.geo", 9usize)
                if string_compare(dropped, "geo") != 0 {
                    return 16
                }
                let dropped_all: string = string_drop("compiler.geo", 64usize)
                if string_len(dropped_all) != 0usize {
                    return 17
                }
                let dropped_zero: string = string_drop("compiler.geo", 0usize)
                if string_compare(dropped_zero, "compiler.geo") != 0 {
                    return 18
                }
                let taken_last: string = string_take_last("compiler.geo", 3usize)
                if string_compare(taken_last, "geo") != 0 {
                    return 19
                }
                let taken_last_all: string = string_take_last("compiler.geo", 64usize)
                if string_compare(taken_last_all, "compiler.geo") != 0 {
                    return 20
                }
                let taken_last_zero: string = string_take_last("compiler.geo", 0usize)
                if string_len(taken_last_zero) != 0usize {
                    return 21
                }
                let dropped_last: string = string_drop_last("compiler.geo", 4usize)
                if string_compare(dropped_last, "compiler") != 0 {
                    return 22
                }
                let dropped_last_all: string = string_drop_last("compiler.geo", 64usize)
                if string_len(dropped_last_all) != 0usize {
                    return 23
                }
                let dropped_last_zero: string = string_drop_last("compiler.geo", 0usize)
                if string_compare(dropped_last_zero, "compiler.geo") != 0 {
                    return 24
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_slice fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_case_helpers_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-case-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let lower: string = string_to_lower("Geo-LANG_123")
                let upper: string = string_to_upper("Geo-LANG_123")
                if string_compare(lower, "geo-lang_123") != 0 {
                    return 1
                }
                if string_compare(upper, "GEO-LANG_123") != 0 {
                    return 2
                }
                if ascii_to_lower(65) != 97 {
                    return 3
                }
                if ascii_to_lower(122) != 122 {
                    return 4
                }
                if ascii_to_lower(-1) != -1 {
                    return 5
                }
                if ascii_to_upper(97) != 65 {
                    return 6
                }
                if ascii_to_upper(90) != 90 {
                    return 7
                }
                if ascii_to_upper(256) != 256 {
                    return 8
                }
                if unicode_ascii_to_lower(65) != 97 {
                    return 43
                }
                if unicode_ascii_to_lower(122) != 122 {
                    return 44
                }
                if unicode_ascii_to_lower(-1) != -1 {
                    return 45
                }
                if unicode_ascii_to_upper(97) != 65 {
                    return 46
                }
                if unicode_ascii_to_upper(90) != 90 {
                    return 47
                }
                if unicode_ascii_to_upper(256) != 256 {
                    return 48
                }
                if ascii_digit_value(48) != 0 {
                    return 20
                }
                if ascii_digit_value(57) != 9 {
                    return 21
                }
                if ascii_digit_value(65) != -1 || ascii_digit_value(-1) != -1 {
                    return 22
                }
                if unicode_ascii_digit_value(48) != 0 {
                    return 34
                }
                if unicode_ascii_digit_value(57) != 9 {
                    return 35
                }
                if unicode_ascii_digit_value(65) != -1 || unicode_ascii_digit_value(-1) != -1 {
                    return 36
                }
                if ascii_hex_value(48) != 0 {
                    return 15
                }
                if ascii_hex_value(57) != 9 {
                    return 16
                }
                if ascii_hex_value(65) != 10 || ascii_hex_value(70) != 15 {
                    return 17
                }
                if ascii_hex_value(97) != 10 || ascii_hex_value(102) != 15 {
                    return 18
                }
                if ascii_hex_value(71) != -1 || ascii_hex_value(-1) != -1 {
                    return 19
                }
                if unicode_ascii_hex_value(48) != 0 {
                    return 37
                }
                if unicode_ascii_hex_value(57) != 9 {
                    return 38
                }
                if unicode_ascii_hex_value(65) != 10 || unicode_ascii_hex_value(70) != 15 {
                    return 39
                }
                if unicode_ascii_hex_value(97) != 10 || unicode_ascii_hex_value(102) != 15 {
                    return 40
                }
                if unicode_ascii_hex_value(71) != -1 || unicode_ascii_hex_value(-1) != -1 {
                    return 41
                }
                if !ascii_is_digit(48) || ascii_is_digit(65) || ascii_is_digit(-1) {
                    return 9
                }
                if !ascii_is_hex_digit(48) || !ascii_is_hex_digit(70) || !ascii_is_hex_digit(102) {
                    return 13
                }
                if ascii_is_hex_digit(71) || ascii_is_hex_digit(103) || ascii_is_hex_digit(-1) {
                    return 14
                }
                if !unicode_is_ascii_digit(48) || unicode_is_ascii_digit(65) || unicode_is_ascii_digit(-1) {
                    return 15
                }
                if !unicode_is_ascii_hex_digit(48) || !unicode_is_ascii_hex_digit(70) || !unicode_is_ascii_hex_digit(102) {
                    return 16
                }
                if unicode_is_ascii_hex_digit(71) || unicode_is_ascii_hex_digit(103) || unicode_is_ascii_hex_digit(-1) {
                    return 17
                }
                if !ascii_is_identifier_start(65) || !ascii_is_identifier_start(95) || ascii_is_identifier_start(48) {
                    return 23
                }
                if !ascii_is_identifier_continue(65) || !ascii_is_identifier_continue(95) || !ascii_is_identifier_continue(48) || ascii_is_identifier_continue(45) {
                    return 24
                }
                if !unicode_is_ascii_identifier_start(65) || !unicode_is_ascii_identifier_start(95) || unicode_is_ascii_identifier_start(48) {
                    return 25
                }
                if !unicode_is_ascii_identifier_continue(65) || !unicode_is_ascii_identifier_continue(95) || !unicode_is_ascii_identifier_continue(48) || unicode_is_ascii_identifier_continue(45) {
                    return 26
                }
                let lambda: string = string_unescape_unicode("\\u{03bb}")
                let lambda_codepoint: int = string_utf8_codepoint_at(lambda, 0usize)
                if unicode_is_ascii_identifier_start(lambda_codepoint) || unicode_is_ascii_identifier_continue(lambda_codepoint) {
                    return 27
                }
                if unicode_is_ascii_digit(lambda_codepoint) || unicode_is_ascii_hex_digit(lambda_codepoint) || unicode_is_ascii_whitespace(lambda_codepoint) {
                    return 29
                }
                if unicode_ascii_digit_value(lambda_codepoint) != -1 || unicode_ascii_hex_value(lambda_codepoint) != -1 {
                    return 42
                }
                if unicode_ascii_to_lower(lambda_codepoint) != lambda_codepoint || unicode_ascii_to_upper(lambda_codepoint) != lambda_codepoint {
                    return 49
                }
                if unicode_is_ascii_alpha(lambda_codepoint) || unicode_is_ascii_alnum(lambda_codepoint) {
                    return 31
                }
                let ascii_codepoint: int = string_utf8_codepoint_at("Geo", 0usize)
                if !unicode_is_ascii_identifier_start(ascii_codepoint) {
                    return 28
                }
                if !ascii_is_alpha(65) || !ascii_is_alpha(122) || ascii_is_alpha(48) {
                    return 10
                }
                if !ascii_is_alnum(65) || !ascii_is_alnum(57) || ascii_is_alnum(95) {
                    return 11
                }
                if !unicode_is_ascii_alpha(65) || !unicode_is_ascii_alpha(122) || unicode_is_ascii_alpha(48) || unicode_is_ascii_alpha(-1) {
                    return 32
                }
                if !unicode_is_ascii_alnum(65) || !unicode_is_ascii_alnum(57) || unicode_is_ascii_alnum(95) || unicode_is_ascii_alnum(-1) {
                    return 33
                }
                if !ascii_is_whitespace(32) || !ascii_is_whitespace(10) || ascii_is_whitespace(65) {
                    return 12
                }
                if !unicode_is_ascii_whitespace(32) || !unicode_is_ascii_whitespace(10) || unicode_is_ascii_whitespace(65) {
                    return 30
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string case fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_reverse_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-reverse-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let reversed: string = string_reverse("GeoLang")
                if string_compare(reversed, "gnaLoeG") != 0 {
                    return 1
                }
                let empty: string = string_reverse("")
                if string_len(empty) != 0 {
                    return 2
                }
                let single: string = string_reverse("x")
                if string_compare(single, "x") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_reverse fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_replace_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-replace-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let path: string = string_replace("src\\main.geo", "\\", "/")
                if string_compare(path, "src/main.geo") != 0 {
                    return 1
                }
                let same: string = string_replace("geo", "missing", "x")
                if string_compare(same, "geo") != 0 {
                    return 2
                }
                let empty: string = string_replace("geo", "", "x")
                if string_compare(empty, "geo") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_replace fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_replace_all_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-replace-all-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let path: string = string_replace_all("src\\main\\compiler.geo", "\\", "/")
                if string_compare(path, "src/main/compiler.geo") != 0 {
                    return 1
                }
                let same: string = string_replace_all("geo", "missing", "x")
                if string_compare(same, "geo") != 0 {
                    return 2
                }
                let empty: string = string_replace_all("geo", "", "x")
                if string_compare(empty, "geo") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_replace_all fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_escape_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-escape-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let escaped: string = string_escape("Geo\n\t\"lang\"\\")
                if string_compare(escaped, "Geo\\n\\t\\\"lang\\\"\\\\") != 0 {
                    return 1
                }
                if string_compare(string_escape("plain"), "plain") != 0 {
                    return 2
                }
                if string_len(string_escape("")) != 0usize {
                    return 3
                }
                let ascii: string = string_escape_ascii("Geo\n")
                if string_compare(ascii, "Geo\\n") != 0 {
                    return 15
                }
                let high: string = string_escape_ascii(string_from_byte(255))
                if string_compare(high, "\\xFF") != 0 {
                    return 16
                }
                let decoded: string = string_unescape("Geo\\n\\t\\\"lang\\\"\\\\")
                if string_compare(decoded, "Geo\n\t\"lang\"\\") != 0 {
                    return 4
                }
                if string_compare(string_unescape("plain"), "plain") != 0 {
                    return 5
                }
                if string_compare(string_unescape("\\q"), "q") != 0 {
                    return 6
                }
                if string_compare(string_unescape("\\"), "") != 0 {
                    return 7
                }
                if string_compare(string_unescape("\\x41\\x7a"), "Az") != 0 {
                    return 8
                }
                if string_compare(string_unescape("\\x0D"), "\r") != 0 {
                    return 9
                }
                if string_compare(string_unescape("\\x4g"), "x4g") != 0 {
                    return 10
                }
                if string_compare(string_unescape_hex("\\x47\\x65\\x6f"), "Geo") != 0 {
                    return 11
                }
                let lambda: string = string_unescape_unicode("\\u{03bb}")
                if string_len(lambda) != 2usize || string_byte_at(lambda, 0usize) != 206 || string_byte_at(lambda, 1usize) != 187 {
                    return 12
                }
                let face: string = string_unescape("\\u{1f600}")
                if string_len(face) != 4usize || string_byte_at(face, 0usize) != 240 || string_byte_at(face, 1usize) != 159 || string_byte_at(face, 2usize) != 152 || string_byte_at(face, 3usize) != 128 {
                    return 13
                }
                if string_compare(string_unescape_unicode("\\u{}"), "u{}") != 0 {
                    return 14
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_escape fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_repeat_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-string-repeat-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let repeated: string = string_repeat("geo", 3usize)
                if string_compare(repeated, "geogeogeo") != 0 {
                    return 1
                }
                let none: string = string_repeat("geo", 0usize)
                if string_len(none) != 0 {
                    return 2
                }
                let empty: string = string_repeat("", 5usize)
                if string_len(empty) != 0 {
                    return 3
                }
                let padded_left: string = string_pad_start("42", 5usize, "0")
                if string_compare(padded_left, "00042") != 0 {
                    return 4
                }
                let padded_right: string = string_pad_end("geo", 5usize, ".")
                if string_compare(padded_right, "geo..") != 0 {
                    return 5
                }
                let already_wide: string = string_pad_start("geolang", 3usize, "0")
                if string_compare(already_wide, "geolang") != 0 {
                    return 6
                }
                let empty_pad: string = string_pad_end("geo", 5usize, "")
                if string_compare(empty_pad, "geo") != 0 {
                    return 7
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_repeat fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_string_parse_int_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-parse-int-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let positive: int = string_parse_int("40")
                let negative: int = string_parse_int("-2")
                let padded: int = string_parse_int("  5")
                let invalid: int = string_parse_int("geo")
                return positive + negative + padded + invalid
            }
        "#,
    )
    .expect("failed to write string_parse_int fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(43));
}

#[test]
fn native_run_uses_string_parse_usize_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-parse-usize-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> usize {
                let positive: usize = string_parse_usize("40")
                let padded: usize = string_parse_usize("  2")
                let invalid: usize = string_parse_usize("geo")
                let negative: usize = string_parse_usize("-1")
                return positive + padded + invalid + negative
            }
        "#,
    )
    .expect("failed to write string_parse_usize fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(42));
}

#[test]
fn native_run_uses_string_try_parse_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-string-try-parse-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                var signed: int = 0
                var size: usize = 0usize
                if !string_try_parse_int(" -42 ", &signed) {
                    return 1
                }
                if signed != -42 {
                    return 2
                }
                if string_try_parse_int("12geo", &signed) {
                    return 3
                }
                if signed != -42 {
                    return 4
                }
                if !string_try_parse_usize("42", &size) {
                    return 5
                }
                if size != 42usize {
                    return 6
                }
                if string_try_parse_usize("-1", &size) {
                    return 7
                }
                if size != 42usize {
                    return 8
                }
                if string_try_parse_usize("7x", &size) {
                    return 9
                }
                return 0
            }
        "#,
    )
    .expect("failed to write string_try_parse fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_integer_to_string_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path =
        std::env::temp_dir().join(format!("geo-integer-to-string-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let negative: string = int_to_string(-42)
                if string_compare(negative, "-42") != 0 {
                    return 1
                }
                let zero: string = int_to_string(0)
                if string_compare(zero, "0") != 0 {
                    return 2
                }
                let size: string = usize_to_string(42usize)
                if string_compare(size, "42") != 0 {
                    return 3
                }
                return 0
            }
        "#,
    )
    .expect("failed to write integer_to_string fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_bool_to_string_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-bool-to-string-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.string

            fn main() -> int {
                let yes: string = bool_to_string(true)
                if string_compare(yes, "true") != 0 {
                    return 1
                }
                let no: string = bool_to_string(false)
                if string_compare(no, "false") != 0 {
                    return 2
                }
                return 0
            }
        "#,
    )
    .expect("failed to write bool_to_string fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_uses_runtime_array_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-array-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.array

            fn main() -> u8 {
                var items: *u8 = array_new(1, 2)
                let first: u8 = 3
                let second: u8 = 4
                let third: u8 = 8
                unsafe {
                    if !array_is_empty(items) {
                        return 33
                    }
                    array_push(items, &first)
                    if array_is_empty(items) {
                        return 34
                    }
                    array_push(items, &second)
                    let grown: *u8 = array_reserve(items, 4usize)
                    if array_capacity(grown) != 4usize {
                        return 7
                    }
                    if array_len(grown) != 2usize {
                        return 8
                    }
                    items = grown
                    if array_push(items, &third) != 0 {
                        return 9
                    }
                    let cloned: *u8 = array_clone(items)
                    if array_len(cloned) != 3usize {
                        return 36
                    }
                    if array_capacity(cloned) != 4usize {
                        return 37
                    }
                    let cloned_first: *u8 = array_get(cloned, 0usize)
                    if *cloned_first != 3 {
                        return 38
                    }
                    let first_direct: *u8 = array_first(items)
                    if *first_direct != 3 {
                        return 53
                    }
                    let last_direct: *u8 = array_last(items)
                    if *last_direct != 8 {
                        return 54
                    }
                    let replacement: u8 = 9
                    if array_set(items, 0usize, &replacement) != 0 {
                        return 5
                    }
                    if *cloned_first != 3 {
                        return 39
                    }
                    let first_ptr: *u8 = array_get(items, 0usize)
                    if *first_ptr != 9 {
                        return 3
                    }
                    if array_set(items, 9usize, &replacement) != 1 {
                        return 6
                    }
                    if array_swap(items, 0usize, 2usize) != 0 {
                        return 18
                    }
                    let swapped_first: *u8 = array_get(items, 0usize)
                    if *swapped_first != 8 {
                        return 19
                    }
                    let swapped_last: *u8 = array_get(items, 2usize)
                    if *swapped_last != 9 {
                        return 20
                    }
                    if array_fill(items, &replacement) != 0 {
                        return 40
                    }
                    let filled_first: *u8 = array_get(items, 0usize)
                    if *filled_first != 9 {
                        return 41
                    }
                    let filled_last: *u8 = array_get(items, 2usize)
                    if *filled_last != 9 {
                        return 42
                    }
                    if array_index_of(items, &replacement) != 0 {
                        return 25
                    }
                    let missing: u8 = 6
                    if array_index_of(items, &missing) != -1 {
                        return 26
                    }
                    if array_push(items, &replacement) != 0 {
                        return 27
                    }
                    if array_last_index_of(items, &replacement) != 3 {
                        return 28
                    }
                    if array_last_index_of(items, &missing) != -1 {
                        return 29
                    }
                    if !array_contains(items, &replacement) {
                        return 30
                    }
                    if array_contains(items, &missing) {
                        return 31
                    }
                    if array_count(items, &replacement) != 4usize {
                        return 74
                    }
                    if array_count(items, &missing) != 0usize {
                        return 75
                    }
                    if array_count(items, 0 as *u8) != 0usize {
                        return 76
                    }
                    if array_remove(items, 3usize) != 0 {
                        return 32
                    }
                    if array_swap(items, 0usize, 9usize) != 1 {
                        return 21
                    }
                    if array_reverse(items) != 0 {
                        return 61
                    }
                    let reversed_first: *u8 = array_get(items, 0usize)
                    if *reversed_first != 9 {
                        return 62
                    }
                    let reversed_last: *u8 = array_get(items, 2usize)
                    if *reversed_last != 9 {
                        return 63
                    }
                    let inserted: u8 = 6
                    if array_insert(items, 1usize, &inserted) != 0 {
                        return 14
                    }
                    if array_len(items) != 4usize {
                        return 15
                    }
                    let inserted_ptr: *u8 = array_get(items, 1usize)
                    if *inserted_ptr != 6 {
                        return 16
                    }
                    if array_insert(items, 9usize, &inserted) != 1 {
                        return 17
                    }
                    if array_swap_remove(items, 1usize) != 0 {
                        return 70
                    }
                    if array_len(items) != 3usize {
                        return 71
                    }
                    let swapped_removed_slot: *u8 = array_get(items, 1usize)
                    if *swapped_removed_slot != 9 {
                        return 72
                    }
                    if array_swap_remove(items, 9usize) != 1 {
                        return 73
                    }
                    if array_truncate(items, 2usize) != 0 {
                        return 22
                    }
                    if array_len(items) != 2usize {
                        return 23
                    }
                    if array_truncate(items, 4usize) != 1 {
                        return 24
                    }
                    if array_remove(items, 1usize) != 0 {
                        return 10
                    }
                    if array_len(items) != 1usize {
                        return 11
                    }
                    let shifted: *u8 = array_get(items, 0usize)
                    if *shifted != 9 {
                        return 12
                    }
                    let popped_first: *u8 = array_pop_first(items)
                    if *popped_first != 9 {
                        return 57
                    }
                    if array_len(items) != 0usize {
                        return 58
                    }
                    if array_pop_first(items) != 0 as *u8 {
                        return 59
                    }
                    if array_push(items, &replacement) != 0 {
                        return 60
                    }
                    if array_remove(items, 9usize) != 1 {
                        return 13
                    }
                    let last: *u8 = array_pop(items)
                    let value: u8 = *last
                    array_clear(items)
                    if array_len(items) != 0usize {
                        return 1
                    }
                    if array_reverse(items) != 0 {
                        return 64
                    }
                    if array_first(items) != 0 as *u8 {
                        return 55
                    }
                    if array_last(items) != 0 as *u8 {
                        return 56
                    }
                    if !array_is_empty(items) {
                        return 35
                    }
                    if array_capacity(items) != 4usize {
                        return 2
                    }
                    if array_extend(items, cloned) != 0 {
                        return 43
                    }
                    if array_len(items) != 3usize {
                        return 44
                    }
                    let extended_last: *u8 = array_get(items, 2usize)
                    if *extended_last != 8 {
                        return 45
                    }
                    if array_copy(items, 0usize, cloned, 1usize, 2usize) != 0 {
                        return 65
                    }
                    let copied_first: *u8 = array_get(items, 0usize)
                    if *copied_first != 4 {
                        return 66
                    }
                    let copied_second: *u8 = array_get(items, 1usize)
                    if *copied_second != 8 {
                        return 67
                    }
                    if array_copy(items, 0usize, cloned, 0usize, 0usize) != 0 {
                        return 68
                    }
                    if array_copy(items, 2usize, cloned, 1usize, 2usize) != 1 {
                        return 69
                    }
                    if array_extend(items, cloned) != 1 {
                        return 46
                    }
                    if array_resize(items, 4usize, &replacement) != 0 {
                        return 47
                    }
                    if array_len(items) != 4usize {
                        return 48
                    }
                    let resized_last: *u8 = array_get(items, 3usize)
                    if *resized_last != 9 {
                        return 49
                    }
                    if array_resize(items, 2usize, &replacement) != 0 {
                        return 50
                    }
                    if array_len(items) != 2usize {
                        return 51
                    }
                    if array_resize(items, 5usize, &replacement) != 1 {
                        return 52
                    }
                    array_clear(items)
                    array_free(cloned)
                    array_free(items)
                    return value
                }
            }
        "#,
    )
    .expect("failed to write array fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(9));
}

#[test]
fn native_run_finds_memory_bytes_from_end_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-last-find-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(6)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 66
                    *(buffer + 4) = 68
                    *(buffer + 5) = 66
                }
                if mem_last_find(buffer, 6, 66) != 5 {
                    return 1
                }
                if mem_last_find(buffer + 1, 4, 66) != 2 {
                    return 2
                }
                if mem_last_find(buffer, 6, 90) != -1 {
                    return 3
                }
                if mem_last_find(buffer, 0, 66) != -1 {
                    return 4
                }
                free(buffer)
                return 0
            }
        "#,
    )
    .expect("failed to write memory last-find fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_memory_prefix_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-prefix-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(4)
                let prefix: *u8 = alloc(2)
                let mismatch: *u8 = alloc(2)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 68
                    *(prefix + 0) = 65
                    *(prefix + 1) = 66
                    *(mismatch + 0) = 65
                    *(mismatch + 1) = 90
                }
                if !mem_starts_with(buffer, 4, prefix, 2) {
                    return 1
                }
                if mem_starts_with(buffer, 4, mismatch, 2) {
                    return 2
                }
                if mem_starts_with(buffer, 1, prefix, 2) {
                    return 3
                }
                if !mem_starts_with(buffer, 4, prefix, 0) {
                    return 4
                }
                free(buffer)
                free(prefix)
                free(mismatch)
                return 0
            }
        "#,
    )
    .expect("failed to write memory prefix fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_memory_suffix_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-suffix-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(4)
                let suffix: *u8 = alloc(2)
                let mismatch: *u8 = alloc(2)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                    *(buffer + 3) = 68
                    *(suffix + 0) = 67
                    *(suffix + 1) = 68
                    *(mismatch + 0) = 90
                    *(mismatch + 1) = 68
                }
                if !mem_ends_with(buffer, 4, suffix, 2) {
                    return 1
                }
                if mem_ends_with(buffer, 4, mismatch, 2) {
                    return 2
                }
                if mem_ends_with(buffer, 1, suffix, 2) {
                    return 3
                }
                if !mem_ends_with(buffer, 4, suffix, 0) {
                    return 4
                }
                free(buffer)
                free(suffix)
                free(mismatch)
                return 0
            }
        "#,
    )
    .expect("failed to write memory suffix fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_hashes_memory_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-hash-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(3)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 67
                }
                if mem_hash(buffer, 3) != 18027876433081418475usize {
                    return 1
                }
                if mem_hash(buffer, 0) != 14695981039346656037usize {
                    return 2
                }
                if mem_hash(null, 3) != 0usize {
                    return 3
                }
                if mem_hash_seed(buffer, 3, 12345usize) != 15397442009934069191usize {
                    return 4
                }
                if mem_hash_seed(buffer, 3, 14695981039346656037usize) != mem_hash(buffer, 3) {
                    return 5
                }
                if mem_hash_seed(buffer, 0, 12345usize) != 12345usize {
                    return 6
                }
                if mem_hash_seed(null, 3, 12345usize) != 0usize {
                    return 7
                }
                free(buffer)
                return 0
            }
        "#,
    )
    .expect("failed to write memory hash fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_counts_memory_bytes_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-count-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(6)
                unsafe {
                    *(buffer + 0) = 65
                    *(buffer + 1) = 66
                    *(buffer + 2) = 65
                    *(buffer + 3) = 67
                    *(buffer + 4) = 65
                    *(buffer + 5) = 68
                }
                if mem_count(buffer, 6, 65) != 3usize {
                    return 1
                }
                if mem_count(buffer, 6, 90) != 0usize {
                    return 2
                }
                if mem_count(buffer, 0, 65) != 0usize {
                    return 3
                }
                free(buffer)
                return 0
            }
        "#,
    )
    .expect("failed to write memory count fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_checks_memory_byte_predicates_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-predicates-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let mixed: *u8 = alloc(4)
                let repeated: *u8 = alloc(3)
                unsafe {
                    *(mixed + 0) = 65
                    *(mixed + 1) = 66
                    *(mixed + 2) = 65
                    *(mixed + 3) = 67
                    *(repeated + 0) = 90
                    *(repeated + 1) = 90
                    *(repeated + 2) = 90
                }
                if !mem_contains(mixed, 4, 66) {
                    return 1
                }
                if mem_contains(mixed, 4, 90) {
                    return 2
                }
                if !mem_any(mixed, 4, 65) {
                    return 3
                }
                if mem_any(mixed, 0, 65) {
                    return 4
                }
                if !mem_all(repeated, 3, 90) {
                    return 5
                }
                if mem_all(mixed, 4, 65) {
                    return 6
                }
                if !mem_all(mixed, 0, 65) {
                    return 7
                }
                if mem_contains(null, 4, 65) {
                    return 8
                }
                if mem_any(null, 4, 65) {
                    return 9
                }
                if mem_all(null, 4, 65) {
                    return 10
                }
                free(mixed)
                free(repeated)
                return 0
            }
        "#,
    )
    .expect("failed to write memory predicate fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

#[test]
fn native_run_counts_memory_byte_edges_when_available() {
    if !can_run_native_linux_examples() {
        return;
    }

    let path = std::env::temp_dir().join(format!("geo-mem-byte-edges-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.mem

            fn main() -> int {
                let buffer: *u8 = alloc(7)
                let repeated: *u8 = alloc(3)
                unsafe {
                    *(buffer + 0) = 32
                    *(buffer + 1) = 32
                    *(buffer + 2) = 65
                    *(buffer + 3) = 66
                    *(buffer + 4) = 67
                    *(buffer + 5) = 32
                    *(buffer + 6) = 32
                    *(repeated + 0) = 32
                    *(repeated + 1) = 32
                    *(repeated + 2) = 32
                }
                if mem_leading_count(buffer, 7, 32) != 2usize {
                    return 1
                }
                if mem_trailing_count(buffer, 7, 32) != 2usize {
                    return 2
                }
                if mem_trimmed_len(buffer, 7, 32) != 3usize {
                    return 3
                }
                if mem_leading_count(buffer + 2, 3, 32) != 0usize {
                    return 4
                }
                if mem_trailing_count(buffer + 2, 3, 32) != 0usize {
                    return 5
                }
                if mem_trimmed_len(repeated, 3, 32) != 0usize {
                    return 6
                }
                if mem_trimmed_len(buffer, 0, 32) != 0usize {
                    return 7
                }
                if mem_leading_count(null, 3, 32) != 0usize {
                    return 8
                }
                if mem_trailing_count(null, 3, 32) != 0usize {
                    return 9
                }
                if mem_trimmed_len(null, 3, 32) != 0usize {
                    return 10
                }
                free(buffer)
                free(repeated)
                return 0
            }
        "#,
    )
    .expect("failed to write memory byte edge fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert_eq!(status.code(), Some(0));
}

fn assert_geo_exit(path: &str, expected: i32) {
    let path = workspace_path(path);
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["run", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");

    assert_eq!(status.code(), Some(expected));
}

fn can_run_native_linux_examples() -> bool {
    cfg!(target_os = "linux")
        && command_exists("nasm")
        && (command_exists("gcc") || command_exists("clang"))
}

fn command_exists(command: &str) -> bool {
    std::process::Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn cli_accepts_explicit_linux_target_for_check() {
    let input = workspace_path("examples/return_42.geo");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "check",
            input.to_string_lossy().as_ref(),
            "--target",
            "x86_64-linux",
        ])
        .status()
        .expect("failed to run geo");

    assert!(status.success());
}

#[test]
fn cli_rejects_unknown_target() {
    let input = workspace_path("examples/return_42.geo");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "check",
            input.to_string_lossy().as_ref(),
            "--target",
            "wasm32-browser",
        ])
        .output()
        .expect("failed to run geo");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported target"));
}

#[test]
fn emit_asm_uses_target_abi_argument_registers() {
    let path = std::env::temp_dir().join(format!("geo-abi-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn add(a: int, b: int) -> int {
                return a + b
            }

            fn main() -> int {
                return add(1, 2)
            }
        "#,
    )
    .expect("failed to write abi fixture");

    let linux_asm = emit_asm_cli(&path, "x86_64-linux");
    let windows_asm = emit_asm_cli(&path, "x86_64-windows");
    let _ = std::fs::remove_file(&path);

    assert!(linux_asm.contains("    mov [rbp - 8], rdi"));
    assert!(linux_asm.contains("    mov [rbp - 16], rsi"));
    assert!(windows_asm.contains("    mov [rbp - 8], rcx"));
    assert!(windows_asm.contains("    mov [rbp - 16], rdx"));
}

#[test]
fn emit_asm_uses_stack_passed_arguments_after_registers() {
    let path = std::env::temp_dir().join(format!("geo-stack-abi-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn seventh(a: int, b: int, c: int, d: int, e: int, f: int, g: int) -> int {
                return g
            }

            fn fifth(a: int, b: int, c: int, d: int, e: int) -> int {
                return e
            }

            fn main() -> int {
                return seventh(1, 2, 3, 4, 5, 6, 7) + fifth(1, 2, 3, 4, 5)
            }
        "#,
    )
    .expect("failed to write stack abi fixture");

    let linux_asm = emit_asm_cli(&path, "x86_64-linux");
    let windows_asm = emit_asm_cli(&path, "x86_64-windows");
    let _ = std::fs::remove_file(&path);

    assert!(linux_asm.contains("    mov rax, [rbp + 16]"));
    assert!(linux_asm.contains("    push qword [rbp - "));
    assert!(linux_asm.contains("    add rsp, 8"));
    assert!(windows_asm.contains("    mov rax, [rbp + 48]"));
    assert!(windows_asm.contains("    sub rsp, 32"));
    assert!(windows_asm.contains("    add rsp, 40"));
}

#[test]
fn emit_asm_uses_target_abi_for_bounds_checks() {
    let path = std::env::temp_dir().join(format!("geo-bounds-abi-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            fn main() -> int {
                let values: [int] = [42]
                return values[0]
            }
        "#,
    )
    .expect("failed to write bounds abi fixture");

    let linux_asm = emit_asm_cli(&path, "x86_64-linux");
    let windows_asm = emit_asm_cli(&path, "x86_64-windows");
    let _ = std::fs::remove_file(&path);

    assert!(linux_asm.contains("    mov rdi, [rbp - "));
    assert!(linux_asm.contains("    mov rsi, 1"));
    assert!(windows_asm.contains("    mov rcx, [rbp - "));
    assert!(windows_asm.contains("    mov rdx, 1"));
    assert!(windows_asm.contains("    sub rsp, 32"));
}

fn emit_asm_cli(path: &std::path::Path, target: &str) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("geo");
    let asm_path = std::env::temp_dir().join(format!(
        "geo-abi-{}-{}-{}-{}.asm",
        stem,
        target,
        std::process::id(),
        nonce
    ));
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            path.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
            "--target",
            target,
        ])
        .status()
        .expect("failed to run geo emit-asm");
    assert!(status.success());
    let asm = std::fs::read_to_string(&asm_path).expect("failed to read emitted assembly");
    let _ = std::fs::remove_file(&asm_path);
    asm
}

#[test]
fn cli_check_accepts_aggregate_program_before_runtime_lowering() {
    let path = std::env::temp_dir().join(format!("geo-aggregate-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
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
        "#,
    )
    .expect("failed to write aggregate fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert!(status.success());
}

#[test]
fn cli_check_accepts_std_import_program() {
    let path = std::env::temp_dir().join(format!("geo-std-import-{}.geo", std::process::id()));
    std::fs::write(
        &path,
        r#"
            import std.io
            import std.string

            fn main() -> int {
                let len: usize = string_len("Geo")
                println("Geo")
                return 0
            }
        "#,
    )
    .expect("failed to write std import fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo");
    let _ = std::fs::remove_file(&path);

    assert!(status.success());
}

#[test]
fn cli_fmt_validates_and_normalizes_source_file() {
    let path = std::env::temp_dir().join(format!("geo-fmt-{}.geo", std::process::id()));
    std::fs::write(&path, "fn main()->int{return 0}   \n").expect("failed to write fmt fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["fmt", path.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo fmt");

    let formatted = std::fs::read_to_string(&path).expect("failed to read formatted fixture");
    let _ = std::fs::remove_file(&path);

    assert!(status.success());
    assert_eq!(formatted, "fn main() -> int {\n    return 0\n}\n");
}

#[test]
fn cli_check_renders_source_location_for_lexer_errors() {
    let path = std::env::temp_dir().join(format!("geo-diagnostic-{}.geo", std::process::id()));
    std::fs::write(&path, "fn main() { @ }\n").expect("failed to write diagnostic fixture");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", path.to_string_lossy().as_ref()])
        .output()
        .expect("failed to run geo check");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = std::fs::remove_file(&path);

    assert!(!output.status.success());
    assert!(stderr.contains("unexpected character '@'"));
    assert!(
        stderr.contains(":1:13"),
        "diagnostic lacked a source column: {stderr}"
    );
    assert!(stderr.contains("1 | fn main() { @ }"));
    assert!(stderr.contains("^"));
}

#[test]
fn cli_test_checks_geo_files_in_directory() {
    let dir = std::env::temp_dir().join(format!("geo-test-{}", std::process::id()));
    let nested = dir.join("nested");
    std::fs::create_dir_all(&nested).expect("failed to create test fixture directory");
    std::fs::write(
        dir.join("main.geo"),
        "import nested.helper\nfn main() -> int { return helper() }\n",
    )
    .expect("failed to write package fixture");
    std::fs::write(
        nested.join("helper.geo"),
        "fn helper() -> int { return 1 }\n",
    )
    .expect("failed to write nested fixture");

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["test", dir.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo test");
    let _ = std::fs::remove_dir_all(&dir);

    assert!(status.success());
}

#[test]
fn cli_check_and_emit_asm_resolve_imported_geo_module() {
    let dir = std::env::temp_dir().join(format!("geo-cli-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create module fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import math

            fn main() -> int {
                return forty_two()
            }
        "#,
    )
    .expect("failed to write module main fixture");
    std::fs::write(
        dir.join("math.geo"),
        r#"
            fn forty_two() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write imported module fixture");

    let check = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args(["check", main.to_string_lossy().as_ref()])
        .status()
        .expect("failed to run geo check");
    let asm_path = dir.join("out.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(check.success());
    assert!(emit.success());
    assert!(asm.contains("forty_two:"));
    assert!(asm.contains("call forty_two"));
}

#[test]
fn cli_emit_asm_resolves_qualified_imported_function_call() {
    let dir = std::env::temp_dir().join(format!("geo-cli-qualified-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified module fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import math

            fn main() -> int {
                return math.forty_two()
            }
        "#,
    )
    .expect("failed to write qualified module main fixture");
    std::fs::write(
        dir.join("math.geo"),
        r#"
            fn forty_two() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write qualified imported module fixture");

    let asm_path = dir.join("qualified.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm for qualified module fixture");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(emit.success());
    assert!(asm.contains("call forty_two"));
    assert!(!asm.contains("call math.forty_two"));
}

#[test]
fn cli_emit_asm_resolves_qualified_imported_type_names() {
    let dir = std::env::temp_dir().join(format!("geo-cli-qualified-type-{}", std::process::id()));
    let model_dir = dir.join("model");
    std::fs::create_dir_all(&model_dir).expect("failed to create qualified type fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> int {
                let token: model.Token = model.Token { kind: 42 }
                return token.kind
            }
        "#,
    )
    .expect("failed to write qualified type main fixture");
    std::fs::write(
        model_dir.join("mod.geo"),
        r#"
            struct Token {
                kind: int
            }
        "#,
    )
    .expect("failed to write qualified type model fixture");

    let asm_path = dir.join("qualified_type.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm for qualified type fixture");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(emit.success());
    assert!(asm.contains(", 42"));
}

#[test]
fn cli_emit_asm_resolves_qualified_imported_enum_variants() {
    let dir = std::env::temp_dir().join(format!("geo-cli-qualified-enum-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified enum fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model

            fn main() -> int {
                let kind: model.TokenKind = model.TokenKind.Number
                return match kind {
                    model.TokenKind.Eof => 0
                    model.TokenKind.Number => 42
                }
            }
        "#,
    )
    .expect("failed to write qualified enum main fixture");
    std::fs::write(
        dir.join("model.geo"),
        r#"
            enum TokenKind {
                Eof
                Number
            }
        "#,
    )
    .expect("failed to write qualified enum model fixture");

    let asm_path = dir.join("qualified_enum.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm for qualified enum fixture");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(emit.success());
    assert!(asm.contains(", 42"));
}

#[test]
fn cli_emit_asm_resolves_qualified_imported_constants() {
    let dir = std::env::temp_dir().join(format!("geo-cli-qualified-const-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create qualified const fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import config

            fn main() -> int {
                return config.LIMIT
            }
        "#,
    )
    .expect("failed to write qualified const main fixture");
    std::fs::write(
        dir.join("config.geo"),
        r#"
            const LIMIT: int = 42
        "#,
    )
    .expect("failed to write qualified const fixture");

    let asm_path = dir.join("qualified_const.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm for qualified const fixture");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(emit.success());
    assert!(asm.contains(", 42"));
}

#[test]
fn cli_emit_asm_resolves_aliased_imported_names() {
    let dir = std::env::temp_dir().join(format!("geo-cli-aliased-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create aliased import fixture directory");
    let main = dir.join("main.geo");
    std::fs::write(
        &main,
        r#"
            import model as m

            fn main() -> int {
                let kind: m.TokenKind = m.TokenKind.Number
                return match kind {
                    m.TokenKind.Eof => 0
                    m.TokenKind.Number => m.score()
                }
            }
        "#,
    )
    .expect("failed to write aliased import main fixture");
    std::fs::write(
        dir.join("model.geo"),
        r#"
            enum TokenKind {
                Eof
                Number
            }

            fn score() -> int {
                return 42
            }
        "#,
    )
    .expect("failed to write aliased import model fixture");

    let asm_path = dir.join("aliased_import.asm");
    let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
        .args([
            "emit-asm",
            main.to_string_lossy().as_ref(),
            "-o",
            asm_path.to_string_lossy().as_ref(),
        ])
        .status()
        .expect("failed to run geo emit-asm for aliased import fixture");
    let asm = std::fs::read_to_string(&asm_path).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(emit.success());
    assert!(asm.contains("call score"));
    assert!(!asm.contains("call m.score"));
    assert!(asm.contains(", 42"));
}

#[test]
fn v1_examples_check_and_emit_for_linux_and_windows() {
    for path in v1_example_paths() {
        let input = workspace_path(path);
        let input_arg = input.to_string_lossy();
        let check = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
            .args(["check", input_arg.as_ref()])
            .status()
            .expect("failed to run geo check");
        assert!(check.success(), "check failed for {path}");

        for target in ["x86_64-linux", "x86_64-windows"] {
            let asm_path = std::env::temp_dir().join(format!(
                "geo-{}-{}-{}.asm",
                target,
                std::process::id(),
                path.replace(['/', '\\'], "-")
            ));
            let emit = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
                .args([
                    "emit-asm",
                    input_arg.as_ref(),
                    "-o",
                    asm_path.to_string_lossy().as_ref(),
                    "--target",
                    target,
                ])
                .status()
                .expect("failed to run geo emit-asm");
            let _ = std::fs::remove_file(&asm_path);

            assert!(emit.success(), "emit-asm failed for {path} on {target}");
        }
    }
}

#[test]
fn v1_examples_build_with_compiler_owned_executable_writers() {
    for path in v1_example_paths() {
        let input = workspace_path(path);
        let input_arg = input.to_string_lossy();
        for target in ["x86_64-linux", "x86_64-windows"] {
            let extension = if target.ends_with("windows") {
                "exe"
            } else {
                "bin"
            };
            let output = std::env::temp_dir().join(format!(
                "geo-v1-{}-{}-{}.{}",
                target,
                std::process::id(),
                path.replace(['/', '\\'], "-"),
                extension
            ));
            let build = std::process::Command::new(env!("CARGO_BIN_EXE_geo"))
                .args([
                    "build",
                    input_arg.as_ref(),
                    "-o",
                    output.to_string_lossy().as_ref(),
                    "--target",
                    target,
                ])
                .status()
                .expect("failed to run geo build");
            let _ = std::fs::remove_file(&output);
            assert!(
                build.success(),
                "direct build failed for {path} on {target}"
            );
        }
    }
}

fn v1_example_paths() -> [&'static str; 6] {
    [
        "examples/v1/buffer.geo",
        "examples/v1/lexer.geo",
        "examples/v1/diagnostics.geo",
        "examples/v1/ast.geo",
        "examples/v1/file_echo.geo",
        "examples/v1/mini_parser.geo",
    ]
}
