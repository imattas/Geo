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
