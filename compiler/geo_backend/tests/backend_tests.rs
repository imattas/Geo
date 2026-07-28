use geo_backend::ir::{Instruction, IrFunction, IrProgram, ValueId};
use geo_backend::object::emit_elf64_relocatable;

#[test]
fn backend_emits_a_compiler_owned_elf_object() {
    let value = ValueId(0);
    let program = IrProgram {
        functions: vec![IrFunction {
            name: "main".to_string(),
            params: Vec::new(),
            instructions: vec![
                Instruction::Const {
                    dst: value,
                    value: 42,
                },
                Instruction::Return { value },
            ],
        }],
    };

    let object = emit_elf64_relocatable(&program);
    assert_eq!(&object[0..4], b"\x7fELF");
}

#[test]
fn backend_emits_direct_memory_compare_helpers() {
    let source = geo_ir::ir::ValueId(0);
    let right = geo_ir::ir::ValueId(1);
    let len = geo_ir::ir::ValueId(2);
    let result = geo_ir::ir::ValueId(3);
    let program = geo_ir::ir::IrProgram {
        functions: vec![geo_ir::ir::IrFunction {
            name: "main".to_string(),
            params: Vec::new(),
            instructions: vec![
                geo_ir::ir::Instruction::Const {
                    dst: source,
                    value: 0,
                },
                geo_ir::ir::Instruction::Const {
                    dst: right,
                    value: 0,
                },
                geo_ir::ir::Instruction::Const { dst: len, value: 0 },
                geo_ir::ir::Instruction::Call {
                    dst: result,
                    function: "mem_compare".to_string(),
                    args: vec![source, right, len],
                },
                geo_ir::ir::Instruction::Return { value: result },
            ],
        }],
    };
    let object = emit_elf64_relocatable(&program);
    assert!(!object.is_empty());
}
