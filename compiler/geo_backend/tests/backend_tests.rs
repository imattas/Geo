use geo_backend::elf::emit_elf64_executable;
use geo_backend::ir::{Instruction, IrFunction, IrProgram, ValueId};
use geo_backend::object::emit_elf64_relocatable;
use geo_backend::pe::emit_pe64_console;

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

#[test]
fn backend_emits_memory_predicate_helpers_for_both_native_formats() {
    let left = ValueId(0);
    let right = ValueId(1);
    let len = ValueId(2);
    let equal = ValueId(3);
    let zero = ValueId(4);
    let result = ValueId(5);
    let program = IrProgram {
        functions: vec![IrFunction {
            name: "main".to_string(),
            params: Vec::new(),
            instructions: vec![
                Instruction::Const {
                    dst: left,
                    value: 0,
                },
                Instruction::Const {
                    dst: right,
                    value: 0,
                },
                Instruction::Const { dst: len, value: 0 },
                Instruction::Call {
                    dst: equal,
                    function: "mem_equal".to_string(),
                    args: vec![left, right, len],
                },
                Instruction::Call {
                    dst: zero,
                    function: "mem_is_zero".to_string(),
                    args: vec![left, len],
                },
                Instruction::Add {
                    dst: result,
                    left: equal,
                    right: zero,
                },
                Instruction::Return { value: result },
            ],
        }],
    };
    let elf = emit_elf64_executable(&program).expect("ELF predicate helpers should emit");
    let pe = emit_pe64_console(&program).expect("PE predicate helpers should emit");
    assert!(elf.windows(3).any(|bytes| bytes == [0x48, 0x85, 0xd2]));
    assert!(pe.windows(3).any(|bytes| bytes == [0x4d, 0x85, 0xc0]));
}

#[test]
fn backend_emits_memory_reordering_helpers_for_both_native_formats() {
    let left = ValueId(0);
    let len = ValueId(1);
    let program = IrProgram {
        functions: vec![IrFunction {
            name: "main".to_string(),
            params: Vec::new(),
            instructions: vec![
                Instruction::Const {
                    dst: left,
                    value: 0,
                },
                Instruction::Const { dst: len, value: 0 },
                Instruction::Call {
                    dst: ValueId(2),
                    function: "mem_reverse".to_string(),
                    args: vec![left, len],
                },
                Instruction::Const {
                    dst: ValueId(3),
                    value: 0,
                },
                Instruction::Return { value: ValueId(3) },
            ],
        }],
    };

    let elf = emit_elf64_executable(&program).expect("ELF helper emission failed");
    let pe = emit_pe64_console(&program).expect("PE helper emission failed");
    assert!(elf.windows(3).any(|bytes| bytes == [0x48, 0x85, 0xf6]));
    assert!(pe
        .windows(5)
        .any(|bytes| bytes == [0x48, 0x8d, 0x54, 0x11, 0xff]));
}
