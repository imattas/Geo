use geo_codegen::ast::{Function, Program, Stmt, Type};
use geo_codegen::lower;

#[test]
fn lowering_produces_machine_independent_ir() {
    let program = Program {
        imports: Vec::new(),
        type_aliases: Vec::new(),
        consts: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        externs: Vec::new(),
        functions: vec![Function {
            name: "main".to_string(),
            params: Vec::new(),
            return_type: Type::Int,
            body: vec![Stmt::Return(Some(geo_codegen::ast::Expr::Int(42)))],
            span: Default::default(),
            statement_spans: Vec::new(),
            expression_spans: Vec::new(),
            statement_expression_ranges: Vec::new(),
            source_path: None,
        }],
    };

    let ir = lower::lower(&program);
    assert_eq!(ir.functions.len(), 1);
    assert_eq!(ir.functions[0].name, "main");
}
