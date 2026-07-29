use geo_syntax::ast::{Expr, Stmt, Type};
use geo_syntax::lexer::lex;
use geo_syntax::parser::parse;
use geo_syntax::token::TokenKind;

#[test]
fn lexes_and_parses_canonical_function_syntax() {
    let source = r#"
        fn main() -> int {
            let answer: int = 42
            return answer
        }
    "#;

    let tokens = lex(source).expect("syntax should lex");
    assert!(tokens.iter().any(|token| token.kind == TokenKind::Fn));

    let program = parse(&tokens).expect("syntax should parse");
    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].return_type, Type::Int);
    assert!(matches!(
        program.functions[0].body[0],
        Stmt::Let { ref name, ty: Some(Type::Int), value: Expr::Int(42), .. } if name == "answer"
    ));
}

#[test]
fn lexer_errors_carry_the_offending_character_span() {
    let diagnostics = lex("fn main() { @ }").expect_err("source should fail to lex");
    assert_eq!(
        diagnostics[0].span.map(|span| (span.offset, span.len)),
        Some((12, 1))
    );
}

#[test]
fn parser_errors_carry_the_current_token_span() {
    let tokens = lex("fn main( { return 0 }").expect("source should lex");
    let diagnostics = parse(&tokens).expect_err("source should fail to parse");
    assert_eq!(diagnostics[0].span.map(|span| span.len), Some(1));
}
