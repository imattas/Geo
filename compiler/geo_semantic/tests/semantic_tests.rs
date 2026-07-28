use geo_semantic::ast::{Function, Param, Program, Stmt, Type};
use geo_semantic::borrow;
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
        }],
    };

    typecheck::check(&program).expect("type checking should succeed");
    borrow::check(&program).expect("borrow checking should succeed");
}
