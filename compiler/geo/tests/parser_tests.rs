use geo::ast::{BinaryOp, Expr, MatchPattern, Stmt, Type, UnaryOp};
use geo::lexer::lex;
use geo::parser::parse;

fn parse_source(source: &str) -> geo::ast::Program {
    let tokens = lex(source).unwrap();
    parse(&tokens).unwrap()
}

#[test]
fn parses_return_42() {
    let tokens = lex("fn main() -> int { return 42 }").unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].name, "main");
    assert_eq!(program.functions[0].return_type, Type::Int);
    assert_eq!(
        program.functions[0].body,
        vec![Stmt::Return(Some(Expr::Int(42)))]
    );
}

#[test]
fn parses_top_level_const_declaration() {
    let program = parse_source("const LIMIT: int = 42 fn main() -> int { return LIMIT }");

    assert_eq!(program.consts.len(), 1);
    assert_eq!(program.consts[0].name, "LIMIT");
    assert_eq!(program.consts[0].ty, Type::Int);
    assert_eq!(program.consts[0].value, Expr::Int(42));
}

#[test]
fn parses_top_level_type_alias_declaration() {
    let program = parse_source("type Byte = u8 fn main() -> Byte { return 255 }");

    assert_eq!(program.type_aliases.len(), 1);
    assert_eq!(program.type_aliases[0].name, "Byte");
    assert_eq!(program.type_aliases[0].ty, Type::U8);
    assert_eq!(
        program.functions[0].return_type,
        Type::Named("Byte".to_string())
    );
}

#[test]
fn parses_qualified_named_type() {
    let program = parse_source("fn main() -> model.Token { return model.make_token() }");

    assert_eq!(
        program.functions[0].return_type,
        Type::Named("model.Token".to_string())
    );
}

#[test]
fn parses_import_alias() {
    let program = parse_source("import model as m fn main() {}");

    assert_eq!(program.imports[0].path, vec!["model"]);
    assert_eq!(program.imports[0].alias.as_deref(), Some("m"));
}

#[test]
fn parses_enum_declaration_and_variant_expression() {
    let program = parse_source(
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

    assert_eq!(program.enums.len(), 1);
    assert_eq!(program.enums[0].name, "TokenKind");
    assert_eq!(
        program.enums[0]
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Eof", "Ident", "Number"]
    );
    assert!(matches!(
        &program.functions[0].body[0],
        Stmt::Return(Some(Expr::Field { name, .. })) if name == "Number"
    ));
}

#[test]
fn parses_enum_declaration_with_explicit_discriminants() {
    let program = parse_source(
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

    assert_eq!(program.enums[0].variants[0].name, "Ok");
    assert_eq!(program.enums[0].variants[0].value, Some(0));
    assert_eq!(program.enums[0].variants[1].name, "Warning");
    assert_eq!(program.enums[0].variants[1].value, Some(7));
    assert_eq!(program.enums[0].variants[2].name, "Error");
    assert_eq!(program.enums[0].variants[2].value, Some(42));
}

#[test]
fn parses_match_expression() {
    let program = parse_source(
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

    let Stmt::Return(Some(Expr::Match { value, arms })) = &program.functions[0].body[1] else {
        panic!("expected match return");
    };
    assert_eq!(value.as_ref(), &Expr::Var("kind".to_string()));
    assert!(matches!(
        &arms[0].pattern,
        MatchPattern::EnumVariant { enum_name, variant } if enum_name == "TokenKind" && variant == "Eof"
    ));
    assert!(matches!(&arms[2].pattern, MatchPattern::Wildcard));
}

#[test]
fn parses_qualified_enum_match_pattern() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let kind: model.TokenKind = model.TokenKind.Number
                return match kind {
                    model.TokenKind.Eof => 0
                    model.TokenKind.Number => 1
                }
            }
        "#,
    );

    let Stmt::Return(Some(Expr::Match { arms, .. })) = &program.functions[0].body[1] else {
        panic!("expected match return");
    };
    assert!(matches!(
        &arms[0].pattern,
        MatchPattern::EnumVariant { enum_name, variant }
            if enum_name == "model.TokenKind" && variant == "Eof"
    ));
}

#[test]
fn parses_if_expression() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let value: int = if true { 1 } else { 2 }
                return value
            }
        "#,
    );

    let Stmt::Let { value, .. } = &program.functions[0].body[0] else {
        panic!("expected let statement");
    };
    let Expr::If {
        condition,
        then_value,
        else_value,
    } = value
    else {
        panic!("expected if expression");
    };
    assert_eq!(condition.as_ref(), &Expr::Bool(true));
    assert_eq!(then_value.as_ref(), &Expr::Int(1));
    assert_eq!(else_value.as_ref(), &Expr::Int(2));
}

#[test]
fn parses_if_expression_with_variable_condition() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let enabled: bool = true
                return if enabled { 1 } else { 0 }
            }
        "#,
    );

    let Stmt::Return(Some(Expr::If { condition, .. })) = &program.functions[0].body[1] else {
        panic!("expected if expression return");
    };
    assert_eq!(condition.as_ref(), &Expr::Var("enabled".to_string()));
}

#[test]
fn parses_statement_if_comparison_condition_without_parentheses() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let ptr: *u8 = null
                if null == ptr {
                    return 42
                }
                return 1
            }
        "#,
    );

    let Stmt::If { condition, .. } = &program.functions[0].body[1] else {
        panic!("expected if statement");
    };
    assert!(matches!(
        condition,
        Expr::Binary {
            op: BinaryOp::Equal,
            ..
        }
    ));
}

#[test]
fn parses_while_comparison_condition_without_parentheses() {
    let program = parse_source(
        r#"
            fn main() -> int {
                var value: int = 0
                while value < 3 {
                    value += 1
                }
                return value
            }
        "#,
    );

    let Stmt::While { condition, .. } = &program.functions[0].body[1] else {
        panic!("expected while statement");
    };
    assert!(matches!(
        condition,
        Expr::Binary {
            op: BinaryOp::Less,
            ..
        }
    ));
}

#[test]
fn parses_for_range_end_variable_without_parentheses() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let start: int = 0
                let end: int = 3
                var total: int = 0
                for i in start..end {
                    total += i
                }
                return total
            }
        "#,
    );

    let Stmt::For { start, end, .. } = &program.functions[0].body[3] else {
        panic!("expected for statement");
    };
    assert_eq!(start, &Expr::Var("start".to_string()));
    assert_eq!(end, &Expr::Var("end".to_string()));
}

#[test]
fn parses_block_expression_with_local_setup_and_tail_value() {
    let program = parse_source(
        r#"
            fn main() -> int {
                return {
                    let base: int = 40
                    base + 2
                }
            }
        "#,
    );

    let Stmt::Return(Some(Expr::Block { statements, value })) = &program.functions[0].body[0]
    else {
        panic!("expected block expression return");
    };
    assert_eq!(statements.len(), 1);
    assert!(matches!(statements[0], Stmt::Let { .. }));
    assert!(matches!(value.as_ref(), Expr::Binary { .. }));
}

#[test]
fn parses_parameters_calls_and_precedence() {
    let source = "fn add(a: int, b: int) -> int { return a + b * 2 } fn main() -> int { return add(10, 11) }";
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.functions[0].params.len(), 2);
    let Stmt::Return(Some(Expr::Binary { op, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::Add);
    let Stmt::Return(Some(Expr::Call { name, args })) = &program.functions[1].body[0] else {
        panic!("expected call return");
    };
    assert_eq!(name, "add");
    assert_eq!(args.len(), 2);
}

#[test]
fn parses_qualified_function_call() {
    let program = parse_source("fn main() -> int { return math.forty_two() }");

    let Stmt::Return(Some(Expr::Call { name, args })) = &program.functions[0].body[0] else {
        panic!("expected qualified call return");
    };
    assert_eq!(name, "math.forty_two");
    assert!(args.is_empty());
}

#[test]
fn parses_qualified_struct_literal() {
    let program = parse_source(
        r#"
            fn main() -> int {
                let token: model.Token = model.Token { kind: 42 }
                return token.kind
            }
        "#,
    );

    let Stmt::Let { value, .. } = &program.functions[0].body[0] else {
        panic!("expected qualified struct literal local");
    };
    assert!(matches!(
        value,
        Expr::Struct { name, .. } if name == "model.Token"
    ));
}

#[test]
fn parses_trailing_commas_in_params_calls_and_arrays() {
    let program = parse_source(
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

    assert_eq!(program.functions[0].params.len(), 2);
    let Stmt::Let { value, .. } = &program.functions[1].body[0] else {
        panic!("expected array local");
    };
    assert!(matches!(value, Expr::Array(values) if values.len() == 2));
    let Stmt::Return(Some(Expr::Call { args, .. })) = &program.functions[1].body[1] else {
        panic!("expected call return");
    };
    assert_eq!(args.len(), 2);
}

#[test]
fn parses_remainder_expression() {
    let program = parse_source("fn main() -> int { return 10 % 4 }");
    let Stmt::Return(Some(Expr::Binary { op, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::Rem);
}

#[test]
fn parses_prefixed_and_underscored_integer_literals() {
    let program = parse_source("fn main() -> int { return 0xff + 0b1010 + 0o755 + 1_000 }");

    let Stmt::Return(Some(Expr::Binary { .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
}

#[test]
fn parses_typed_integer_literal_suffixes() {
    let program = parse_source("fn main() -> u8 { return 255u8 }");

    assert!(matches!(
        &program.functions[0].body[0],
        Stmt::Return(Some(Expr::TypedInt { value: 255, ty })) if ty == &Type::U8
    ));
}

#[test]
fn parses_compound_assignment() {
    parse_source("fn main() -> int { var x: int = 10 x += 5 x %= 4 return x }");
}

#[test]
fn parses_boolean_logic_precedence() {
    let program = parse_source("fn main() -> bool { return true || false && false }");
    let Stmt::Return(Some(Expr::Binary { op, right, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::Or);
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: BinaryOp::And,
            ..
        }
    ));
}

#[test]
fn parses_bitwise_precedence() {
    let program = parse_source("fn main() -> int { return 10 | 6 ^ 3 & 1 }");
    let Stmt::Return(Some(Expr::Binary { op, right, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::BitOr);
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: BinaryOp::BitXor,
            ..
        }
    ));
}

#[test]
fn parses_shift_precedence() {
    let program = parse_source("fn main() -> int { return 1 & 2 << 3 + 1 }");
    let Stmt::Return(Some(Expr::Binary { op, right, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::BitAnd);
    assert!(matches!(
        right.as_ref(),
        Expr::Binary {
            op: BinaryOp::ShiftLeft,
            ..
        }
    ));
}

#[test]
fn parses_bitwise_not_expression() {
    let program = parse_source("fn main() -> int { return ~10 }");
    let Stmt::Return(Some(Expr::Unary { op, .. })) = &program.functions[0].body[0] else {
        panic!("expected unary return");
    };
    assert_eq!(*op, UnaryOp::BitNot);
}

#[test]
fn parses_integer_cast_expression() {
    let program = parse_source("fn main() -> i32 { return 42 as i32 }");
    let Stmt::Return(Some(Expr::Cast { ty, expr })) = &program.functions[0].body[0] else {
        panic!("expected cast return");
    };
    assert_eq!(*ty, Type::I32);
    assert_eq!(expr.as_ref(), &Expr::Int(42));
}

#[test]
fn parses_sizeof_type_expression() {
    let program = parse_source("fn main() -> usize { return sizeof(*u8) }");

    let Stmt::Return(Some(Expr::SizeOf(ty))) = &program.functions[0].body[0] else {
        panic!("expected sizeof return");
    };
    assert_eq!(*ty, Type::Pointer(Box::new(Type::U8)));
}

#[test]
fn parses_alignof_type_expression() {
    let program = parse_source("fn main() -> usize { return alignof(&mut int) }");

    let Stmt::Return(Some(Expr::AlignOf(ty))) = &program.functions[0].body[0] else {
        panic!("expected alignof return");
    };
    assert_eq!(
        *ty,
        Type::Reference {
            mutable: true,
            inner: Box::new(Type::Int)
        }
    );
}

#[test]
fn parses_offsetof_field_expression() {
    let program = parse_source("fn main() -> usize { return offsetof(Header, next) }");

    let Stmt::Return(Some(Expr::OffsetOf { ty, field })) = &program.functions[0].body[0] else {
        panic!("expected offsetof return");
    };
    assert_eq!(*ty, Type::Named("Header".to_string()));
    assert_eq!(field, "next");
}

#[test]
fn parses_null_literal() {
    let program = parse_source("fn main() -> int { let p: *u8 = null return 0 }");

    let Stmt::Let {
        value: Expr::Null, ..
    } = &program.functions[0].body[0]
    else {
        panic!("expected null initializer");
    };
}

#[test]
fn parses_cast_tighter_than_multiplication() {
    let program = parse_source("fn main() -> int { return 21 as int * 2 }");
    let Stmt::Return(Some(Expr::Binary { op, left, .. })) = &program.functions[0].body[0] else {
        panic!("expected binary return");
    };
    assert_eq!(*op, BinaryOp::Mul);
    assert!(matches!(left.as_ref(), Expr::Cast { ty: Type::Int, .. }));
}

#[test]
fn parses_let_assignment_if_and_while() {
    let source = r#"
        fn main() -> int {
            let x: int = 0
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
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();
    assert_eq!(program.functions[0].body.len(), 3);
}

#[test]
fn parses_for_in_integer_range_loop() {
    let program = parse_source(
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

    let Stmt::For {
        name,
        start,
        end,
        inclusive,
        body,
    } = &program.functions[0].body[1]
    else {
        panic!("expected for loop");
    };
    assert_eq!(name, "i");
    assert_eq!(start, &Expr::Int(0));
    assert_eq!(end, &Expr::Int(4));
    assert!(!inclusive);
    assert!(matches!(body[0], Stmt::Assign { .. }));
}

#[test]
fn parses_for_in_inclusive_integer_range_loop() {
    let program = parse_source(
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

    let Stmt::For {
        name,
        start,
        end,
        inclusive,
        body,
    } = &program.functions[0].body[1]
    else {
        panic!("expected for loop");
    };
    assert_eq!(name, "i");
    assert_eq!(start, &Expr::Int(0));
    assert_eq!(end, &Expr::Int(4));
    assert!(*inclusive);
    assert!(matches!(body[0], Stmt::Assign { .. }));
}

#[test]
fn parses_unconditional_loop_block() {
    let program = parse_source(
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

    let Stmt::Loop(body) = &program.functions[0].body[1] else {
        panic!("expected loop block");
    };
    assert!(matches!(body[0], Stmt::Assign { .. }));
    assert!(matches!(body[1], Stmt::If { .. }));
}

#[test]
fn parses_v1_literals_unary_and_loop_control() {
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
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.functions[0].body.len(), 5);
    assert!(matches!(
        program.functions[0].body[0],
        Stmt::Let {
            ty: Some(Type::String),
            ..
        }
    ));
}

#[test]
fn parses_struct_array_field_and_index_syntax() {
    let source = r#"
        struct Token {
            kind: int
            start: usize
        }

        fn main() -> int {
            let tokens: [Token] = []
            let view: []Token = tokens
            let first: Token = Token { kind: 1 start: 0 }
            return first.kind + view[0].kind
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.structs.len(), 1);
    assert_eq!(program.structs[0].name, "Token");
    assert_eq!(program.functions[0].body.len(), 4);
}

#[test]
fn parses_comma_separated_struct_declaration_fields() {
    let program = parse_source(
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

    assert_eq!(program.structs[0].fields.len(), 2);
    assert_eq!(program.structs[0].fields[0].name, "kind");
    assert_eq!(program.structs[0].fields[1].name, "start");
}

#[test]
fn parses_struct_literal_field_shorthand() {
    let program = parse_source(
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

    let Stmt::Let { value, .. } = &program.functions[0].body[2] else {
        panic!("expected token local");
    };
    let Expr::Struct { fields, .. } = value else {
        panic!("expected struct literal");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].0, "kind");
    assert_eq!(fields[0].1, Expr::Var("kind".to_string()));
    assert_eq!(fields[1].0, "start");
    assert_eq!(fields[1].1, Expr::Var("start".to_string()));
}

#[test]
fn parses_field_and_index_assignment_places() {
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
    let program = parse_source(source);

    assert!(matches!(
        &program.functions[0].body[2],
        Stmt::PlaceAssign {
            target: Expr::Field { name, .. },
            op: None,
            value: Expr::Int(2),
        } if name == "kind"
    ));
    assert!(matches!(
        &program.functions[0].body[3],
        Stmt::PlaceAssign {
            target: Expr::Index { .. },
            op: Some(BinaryOp::Add),
            ..
        }
    ));
}

#[test]
fn parses_imports_externs_and_pointer_types() {
    let source = r#"
        import std.io
        extern fn puts(message: *u8) -> int

        fn main() -> int {
            return 0
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    assert_eq!(program.imports[0].path, vec!["std", "io"]);
    assert_eq!(program.externs[0].name, "puts");
}

#[test]
fn parses_unsafe_address_of_and_deref() {
    let source = r#"
        fn main() -> int {
            let x: int = 42
            unsafe {
                let p: *int = &x
                return *p
            }
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    let Stmt::Unsafe(body) = &program.functions[0].body[1] else {
        panic!("expected unsafe block");
    };
    let Stmt::Let { value, .. } = &body[0] else {
        panic!("expected pointer local");
    };
    assert!(matches!(
        value,
        Expr::Unary {
            op: UnaryOp::AddressOf,
            ..
        }
    ));
    let Stmt::Return(Some(Expr::Unary {
        op: UnaryOp::Deref, ..
    })) = &body[1]
    else {
        panic!("expected deref return");
    };
}

#[test]
fn parses_unsafe_pointer_assignment() {
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
    let program = parse_source(source);

    let Stmt::Unsafe(body) = &program.functions[0].body[1] else {
        panic!("expected unsafe block");
    };
    assert!(matches!(
        &body[1],
        Stmt::PointerAssign {
            pointer: Expr::Var(name),
            op: None,
            value: Expr::Int(42),
        } if name == "p"
    ));
}

#[test]
fn parses_dereference_compound_assignment() {
    let program = parse_source(
        r#"
            fn main() -> int {
                var value: int = 1
                let slot: &mut int = &mut value
                *slot += 41
                return value
            }
        "#,
    );

    assert!(matches!(
        &program.functions[0].body[2],
        Stmt::PointerAssign {
            pointer: Expr::Var(name),
            op: Some(BinaryOp::Add),
            value: Expr::Int(41),
        } if name == "slot"
    ));
}

#[test]
fn parses_clean_core_unit_main_and_inferred_locals() {
    let program = parse_source(
        r#"
            import std.io;

            fn greet(name: str) -> str {
                return "Hello, " + name;
            }

            fn main() {
                let message = greet("world");
                var count = 0;
                count = count + 1;
                println(message);
            }
        "#,
    );

    assert_eq!(program.imports[0].path, vec!["std", "io"]);
    assert_eq!(program.functions[0].return_type, Type::String);
    assert_eq!(program.functions[0].params[0].ty, Type::String);
    assert_eq!(program.functions[1].return_type, Type::Unit);
    assert!(matches!(
        &program.functions[1].body[0],
        Stmt::Let {
            name,
            ty: None,
            mutable: false,
            ..
        } if name == "message"
    ));
    assert!(matches!(
        &program.functions[1].body[1],
        Stmt::Let {
            name,
            ty: None,
            mutable: true,
            ..
        } if name == "count"
    ));
}

#[test]
fn parses_optional_else_and_else_if() {
    let program = parse_source(
        r#"
            fn classify(score: int) -> str {
                if score >= 90 {
                    return "excellent"
                } else if score >= 60 {
                    return "passing"
                }

                return "failing"
            }
        "#,
    );

    let Stmt::If { else_body, .. } = &program.functions[0].body[0] else {
        panic!("expected if statement");
    };
    assert!(matches!(else_body.first(), Some(Stmt::If { .. })));
}

#[test]
fn parses_multi_level_else_if_chain() {
    let program = parse_source(
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
        "#,
    );

    let Stmt::If { else_body, .. } = &program.functions[0].body[0] else {
        panic!("expected outer if statement");
    };
    let Some(Stmt::If {
        else_body: nested_else_body,
        ..
    }) = else_body.first()
    else {
        panic!("expected first else-if as nested if statement");
    };
    assert!(matches!(nested_else_body.first(), Some(Stmt::If { .. })));
}

#[test]
fn parses_reference_types_and_mutable_borrows() {
    let source = r#"
        fn main() -> int {
            let x: int = 1
            let shared: &int = &x
            let unique: &mut int = &mut x
            return *shared
        }
    "#;
    let tokens = lex(source).unwrap();
    let program = parse(&tokens).unwrap();

    assert!(matches!(
        program.functions[0].body[1],
        Stmt::Let {
            ty: Some(Type::Reference { mutable: false, .. }),
            ..
        }
    ));
    assert!(matches!(
        program.functions[0].body[2],
        Stmt::Let {
            ty: Some(Type::Reference { mutable: true, .. }),
            value: Expr::Unary {
                op: UnaryOp::MutableAddressOf,
                ..
            },
            ..
        }
    ));
}
