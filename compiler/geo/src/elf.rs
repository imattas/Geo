use crate::ir::IrProgram;
use crate::object::{build_linux_code_image, ObjectImage};
use std::collections::HashMap;

const ELFCLASS64: u8 = 2;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 0x3e;
const PT_LOAD: u32 = 1;
const PF_R: u32 = 4;
const PF_X: u32 = 1;
const BASE: u64 = 0x400000;
const TEXT_OFFSET: usize = 0x1000;
const PAGE_SIZE: usize = 0x1000;
const START_LEN: usize = 14;

pub fn emit_elf64_executable(program: &IrProgram) -> Option<Vec<u8>> {
    let image = build_linux_code_image(program);
    let main = image
        .functions
        .iter()
        .find(|function| function.name == "main")?;
    let runtime = build_runtime_text(&image);
    let text_len = START_LEN + image.text.len() + runtime.code.len();
    let rdata_offset = align_to(TEXT_OFFSET + text_len, PAGE_SIZE);
    let defined = defined_symbols(&image, rdata_offset, &runtime.symbols);
    if image
        .relocations
        .iter()
        .any(|relocation| !defined.contains_key(&relocation.symbol))
    {
        return None;
    }

    let file_size = rdata_offset + image.rodata.len();
    let entry = BASE + TEXT_OFFSET as u64;
    let mut output = vec![0_u8; file_size];
    write_elf_header(&mut output, entry);
    write_load_header(&mut output, file_size);

    let start = &mut output[TEXT_OFFSET..TEXT_OFFSET + START_LEN];
    start[0] = 0xe8;
    let main_target = START_LEN as i64 + main.offset as i64;
    let main_next = 5_i64;
    start[1..5].copy_from_slice(&((main_target - main_next) as i32).to_le_bytes());
    start[5..].copy_from_slice(&[0x89, 0xc7, 0xb8, 60, 0, 0, 0, 0x0f, 0x05]);

    let image_text_end = TEXT_OFFSET + START_LEN + image.text.len();
    output[TEXT_OFFSET + START_LEN..image_text_end].copy_from_slice(&image.text);
    output[image_text_end..TEXT_OFFSET + text_len].copy_from_slice(&runtime.code);
    output[rdata_offset..].copy_from_slice(&image.rodata);
    patch_relocations(&mut output, &image, rdata_offset, &runtime.symbols);
    Some(output)
}

struct RuntimeText {
    code: Vec<u8>,
    symbols: HashMap<String, u64>,
}

fn build_runtime_text(image: &ObjectImage) -> RuntimeText {
    let defined: HashMap<&str, ()> = image
        .functions
        .iter()
        .map(|function| (function.name.as_str(), ()))
        .chain(
            image
                .data_symbols
                .iter()
                .map(|symbol| (symbol.name.as_str(), ())),
        )
        .collect();
    let mut code = Vec::new();
    let mut symbols = HashMap::new();
    for name in image
        .relocations
        .iter()
        .map(|relocation| relocation.symbol.as_str())
        .filter(|name| !defined.contains_key(name))
    {
        if symbols.contains_key(name) {
            continue;
        }
        let offset = code.len();
        match name {
            "string_len" => emit_string_len_runtime(&mut code),
            "print" => emit_print_runtime(&mut code, false),
            "println" => emit_print_runtime(&mut code, true),
            "string_concat" => emit_string_concat_runtime(&mut code),
            "exit_geo" => emit_exit_runtime(&mut code),
            "alloc" | "alloc_zeroed" => emit_alloc_runtime(&mut code, false),
            "alloc_array" => emit_alloc_runtime(&mut code, true),
            _ => continue,
        }
        symbols.insert(
            name.to_string(),
            BASE + TEXT_OFFSET as u64 + START_LEN as u64 + image.text.len() as u64 + offset as u64,
        );
    }
    RuntimeText { code, symbols }
}

fn defined_symbols(
    image: &ObjectImage,
    rdata_offset: usize,
    runtime_symbols: &HashMap<String, u64>,
) -> HashMap<String, u64> {
    let mut symbols = HashMap::new();
    for function in &image.functions {
        symbols.insert(
            function.name.clone(),
            BASE + TEXT_OFFSET as u64 + START_LEN as u64 + function.offset,
        );
    }
    for symbol in &image.data_symbols {
        symbols.insert(
            symbol.name.clone(),
            BASE + rdata_offset as u64 + symbol.offset,
        );
    }
    symbols.extend(runtime_symbols.clone());
    symbols
}

fn patch_relocations(
    output: &mut [u8],
    image: &ObjectImage,
    rdata_offset: usize,
    runtime_symbols: &HashMap<String, u64>,
) {
    let symbols = defined_symbols(image, rdata_offset, runtime_symbols);
    for relocation in &image.relocations {
        let symbol = symbols
            .get(&relocation.symbol)
            .copied()
            .expect("direct ELF relocations are resolved before emission");
        let field_offset = TEXT_OFFSET + START_LEN + relocation.offset as usize;
        let field_address = BASE + field_offset as u64;
        let displacement = symbol as i64 - (field_address + 4) as i64;
        output[field_offset..field_offset + 4]
            .copy_from_slice(&(displacement as i32).to_le_bytes());
    }

    for symbol in &image.data_symbols {
        let expected = BASE + rdata_offset as u64 + symbol.offset;
        debug_assert_eq!(symbols[&symbol.name], expected);
    }
}

fn emit_string_len_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x0e]);
    code.extend_from_slice(&[0x85, 0xc9]);
    code.extend_from_slice(&[0x74, 0x08]);
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf1, 0xc3]);
}

fn emit_print_runtime(code: &mut Vec<u8>, newline: bool) {
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x49, 0x89, 0xf8]);
    code.extend_from_slice(&[0x31, 0xd2]);
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    code.extend_from_slice(&[0x74, 0x07]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xff, 0xc2]);
    code.extend_from_slice(&[0xeb, 0xf1]);
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0]);
    code.extend_from_slice(&[0xbf, 1, 0, 0, 0]);
    code.extend_from_slice(&[0x0f, 0x05]);
    if newline {
        code.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]);
        code.extend_from_slice(&[0xc6, 0x04, 0x24, b'\n']);
        code.extend_from_slice(&[0xb8, 1, 0, 0, 0]);
        code.extend_from_slice(&[0xbf, 1, 0, 0, 0]);
        code.extend_from_slice(&[0x48, 0x89, 0xe6]);
        code.extend_from_slice(&[0xba, 1, 0, 0, 0]);
        code.extend_from_slice(&[0x0f, 0x05]);
        code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
    }
    code.push(0xc3);
}

fn emit_string_concat_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x49, 0x89, 0xf8]);
    code.extend_from_slice(&[0x31, 0xc0]);
    let left_len_loop = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    let left_len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, left_len_loop);
    let left_len_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x10]);

    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x49, 0x89, 0xf0]);
    code.extend_from_slice(&[0x31, 0xc0]);
    let right_len_loop = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    let right_len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, right_len_loop);
    let right_len_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);

    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x03, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0x48, 0x89, 0xc6]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);

    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x49, 0x89, 0xc1]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x49, 0x89, 0xf8]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x10]);
    let left_copy_loop = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xfa, 0x00]);
    let left_copy_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x8a, 0x00]);
    code.extend_from_slice(&[0x41, 0x88, 0x01]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xca]);
    emit_short_jump_back(code, left_copy_loop);

    let left_copy_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x49, 0x89, 0xf0]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x54, 0x24, 0x18]);
    let right_copy_loop = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xfa, 0x00]);
    let right_copy_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x8a, 0x00]);
    code.extend_from_slice(&[0x41, 0x88, 0x01]);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xca]);
    emit_short_jump_back(code, right_copy_loop);

    let done = code.len();
    code.extend_from_slice(&[0x41, 0xc6, 0x01, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]);
    code.push(0xc3);

    patch_short_jump(code, left_len_done, left_len_target);
    patch_short_jump(code, right_len_done, right_len_target);
    patch_short_jump(code, left_copy_done, left_copy_target);
    patch_short_jump(code, right_copy_done, done);
}

fn emit_exit_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0xb8, 60, 0, 0, 0, 0x0f, 0x05]);
    code.push(0xc3);
}

fn emit_alloc_runtime(code: &mut Vec<u8>, array: bool) {
    if array {
        code.extend_from_slice(&[0x48, 0x0f, 0xaf, 0xfe]);
    }
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0xc3]);
}

fn emit_short_jump_placeholder(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.push(opcode);
    let offset = code.len();
    code.push(0);
    offset
}

fn emit_short_jump_back(code: &mut Vec<u8>, target: usize) {
    code.push(0xeb);
    let next = code.len() + 1;
    code.push((target as isize - next as isize) as i8 as u8);
}

fn patch_short_jump(code: &mut [u8], displacement: usize, target: usize) {
    let next = displacement + 1;
    code[displacement] = (target as isize - next as isize) as i8 as u8;
}

fn write_elf_header(output: &mut [u8], entry: u64) {
    output[0..16].copy_from_slice(&[
        0x7f, b'E', b'L', b'F', ELFCLASS64, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    write_u16(output, 16, ET_EXEC);
    write_u16(output, 18, EM_X86_64);
    write_u32(output, 20, 1);
    write_u64(output, 24, entry);
    write_u64(output, 32, 64);
    write_u64(output, 40, 0);
    write_u32(output, 48, 0);
    write_u16(output, 52, 64);
    write_u16(output, 54, 56);
    write_u16(output, 56, 1);
    write_u16(output, 58, 0);
    write_u16(output, 60, 0);
    write_u16(output, 62, 0);
}

fn write_load_header(output: &mut [u8], file_size: usize) {
    let offset = 64;
    write_u32(output, offset, PT_LOAD);
    write_u32(output, offset + 4, PF_R | PF_X);
    write_u64(output, offset + 8, 0);
    write_u64(output, offset + 16, BASE);
    write_u64(output, offset + 24, BASE);
    write_u64(output, offset + 32, file_size as u64);
    write_u64(output, offset + 40, file_size as u64);
    write_u64(output, offset + 48, PAGE_SIZE as u64);
}

fn write_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) / alignment * alignment
}
