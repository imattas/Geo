use crate::ir::{Instruction, IrFunction, IrProgram};
use std::collections::HashMap;

const SHT_PROGBITS: u32 = 1;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;
const R_X86_64_PLT32: u32 = 4;

pub fn emit_elf64_relocatable(program: &IrProgram) -> Vec<u8> {
    let text = build_text(program);
    let names = build_names(program, &text);
    let strtab = names.strtab;
    let shstrtab = section_name_table();
    let symtab = build_symtab(&text, &names.symbol_offsets);
    let rela_text = build_relocations(&text, &names.symbol_indices);

    build_elf(
        &text.bytes,
        &rela_text,
        &symtab,
        &strtab,
        &shstrtab.bytes,
        &shstrtab.offsets,
    )
}

struct TextImage {
    bytes: Vec<u8>,
    functions: Vec<FunctionSymbol>,
    relocations: Vec<CallRelocation>,
}

struct FunctionSymbol {
    name: String,
    offset: u64,
    size: u64,
}

struct CallRelocation {
    offset: u64,
    symbol: String,
}

fn build_text(program: &IrProgram) -> TextImage {
    let mut bytes = Vec::new();
    let mut functions = Vec::new();
    let mut relocations = Vec::new();

    for function in &program.functions {
        let offset = bytes.len() as u64;
        emit_function_text(function, &mut bytes, &mut relocations);
        let size = bytes.len() as u64 - offset;
        functions.push(FunctionSymbol {
            name: function.name.clone(),
            offset,
            size,
        });
    }

    TextImage {
        bytes,
        functions,
        relocations,
    }
}

fn emit_function_text(
    function: &IrFunction,
    bytes: &mut Vec<u8>,
    relocations: &mut Vec<CallRelocation>,
) {
    let frame = FrameLayout::new(function);
    bytes.extend_from_slice(&[0x55, 0x48, 0x89, 0xe5]);
    emit_stack_alloc(bytes, frame.stack_size);

    for instruction in &function.instructions {
        match instruction {
            Instruction::Const { dst, value } => {
                emit_mov_mem_imm32(bytes, frame.value_offset(*dst), *value as i32);
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
            Instruction::BitAnd { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x23, &frame, *dst, *left, *right);
            }
            Instruction::BitOr { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x0b, &frame, *dst, *left, *right);
            }
            Instruction::BitXor { dst, left, right } => {
                emit_binary_mem_op(bytes, 0x33, &frame, *dst, *left, *right);
            }
            Instruction::Load { dst, local } => {
                emit_load_rax(bytes, frame.local_offset(local));
                emit_store_rax(bytes, frame.value_offset(*dst));
            }
            Instruction::Store { local, value } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                emit_store_rax(bytes, frame.local_offset(local));
            }
            Instruction::Call { function, .. } => {
                let call_offset = bytes.len() as u64;
                bytes.push(0xe8);
                bytes.extend_from_slice(&0_i32.to_le_bytes());
                relocations.push(CallRelocation {
                    offset: call_offset + 1,
                    symbol: function.clone(),
                });
                if let Instruction::Call { dst, .. } = instruction {
                    emit_store_rax(bytes, frame.value_offset(*dst));
                }
            }
            Instruction::BoundsCheck { .. } => {
                let call_offset = bytes.len() as u64;
                bytes.push(0xe8);
                bytes.extend_from_slice(&0_i32.to_le_bytes());
                relocations.push(CallRelocation {
                    offset: call_offset + 1,
                    symbol: "__geo_bounds_check".to_string(),
                });
            }
            Instruction::Return { value } => {
                emit_load_rax(bytes, frame.value_offset(*value));
                bytes.extend_from_slice(&[0xc9, 0xc3]);
            }
            Instruction::StringConst { .. }
            | Instruction::And { .. }
            | Instruction::Or { .. }
            | Instruction::ShiftLeft { .. }
            | Instruction::ShiftRight { .. }
            | Instruction::Div { .. }
            | Instruction::Rem { .. }
            | Instruction::AddressOf { .. }
            | Instruction::Deref { .. }
            | Instruction::BitNot { .. }
            | Instruction::StoreDeref { .. }
            | Instruction::Cmp { .. }
            | Instruction::Jump { .. }
            | Instruction::JumpIfZero { .. }
            | Instruction::Label { .. } => {}
        }
    }

    if !matches!(
        function.instructions.last(),
        Some(Instruction::Return { .. })
    ) {
        bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0, 0xc9, 0xc3]);
    }
}

struct FrameLayout {
    value_offsets: HashMap<crate::ir::ValueId, u32>,
    local_offsets: HashMap<String, u32>,
    stack_size: u32,
}

impl FrameLayout {
    fn new(function: &IrFunction) -> Self {
        let mut max_value = None;
        let mut locals = Vec::new();

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

fn emit_store_rax(bytes: &mut Vec<u8>, offset: u32) {
    bytes.extend_from_slice(&[0x48, 0x89]);
    emit_rbp_operand(bytes, 0, offset);
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

struct Names {
    strtab: Vec<u8>,
    symbol_offsets: HashMap<String, u32>,
    symbol_indices: HashMap<String, u32>,
}

fn build_names(program: &IrProgram, text: &TextImage) -> Names {
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
    for relocation in &text.relocations {
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

fn build_symtab(text: &TextImage, symbol_offsets: &HashMap<String, u32>) -> Vec<u8> {
    let mut out = Vec::new();
    write_symbol(&mut out, 0, 0, 0, 0, 0, 0);
    write_symbol(&mut out, 0, 0x03, 0, 1, 0, 0);

    for function in &text.functions {
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

    let defined: Vec<&str> = text
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .collect();
    let mut emitted = Vec::new();
    for relocation in &text.relocations {
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

fn build_relocations(text: &TextImage, symbol_indices: &HashMap<String, u32>) -> Vec<u8> {
    let mut out = Vec::new();
    for relocation in &text.relocations {
        let symbol = *symbol_indices
            .get(&relocation.symbol)
            .expect("relocation symbol index") as u64;
        out.extend_from_slice(&relocation.offset.to_le_bytes());
        out.extend_from_slice(&((symbol << 32) | u64::from(R_X86_64_PLT32)).to_le_bytes());
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
    for name in [".text", ".rela.text", ".symtab", ".strtab", ".shstrtab"] {
        offsets.insert(name, bytes.len() as u32);
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
    }
    SectionNameTable { bytes, offsets }
}

fn build_elf(
    text: &[u8],
    rela_text: &[u8],
    symtab: &[u8],
    strtab: &[u8],
    shstrtab: &[u8],
    shstr_offsets: &HashMap<&'static str, u32>,
) -> Vec<u8> {
    let mut out = vec![0; 64];
    let text_offset = append_section(&mut out, text, 16);
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
        shstr_offsets[".rela.text"],
        SHT_RELA,
        0,
        rela_offset,
        rela_text.len() as u64,
        3,
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
        4,
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
    out[60..62].copy_from_slice(&6_u16.to_le_bytes());
    out[62..64].copy_from_slice(&5_u16.to_le_bytes());
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
