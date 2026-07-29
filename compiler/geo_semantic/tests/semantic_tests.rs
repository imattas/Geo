use geo_semantic::ast::{Function, Param, Program, Stmt, Type};
use geo_semantic::borrow;
use geo_semantic::lexer;
use geo_semantic::parser;
use geo_semantic::typecheck;

#[test]
fn semantic_crate_checks_a_minimal_program() {
    let program = Program {
        imports: Vec::new(),
        type_aliases: Vec::new(),
        consts: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        externs: Vec::new(),
        functions: vec![Function {
            name: "main".to_string(),
            params: Vec::<Param>::new(),
            return_type: Type::Unit,
            body: vec![Stmt::Return(None)],
            span: Default::default(),
            statement_spans: Vec::new(),
            expression_spans: Vec::new(),
            statement_expression_ranges: Vec::new(),
            source_path: None,
        }],
    };

    typecheck::check(&program).expect("type checking should succeed");
    borrow::check(&program).expect("borrow checking should succeed");
}

#[test]
fn type_errors_point_at_the_expression_span() {
    let source = "fn main() {\n    let value: bool = 42\n}";
    let tokens = lexer::lex(source).expect("source should lex");
    let program = parser::parse(&tokens).expect("source should parse");
    let diagnostics = typecheck::check(&program).expect_err("type mismatch should fail");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("let initializer type mismatch"))
        .expect("initializer mismatch should be reported");
    let span = diagnostic.span.expect("diagnostic should have a span");
    let offset = source.find("42").expect("literal should be present");
    assert_eq!(span.offset, offset);
    assert_eq!(span.len, 2);
}
