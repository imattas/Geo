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
    let mut consts = HashMap::new();
    bytes.extend_from_slice(&[0x55, 0x48, 0x89, 0xe5]);

    for instruction in &function.instructions {
        match instruction {
            Instruction::Const { dst, value } => {
                consts.insert(*dst, *value);
            }
            Instruction::Call { function, .. } => {
                let call_offset = bytes.len() as u64;
                bytes.push(0xe8);
                bytes.extend_from_slice(&0_i32.to_le_bytes());
                relocations.push(CallRelocation {
                    offset: call_offset + 1,
                    symbol: function.clone(),
                });
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
                let value = consts.get(value).copied().unwrap_or(0);
                bytes.push(0xb8);
                bytes.extend_from_slice(&(value as i32).to_le_bytes());
                bytes.extend_from_slice(&[0x5d, 0xc3]);
            }
            Instruction::StringConst { .. }
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
            | Instruction::Load { .. }
            | Instruction::AddressOf { .. }
            | Instruction::Deref { .. }
            | Instruction::BitNot { .. }
            | Instruction::Store { .. }
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
        bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0, 0x5d, 0xc3]);
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
