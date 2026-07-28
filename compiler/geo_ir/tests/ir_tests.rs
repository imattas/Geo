use geo_ir::ir::{Instruction, IrFunction, IrProgram, ValueId};

#[test]
fn ir_values_and_programs_are_stable_data() {
    let value = ValueId(0);
    let function = IrFunction {
        name: "main".to_string(),
        params: Vec::new(),
        instructions: vec![
            Instruction::Const {
                dst: value,
                value: 42,
            },
            Instruction::Return { value },
        ],
    };
    let program = IrProgram {
        functions: vec![function],
    };

    assert_eq!(program.functions[0].name, "main");
    assert_eq!(program.functions[0].instructions.len(), 2);
}
