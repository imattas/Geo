use crate::ir::{CmpOp, Instruction, IrFunction, IrProgram};
use std::collections::HashMap;

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_SYM_CLASS_EXTERNAL: u8 = 2;
const IMAGE_SYM_DTYPE_FUNCTION: u16 = 0x20;
const IMAGE_SCN_CNT_CODE: u32 = 0x0000_0020;
const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x0000_0040;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_REL_AMD64_REL32: u16 = 0x0004;
const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;
const R_X86_64_PC32: u32 = 2;
const R_X86_64_PLT32: u32 = 4;

pub fn emit_elf64_relocatable(program: &IrProgram) -> Vec<u8> {
    let image = build_image(program);
    let names = build_names(program, &image);
    let strtab = names.strtab;
    let shstrtab = section_name_table();
    let symtab = build_symtab(&image, &names.symbol_offsets);
    let rela_text = build_relocations(&image, &names.symbol_indices);

    build_elf(
        &image.text,
        &image.rodata,
        &rela_text,
        &symtab,
        &strtab,
        &shstrtab.bytes,
        &shstrtab.offsets,
    )
}

pub fn emit_coff_x64_relocatable(program: &IrProgram) -> Option<Vec<u8>> {
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")?;
    let image = emit_coff_image_subset(main)?;
    let section_table_offset = 20_usize;
    let section_count = if image.rodata.is_empty() { 1_u16 } else { 2 };
    let text_offset = section_table_offset + section_count as usize * 40;
    let rdata_offset = text_offset + image.text.len();
    let text_relocation_offset = rdata_offset + image.rodata.len();
    let symbol_table_offset = text_relocation_offset + image.relocations.len() * 10;
    let symbol_count = 1_u32 + image.data_symbols.len() as u32;
    let symbol_indices = coff_symbol_indices(&image);
    let mut string_table = vec![0_u8; 4];

    let mut out = Vec::new();
    out.extend_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
    out.extend_from_slice(&section_count.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&(symbol_table_offset as u32).to_le_bytes());
    out.extend_from_slice(&symbol_count.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());

    write_coff_short_name(&mut out, b".text");
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&(image.text.len() as u32).to_le_bytes());
    out.extend_from_slice(&(text_offset as u32).to_le_bytes());
    out.extend_from_slice(&(text_relocation_offset as u32).to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&(image.relocations.len() as u16).to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out.extend_from_slice(
        &(IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ).to_le_bytes(),
    );

    if !image.rodata.is_empty() {
        write_coff_short_name(&mut out, b".rdata");
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(image.rodata.len() as u32).to_le_bytes());
        out.extend_from_slice(&(rdata_offset as u32).to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&(IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ).to_le_bytes());
    }

    out.extend_from_slice(&image.text);
    out.extend_from_slice(&image.rodata);
    for relocation in &image.relocations {
        let RelocationKind::Pc32 = relocation.kind else {
            return None;
        };
        let symbol_index = *symbol_indices.get(&relocation.symbol)?;
        out.extend_from_slice(&(relocation.offset as u32).to_le_bytes());
        out.extend_from_slice(&symbol_index.to_le_bytes());
        out.extend_from_slice(&IMAGE_REL_AMD64_REL32.to_le_bytes());
    }

    write_coff_symbol_name(&mut out, b"main", &mut string_table);
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&1_i16.to_le_bytes());
    out.extend_from_slice(&IMAGE_SYM_DTYPE_FUNCTION.to_le_bytes());
    out.push(IMAGE_SYM_CLASS_EXTERNAL);
    out.push(0);
    for symbol in &image.data_symbols {
        write_coff_symbol_name(&mut out, symbol.name.as_bytes(), &mut string_table);
        out.extend_from_slice(&(symbol.offset as u32).to_le_bytes());
        out.extend_from_slice(&2_i16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.push(IMAGE_SYM_CLASS_EXTERNAL);
        out.push(0);
    }
    let string_table_len = string_table.len() as u32;
    string_table[0..4].copy_from_slice(&string_table_len.to_le_bytes());
    out.extend_from_slice(&string_table);

    Some(out)
}

fn emit_coff_image_subset(function: &IrFunction) -> Option<ObjectImage> {
    let mut text = Vec::new();
    let mut rdata = Vec::new();
    let mut data_symbols = Vec::new();
    let mut relocations = Vec::new();
    emit_function_text(
        function,
        &mut text,
        &mut rdata,
        &mut data_symbols,
        &mut relocations,
    );
    if relocations
        .iter()
        .any(|relocation| !matches!(relocation.kind, RelocationKind::Pc32))
    {
        return None;
    }

    Some(ObjectImage {
        text,
        rodata: rdata,
        functions: vec![FunctionSymbol {
            name: function.name.clone(),
            offset: 0,
            size: 0,
        }],
        data_symbols,
        relocations,
    })
}

fn coff_symbol_indices(image: &ObjectImage) -> HashMap<String, u32> {
    let mut indices = HashMap::new();
    indices.insert("main".to_string(), 0);
    for (index, symbol) in image.data_symbols.iter().enumerate() {
        indices.insert(symbol.name.clone(), index as u32 + 1);
    }
    indices
}

fn write_coff_short_name(out: &mut Vec<u8>, name: &[u8]) {
    let mut bytes = [0_u8; 8];
    bytes[..name.len()].copy_from_slice(name);
    out.extend_from_slice(&bytes);
}

fn write_coff_symbol_name(out: &mut Vec<u8>, name: &[u8], string_table: &mut Vec<u8>) {
    if name.len() <= 8 {
        write_coff_short_name(out, name);
        return;
    }

    let offset = string_table.len() as u32;
    string_table.extend_from_slice(name);
    string_table.push(0);
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
}

struct ObjectImage {
    text: Vec<u8>,
    rodata: Vec<u8>,
    functions: Vec<FunctionSymbol>,
    data_symbols: Vec<DataSymbol>,
    relocations: Vec<TextRelocation>,
}

struct FunctionSymbol {
    name: String,
    offset: u64,
    size: u64,
}

struct DataSymbol {
    name: String,
    offset: u64,
    size: u64,
}

struct TextRelocation {
    offset: u64,
    symbol: String,
    kind: RelocationKind,
}

enum RelocationKind {
    Pc32,
    Plt32,
}

fn build_image(program: &IrProgram) -> ObjectImage {
    let mut text = Vec::new();
    let mut rodata = Vec::new();
    let mut functions = Vec::new();
    let mut data_symbols = Vec::new();
    let mut relocations = Vec::new();

    for function in &program.functions {
        let offset = text.len() as u64;
        emit_function_text(
            function,
            &mut text,
            &mut rodata,
            &mut data_symbols,
            &mut relocations,
        );
        let size = text.len() as u64 - offset;
        functions.push(FunctionSymbol {
            name: function.name.clone(),
            offset,
            size,
        });
    }

    ObjectImage {
        text,
        rodata,
        functions,
        data_symbols,
        relocations,
    }
}

fn emit_function_text(
    function: &IrFunction,
    bytes: &mut Vec<u8>,
    rodata: &mut Vec<u8>,
    data_symbols: &mut Vec<DataSymbol>,
    relocations: &mut Vec<TextRelocation>,
) {
    let frame = FrameLayout::new(function);
    let mut labels = HashMap::new();
    let mut jumps = Vec::new();
    bytes.extend_from_slice(&[0x55, 0x48, 0x89, 0xe5]);
    emit_stack_alloc(bytes, frame.stack_size);
    emit_parameter_spills(bytes, &frame, &function.params);

    for instruction in &function.instructions {
        match instruction {
            Instruction::Const { dst, value } => {
                emit_mov_mem_imm32(bytes, frame.value_offset(*dst), *value as i32);
            }
            Instruction::StringConst { dst, label, value } => {
                let offset = rodata.len() as u64;
                rodata.extend_from_slice(value.as_bytes());
                rodata.push(0);
                data_symbols.push(DataSymbol {
                    name: label.clone(),
                    offset,
                    size: value.len() as u64 + 1,
                });
                emit_lea_rax_symbol(bytes, label, relocations);
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::Add { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x03, &frame, *dst, *left, *right);
            }
            Instruction::Sub { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x2b, &frame, *dst, *left, *right);
            }
            Instruction::Mul { dst, left, right } => {
                emit_load_rax(bytes, frame.value_offset(*left));
                bytes.extend_from_slice(&[0x48, 0x0f, 0xaf]);
                emit_rbp_operand(bytes, 0, frame.value_offset(*right));
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::Div { dst, left, right } => {
                emit_division(bytes, &frame, *dst, *left, *right, DivisionResult::Quotient);
            }
            Instruction::Rem { dst, left, right } => {
                emit_division(
                    bytes,
                    &frame,
                    *dst,
                    *left,
                    *right,
                    DivisionResult::Remainder,
                );
            }
            Instruction::And { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x23, &frame, *dst, *left, *right);
            }
            Instruction::Or { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x0b, &frame, *dst, *left, *right);
            }
            Instruction::BitAnd { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x23, &frame, *dst, *left, *right);
            }
            Instruction::BitOr { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x0b, &frame, *dst, *left, *right);
            }
            Instruction::BitXor { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x33, &frame, *dst, *left, *right);
            }
            Instruction::ShiftLeft { dst, left, right } => {
                emit_shift(bytes, &frame, *dst, *left, *right, ShiftOp::Left);
            }
            Instruction::ShiftRight { dst, left, right } => {
                emit_shift(bytes, &frame, *dst, *left, *right, ShiftOp::Right);
            }
            Instruction::Load { dst, local } => {
                emit_load_rax(bytes, frame.local_offset(local));
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::AddressOf { dst, local } => {
                emit_lea_rax_local(bytes, frame.local_offset(local));
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::Deref { dst, pointer } => {
                emit_load_rax(bytes, frame.value_offset(*pointer));
                bytes.extend_from_slice(&[0x48, 0x8b, 0x00]);
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::BitNot { dst, value } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                bytes.extend_from_slice(&[0x48, 0xf7, 0xd0]);
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::Store { local, value } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                emit_store_rax(bytes, frame.local_offset(local));
            }
            Instruction::StoreDeref { pointer, value } => {
                emit_load_rax(bytes, frame.value_offset(*pointer));
                emit_load_r10(bytes, frame.value_offset(*value));
                bytes.extend_from_slice(&[0x4c, 0x89, 0x10]);
            }
            Instruction::Cmp {
                dst,
                op,
                left,
                right,
            } => {
                emit_compare(bytes, *op, &frame, *dst, *left, *right);
            }
            Instruction::Call { function, args, .. } => {
                let stack_arg_bytes = emit_call_args(bytes, &frame, args);
                let call_offset = bytes.len() as u64;
                bytes.push(0xe8);
                bytes.extend_from_slice(&0_i32.to_le_bytes());
                emit_stack_dealloc(bytes, stack_arg_bytes);
                relocations.push(TextRelocation {
                    offset: call_offset + 1,
                    symbol: function.clone(),
                    kind: RelocationKind::Plt32,
                });
                if let Instruction::Call { dst, .. } = instruction {
                    emit_store_rax(bytes, frame.value_offset(*dst));
                }
            }
            Instruction::BoundsCheck { index, len } => {
                emit_load_arg_register(bytes, 0, frame.value_offset(*index));
                emit_mov_arg_imm32(bytes, 1, *len as i32);
                let call_offset = bytes.len() as u64;
                bytes.push(0xe8);
                bytes.extend_from_slice(&0_i32.to_le_bytes());
                relocations.push(TextRelocation {
                    offset: call_offset + 1,
                    symbol: "__geo_bounds_check".to_string(),
                    kind: RelocationKind::Plt32,
                });
            }
            Instruction::Jump { label } => {
                emit_jump(bytes, label, &mut jumps);
            }
            Instruction::JumpIfZero { value, label } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                bytes.extend_from_slice(&[0x48, 0x83, 0xf8, 0x00]);
                emit_jump_if_equal(bytes, label, &mut jumps);
            }
            Instruction::Label { name } => {
                labels.insert(name.clone(), bytes.len() as u64);
            }
            Instruction::Return { value } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                bytes.extend_from_slice(&[0xc9, 0xc3]);
            }
        }
    }

    if !matches!(
        function.instructions.last(),
        Some(Instruction::Return { .. })
    ) {
        bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0, 0xc9, 0xc3]);
    }

    patch_jumps(bytes, &labels, &jumps);
}

struct JumpPatch {
    displacement_offset: usize,
    label: String,
}

enum DivisionResult {
    Quotient,
    Remainder,
}

enum ShiftOp {
    Left,
    Right,
}

struct FrameLayout {
    value_offsets: HashMap<crate::ir::ValueId, u32>,
    local_offsets: HashMap<String, u32>,
    stack_size: u32,
}

impl FrameLayout {
    fn new(function: &IrFunction) -> Self {
        let mut max_value = None;
        let mut locals = function.params.clone();

        for instruction in &function.instructions {
            collect_instruction_values(instruction, &mut max_value);
            match instruction {
                Instruction::Load { local, .. }
                | Instruction::AddressOf { local, .. }
                | Instruction::Store { local, .. } => {
                    if !locals.contains(local) {
                        locals.push(local.clone());
                    }
                }
                Instruction::Const { .. }
                | Instruction::StringConst { .. }
                | Instruction::And { .. }
                | Instruction::Or { .. }
                | Instruction::BitAnd { .. }
                | Instruction::BitOr { .. }
                | Instruction::BitXor { .. }
                | Instruction::ShiftLeft { .. }
                | Instruction::ShiftRight { .. }
                | Instruction::Add { .. }
                | Instruction::Sub { .. }
                | Instruction::Mul { .. }
                | Instruction::Div { .. }
                | Instruction::Rem { .. }
                | Instruction::Deref { .. }
                | Instruction::BitNot { .. }
                | Instruction::BoundsCheck { .. }
                | Instruction::StoreDeref { .. }
                | Instruction::Cmp { .. }
                | Instruction::Jump { .. }
                | Instruction::JumpIfZero { .. }
                | Instruction::Label { .. }
                | Instruction::Call { .. }
                | Instruction::Return { .. } => {}
            }
        }

        let value_count = max_value.map(|value| value.0 + 1).unwrap_or(0);
        let mut next_offset = 8_u32;
        let mut value_offsets = HashMap::new();
        for index in 0..value_count {
            value_offsets.insert(crate::ir::ValueId(index), next_offset);
            next_offset += 8;
        }

        let mut local_offsets = HashMap::new();
        for local in locals {
            local_offsets.insert(local, next_offset);
            next_offset += 8;
        }

        let used = next_offset.saturating_sub(8);
        Self {
            value_offsets,
            local_offsets,
            stack_size: align_to(used, 16),
        }
    }

    fn value_offset(&self, value: crate::ir::ValueId) -> u32 {
        *self.value_offsets.get(&value).expect("value stack slot")
    }

    fn local_offset(&self, local: &str) -> u32 {
        *self.local_offsets.get(local).expect("local stack slot")
    }
}

fn collect_instruction_values(
    instruction: &Instruction,
    max_value: &mut Option<crate::ir::ValueId>,
) {
    match instruction {
        Instruction::Const { dst, .. }
        | Instruction::StringConst { dst, .. }
        | Instruction::Load { dst, .. }
        | Instruction::AddressOf { dst, .. }
        | Instruction::Deref { dst, .. }
        | Instruction::BitNot { dst, .. }
        | Instruction::Cmp { dst, .. } => update_max_value(max_value, *dst),
        Instruction::Call { dst, args, .. } => {
            update_max_value(max_value, *dst);
            for arg in args {
                update_max_value(max_value, *arg);
            }
        }
        Instruction::And { dst, left, right }
        | Instruction::Or { dst, left, right }
        | Instruction::BitAnd { dst, left, right }
        | Instruction::BitOr { dst, left, right }
        | Instruction::BitXor { dst, left, right }
        | Instruction::ShiftLeft { dst, left, right }
        | Instruction::ShiftRight { dst, left, right }
        | Instruction::Add { dst, left, right }
        | Instruction::Sub { dst, left, right }
        | Instruction::Mul { dst, left, right }
        | Instruction::Div { dst, left, right }
        | Instruction::Rem { dst, left, right } => {
            update_max_value(max_value, *dst);
            update_max_value(max_value, *left);
            update_max_value(max_value, *right);
        }
        Instruction::BoundsCheck { index, .. }
        | Instruction::Store { value: index, .. }
        | Instruction::JumpIfZero { value: index, .. }
        | Instruction::Return { value: index } => update_max_value(max_value, *index),
        Instruction::StoreDeref { pointer, value } => {
            update_max_value(max_value, *pointer);
            update_max_value(max_value, *value);
        }
        Instruction::Jump { .. } | Instruction::Label { .. } => {}
    }
}

fn update_max_value(max_value: &mut Option<crate::ir::ValueId>, value: crate::ir::ValueId) {
    if max_value.map(|current| value.0 > current.0).unwrap_or(true) {
        *max_value = Some(value);
    }
}

fn emit_binary_mem_op(
    bytes: &mut Vec<u8>,
    opcode: u8,
    frame: &FrameLayout,
    dst: crate::ir::ValueId,
    left: crate::ir::ValueId,
    right: crate::ir::ValueId,
) {
    emit_load_rax(bytes, frame.value_offset(left));
    bytes.extend_from_slice(&[0x48, opcode]);
    emit_rbp_operand(bytes, 0, frame.value_offset(right));
    emit_store_rax(bytes, frame.value_offset(dst));
}

fn emit_compare(
    bytes: &mut Vec<u8>,
    op: CmpOp,
    frame: &FrameLayout,
    dst: crate::ir::ValueId,
    left: crate::ir::ValueId,
    right: crate::ir::ValueId,
) {
    emit_load_rax(bytes, frame.value_offset(left));
    bytes.extend_from_slice(&[0x48, 0x3b]);
    emit_rbp_operand(bytes, 0, frame.value_offset(right));
    bytes.extend_from_slice(&[0x0f, setcc_opcode(op), 0xc0]);
    bytes.extend_from_slice(&[0x48, 0x0f, 0xb6, 0xc0]);
    emit_store_rax(bytes, frame.value_offset(dst));
}

fn emit_division(
    bytes: &mut Vec<u8>,
    frame: &FrameLayout,
    dst: crate::ir::ValueId,
    left: crate::ir::ValueId,
    right: crate::ir::ValueId,
    result: DivisionResult,
) {
    emit_load_rax(bytes, frame.value_offset(left));
    bytes.extend_from_slice(&[0x48, 0x99]);
    bytes.extend_from_slice(&[0x48, 0xf7]);
    emit_rbp_operand(bytes, 7, frame.value_offset(right));
    match result {
        DivisionResult::Quotient => emit_store_rax(bytes, frame.value_offset(dst)),
        DivisionResult::Remainder => emit_store_rdx(bytes, frame.value_offset(dst)),
    }
}

fn emit_shift(
    bytes: &mut Vec<u8>,
    frame: &FrameLayout,
    dst: crate::ir::ValueId,
    left: crate::ir::ValueId,
    right: crate::ir::ValueId,
    op: ShiftOp,
) {
    emit_load_rax(bytes, frame.value_offset(left));
    emit_load_rcx(bytes, frame.value_offset(right));
    match op {
        ShiftOp::Left => bytes.extend_from_slice(&[0x48, 0xd3, 0xe0]),
        ShiftOp::Right => bytes.extend_from_slice(&[0x48, 0xd3, 0xf8]),
    }
    emit_store_rax(bytes, frame.value_offset(dst));
}

fn setcc_opcode(op: CmpOp) -> u8 {
    match op {
        CmpOp::Equal => 0x94,
        CmpOp::NotEqual => 0x95,
        CmpOp::Less => 0x9c,
        CmpOp::LessEqual => 0x9e,
        CmpOp::Greater => 0x9f,
        CmpOp::GreaterEqual => 0x9d,
    }
}

fn emit_jump(bytes: &mut Vec<u8>, label: &str, jumps: &mut Vec<JumpPatch>) {
    bytes.push(0xe9);
    let displacement_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    jumps.push(JumpPatch {
        displacement_offset,
        label: label.to_string(),
    });
}

fn emit_jump_if_equal(bytes: &mut Vec<u8>, label: &str, jumps: &mut Vec<JumpPatch>) {
    bytes.extend_from_slice(&[0x0f, 0x84]);
    let displacement_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    jumps.push(JumpPatch {
        displacement_offset,
        label: label.to_string(),
    });
}

fn emit_lea_rax_symbol(bytes: &mut Vec<u8>, symbol: &str, relocations: &mut Vec<TextRelocation>) {
    bytes.extend_from_slice(&[0x48, 0x8d, 0x05]);
    let displacement_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    relocations.push(TextRelocation {
        offset: displacement_offset as u64,
        symbol: symbol.to_string(),
        kind: RelocationKind::Pc32,
    });
}

fn patch_jumps(bytes: &mut [u8], labels: &HashMap<String, u64>, jumps: &[JumpPatch]) {
    for jump in jumps {
        let target = *labels
            .get(&jump.label)
            .unwrap_or_else(|| panic!("missing object label '{}'", jump.label));
        let next = jump.displacement_offset as u64 + 4;
        let displacement = target as i64 - next as i64;
        bytes[jump.displacement_offset..jump.displacement_offset + 4]
            .copy_from_slice(&(displacement as i32).to_le_bytes());
    }
}

fn emit_stack_alloc(bytes: &mut Vec<u8>, size: u32) {
    if size == 0 {
        return;
    }
    if size <= 127 {
        bytes.extend_from_slice(&[0x48, 0x83, 0xec, size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, 0xec]);
        bytes.extend_from_slice(&size.to_le_bytes());
    }
}

fn emit_stack_dealloc(bytes: &mut Vec<u8>, size: u32) {
    if size == 0 {
        return;
    }
    if size <= 127 {
        bytes.extend_from_slice(&[0x48, 0x83, 0xc4, size as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, 0xc4]);
        bytes.extend_from_slice(&size.to_le_bytes());
    }
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn emit_mov_mem_imm32(bytes: &mut Vec<u8>, offset: u32, value: i32) {
    bytes.extend_from_slice(&[0x48, 0xc7]);
    emit_rbp_operand(bytes, 0, offset);
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn emit_load_rax(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x8b]);
    emit_rbp_operand(bytes, 0, offset);
}

fn emit_load_rax_from_incoming_arg(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x8b]);
    emit_positive_rbp_operand(bytes, 0, offset);
}

fn emit_load_rcx(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x8b]);
    emit_rbp_operand(bytes, 1, offset);
}

fn emit_load_r10(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x4c, 0x8b]);
    emit_rbp_operand(bytes, 2, offset);
}

fn emit_call_args(bytes: &mut Vec<u8>, frame: &FrameLayout, args: &[crate::ir::ValueId]) -> u32 {
    for (index, arg) in args.iter().enumerate().take(6) {
        emit_load_arg_register(bytes, index, frame.value_offset(*arg));
    }

    let stack_args = args.len().saturating_sub(6);
    let padding = if stack_args % 2 == 1 { 8 } else { 0 };
    if padding > 0 {
        emit_stack_alloc(bytes, padding);
    }
    for arg in args.iter().skip(6).rev() {
        emit_load_rax(bytes, frame.value_offset(*arg));
        bytes.push(0x50);
    }

    (stack_args as u32 * 8) + padding
}

fn emit_parameter_spills(bytes: &mut Vec<u8>, frame: &FrameLayout, params: &[String]) {
    for (index, param) in params.iter().enumerate().take(6) {
        emit_store_arg_register(bytes, index, frame.local_offset(param));
    }
    for (stack_index, param) in params.iter().enumerate().skip(6) {
        let incoming_offset = 16 + ((stack_index - 6) as u32 * 8);
        emit_load_rax_from_incoming_arg(bytes, incoming_offset);
        emit_store_rax(bytes, frame.local_offset(param));
    }
}

fn emit_load_arg_register(bytes: &mut Vec<u8>, index: usize, offset: u32) {
    match index {
        0 => {
            bytes.extend_from_slice(&[0x48, 0x8b]);
            emit_rbp_operand(bytes, 7, offset);
        }
        1 => {
            bytes.extend_from_slice(&[0x48, 0x8b]);
            emit_rbp_operand(bytes, 6, offset);
        }
        2 => {
            bytes.extend_from_slice(&[0x48, 0x8b]);
            emit_rbp_operand(bytes, 2, offset);
        }
        3 => {
            bytes.extend_from_slice(&[0x48, 0x8b]);
            emit_rbp_operand(bytes, 1, offset);
        }
        4 => {
            bytes.extend_from_slice(&[0x4c, 0x8b]);
            emit_rbp_operand(bytes, 0, offset);
        }
        5 => {
            bytes.extend_from_slice(&[0x4c, 0x8b]);
            emit_rbp_operand(bytes, 1, offset);
        }
        _ => {}
    }
}

fn emit_mov_arg_imm32(bytes: &mut Vec<u8>, index: usize, value: i32) {
    match index {
        0 => bytes.extend_from_slice(&[0x48, 0xc7, 0xc7]),
        1 => bytes.extend_from_slice(&[0x48, 0xc7, 0xc6]),
        2 => bytes.extend_from_slice(&[0x48, 0xc7, 0xc2]),
        3 => bytes.extend_from_slice(&[0x48, 0xc7, 0xc1]),
        4 => bytes.extend_from_slice(&[0x49, 0xc7, 0xc0]),
        5 => bytes.extend_from_slice(&[0x49, 0xc7, 0xc1]),
        _ => return,
    }
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn emit_store_arg_register(bytes: &mut Vec<u8>, index: usize, offset: u32) {
    match index {
        0 => {
            bytes.extend_from_slice(&[0x48, 0x89]);
            emit_rbp_operand(bytes, 7, offset);
        }
        1 => {
            bytes.extend_from_slice(&[0x48, 0x89]);
            emit_rbp_operand(bytes, 6, offset);
        }
        2 => {
            bytes.extend_from_slice(&[0x48, 0x89]);
            emit_rbp_operand(bytes, 2, offset);
        }
        3 => {
            bytes.extend_from_slice(&[0x48, 0x89]);
            emit_rbp_operand(bytes, 1, offset);
        }
        4 => {
            bytes.extend_from_slice(&[0x4c, 0x89]);
            emit_rbp_operand(bytes, 0, offset);
        }
        5 => {
            bytes.extend_from_slice(&[0x4c, 0x89]);
            emit_rbp_operand(bytes, 1, offset);
        }
        _ => {}
    }
}

fn emit_lea_rax_local(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x8d]);
    emit_rbp_operand(bytes, 0, offset);
}

fn emit_store_rax(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x89]);
    emit_rbp_operand(bytes, 0, offset);
}

fn emit_store_rdx(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x89]);
    emit_rbp_operand(bytes, 2, offset);
}

fn emit_rbp_operand(bytes: &mut Vec<u8>, reg: u8, offset: u32) {
    if offset <= 128 {
        bytes.push(0x40 | (reg << 3) | 0x05);
        bytes.push((-(offset as i32)) as i8 as u8);
    } else {
        bytes.push(0x80 | (reg << 3) | 0x05);
        bytes.extend_from_slice(&(-(offset as i32)).to_le_bytes());
    }
}

fn emit_positive_rbp_operand(bytes: &mut Vec<u8>, reg: u8, offset: u32) {
    if offset <= 127 {
        bytes.push(0x40 | (reg << 3) | 0x05);
        bytes.push(offset as u8);
    } else {
        bytes.push(0x80 | (reg << 3) | 0x05);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
}

struct Names {
    strtab: Vec<u8>,
    symbol_offsets: HashMap<String, u32>,
    symbol_indices: HashMap<String, u32>,
}

fn build_names(program: &IrProgram, image: &ObjectImage) -> Names {
    let mut strtab = vec![0];
    let mut symbol_offsets = HashMap::new();
    let mut symbol_indices = HashMap::new();
    let mut next_index = 2_u32;

    for function in &program.functions {
        add_symbol_name(
            &mut strtab,
            &mut symbol_offsets,
            &mut symbol_indices,
            &function.name,
            &mut next_index,
        );
    }
    for symbol in &image.data_symbols {
        add_symbol_name(
            &mut strtab,
            &mut symbol_offsets,
            &mut symbol_indices,
            &symbol.name,
            &mut next_index,
        );
    }
    for relocation in &image.relocations {
        add_symbol_name(
            &mut strtab,
            &mut symbol_offsets,
            &mut symbol_indices,
            &relocation.symbol,
            &mut next_index,
        );
    }

    Names {
        strtab,
        symbol_offsets,
        symbol_indices,
    }
}

fn add_symbol_name(
    strtab: &mut Vec<u8>,
    offsets: &mut HashMap<String, u32>,
    indices: &mut HashMap<String, u32>,
    name: &str,
    next_index: &mut u32,
) {
    if offsets.contains_key(name) {
        return;
    }
    let offset = strtab.len() as u32;
    strtab.extend_from_slice(name.as_bytes());
    strtab.push(0);
    offsets.insert(name.to_string(), offset);
    indices.insert(name.to_string(), *next_index);
    *next_index += 1;
}

fn build_symtab(image: &ObjectImage, symbol_offsets: &HashMap<String, u32>) -> Vec<u8> {
    let mut out = Vec::new();
    write_symbol(&mut out, 0, 0, 0, 0, 0, 0);
    write_symbol(&mut out, 0, 0x03, 0, 1, 0, 0);

    for function in &image.functions {
        write_symbol(
            &mut out,
            *symbol_offsets.get(&function.name).expect("function symbol"),
            0x12,
            0,
            1,
            function.offset,
            function.size,
        );
    }

    for symbol in &image.data_symbols {
        write_symbol(
            &mut out,
            *symbol_offsets.get(&symbol.name).expect("data symbol"),
            0x11,
            0,
            2,
            symbol.offset,
            symbol.size,
        );
    }

    let defined: Vec<&str> = image
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .chain(image.data_symbols.iter().map(|symbol| symbol.name.as_str()))
        .collect();
    let mut emitted = Vec::new();
    for relocation in &image.relocations {
        if defined.contains(&relocation.symbol.as_str()) || emitted.contains(&relocation.symbol) {
            continue;
        }
        emitted.push(relocation.symbol.clone());
        write_symbol(
            &mut out,
            *symbol_offsets
                .get(&relocation.symbol)
                .expect("relocation symbol"),
            0x10,
            0,
            0,
            0,
            0,
        );
    }

    out
}

fn write_symbol(
    out: &mut Vec<u8>,
    name: u32,
    info: u8,
    other: u8,
    shndx: u16,
    value: u64,
    size: u64,
) {
    out.extend_from_slice(&name.to_le_bytes());
    out.push(info);
    out.push(other);
    out.extend_from_slice(&shndx.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
}

fn build_relocations(image: &ObjectImage, symbol_indices: &HashMap<String, u32>) -> Vec<u8> {
    let mut out = Vec::new();
    for relocation in &image.relocations {
        let symbol = *symbol_indices
            .get(&relocation.symbol)
            .expect("relocation symbol index") as u64;
        let kind = match relocation.kind {
            RelocationKind::Pc32 => R_X86_64_PC32,
            RelocationKind::Plt32 => R_X86_64_PLT32,
        };
        out.extend_from_slice(&relocation.offset.to_le_bytes());
        out.extend_from_slice(&((symbol << 32) | u64::from(kind)).to_le_bytes());
        out.extend_from_slice(&(-4_i64).to_le_bytes());
    }
    out
}

struct SectionNameTable {
    bytes: Vec<u8>,
    offsets: HashMap<&'static str, u32>,
}

fn section_name_table() -> SectionNameTable {
    let mut bytes = vec![0];
    let mut offsets = HashMap::new();
    for name in [
        ".text",
        ".rodata",
        ".rela.text",
        ".symtab",
        ".strtab",
        ".shstrtab",
    ] {
        offsets.insert(name, bytes.len() as u32);
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
    }
    SectionNameTable { bytes, offsets }
}

fn build_elf(
    text: &[u8],
    rodata: &[u8],
    rela_text: &[u8],
    symtab: &[u8],
    strtab: &[u8],
    shstrtab: &[u8],
    shstr_offsets: &HashMap<&'static str, u32>,
) -> Vec<u8> {
    let mut out = vec![0; 64];
    let text_offset = append_section(&mut out, text, 16);
    let rodata_offset = append_section(&mut out, rodata, 8);
    let rela_offset = append_section(&mut out, rela_text, 8);
    let symtab_offset = append_section(&mut out, symtab, 8);
    let strtab_offset = append_section(&mut out, strtab, 1);
    let shstrtab_offset = append_section(&mut out, shstrtab, 1);
    align_vec(&mut out, 8);
    let section_header_offset = out.len() as u64;

    write_null_section(&mut out);
    write_section(
        &mut out,
        shstr_offsets[".text"],
        SHT_PROGBITS,
        SHF_ALLOC | SHF_EXECINSTR,
        text_offset,
        text.len() as u64,
        0,
        0,
        16,
        0,
    );
    write_section(
        &mut out,
        shstr_offsets[".rodata"],
        SHT_PROGBITS,
        SHF_ALLOC,
        rodata_offset,
        rodata.len() as u64,
        0,
        0,
        8,
        0,
    );
    write_section(
        &mut out,
        shstr_offsets[".rela.text"],
        SHT_RELA,
        0,
        rela_offset,
        rela_text.len() as u64,
        4,
        1,
        8,
        24,
    );
    write_section(
        &mut out,
        shstr_offsets[".symtab"],
        SHT_SYMTAB,
        0,
        symtab_offset,
        symtab.len() as u64,
        5,
        2,
        8,
        24,
    );
    write_section(
        &mut out,
        shstr_offsets[".strtab"],
        SHT_STRTAB,
        0,
        strtab_offset,
        strtab.len() as u64,
        0,
        0,
        1,
        0,
    );
    write_section(
        &mut out,
        shstr_offsets[".shstrtab"],
        SHT_STRTAB,
        0,
        shstrtab_offset,
        shstrtab.len() as u64,
        0,
        0,
        1,
        0,
    );

    write_elf_header(&mut out, section_header_offset);
    out
}

fn append_section(out: &mut Vec<u8>, data: &[u8], alignment: usize) -> u64 {
    align_vec(out, alignment);
    let offset = out.len() as u64;
    out.extend_from_slice(data);
    offset
}

fn align_vec(out: &mut Vec<u8>, alignment: usize) {
    while out.len() % alignment != 0 {
        out.push(0);
    }
}

fn write_elf_header(out: &mut [u8], section_header_offset: u64) {
    out[0..4].copy_from_slice(b"\x7fELF");
    out[4] = 2;
    out[5] = 1;
    out[6] = 1;
    out[16..18].copy_from_slice(&1_u16.to_le_bytes());
    out[18..20].copy_from_slice(&62_u16.to_le_bytes());
    out[20..24].copy_from_slice(&1_u32.to_le_bytes());
    out[40..48].copy_from_slice(&section_header_offset.to_le_bytes());
    out[52..54].copy_from_slice(&64_u16.to_le_bytes());
    out[58..60].copy_from_slice(&64_u16.to_le_bytes());
    out[60..62].copy_from_slice(&7_u16.to_le_bytes());
    out[62..64].copy_from_slice(&6_u16.to_le_bytes());
}

fn write_null_section(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0; 64]);
}

#[allow(clippy::too_many_arguments)]
fn write_section(
    out: &mut Vec<u8>,
    name: u32,
    section_type: u32,
    flags: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    addralign: u64,
    entsize: u64,
) {
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&section_type.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&0_u64.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&link.to_le_bytes());
    out.extend_from_slice(&info.to_le_bytes());
    out.extend_from_slice(&addralign.to_le_bytes());
    out.extend_from_slice(&entsize.to_le_bytes());
}
