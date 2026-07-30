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
const START_LEN: usize = 17;

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
    start.copy_from_slice(&[
        0x49, 0x89, 0xe4, // preserve the initial process stack in r12
        0xe8, 0, 0, 0, 0, // call main
        0x89, 0xc7, 0xb8, 60, 0, 0, 0, 0x0f, 0x05,
    ]);
    let main_target = START_LEN as i64 + main.offset as i64;
    let main_next = 3_i64 + 5_i64;
    start[4..8].copy_from_slice(&((main_target - main_next) as i32).to_le_bytes());

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
    let runtime_base = BASE + TEXT_OFFSET as u64 + START_LEN as u64 + image.text.len() as u64;
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
            "__geo_bounds_check" => emit_bounds_check_runtime(&mut code),
            "string_len" => emit_string_len_runtime(&mut code),
            "string_utf8_len" => emit_string_utf8_len_runtime(&mut code),
            "string_utf8_codepoint_at" => emit_string_utf8_codepoint_at_runtime(&mut code),
            "string_is_utf8" => emit_string_utf8_valid_runtime(&mut code),
            "string_utf8_is_valid" => emit_string_utf8_valid_runtime(&mut code),
            "string_utf8_byte_offset" => {
                emit_string_utf8_navigation_runtime(&mut code, Utf8NavigationKind::ByteOffset)
            }
            "string_utf8_next_offset" => {
                emit_string_utf8_navigation_runtime(&mut code, Utf8NavigationKind::NextOffset)
            }
            "string_utf8_prev_offset" => {
                emit_string_utf8_navigation_runtime(&mut code, Utf8NavigationKind::PrevOffset)
            }
            "string_utf8_index_at" => {
                emit_string_utf8_navigation_runtime(&mut code, Utf8NavigationKind::IndexAt)
            }
            "string_utf8_is_boundary" => {
                emit_string_utf8_navigation_runtime(&mut code, Utf8NavigationKind::IsBoundary)
            }
            "string_utf8_slice" => {
                let wrapper_offset =
                    ensure_string_utf8_slice_runtime(&mut code, &mut symbols, runtime_base);
                symbols.insert(name.to_string(), runtime_base + wrapper_offset as u64);
                continue;
            }
            "string_utf8_char_at" => {
                let slice_target =
                    ensure_string_utf8_slice_runtime(&mut code, &mut symbols, runtime_base);
                let offset = code.len();
                code.extend_from_slice(&[0x48, 0x89, 0xf2, 0x48, 0xff, 0xc2]);
                emit_internal_call(&mut code, slice_target);
                code.push(0xc3);
                symbols.insert(name.to_string(), runtime_base + offset as u64);
                continue;
            }
            "string_utf8_find_codepoint" => {
                let codepoint_target = if let Some(symbol) = symbols.get("string_utf8_codepoint_at")
                {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_utf8_codepoint_at_runtime(&mut code);
                    symbols.insert(
                        "string_utf8_codepoint_at".to_string(),
                        runtime_base + offset as u64,
                    );
                    offset
                };
                let offset = code.len();
                emit_string_utf8_find_codepoint_runtime(&mut code, codepoint_target);
                symbols.insert(name.to_string(), runtime_base + offset as u64);
                continue;
            }
            "array_new" => emit_array_new_runtime(&mut code),
            "array_clone" => emit_array_clone_runtime(&mut code),
            "array_reserve" => emit_array_reserve_runtime(&mut code),
            "array_len" => emit_array_len_runtime(&mut code),
            "array_capacity" => emit_array_capacity_runtime(&mut code),
            "array_is_empty" => emit_array_is_empty_runtime(&mut code),
            "array_get" => emit_array_get_runtime(&mut code),
            "array_set" => emit_array_set_runtime(&mut code),
            "array_push" => emit_array_push_runtime(&mut code),
            "array_truncate" => emit_array_truncate_runtime(&mut code),
            "array_pop" => emit_array_pop_runtime(&mut code),
            "array_pop_first" => emit_array_pop_first_runtime(&mut code),
            "array_swap" => emit_array_swap_runtime(&mut code),
            "array_swap_remove" => emit_array_swap_remove_runtime(&mut code),
            "array_remove" => emit_array_remove_runtime(&mut code),
            "array_insert" => emit_array_insert_runtime(&mut code),
            "array_extend" => emit_array_extend_runtime(&mut code),
            "array_copy" => emit_array_copy_runtime(&mut code),
            "array_resize" => emit_array_resize_runtime(&mut code),
            "array_first" => emit_array_first_runtime(&mut code),
            "array_last" => emit_array_last_runtime(&mut code),
            "array_fill" => emit_array_fill_runtime(&mut code),
            "array_reverse" => emit_array_reverse_runtime(&mut code),
            "array_index_of" => emit_array_index_of_runtime(&mut code),
            "array_last_index_of" => emit_array_last_index_of_runtime(&mut code),
            "array_contains" => emit_array_contains_runtime(&mut code),
            "array_count" => emit_array_count_runtime(&mut code),
            "array_clear" => emit_array_clear_runtime(&mut code),
            "array_free" => emit_array_free_runtime(&mut code),
            "string_byte_at" => emit_string_byte_at_runtime(&mut code),
            "string_is_empty" => emit_string_is_empty_runtime(&mut code),
            "string_is_ascii" => emit_string_is_ascii_runtime(&mut code),
            "string_find_byte" => emit_string_find_byte_runtime(&mut code),
            "string_last_find_byte" => emit_string_last_find_byte_runtime(&mut code),
            "string_contains" => emit_string_contains_runtime(&mut code),
            "string_starts_with" => emit_string_starts_with_runtime(&mut code),
            "string_ends_with" => emit_string_ends_with_runtime(&mut code),
            "string_index_of" => emit_string_index_of_runtime(&mut code),
            "string_last_index_of" => emit_string_last_index_of_runtime(&mut code),
            "string_count" => emit_string_count_runtime(&mut code),
            "string_compare" => emit_string_compare_runtime(&mut code, StringCompareKind::Compare),
            "string_eq" => emit_string_compare_runtime(&mut code, StringCompareKind::Equal),
            "string_not_eq" => emit_string_compare_runtime(&mut code, StringCompareKind::NotEqual),
            "string_less" => emit_string_compare_runtime(&mut code, StringCompareKind::Less),
            "string_less_or_equal" => {
                emit_string_compare_runtime(&mut code, StringCompareKind::LessOrEqual)
            }
            "string_greater" => emit_string_compare_runtime(&mut code, StringCompareKind::Greater),
            "string_greater_or_equal" => {
                emit_string_compare_runtime(&mut code, StringCompareKind::GreaterOrEqual)
            }
            "print" => emit_print_runtime(&mut code, false, 1),
            "println" => emit_print_runtime(&mut code, true, 1),
            "eprint" => emit_print_runtime(&mut code, false, 2),
            "string_concat" => emit_string_concat_runtime(&mut code),
            "exit_geo" => emit_exit_runtime(&mut code),
            "arg_count" => emit_arg_count_runtime(&mut code),
            "arg" => emit_arg_runtime(&mut code),
            "arg_exists" => emit_arg_exists_runtime(&mut code),
            "arg_or" => emit_arg_or_runtime(&mut code),
            "process_id" => emit_process_id_runtime(&mut code),
            "platform_path_separator" => emit_platform_path_separator_runtime(&mut code),
            "platform_os" => emit_owned_constant_runtime(&mut code, b"linux\0"),
            "platform_arch" => emit_owned_constant_runtime(&mut code, b"x86_64\0"),
            "platform_newline" => emit_owned_constant_runtime(&mut code, b"\n\0"),
            "path_is_absolute" => emit_path_is_absolute_runtime(&mut code),
            "path_file_name" => {
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let path_offset = code.len();
                emit_path_file_name_runtime(&mut code, slice_offset);
                symbols.insert(
                    "path_file_name".to_string(),
                    runtime_base + path_offset as u64,
                );
                continue;
            }
            "path_parent" => {
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let parent_offset = code.len();
                emit_path_parent_runtime(&mut code, slice_offset);
                symbols.insert(
                    "path_parent".to_string(),
                    runtime_base + parent_offset as u64,
                );
                continue;
            }
            "path_extension" => {
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let extension_offset = code.len();
                emit_path_extension_runtime(&mut code, slice_offset);
                symbols.insert(
                    "path_extension".to_string(),
                    runtime_base + extension_offset as u64,
                );
                continue;
            }
            "path_stem" => {
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let stem_offset = code.len();
                emit_path_stem_runtime(&mut code, slice_offset);
                symbols.insert("path_stem".to_string(), runtime_base + stem_offset as u64);
                continue;
            }
            "path_without_extension" => {
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let without_extension_offset = code.len();
                emit_path_without_extension_runtime(&mut code, slice_offset);
                symbols.insert(
                    "path_without_extension".to_string(),
                    runtime_base + without_extension_offset as u64,
                );
                continue;
            }
            "path_with_extension" => {
                let without_extension_offset = if let Some(symbol) =
                    symbols.get("path_without_extension")
                {
                    (*symbol - runtime_base) as usize
                } else {
                    let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                        (*symbol - runtime_base) as usize
                    } else {
                        let offset = code.len();
                        emit_string_slice_runtime(&mut code);
                        symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                        offset
                    };
                    let offset = code.len();
                    emit_path_without_extension_runtime(&mut code, slice_offset);
                    symbols.insert(
                        "path_without_extension".to_string(),
                        runtime_base + offset as u64,
                    );
                    offset
                };
                let length_offset = if let Some(symbol) = symbols.get("string_len") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_len_runtime(&mut code);
                    symbols.insert("string_len".to_string(), runtime_base + offset as u64);
                    offset
                };
                let from_byte_offset = if let Some(symbol) = symbols.get("string_from_byte") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_from_byte_runtime(&mut code);
                    symbols.insert("string_from_byte".to_string(), runtime_base + offset as u64);
                    offset
                };
                let slice_offset = if let Some(symbol) = symbols.get("string_slice") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_slice_runtime(&mut code);
                    symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
                    offset
                };
                let concat_offset = if let Some(symbol) = symbols.get("string_concat") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_concat_runtime(&mut code);
                    symbols.insert("string_concat".to_string(), runtime_base + offset as u64);
                    offset
                };
                let free_offset = if let Some(symbol) = symbols.get("string_free") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_free_runtime(&mut code);
                    symbols.insert("string_free".to_string(), runtime_base + offset as u64);
                    offset
                };
                let with_extension_offset = code.len();
                emit_path_with_extension_runtime(
                    &mut code,
                    without_extension_offset,
                    length_offset,
                    from_byte_offset,
                    slice_offset,
                    concat_offset,
                    free_offset,
                );
                symbols.insert(
                    "path_with_extension".to_string(),
                    runtime_base + with_extension_offset as u64,
                );
                continue;
            }
            "alloc" | "alloc_zeroed" => emit_alloc_runtime(&mut code, false),
            "alloc_array" => emit_alloc_runtime(&mut code, true),
            "free_geo" => emit_free_runtime(&mut code),
            "string_free" => emit_free_runtime(&mut code),
            "realloc_geo" => emit_realloc_runtime(&mut code),
            "alloc_copy" => emit_alloc_copy_runtime(&mut code),
            "mem_copy" => emit_mem_copy_runtime(&mut code),
            "mem_zero" => emit_mem_zero_runtime(&mut code),
            "mem_move" => emit_mem_move_runtime(&mut code),
            "mem_fill" => emit_mem_fill_runtime(&mut code),
            "mem_find" => emit_mem_find_runtime(&mut code),
            "mem_compare" => emit_mem_compare_runtime(&mut code),
            "mem_equal" => emit_mem_equal_runtime(&mut code),
            "mem_is_zero" => emit_mem_is_zero_runtime(&mut code),
            "mem_reverse" => emit_mem_reverse_runtime(&mut code),
            "string_from_byte" => emit_string_from_byte_runtime(&mut code),
            "string_from_utf8_codepoint" => emit_string_from_utf8_codepoint_runtime(&mut code),
            "int_to_string" | "usize_to_string" => emit_integer_to_string_runtime(&mut code),
            "bool_to_string" => emit_bool_to_string_runtime(&mut code),
            "string_clone" => emit_string_clone_runtime(&mut code),
            "string_slice" => emit_string_slice_runtime(&mut code),
            "write_file" => emit_write_file_runtime(&mut code),
            "append_file" => emit_append_file_runtime(&mut code),
            "copy_file" => emit_copy_file_runtime(&mut code),
            "touch_file" => emit_touch_file_runtime(&mut code),
            "truncate_file" => emit_truncate_file_runtime(&mut code),
            "remove_file" => emit_remove_file_runtime(&mut code),
            "rename_file" => emit_rename_file_runtime(&mut code),
            "create_dir" => emit_create_dir_runtime(&mut code),
            "create_dir_all" => emit_create_dir_all_runtime(&mut code),
            "remove_dir" => emit_remove_dir_runtime(&mut code),
            "file_open" => emit_file_open_runtime(&mut code, 0),
            "file_open_write" => emit_file_open_runtime(&mut code, 0x241),
            "file_open_append" => emit_file_open_runtime(&mut code, 0x441),
            "file_write" => emit_file_write_runtime(&mut code),
            "file_flush" => emit_file_flush_runtime(&mut code),
            "file_close" => emit_file_close_runtime(&mut code),
            "file_seek" => emit_file_seek_runtime(&mut code),
            "file_read" | "file_read_to_string" => emit_file_read_to_string_runtime(&mut code),
            "file_exists" => emit_file_exists_runtime(&mut code),
            "file_is_file" => emit_file_stat_runtime(&mut code, FileStatKind::File),
            "file_is_dir" => emit_file_stat_runtime(&mut code, FileStatKind::Directory),
            "file_is_empty" => emit_file_stat_runtime(&mut code, FileStatKind::Empty),
            "file_size" => emit_file_stat_runtime(&mut code, FileStatKind::Size),
            "file_accessed_time" => emit_file_stat_runtime(&mut code, FileStatKind::AccessedTime),
            "file_modified_time" => emit_file_stat_runtime(&mut code, FileStatKind::ModifiedTime),
            "file_created_time" => emit_file_stat_runtime(&mut code, FileStatKind::CreatedTime),
            "dir_entry_count" => emit_dir_entry_count_runtime(&mut code),
            "dir_entry_name" => emit_dir_entry_name_runtime(&mut code),
            "dir_entry_path" => {
                let name_offset = if let Some(symbol) = symbols.get("dir_entry_name") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_dir_entry_name_runtime(&mut code);
                    symbols.insert("dir_entry_name".to_string(), runtime_base + offset as u64);
                    offset
                };
                let concat_offset = if let Some(symbol) = symbols.get("string_concat") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_string_concat_runtime(&mut code);
                    symbols.insert("string_concat".to_string(), runtime_base + offset as u64);
                    offset
                };
                let free_offset = if let Some(symbol) = symbols.get("string_free") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_free_runtime(&mut code);
                    symbols.insert("string_free".to_string(), runtime_base + offset as u64);
                    offset
                };
                let path_offset = code.len();
                emit_dir_entry_path_runtime(&mut code, name_offset, concat_offset, free_offset);
                symbols.insert(
                    "dir_entry_path".to_string(),
                    runtime_base + path_offset as u64,
                );
                continue;
            }
            "read_file" => emit_read_file_runtime(&mut code),
            "read_line" => emit_read_line_runtime(&mut code),
            "read_file_or" => {
                let read_file_offset = if let Some(symbol) = symbols.get("read_file") {
                    (*symbol - runtime_base) as usize
                } else {
                    let offset = code.len();
                    emit_read_file_runtime(&mut code);
                    symbols.insert("read_file".to_string(), runtime_base + offset as u64);
                    offset
                };
                let wrapper_offset = code.len();
                emit_read_file_or_runtime(&mut code, read_file_offset);
                symbols.insert(name.to_string(), runtime_base + wrapper_offset as u64);
                continue;
            }
            _ => continue,
        }
        symbols.insert(name.to_string(), runtime_base + offset as u64);
    }
    RuntimeText { code, symbols }
}

fn ensure_string_utf8_slice_runtime(
    code: &mut Vec<u8>,
    symbols: &mut HashMap<String, u64>,
    runtime_base: u64,
) -> usize {
    if let Some(symbol) = symbols.get("string_utf8_slice") {
        return (*symbol - runtime_base) as usize;
    }
    let byte_offset_offset = if let Some(symbol) = symbols.get("string_utf8_byte_offset") {
        (*symbol - runtime_base) as usize
    } else {
        let offset = code.len();
        emit_string_utf8_navigation_runtime(code, Utf8NavigationKind::ByteOffset);
        symbols.insert(
            "string_utf8_byte_offset".to_string(),
            runtime_base + offset as u64,
        );
        offset
    };
    let string_len_offset = if let Some(symbol) = symbols.get("string_len") {
        (*symbol - runtime_base) as usize
    } else {
        let offset = code.len();
        emit_string_len_runtime(code);
        symbols.insert("string_len".to_string(), runtime_base + offset as u64);
        offset
    };
    let string_slice_offset = if let Some(symbol) = symbols.get("string_slice") {
        (*symbol - runtime_base) as usize
    } else {
        let offset = code.len();
        emit_string_slice_runtime(code);
        symbols.insert("string_slice".to_string(), runtime_base + offset as u64);
        offset
    };
    let wrapper_offset = code.len();
    emit_string_utf8_slice_runtime(
        code,
        byte_offset_offset,
        string_len_offset,
        string_slice_offset,
    );
    symbols.insert(
        "string_utf8_slice".to_string(),
        runtime_base + wrapper_offset as u64,
    );
    wrapper_offset
}

#[derive(Clone, Copy)]
enum FileStatKind {
    File,
    Directory,
    Empty,
    Size,
    AccessedTime,
    ModifiedTime,
    CreatedTime,
}

fn emit_file_stat_runtime(code: &mut Vec<u8>, kind: FileStatKind) {
    code.extend_from_slice(&[0x48, 0x81, 0xec, 0xa0, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0xbf, 0x9c, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x8d, 0x14, 0x24]);
    code.extend_from_slice(&[0x45, 0x31, 0xd2]);
    code.extend_from_slice(&[0xb8, 0x06, 0x01, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);

    match kind {
        FileStatKind::File | FileStatKind::Directory => {
            code.extend_from_slice(&[0x8b, 0x44, 0x24, 0x18]);
            code.extend_from_slice(&[0x25, 0x00, 0xf0, 0x00, 0x00]);
            code.extend_from_slice(&[0x3d]);
            let mode = if matches!(kind, FileStatKind::File) {
                0x8000_u32
            } else {
                0x4000_u32
            };
            code.extend_from_slice(&mode.to_le_bytes());
            code.extend_from_slice(&[0x0f, 0x94, 0xc0, 0x0f, 0xb6, 0xc0]);
        }
        FileStatKind::Empty => {
            code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x30, 0x00]);
            code.extend_from_slice(&[0x0f, 0x94, 0xc0, 0x0f, 0xb6, 0xc0]);
        }
        FileStatKind::Size => {
            code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x30]);
        }
        FileStatKind::AccessedTime => {
            // Linux struct stat: st_atime seconds at offset 0x48.
            code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48]);
        }
        FileStatKind::ModifiedTime => {
            // Linux struct stat: st_mtime seconds at offset 0x58.
            code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x58]);
        }
        FileStatKind::CreatedTime => {
            // Linux has no portable birth-time field in the legacy stat ABI;
            // expose the inode-change timestamp consistently for v1.
            code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x68]);
        }
    }
    code.extend_from_slice(&[0x48, 0x81, 0xc4, 0xa0, 0x00, 0x00, 0x00, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x81, 0xc4, 0xa0, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_dir_entry_count_runtime(code: &mut Vec<u8>) {
    // Count Linux getdents64 records, excluding the implicit . and .. entries.
    code.extend_from_slice(&[0x48, 0x81, 0xec, 0x40, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc6, 0x00, 0x00, 0x01, 0x00]);
    code.extend_from_slice(&[0x31, 0xd2, 0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x08, 0x48, 0x85, 0xc0]);
    let open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x10]);

    let read_loop = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8d, 0x74, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x00, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0xd9, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let read_empty = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8d, 0x44, 0x24, 0x40, 0x48, 0x89, 0x44, 0x24, 0x20]);

    let entry_loop = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x18, 0x00]);
    let next_batch = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x0f, 0xb7, 0x48, 0x10, 0x85, 0xc9]);
    let malformed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x3b, 0x4c, 0x24, 0x18]);
    let oversized = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[0x48, 0x01, 0x4c, 0x24, 0x20, 0x48, 0x29, 0x4c, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0xff, 0x44, 0x24, 0x10]);
    let next_entry = emit_near_unconditional_placeholder(code);

    let finish = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x08, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x10, 0x02]);
    let no_dot_entries = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x48, 0x83, 0x6c, 0x24, 0x10, 0x02]);
    let return_count = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x44, 0x24, 0x10, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3,
    ]);

    let failure = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x08, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3]);

    patch_near_jump(code, open_failed, failure);
    patch_near_jump(code, read_failed, failure);
    patch_near_jump(code, read_empty, finish);
    patch_near_jump(code, next_batch, read_loop);
    patch_near_jump(code, malformed, failure);
    patch_near_jump(code, oversized, failure);
    patch_near_jump(code, next_entry, entry_loop);
    patch_short_jump(code, no_dot_entries, return_count);
}

fn emit_dir_entry_name_runtime(code: &mut Vec<u8>) {
    // Enumerate getdents64 records, skip . and .., and return an owned Geo string.
    code.extend_from_slice(&[0x48, 0x81, 0xec, 0x40, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc6, 0x00, 0x00, 0x01, 0x00]);
    code.extend_from_slice(&[0x31, 0xd2, 0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x10, 0x48, 0x85, 0xc0]);
    let open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x18]);

    let read_loop = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8d, 0x74, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x00, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0xd9, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let read_empty = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8d, 0x44, 0x24, 0x40, 0x48, 0x89, 0x44, 0x24, 0x28]);

    let entry_loop = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x20, 0x00]);
    let next_batch = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x44, 0x24, 0x28, 0x0f, 0xb7, 0x48, 0x10, 0x85, 0xc9,
    ]);
    let malformed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x3b, 0x4c, 0x24, 0x20]);
    let oversized = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[0x48, 0x8d, 0x50, 0x13]);
    code.extend_from_slice(&[0x80, 0x3a, 0x2e]);
    let not_dot = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x80, 0x7a, 0x01, 0x00]);
    let skip_entry_dot = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x80, 0x7a, 0x01, 0x2e]);
    let candidate = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x80, 0x7a, 0x02, 0x00]);
    let skip_entry_dot_dot = emit_near_jump_placeholder(code, 0x0f, 0x84);

    let candidate_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x18, 0x48, 0x3b, 0x44, 0x24, 0x08]);
    let found = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0xff, 0x44, 0x24, 0x18]);
    let advance_target = code.len();
    code.extend_from_slice(&[0x48, 0x01, 0x4c, 0x24, 0x28, 0x48, 0x29, 0x4c, 0x24, 0x20]);
    let advance = emit_near_unconditional_placeholder(code);

    let found_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x30, 0x31, 0xc0]);
    let name_loop = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x02, 0x00]);
    let name_done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, name_loop);
    let name_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x38, 0x48, 0x83, 0xc0, 0x09]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20, 0x31, 0xff]);
    code.extend_from_slice(&[0x48, 0x89, 0xc6, 0x48, 0xc7, 0xc2, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x20, 0x48, 0x89, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x28, 0x48, 0x83, 0xc7, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x30, 0x48, 0x8b, 0x4c, 0x24, 0x38]);
    code.extend_from_slice(&[0xf3, 0xa4, 0xc6, 0x04, 0x0f, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x28, 0x48, 0x83, 0xc0, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3]);

    patch_near_jump(code, open_failed, failure);
    patch_near_jump(code, read_failed, failure);
    patch_near_jump(code, read_empty, failure);
    patch_near_jump(code, next_batch, read_loop);
    patch_near_jump(code, malformed, failure);
    patch_near_jump(code, oversized, failure);
    patch_near_jump(code, not_dot, candidate_target);
    patch_near_jump(code, skip_entry_dot, advance_target);
    patch_near_jump(code, candidate, candidate_target);
    patch_near_jump(code, skip_entry_dot_dot, advance_target);
    patch_near_jump(code, found, found_target);
    patch_near_jump(code, advance, entry_loop);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, name_done, name_target);
}

fn emit_dir_entry_path_runtime(
    code: &mut Vec<u8>,
    name_offset: usize,
    concat_offset: usize,
    free_offset: usize,
) {
    // Build path + "/" + name while releasing the intermediate strings.
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x58]);
    code.extend_from_slice(&[0x48, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x89, 0x74, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x20, 0x48, 0x8b, 0x74, 0x24, 0x28]);
    emit_internal_call(code, name_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[
        0xc6, 0x44, 0x24, 0x38, 0x2f, 0xc6, 0x44, 0x24, 0x39, 0x00, 0x48, 0x8b, 0x7c, 0x24, 0x20,
        0x48, 0x8d, 0x74, 0x24, 0x38,
    ]);
    emit_internal_call(code, concat_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x40]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x40, 0x48, 0x8b, 0x74, 0x24, 0x30]);
    emit_internal_call(code, concat_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x30]);
    emit_internal_call(code, free_offset);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x40]);
    emit_internal_call(code, free_offset);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x48, 0x48, 0x83, 0xc4, 0x58, 0xc3]);
}

fn emit_string_byte_at_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x85, 0xff]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3c, 0x37, 0x00]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x0f, 0xb6, 0x04, 0x37, 0xc3]);
    let end_target = code.len();
    code.push(0xc3);
    patch_short_jump(code, null_value, end_target);
    patch_short_jump(code, end, end_target);
}

fn emit_string_is_empty_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3f, 0x00]);
    let non_empty = emit_short_jump_placeholder(code, 0x75);
    let empty_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let non_empty_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_value, empty_target);
    patch_short_jump(code, non_empty, non_empty_target);
}

fn emit_string_is_ascii_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x80, 0x3f, 0x00]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3f, 0x7f]);
    let invalid = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0xff, 0xc7]);
    emit_short_jump_back(code, loop_start);
    let done_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let invalid_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_value, done_target);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, invalid, invalid_target);
}

fn emit_string_find_byte_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc9, 0x48, 0x85, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x41, 0x89, 0xf0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x44, 0x38, 0x04, 0x0f]);
    let found = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let missing_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    let found_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xc8, 0xc3]);
    patch_short_jump(code, missing, missing_target);
    patch_short_jump(code, end, missing_target);
    patch_short_jump(code, found, found_target);
}

fn emit_string_last_find_byte_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x83, 0xfe, 0x00]);
    let below_zero = emit_short_jump_placeholder(code, 0x7c);
    code.extend_from_slice(&[0x81, 0xfe, 0xff, 0x00, 0x00, 0x00]);
    let above_byte = emit_short_jump_placeholder(code, 0x7f);
    code.extend_from_slice(&[0x41, 0x89, 0xf0]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc2, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let end = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x44, 0x38, 0x04, 0x0f]);
    let no_match = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0x89, 0xca]);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let end_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xd0, 0xc3]);
    patch_short_jump(code, below_zero, end_target);
    patch_short_jump(code, above_byte, end_target);
    patch_short_jump(code, end, end_target);
    patch_short_jump(code, no_match, advance);
}

fn emit_string_contains_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3e, 0x00]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let outer = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let no_match = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8d, 0x0c, 0x0f, 0x4d, 0x31, 0xc0]);
    let inner = code.len();
    code.extend_from_slice(&[0x42, 0x8a, 0x04, 0x06]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x43, 0x38, 0x04, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, inner);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let true_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_needle, true_target);
    patch_short_jump(code, empty_needle, true_target);
    patch_short_jump(code, missing, false_target);
    patch_short_jump(code, no_match, false_target);
    patch_short_jump(code, matched, true_target);
    patch_short_jump(code, mismatch, advance);
}

fn emit_string_starts_with_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_prefix = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x04, 0x0e]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x44, 0x8a, 0x04, 0x0f]);
    code.extend_from_slice(&[0x44, 0x38, 0xc0]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let true_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_prefix, true_target);
    patch_short_jump(code, missing, false_target);
    patch_short_jump(code, matched, true_target);
    patch_short_jump(code, mismatch, false_target);
}

fn emit_string_ends_with_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let source_missing = emit_short_jump_placeholder(code, 0x74);
    let source_length_loop = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let source_length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, source_length_loop);
    let source_length_target = code.len();

    code.extend_from_slice(&[0x45, 0x31, 0xc0, 0x48, 0x85, 0xf6]);
    let suffix_missing = emit_short_jump_placeholder(code, 0x74);
    let suffix_length_loop = code.len();
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x06, 0x00]);
    let suffix_length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, suffix_length_loop);
    let suffix_length_target = code.len();
    code.extend_from_slice(&[0x4c, 0x39, 0xc1]);
    let too_long = emit_short_jump_placeholder(code, 0x72);
    code.extend_from_slice(&[0x4c, 0x29, 0xc1, 0x4d, 0x31, 0xc9]);
    let compare_loop = code.len();
    code.extend_from_slice(&[0x4c, 0x8d, 0x14, 0x0f]);
    code.extend_from_slice(&[0x4a, 0x8a, 0x04, 0x0e]);
    code.extend_from_slice(&[0x4f, 0x8a, 0x1c, 0x0a]);
    code.extend_from_slice(&[0x44, 0x38, 0xd8]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    code.extend_from_slice(&[0x49, 0xff, 0xc8]);
    let compare_next = emit_short_jump_placeholder(code, 0x75);
    let true_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, source_missing, false_target);
    patch_short_jump(code, source_length_done, source_length_target);
    patch_short_jump(code, suffix_missing, true_target);
    patch_short_jump(code, suffix_length_done, suffix_length_target);
    patch_short_jump(code, too_long, false_target);
    patch_short_jump(code, mismatch, false_target);
    patch_short_jump(code, compare_next, compare_loop);
}

fn emit_string_index_of_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3e, 0x00]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let missing = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let outer = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let no_match = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8d, 0x0c, 0x0f, 0x4d, 0x31, 0xc0]);
    let inner = code.len();
    code.extend_from_slice(&[0x42, 0x8a, 0x04, 0x06]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x43, 0x38, 0x04, 0x01]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, inner);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let found_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xc8, 0xc3]);
    let missing_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    patch_short_jump(code, null_needle, found_target);
    patch_short_jump(code, empty_needle, found_target);
    patch_short_jump(code, missing, missing_target);
    patch_short_jump(code, no_match, missing_target);
    patch_short_jump(code, matched, found_target);
    patch_short_jump(code, mismatch, advance);
}

fn emit_string_last_index_of_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc2, 0xff, 0xff, 0xff, 0xff]);
    let null_needle = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3e, 0x00]);
    let empty_needle = emit_short_jump_placeholder(code, 0x74);
    let outer = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let finished = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8d, 0x1c, 0x0f, 0x4d, 0x31, 0xc9]);
    let inner = code.len();
    code.extend_from_slice(&[0x4a, 0x8a, 0x04, 0x0e]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4b, 0x80, 0x3c, 0x0b, 0x00]);
    let advance = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4b, 0x38, 0x04, 0x0b]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_short_jump_back(code, inner);
    let match_target = code.len();
    code.extend_from_slice(&[0x49, 0x89, 0xca, 0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let advance_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let finished_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xd0, 0xc3]);
    patch_short_jump(code, null_needle, finished_target);
    patch_short_jump(code, empty_needle, finished_target);
    patch_short_jump(code, finished, finished_target);
    patch_short_jump(code, matched, match_target);
    patch_short_jump(code, advance, advance_target);
    patch_short_jump(code, mismatch, advance_target);
}

fn emit_string_count_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x45, 0x31, 0xc0, 0x48, 0x85, 0xf6]);
    let no_needle = emit_short_jump_placeholder(code, 0x74);
    let needle_loop = code.len();
    code.extend_from_slice(&[0x42, 0x80, 0x3c, 0x06, 0x00]);
    let needle_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    emit_short_jump_back(code, needle_loop);
    code.extend_from_slice(&[0x45, 0x31, 0xd2, 0x48, 0x31, 0xc9]);
    let outer = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x0f, 0x00]);
    let finished = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8d, 0x1c, 0x0f, 0x4d, 0x31, 0xc9]);
    let inner = code.len();
    code.extend_from_slice(&[0x4a, 0x8a, 0x04, 0x0e]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let matched = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4b, 0x38, 0x04, 0x0b]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_short_jump_back(code, inner);
    let match_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc2]);
    code.extend_from_slice(&[0x4c, 0x01, 0xc1]);
    emit_short_jump_back(code, outer);
    let mismatch_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, outer);
    let finished_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xd0, 0xc3]);
    let empty_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, no_needle, empty_target);
    patch_short_jump(code, needle_done, empty_target);
    patch_short_jump(code, finished, finished_target);
    patch_short_jump(code, matched, match_target);
    patch_short_jump(code, mismatch, mismatch_target);
}

#[derive(Clone, Copy)]
enum StringCompareKind {
    Compare,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

fn emit_string_compare_runtime(code: &mut Vec<u8>, kind: StringCompareKind) {
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x0f, 0xb6, 0x04, 0x0f]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x14, 0x0e]);
    code.extend_from_slice(&[0x39, 0xd0]);
    let less = emit_short_jump_placeholder(code, 0x72);
    let greater = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x85, 0xc0]);
    let equal = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);

    let less_target = code.len();
    emit_compare_result(code, kind, CompareOutcome::Less);
    let greater_target = code.len();
    emit_compare_result(code, kind, CompareOutcome::Greater);
    let equal_target = code.len();
    emit_compare_result(code, kind, CompareOutcome::Equal);
    patch_short_jump(code, less, less_target);
    patch_short_jump(code, greater, greater_target);
    patch_short_jump(code, equal, equal_target);
}

#[derive(Clone, Copy)]
enum CompareOutcome {
    Less,
    Equal,
    Greater,
}

fn emit_compare_result(code: &mut Vec<u8>, kind: StringCompareKind, outcome: CompareOutcome) {
    let value = match kind {
        StringCompareKind::Compare => match outcome {
            CompareOutcome::Less => -1,
            CompareOutcome::Equal => 0,
            CompareOutcome::Greater => 1,
        },
        StringCompareKind::Equal => i32::from(matches!(outcome, CompareOutcome::Equal)),
        StringCompareKind::NotEqual => i32::from(!matches!(outcome, CompareOutcome::Equal)),
        StringCompareKind::Less => i32::from(matches!(outcome, CompareOutcome::Less)),
        StringCompareKind::LessOrEqual => i32::from(matches!(
            outcome,
            CompareOutcome::Less | CompareOutcome::Equal
        )),
        StringCompareKind::Greater => i32::from(matches!(outcome, CompareOutcome::Greater)),
        StringCompareKind::GreaterOrEqual => i32::from(matches!(
            outcome,
            CompareOutcome::Greater | CompareOutcome::Equal
        )),
    };
    code.extend_from_slice(&[0xb8]);
    code.extend_from_slice(&value.to_le_bytes());
    code.push(0xc3);
}

fn emit_file_exists_runtime(code: &mut Vec<u8>) {
    // access(path, F_OK) returns zero when the path exists.
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let non_null = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let call = code.len();
    code.extend_from_slice(&[0x31, 0xf6, 0xb8, 21, 0, 0, 0, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let exists = emit_short_jump_placeholder(code, 0x79);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let exists_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    patch_short_jump(code, non_null, call);
    patch_short_jump(code, exists, exists_target);
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
    code.extend_from_slice(&[0x48, 0x85, 0xff, 0x74, 0x15]);
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x0e]);
    code.extend_from_slice(&[0x85, 0xc9]);
    code.extend_from_slice(&[0x74, 0x08]);
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0xeb, 0xf1, 0xc3]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_string_utf8_len_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x89, 0xfe, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x16, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xe2, 0xc0, 0x80, 0xfa, 0x80]);
    let continuation = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    let next = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    emit_short_jump_back(code, loop_start);
    let done_target = code.len();
    code.push(0xc3);
    let null_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null_value, null_target);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, continuation, next);
}

fn emit_string_utf8_valid_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xf8]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x08, 0x45, 0x84, 0xc9]);
    let done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x80]);
    let ascii = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xc2]);
    let invalid_lead = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let two = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf0]);
    let three = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf5]);
    let four_invalid = emit_near_jump_placeholder(code, 0x0f, 0x87);

    code.extend_from_slice(&[
        0x45, 0x8a, 0x50, 0x01, 0x45, 0x8a, 0x58, 0x02, 0x41, 0x8a, 0x4c, 0x20, 0x03,
    ]);
    let four_second_checks = emit_utf8_continuation_check(code, 0x41, 0xfa);
    let four_third_checks = emit_utf8_continuation_check(code, 0x41, 0xfb);
    let four_fourth_checks = emit_utf8_continuation_check(code, 0x00, 0xf9);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x90]);
    let four_second_low = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf4]);
    let four_not_f4 = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x8f]);
    let four_second_high = emit_near_jump_placeholder(code, 0x0f, 0x87);
    let four_advance = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x04]);
    let four_loop = emit_near_unconditional_placeholder(code);

    let three_target = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x50, 0x01, 0x45, 0x8a, 0x58, 0x02]);
    let three_second_checks = emit_utf8_continuation_check(code, 0x41, 0xfa);
    let three_third_checks = emit_utf8_continuation_check(code, 0x41, 0xfb);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let three_not_e0 = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xa0]);
    let three_e0_low = emit_near_jump_placeholder(code, 0x0f, 0x82);
    let three_e0_done = emit_near_unconditional_placeholder(code);
    let three_ed_check = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xed]);
    let three_not_ed = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x9f]);
    let three_ed_high = emit_near_jump_placeholder(code, 0x0f, 0x87);
    let three_advance = code.len();
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x03]);
    let three_loop = emit_near_unconditional_placeholder(code);

    let two_target = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x50, 0x01]);
    let two_checks = emit_utf8_continuation_check(code, 0x41, 0xfa);
    code.extend_from_slice(&[0x49, 0x83, 0xc0, 0x02]);
    let two_loop = emit_near_unconditional_placeholder(code);

    let ascii_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    let ascii_loop = emit_near_unconditional_placeholder(code);

    let success = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);

    patch_near_jump(code, null_value, success);
    patch_near_jump(code, done, success);
    patch_near_jump(code, ascii, ascii_target);
    patch_near_jump(code, invalid_lead, failure);
    patch_near_jump(code, two, two_target);
    patch_near_jump(code, three, three_target);
    patch_near_jump(code, four_invalid, failure);
    patch_near_jump(code, four_second_low, failure);
    patch_near_jump(code, four_not_f4, four_advance);
    patch_near_jump(code, four_second_high, failure);
    patch_near_jump(code, three_not_e0, three_ed_check);
    patch_near_jump(code, three_e0_low, failure);
    patch_near_jump(code, three_e0_done, three_advance);
    patch_near_jump(code, three_not_ed, three_advance);
    patch_near_jump(code, three_ed_high, failure);
    patch_near_jump(code, four_loop, loop_start);
    patch_near_jump(code, three_loop, loop_start);
    patch_near_jump(code, two_loop, loop_start);
    patch_near_jump(code, ascii_loop, loop_start);
    for displacement in four_second_checks
        .into_iter()
        .chain(four_third_checks)
        .chain(four_fourth_checks)
        .chain(three_second_checks)
        .chain(three_third_checks)
        .chain(two_checks)
    {
        patch_near_jump(code, displacement, failure);
    }
}

fn emit_utf8_continuation_check(code: &mut Vec<u8>, rex: u8, modrm: u8) -> [usize; 2] {
    if rex != 0 {
        code.extend_from_slice(&[rex, 0x80, modrm, 0x80]);
    } else {
        code.extend_from_slice(&[0x80, modrm, 0x80]);
    }
    let below = emit_near_jump_placeholder(code, 0x0f, 0x82);
    if rex != 0 {
        code.extend_from_slice(&[rex, 0x80, modrm, 0xbf]);
    } else {
        code.extend_from_slice(&[0x80, modrm, 0xbf]);
    }
    let above = emit_near_jump_placeholder(code, 0x0f, 0x87);
    [below, above]
}

fn emit_near_unconditional_placeholder(code: &mut Vec<u8>) -> usize {
    code.push(0xe9);
    let displacement = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    displacement
}

fn emit_near_jump_back(code: &mut Vec<u8>, target: usize) {
    code.push(0xe9);
    let displacement = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    patch_near_jump(code, displacement, target);
}

fn emit_internal_call(code: &mut Vec<u8>, target: usize) {
    code.push(0xe8);
    let displacement = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    patch_near_jump(code, displacement, target);
}

#[derive(Clone, Copy)]
enum Utf8NavigationKind {
    ByteOffset,
    NextOffset,
    PrevOffset,
    IndexAt,
    IsBoundary,
}

fn emit_string_utf8_navigation_runtime(code: &mut Vec<u8>, kind: Utf8NavigationKind) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x41, 0x54]);
    code.extend_from_slice(&[
        0x49, 0x89, 0xf8, 0x31, 0xc0, 0x45, 0x31, 0xd2, 0x45, 0x31, 0xe4,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x08, 0x45, 0x84, 0xc9]);
    let end = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0x80]);
    let width_one = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xe0]);
    let width_two = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf0]);
    let width_three = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xf9, 0xf5]);
    let invalid = emit_near_jump_placeholder(code, 0x0f, 0x83);

    code.extend_from_slice(&[0x41, 0xbb, 0x04, 0x00, 0x00, 0x00]);
    let width_four_loop = emit_near_unconditional_placeholder(code);
    let width_three_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x03, 0x00, 0x00, 0x00]);
    let width_three_loop = emit_near_unconditional_placeholder(code);
    let width_two_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x02, 0x00, 0x00, 0x00]);
    let width_two_loop = emit_near_unconditional_placeholder(code);
    let width_one_target = code.len();
    code.extend_from_slice(&[0x41, 0xbb, 0x01, 0x00, 0x00, 0x00]);
    let width_one_loop = emit_near_unconditional_placeholder(code);
    let width_ready = code.len();

    let (boundary, inside) = if matches!(kind, Utf8NavigationKind::ByteOffset) {
        code.extend_from_slice(&[0x4c, 0x39, 0xe6]);
        (Some(emit_near_jump_placeholder(code, 0x0f, 0x84)), None)
    } else {
        code.extend_from_slice(&[0x48, 0x39, 0xf0]);
        let boundary = emit_near_jump_placeholder(code, 0x0f, 0x84);
        code.extend_from_slice(&[0x48, 0x89, 0xc2, 0x4c, 0x01, 0xda, 0x48, 0x39, 0xd6]);
        (
            Some(boundary),
            Some(emit_near_jump_placeholder(code, 0x0f, 0x82)),
        )
    };
    code.extend_from_slice(&[
        0x49, 0x89, 0xc2, 0x4c, 0x01, 0xd8, 0x4d, 0x01, 0xd8, 0x49, 0xff, 0xc4,
    ]);
    let advance = emit_near_unconditional_placeholder(code);

    let boundary_target = code.len();
    emit_navigation_result(code, kind, true);
    let end_target = code.len();
    if matches!(kind, Utf8NavigationKind::ByteOffset) {
        code.extend_from_slice(&[0x4c, 0x39, 0xe6]);
    } else {
        code.extend_from_slice(&[0x48, 0x39, 0xf0]);
    }
    let end_match = emit_near_jump_placeholder(code, 0x0f, 0x84);
    let failure = code.len();
    emit_navigation_failure(code, kind);
    let success = code.len();
    emit_navigation_result(code, kind, false);
    patch_near_jump(code, null_value, failure);
    patch_near_jump(code, end, end_target);
    patch_near_jump(code, end_match, success);
    patch_near_jump(code, invalid, failure);
    patch_near_jump(code, advance, loop_start);
    patch_near_jump(code, width_one, width_one_target);
    patch_near_jump(code, width_two, width_two_target);
    patch_near_jump(code, width_three, width_three_target);
    patch_near_jump(code, width_four_loop, width_ready);
    patch_near_jump(code, width_three_loop, width_ready);
    patch_near_jump(code, width_two_loop, width_ready);
    patch_near_jump(code, width_one_loop, width_ready);
    if let Some(boundary) = boundary {
        patch_near_jump(code, boundary, boundary_target);
    }
    if let Some(inside) = inside {
        patch_near_jump(code, inside, failure);
    }
}

fn emit_navigation_result(code: &mut Vec<u8>, kind: Utf8NavigationKind, at_boundary: bool) {
    match kind {
        Utf8NavigationKind::ByteOffset => {
            code.extend_from_slice(&[0x48, 0x89, 0xc0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKind::NextOffset if at_boundary => {
            code.extend_from_slice(&[0x4c, 0x01, 0xd8, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKind::NextOffset => {
            code.extend_from_slice(&[0x48, 0x89, 0xc0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKind::PrevOffset => {
            code.extend_from_slice(&[0x4c, 0x89, 0xd0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKind::IndexAt => {
            code.extend_from_slice(&[0x4c, 0x89, 0xe0, 0x41, 0x5c, 0xc3])
        }
        Utf8NavigationKind::IsBoundary => {
            code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x41, 0x5c, 0xc3])
        }
    }
}

fn emit_navigation_failure(code: &mut Vec<u8>, kind: Utf8NavigationKind) {
    if matches!(kind, Utf8NavigationKind::IsBoundary) {
        code.extend_from_slice(&[0x31, 0xc0, 0x41, 0x5c, 0xc3]);
    } else {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x41, 0x5c, 0xc3]);
    }
}

fn emit_string_utf8_slice_runtime(
    code: &mut Vec<u8>,
    byte_offset_target: usize,
    string_len_target: usize,
    string_slice_target: usize,
) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[
        0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x89, 0x54, 0x24, 0x10,
    ]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24, 0x48, 0x8b, 0x74, 0x24, 0x08]);
    emit_internal_call(code, byte_offset_target);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let start_failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24, 0x48, 0x8b, 0x74, 0x24, 0x10]);
    emit_internal_call(code, byte_offset_target);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let end_clamp = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    let skip_end_clamp = emit_near_unconditional_placeholder(code);
    let end_clamp_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x00]);
    emit_internal_call(code, string_len_target);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    let end_ready = code.len();

    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20, 0x48, 0x3b, 0x44, 0x24, 0x18]);
    let invalid_range = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x48, 0x2b, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x3c, 0x24, 0x48, 0x8b, 0x74, 0x24, 0x18, 0x48, 0x89, 0xc2,
    ]);
    emit_internal_call(code, string_slice_target);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, start_failed, failure);
    patch_near_jump(code, end_clamp, end_clamp_target);
    patch_near_jump(code, skip_end_clamp, end_ready);
    patch_near_jump(code, invalid_range, failure);
}

fn emit_string_utf8_find_codepoint_runtime(code: &mut Vec<u8>, codepoint_target: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[
        0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x31, 0xc0, 0x48, 0x89, 0x44,
        0x24, 0x10,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24, 0x48, 0x8b, 0x74, 0x24, 0x10]);
    emit_internal_call(code, codepoint_target);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let end = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x3b, 0x44, 0x24, 0x08]);
    let found = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0xff, 0x44, 0x24, 0x10]);
    let advance = emit_near_unconditional_placeholder(code);
    let found_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[
        0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x48, 0x83, 0xc4, 0x28, 0xc3,
    ]);
    patch_near_jump(code, end, failure);
    patch_near_jump(code, found, found_target);
    patch_near_jump(code, advance, loop_start);
}

fn emit_string_utf8_codepoint_at_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_value = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x49, 0x89, 0xf8, 0x45, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x8a, 0x10, 0x45, 0x84, 0xd2]);
    let end = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x49, 0x39, 0xf1]);
    let skip = emit_near_jump_placeholder(code, 0x0f, 0x85);

    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x80]);
    let ascii = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xe0]);
    let two = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xf0]);
    let three = emit_near_jump_placeholder(code, 0x0f, 0x82);

    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x07, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x02,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x03,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);

    let ascii_target = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0xc2, 0xc3]);
    let two_target = code.len();
    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x1f, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);
    let three_target = code.len();
    code.extend_from_slice(&[
        0x41, 0x0f, 0xb6, 0xc2, 0x83, 0xe0, 0x0f, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x01,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc1, 0xe0, 0x06, 0x45, 0x0f, 0xb6, 0x58, 0x02,
        0x41, 0x83, 0xe3, 0x3f, 0x44, 0x09, 0xd8, 0xc3,
    ]);

    let skip_target = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0x80]);
    let advance_one = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xe0]);
    let advance_two = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x41, 0x80, 0xfa, 0xf0]);
    let advance_three = emit_near_jump_placeholder(code, 0x0f, 0x82);
    let advance_four = emit_codepoint_advance_and_loop(code, 4);
    let advance_three_target = code.len();
    let back_three = emit_codepoint_advance_and_loop(code, 3);
    let advance_two_target = code.len();
    let back_two = emit_codepoint_advance_and_loop(code, 2);
    let advance_one_target = code.len();
    let back_one = emit_codepoint_advance_and_loop(code, 1);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    patch_near_jump(code, null_value, failure);
    patch_near_jump(code, end, failure);
    patch_near_jump(code, skip, skip_target);
    patch_near_jump(code, ascii, ascii_target);
    patch_near_jump(code, two, two_target);
    patch_near_jump(code, three, three_target);
    patch_near_jump(code, advance_one, advance_one_target);
    patch_near_jump(code, advance_two, advance_two_target);
    patch_near_jump(code, advance_three, advance_three_target);
    patch_near_jump(code, advance_four, loop_start);
    patch_near_jump(code, back_one, loop_start);
    patch_near_jump(code, back_two, loop_start);
    patch_near_jump(code, back_three, loop_start);
}

fn emit_codepoint_advance_and_loop(code: &mut Vec<u8>, amount: u8) -> usize {
    code.extend_from_slice(&[0x49, 0x83, 0xc0, amount, 0x49, 0xff, 0xc1]);
    emit_near_unconditional_placeholder(code)
}

fn emit_array_new_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x28, 0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08,
    ]);
    code.extend_from_slice(&[
        0x48, 0x89, 0xf8, 0x48, 0x0f, 0xaf, 0xc6, 0x48, 0x83, 0xc0, 0x20, 0x48, 0x89, 0xc6,
    ]);
    code.extend_from_slice(&[
        0x31, 0xff, 0xba, 0x03, 0x00, 0x00, 0x00, 0x41, 0xba, 0x22, 0x00, 0x00, 0x00,
    ]);
    code.extend_from_slice(&[
        0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x45, 0x31, 0xc9, 0xb8, 0x09, 0x00, 0x00, 0x00,
        0x0f, 0x05, 0x48, 0x85, 0xc0,
    ]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[
        0x48, 0x8d, 0x50, 0x20, 0x48, 0x89, 0x10, 0x48, 0xc7, 0x40, 0x08, 0x00, 0x00, 0x00, 0x00,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4c, 0x24, 0x08, 0x48, 0x89, 0x48, 0x10, 0x48, 0x8b, 0x0c, 0x24, 0x48, 0x89,
        0x48, 0x18,
    ]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_near_jump(code, failed, failure);
}

fn emit_array_len_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0xc3]);
    let zero = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, zero);
}

fn emit_array_clone_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x38, 0x48, 0x89, 0x3c, 0x24, 0x48, 0x85, 0xff,
    ]);
    let null = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x47, 0x18, 0x48, 0x89, 0x44, 0x24, 0x08, 0x48, 0x8b, 0x47, 0x10, 0x48, 0x89,
        0x44, 0x24, 0x10, 0x48, 0x8b, 0x47, 0x08, 0x48, 0x89, 0x44, 0x24, 0x18, 0x48, 0x8b, 0x44,
        0x24, 0x10, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x08, 0x48, 0x83, 0xc0, 0x20, 0x48, 0x89, 0x44,
        0x24, 0x20, 0x31, 0xff, 0x48, 0x8b, 0x74, 0x24, 0x20, 0xba, 0x03, 0x00, 0x00, 0x00, 0x41,
        0xba, 0x22, 0x00, 0x00, 0x00, 0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x45, 0x31, 0xc9,
        0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0,
    ]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8d, 0x50, 0x20, 0x48, 0x89, 0x10, 0x48, 0x8b, 0x4c,
        0x24, 0x18, 0x48, 0x89, 0x48, 0x08, 0x48, 0x8b, 0x4c, 0x24, 0x10, 0x48, 0x89, 0x48, 0x10,
        0x48, 0x8b, 0x4c, 0x24, 0x08, 0x48, 0x89, 0x48, 0x18, 0x48, 0x8b, 0x44, 0x24, 0x18, 0x48,
        0x0f, 0xaf, 0x44, 0x24, 0x08, 0x48, 0x89, 0xc1, 0x48, 0x8b, 0x34, 0x24, 0x48, 0x8b, 0x7c,
        0x24, 0x28, 0x48, 0x8b, 0x3f, 0x48, 0x8b, 0x54, 0x24, 0x28, 0x48, 0x8b, 0x36, 0xf3, 0xa4,
        0x48, 0x8b, 0x44, 0x24, 0x28, 0x48, 0x83, 0xc4, 0x38, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, null, failure);
    patch_near_jump(code, allocation_failed, failure);
}

fn emit_array_reserve_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x38, 0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x85,
        0xff,
    ]);
    let null = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x10, 0x48, 0x39, 0xc6]);
    let already_large = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x47, 0x18, 0x48, 0x89, 0x44, 0x24, 0x10, 0x48, 0x8b, 0x47, 0x08, 0x48, 0x89,
        0x44, 0x24, 0x18, 0x48, 0x8b, 0x44, 0x24, 0x08, 0x48, 0x0f, 0xaf, 0x44, 0x24, 0x10, 0x48,
        0x83, 0xc0, 0x20, 0x48, 0x89, 0x44, 0x24, 0x20, 0x31, 0xff, 0x48, 0x8b, 0x74, 0x24, 0x20,
        0xba, 0x03, 0x00, 0x00, 0x00, 0x41, 0xba, 0x22, 0x00, 0x00, 0x00, 0x49, 0xc7, 0xc0, 0xff,
        0xff, 0xff, 0xff, 0x45, 0x31, 0xc9, 0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85,
        0xc0,
    ]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8d, 0x50, 0x20, 0x48, 0x89, 0x10, 0x48, 0x8b, 0x4c,
        0x24, 0x18, 0x48, 0x89, 0x48, 0x08, 0x48, 0x8b, 0x4c, 0x24, 0x08, 0x48, 0x89, 0x48, 0x10,
        0x48, 0x8b, 0x4c, 0x24, 0x10, 0x48, 0x89, 0x48, 0x18, 0x48, 0x8b, 0x44, 0x24, 0x18, 0x48,
        0x0f, 0xaf, 0x44, 0x24, 0x10, 0x48, 0x89, 0xc1, 0x48, 0x8b, 0x34, 0x24, 0x48, 0x8b, 0x7c,
        0x24, 0x28, 0x48, 0x8b, 0x3f, 0x48, 0x8b, 0x36, 0xf3, 0xa4, 0x48, 0x8b, 0x3c, 0x24, 0x48,
        0x8b, 0x47, 0x10, 0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xc8, 0x48, 0x83, 0xc1, 0x20,
        0x48, 0x89, 0xce, 0x48, 0x8b, 0x3c, 0x24, 0xb8, 0x0b, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48,
        0x8b, 0x44, 0x24, 0x28, 0x48, 0x83, 0xc4, 0x38, 0xc3,
    ]);
    let return_source = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x04, 0x24, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, null, failure);
    patch_near_jump(code, already_large, return_source);
    patch_near_jump(code, allocation_failed, failure);
}

fn emit_array_clear_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0xc7, 0x47, 0x08, 0x00, 0x00, 0x00, 0x00, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
}

fn emit_array_free_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x47, 0x10, 0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xc8, 0x48, 0x83, 0xc1,
        0x20, 0x48, 0x89, 0xce, 0xb8, 0x0b, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0,
    ]);
    let failed = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_capacity_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x10, 0xc3]);
    let zero = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, zero);
}

fn emit_array_is_empty_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x83, 0x7f, 0x08, 0x00]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let empty = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, empty);
    patch_short_jump(code, done, empty);
}

fn emit_array_get_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let failed = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xce, 0x48, 0x8b, 0x07, 0x48, 0x8d, 0x04, 0x08,
        0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_set_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let failed = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4f, 0x18, 0x49, 0x89, 0xc8, 0x48, 0x0f, 0xaf, 0xce, 0x48, 0x8b, 0x07, 0x48,
        0x01, 0xc8, 0x4c, 0x89, 0xc1, 0x48, 0x89, 0xc7, 0x48, 0x89, 0xd6, 0xf3, 0xa4, 0x31, 0xc0,
        0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_array_push_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x3b, 0x47, 0x10]);
    let full = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4f, 0x18, 0x49, 0x89, 0xc8, 0x48, 0x0f, 0xaf, 0xc8, 0x48, 0x8b, 0x17, 0x48,
        0x01, 0xca, 0x48, 0xff, 0x47, 0x08, 0x4c, 0x89, 0xc1, 0x48, 0x89, 0xd7, 0xf3, 0xa4, 0x31,
        0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, full, failure);
}

fn emit_array_truncate_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x3b, 0x77, 0x08]);
    let too_large = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0x89, 0x77, 0x08, 0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, too_large, failure);
}

fn emit_array_pop_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0xff, 0xc8, 0x48, 0x89, 0x47, 0x08, 0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xc8,
        0x48, 0x8b, 0x17, 0x48, 0x01, 0xca, 0x48, 0x89, 0xd0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
}

fn emit_array_pop_first_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x17, 0x48, 0xff, 0xc8, 0x48, 0x89, 0x47, 0x08, 0x4c, 0x8b, 0x4f, 0x18, 0x4c,
        0x89, 0xc9, 0x48, 0x0f, 0xaf, 0xc8, 0x48, 0x85, 0xc9,
    ]);
    let no_copy = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4a, 0x8d, 0x34, 0x0a, 0x48, 0x89, 0xd7, 0xf3, 0xa4]);
    let success = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xd0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
    patch_short_jump(code, no_copy, success);
}

fn emit_array_swap_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let left_invalid = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[0x48, 0x39, 0xc2]);
    let right_invalid = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[
        0x4c, 0x8b, 0x47, 0x18, 0x4c, 0x8b, 0x0f, 0x49, 0x89, 0xf2, 0x4d, 0x0f, 0xaf, 0xd0, 0x4d,
        0x01, 0xca, 0x49, 0x89, 0xd3, 0x4d, 0x0f, 0xaf, 0xd8, 0x4d, 0x01, 0xcb,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x41, 0x8a, 0x02, 0x41, 0x8a, 0x0b, 0x41, 0x88, 0x0a, 0x41, 0x88, 0x03, 0x49, 0xff, 0xc2,
        0x49, 0xff, 0xc3, 0x49, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, left_invalid, failure);
    patch_near_jump(code, right_invalid, failure);
    patch_near_jump(code, done, success);
}

fn emit_array_swap_remove_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let invalid = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[
        0x48, 0xff, 0xc8, 0x48, 0x89, 0x47, 0x08, 0x4c, 0x8b, 0x47, 0x18, 0x4c, 0x8b, 0x0f, 0x49,
        0x89, 0xf2, 0x4d, 0x0f, 0xaf, 0xd0, 0x4d, 0x01, 0xca, 0x49, 0x89, 0xc3, 0x4d, 0x0f, 0xaf,
        0xd8, 0x4d, 0x01, 0xcb,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x41, 0x8a, 0x03, 0x41, 0x88, 0x02, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc3, 0x49, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, invalid, failure);
    patch_near_jump(code, done, success);
}

fn emit_array_remove_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let invalid = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[
        0x48, 0xff, 0xc8, 0x48, 0x89, 0x47, 0x08, 0x4c, 0x8b, 0x47, 0x18, 0x4c, 0x8b, 0x0f, 0x49,
        0x89, 0xf2, 0x4d, 0x0f, 0xaf, 0xd0, 0x4d, 0x01, 0xca, 0x4d, 0x89, 0xd3, 0x4d, 0x01, 0xc3,
        0x48, 0x29, 0xf0, 0x49, 0x0f, 0xaf, 0xc0, 0x48, 0x89, 0xd7, 0x4c, 0x89, 0xde, 0x48, 0x89,
        0xc1, 0xf3, 0xa4, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, invalid, failure);
}

fn emit_array_insert_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let invalid_index = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[0x48, 0x3b, 0x47, 0x10]);
    let full = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[
        0x4c, 0x8b, 0x47, 0x18, 0x4c, 0x8b, 0x0f, 0x49, 0x89, 0xf2, 0x4d, 0x0f, 0xaf, 0xd0, 0x4d,
        0x01, 0xca, 0x48, 0xff, 0x47, 0x08, 0x48, 0x29, 0xf0, 0x49, 0x0f, 0xaf, 0xc0, 0x48, 0x85,
        0xc0,
    ]);
    let no_shift = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x4c, 0x89, 0xd6, 0x48, 0x01, 0xc6, 0x48, 0x89, 0xf7, 0x4c, 0x01, 0xc7, 0x48, 0xff, 0xce,
        0x48, 0xff, 0xcf, 0x48, 0x89, 0xc1, 0xfd, 0xf3, 0xa4, 0xfc,
    ]);
    let copy_value = code.len();
    code.extend_from_slice(&[
        0x48, 0x89, 0xd6, 0x4c, 0x89, 0xd7, 0x4c, 0x89, 0xc1, 0xf3, 0xa4, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, invalid_index, failure);
    patch_near_jump(code, full, failure);
    patch_near_jump(code, no_shift, copy_value);
}

fn emit_array_extend_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let other_failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4f, 0x08, 0x48, 0x8b, 0x56, 0x08, 0x48, 0x89, 0xc8, 0x48, 0x01, 0xd0, 0x48,
        0x3b, 0x47, 0x10,
    ]);
    let full = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[
        0x48, 0x89, 0x47, 0x08, 0x48, 0x89, 0xc8, 0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xc8,
        0x4c, 0x8b, 0x0f, 0x49, 0x01, 0xc9, 0x48, 0x8b, 0x4f, 0x18, 0x48, 0x0f, 0xaf, 0xd1, 0x4c,
        0x8b, 0x16,
    ]);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let skip_copy = emit_short_jump_placeholder(code, 0x74);
    let copy_loop = code.len();
    code.extend_from_slice(&[
        0x41, 0x8a, 0x02, 0x41, 0x88, 0x01, 0x49, 0xff, 0xc2, 0x49, 0xff, 0xc1, 0x48, 0xff, 0xca,
    ]);
    let copy_again = emit_short_jump_placeholder(code, 0x75);
    let after_copy = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, other_failed, failure);
    patch_near_jump(code, full, failure);
    patch_short_jump(code, skip_copy, after_copy);
    patch_short_jump(code, copy_again, copy_loop);
}

fn emit_array_copy_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let source_failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x4d, 0x89, 0xc1]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x47, 0x18, 0x4c, 0x3b, 0x42, 0x18]);
    let mismatch = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x4c, 0x8b, 0x57, 0x08, 0x49, 0x39, 0xf2]);
    let dst_invalid = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[0x49, 0x29, 0xf2, 0x4d, 0x39, 0xca]);
    let dst_range = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x4c, 0x8b, 0x5a, 0x08, 0x49, 0x39, 0xcb]);
    let src_invalid = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[0x49, 0x29, 0xcb, 0x4d, 0x39, 0xcb]);
    let src_range = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[
        0x49, 0x0f, 0xaf, 0xf0, 0x49, 0x0f, 0xaf, 0xc8, 0x4d, 0x0f, 0xaf, 0xc8, 0x4c, 0x8b, 0x1f,
        0x49, 0x01, 0xf3, 0x4c, 0x8b, 0x12, 0x49, 0x01, 0xca, 0x4c, 0x89, 0xdf, 0x4c, 0x89, 0xd6,
        0x4c, 0x89, 0xc9, 0xf3, 0xa4, 0x31, 0xc0, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, source_failed, failure);
    patch_near_jump(code, mismatch, failure);
    patch_near_jump(code, dst_invalid, failure);
    patch_near_jump(code, dst_range, failure);
    patch_near_jump(code, src_invalid, failure);
    patch_near_jump(code, src_range, failure);
}

fn emit_array_resize_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x3b, 0x77, 0x10]);
    let too_large = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x39, 0xc6]);
    let shrink = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[0x48, 0x89, 0x77, 0x08, 0x48, 0x29, 0xc6, 0x48, 0x85, 0xf6]);
    let no_fill = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x18, 0x48, 0x89, 0x14, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x8b,
        0x4f, 0x18, 0x48, 0x89, 0x4c, 0x24, 0x10, 0x48, 0x0f, 0xaf, 0xc1, 0x48, 0x8b, 0x3f, 0x48,
        0x01, 0xc7, 0x48, 0x8b, 0x44, 0x24, 0x08,
    ]);
    let fill_loop = code.len();
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let fill_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4c, 0x24, 0x10, 0x48, 0x8b, 0x34, 0x24, 0xf3, 0xa4, 0x48, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, fill_loop);
    let fill_success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let empty_success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let shrink_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x77, 0x08, 0x31, 0xc0, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_near_jump(code, too_large, failure);
    patch_near_jump(code, shrink, shrink_target);
    patch_short_jump(code, no_fill, empty_success);
    patch_short_jump(code, fill_done, fill_success);
}

fn emit_array_first_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x83, 0x7f, 0x08, 0x00]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x07, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
}

fn emit_array_last_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc8, 0x48, 0x8b, 0x0f, 0x48, 0x01, 0xc8, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, failure);
}

fn emit_array_fill_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x18, 0x48, 0x89, 0x34, 0x24, 0x48, 0x8b, 0x47, 0x08, 0x48, 0x8b, 0x4f,
        0x18, 0x48, 0x89, 0x4c, 0x24, 0x08, 0x48, 0x8b, 0x3f,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x4c, 0x24, 0x08, 0x48, 0x8b, 0x34, 0x24, 0xf3, 0xa4, 0x48, 0xff, 0xc8,
    ]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, done, success);
}

fn emit_array_reverse_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let failed = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8b, 0x47, 0x08, 0x48, 0x85, 0xc0]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc8, 0x4c, 0x8b, 0x07, 0x4d, 0x8d, 0x0c, 0x00]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x4d, 0x39, 0xc8]);
    let done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x45, 0x8a, 0x10, 0x45, 0x8a, 0x19, 0x45, 0x88, 0x18, 0x45, 0x88, 0x11, 0x49, 0xff, 0xc0,
        0x49, 0xff, 0xc9,
    ]);
    emit_short_jump_back(code, loop_start);
    let success = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, failed, failure);
    patch_short_jump(code, empty, success);
    patch_short_jump(code, done, success);
}

fn emit_array_index_of_runtime(code: &mut Vec<u8>) {
    emit_array_search_runtime(code, false, false, false);
}

fn emit_array_last_index_of_runtime(code: &mut Vec<u8>) {
    emit_array_search_runtime(code, true, false, false);
}

fn emit_array_contains_runtime(code: &mut Vec<u8>) {
    emit_array_search_runtime(code, false, true, true);
}

fn emit_array_count_runtime(code: &mut Vec<u8>) {
    emit_array_search_runtime(code, false, true, false);
}

fn emit_array_search_runtime(code: &mut Vec<u8>, last: bool, count: bool, contains: bool) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_array = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x41, 0x54, 0x49, 0x89, 0xf4, 0x4c, 0x8b, 0x57, 0x08, 0x4c, 0x8b, 0x5f, 0x18, 0x4c, 0x8b,
        0x0f, 0x31, 0xd2,
    ]);
    if last {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    } else {
        code.extend_from_slice(&[0x31, 0xc0]);
    }
    let loop_start = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x4c, 0x89, 0xcf, 0x4c, 0x89, 0xe6, 0x4c, 0x89, 0xd9, 0xf3, 0xa6,
    ]);
    let mismatch = emit_short_jump_placeholder(code, 0x75);
    if contains {
        code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x41, 0x5c, 0xc3]);
    } else if count {
        code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    } else if last {
        code.extend_from_slice(&[0x48, 0x89, 0xd0]);
    } else {
        code.extend_from_slice(&[0x48, 0x89, 0xd0, 0x41, 0x5c, 0xc3]);
    }
    if count || last {
        code.extend_from_slice(&[0x4d, 0x01, 0xd9, 0x48, 0xff, 0xc2, 0x49, 0xff, 0xca]);
        emit_short_jump_back(code, loop_start);
    }
    let mismatch_target = code.len();
    code.extend_from_slice(&[0x4d, 0x01, 0xd9, 0x48, 0xff, 0xc2, 0x49, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let result_done = code.len();
    code.extend_from_slice(&[0x41, 0x5c]);
    if contains {
        code.extend_from_slice(&[0x31, 0xc0]);
    }
    code.push(0xc3);
    let failure = code.len();
    code.extend_from_slice(&[0x41, 0x5c]);
    if contains || count {
        code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    } else {
        code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    }
    patch_short_jump(code, null_array, failure);
    patch_short_jump(code, null_value, failure);
    patch_short_jump(code, done, result_done);
    patch_short_jump(code, mismatch, mismatch_target);
}

fn emit_bounds_check_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x39, 0xf7]);
    let failed = emit_short_jump_placeholder(code, 0x73);
    code.push(0xc3);
    let failure = code.len();
    code.extend_from_slice(&[0xbf, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x3c, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.push(0xc3);
    patch_short_jump(code, failed, failure);
}

fn emit_print_runtime(code: &mut Vec<u8>, newline: bool, fd: u8) {
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
    code.extend_from_slice(&[0xbf, fd, 0, 0, 0]);
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
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x20,
    ]);

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

fn emit_process_id_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0xb8, 39, 0x00, 0x00, 0x00, 0x0f, 0x05, 0xc3]);
}

fn emit_arg_count_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x49, 0x8b, 0x04, 0x24, 0xc3]);
}

fn emit_arg_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let negative = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x49, 0x3b, 0x3c, 0x24]);
    let out_of_range = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x48, 0x89, 0xf8, // rax = index
        0x48, 0xc1, 0xe0, 0x03, // rax *= sizeof(pointer)
        0x4c, 0x01, 0xe0, // rax += initial stack pointer
        0x48, 0x8b, 0x40, 0x08, // rax = argv[index]
        0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, negative, failure);
    patch_short_jump(code, out_of_range, failure);
}

fn emit_arg_exists_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let negative = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[
        0x49, 0x3b, 0x3c, 0x24, 0x0f, 0x92, 0xc0, 0x0f, 0xb6, 0xc0, 0xc3,
    ]);
    let false_value = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, negative, false_value);
}

fn emit_arg_or_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xf2, 0x48, 0x85, 0xff]);
    let negative = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x49, 0x3b, 0x3c, 0x24]);
    let out_of_range = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[
        0x48, 0x89, 0xf8, 0x48, 0xc1, 0xe0, 0x03, 0x4c, 0x01, 0xe0, 0x48, 0x8b, 0x40, 0x08, 0xc3,
    ]);
    let fallback = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xd0, 0xc3]);
    patch_short_jump(code, negative, fallback);
    patch_short_jump(code, out_of_range, fallback);
}

fn emit_platform_path_separator_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0xb8, b'/', 0x00, 0x00, 0x00, 0xc3]);
}

fn emit_owned_constant_runtime(code: &mut Vec<u8>, bytes: &[u8]) {
    let allocation_size = 8 + bytes.len() as u32;
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]);
    code.extend_from_slice(&[
        0x31,
        0xff,
        0xbe,
        allocation_size as u8,
        (allocation_size >> 8) as u8,
        (allocation_size >> 16) as u8,
        (allocation_size >> 24) as u8,
        0xba,
        0x03,
        0x00,
        0x00,
        0x00,
        0x41,
        0xba,
        0x22,
        0x00,
        0x00,
        0x00,
        0x49,
        0xc7,
        0xc0,
        0xff,
        0xff,
        0xff,
        0xff,
        0x45,
        0x31,
        0xc9,
        0xb8,
        0x09,
        0x00,
        0x00,
        0x00,
        0x0f,
        0x05,
    ]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x89, 0x04, 0x24, 0x48, 0xc7, 0x00]);
    code.extend_from_slice(&allocation_size.to_le_bytes());
    for (index, byte) in bytes.iter().copied().enumerate() {
        code.extend_from_slice(&[0xc6, 0x40, 8 + index as u8, byte]);
    }
    code.extend_from_slice(&[0x48, 0x83, 0xc0, 0x08, 0x48, 0x83, 0xc4, 0x08, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x08, 0xc3]);
    patch_near_jump(code, failed, failure);
}

fn emit_path_is_absolute_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let not_absolute = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3f, b'/']);
    let absolute = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x3f, b'\\']);
    let absolute_backslash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0x7f, 0x01, b':']);
    let not_absolute_drive = emit_short_jump_placeholder(code, 0x75);
    let true_target = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, not_absolute, false_target);
    patch_short_jump(code, absolute, true_target);
    patch_short_jump(code, absolute_backslash, true_target);
    patch_short_jump(code, not_absolute_drive, false_target);
}

fn emit_path_file_name_runtime(code: &mut Vec<u8>, slice_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x7c, 0x24, 0x20]);
    code.extend_from_slice(&[0x31, 0xc9, 0x45, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x14, 0x0f, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'/']);
    let slash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'\\']);
    let backslash = emit_short_jump_placeholder(code, 0x74);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let separator = code.len();
    code.extend_from_slice(&[0x4c, 0x8d, 0x41, 0x01]);
    emit_short_jump_back(code, advance);
    let done_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x4c, 0x24, 0x28, 0x4c, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x20, 0x48, 0x8b, 0x74, 0x24, 0x30, 0x48, 0x8b, 0x54, 0x24, 0x28,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, slash, separator);
    patch_short_jump(code, backslash, separator);
}

fn emit_path_parent_runtime(code: &mut Vec<u8>, slice_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x7c, 0x24, 0x20]);
    code.extend_from_slice(&[0x31, 0xc9, 0x45, 0x31, 0xc0]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x14, 0x0f, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'/']);
    let slash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'\\']);
    let backslash = emit_short_jump_placeholder(code, 0x74);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let separator = code.len();
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    emit_short_jump_back(code, advance);
    let done_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x20, 0x31, 0xf6, 0x48, 0x8b, 0x54, 0x24, 0x28,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, slash, separator);
    patch_short_jump(code, backslash, separator);
}

fn emit_path_extension_runtime(code: &mut Vec<u8>, slice_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x7c, 0x24, 0x20]);
    code.extend_from_slice(&[
        0x31, 0xc9, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9, 0x45, 0x31, 0xd2,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x14, 0x0f, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'/']);
    let slash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'\\']);
    let backslash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'.']);
    let not_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x4c, 0x39, 0xd1]);
    let dot_at_start = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x4c, 0x8d, 0x41, 0x01, 0x41, 0xb9, 1, 0, 0, 0]);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let separator = code.len();
    code.extend_from_slice(&[0x4c, 0x8d, 0x51, 0x01, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9]);
    emit_short_jump_back(code, advance);
    let done_target = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xc9]);
    let has_extension = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0x89, 0xc8]);
    let slice_args = code.len();
    code.extend_from_slice(&[
        0x4c, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x7c, 0x24, 0x20, 0x48, 0x8b, 0x74, 0x24, 0x28,
        0x48, 0x89, 0xca, 0x48, 0x29, 0xf2,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, slash, separator);
    patch_short_jump(code, backslash, separator);
    patch_short_jump(code, not_dot, advance);
    patch_short_jump(code, dot_at_start, advance);
    patch_short_jump(code, has_extension, slice_args);
}

fn emit_path_stem_runtime(code: &mut Vec<u8>, slice_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x7c, 0x24, 0x20]);
    code.extend_from_slice(&[0x31, 0xc9, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x14, 0x0f, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'/']);
    let slash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'\\']);
    let backslash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'.']);
    let not_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x4c, 0x39, 0xc1]);
    let dot_at_start = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0x89, 0xc9]);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let separator = code.len();
    code.extend_from_slice(&[0x4c, 0x8d, 0x41, 0x01, 0x45, 0x31, 0xc9]);
    emit_short_jump_back(code, advance);
    let done_target = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xc9]);
    let has_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0x89, 0xc9]);
    let slice_args = code.len();
    code.extend_from_slice(&[
        0x4c, 0x89, 0x44, 0x24, 0x28, 0x4c, 0x89, 0x4c, 0x24, 0x30, 0x48, 0x8b, 0x7c, 0x24, 0x20,
        0x48, 0x8b, 0x74, 0x24, 0x28, 0x48, 0x8b, 0x54, 0x24, 0x30, 0x48, 0x29, 0xf2,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, slash, separator);
    patch_short_jump(code, backslash, separator);
    patch_short_jump(code, not_dot, advance);
    patch_short_jump(code, dot_at_start, advance);
    patch_short_jump(code, has_dot, slice_args);
}

fn emit_path_without_extension_runtime(code: &mut Vec<u8>, slice_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x48, 0x48, 0x89, 0x7c, 0x24, 0x20]);
    code.extend_from_slice(&[
        0x31, 0xc9, 0x45, 0x31, 0xc0, 0x45, 0x31, 0xc9, 0x45, 0x31, 0xd2,
    ]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x8a, 0x14, 0x0f, 0x84, 0xd2]);
    let done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'/']);
    let slash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'\\']);
    let backslash = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x80, 0xfa, b'.']);
    let not_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x4c, 0x39, 0xd1]);
    let dot_at_start = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0x89, 0xc9]);
    let advance = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, loop_start);
    let separator = code.len();
    code.extend_from_slice(&[0x4c, 0x8d, 0x51, 0x01, 0x45, 0x31, 0xc9]);
    emit_short_jump_back(code, advance);
    let done_target = code.len();
    code.extend_from_slice(&[0x4d, 0x85, 0xc9]);
    let has_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x49, 0x89, 0xc9]);
    let slice_args = code.len();
    code.extend_from_slice(&[
        0x4c, 0x89, 0x4c, 0x24, 0x28, 0x48, 0x8b, 0x7c, 0x24, 0x20, 0x31, 0xf6, 0x48, 0x8b, 0x54,
        0x24, 0x28,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x48, 0xc3]);
    patch_short_jump(code, done, done_target);
    patch_short_jump(code, slash, separator);
    patch_short_jump(code, backslash, separator);
    patch_short_jump(code, not_dot, advance);
    patch_short_jump(code, dot_at_start, advance);
    patch_short_jump(code, has_dot, slice_args);
}

fn emit_path_with_extension_runtime(
    code: &mut Vec<u8>,
    without_extension_offset: usize,
    length_offset: usize,
    from_byte_offset: usize,
    slice_offset: usize,
    concat_offset: usize,
    free_offset: usize,
) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x68]);
    code.extend_from_slice(&[0x48, 0x89, 0x7c, 0x24, 0x20, 0x48, 0x89, 0x74, 0x24, 0x28]);
    code.extend_from_slice(&[0x31, 0xf6]);
    emit_internal_call(code, without_extension_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x30]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x28]);
    emit_internal_call(code, length_offset);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let empty_extension = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x38]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x28, 0x8a, 0x07, 0x3c, b'.']);
    let no_leading_dot = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x40, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xff, 0x4c, 0x24, 0x38]);
    let common_start = emit_short_jump_placeholder(code, 0xeb);
    let no_dot_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0x44, 0x24, 0x40, 0x00, 0x00, 0x00, 0x00]);
    let common_target = code.len();
    code.extend_from_slice(&[0xbf, b'.', 0x00, 0x00, 0x00]);
    emit_internal_call(code, from_byte_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x48]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x30, 0x48, 0x8b, 0x74, 0x24, 0x48]);
    emit_internal_call(code, concat_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x50]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x28, 0x48, 0x8b, 0x74, 0x24, 0x40, 0x48, 0x8b, 0x54, 0x24, 0x38,
    ]);
    emit_internal_call(code, slice_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x58]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x50, 0x48, 0x8b, 0x74, 0x24, 0x58]);
    emit_internal_call(code, concat_offset);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x60]);
    for offset in [0x30, 0x48, 0x50, 0x58] {
        code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, offset]);
        emit_internal_call(code, free_offset);
    }
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x60, 0x48, 0x83, 0xc4, 0x68, 0xc3]);
    let empty_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x30, 0x48, 0x83, 0xc4, 0x68, 0xc3]);
    patch_near_jump(code, empty_extension, empty_target);
    patch_short_jump(code, no_leading_dot, no_dot_target);
    patch_short_jump(code, common_start, common_target);
}

fn emit_alloc_runtime(code: &mut Vec<u8>, array: bool) {
    if array {
        code.extend_from_slice(&[0x48, 0x0f, 0xaf, 0xfe]);
    }
    code.extend_from_slice(&[0x48, 0x83, 0xc7, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, failed, failure);
}

fn emit_free_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x83, 0xef, 0x08, 0x48, 0x8b, 0x37, 0xb8, 0x0b, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48,
        0x85, 0xc0,
    ]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_realloc_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0x48, 0x83, 0xec, 0x38, 0x48, 0x89, 0x3c, 0x24, 0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x8d,
        0x47, 0xf8, 0x48, 0x89, 0x44, 0x24, 0x10, 0x48, 0x8b, 0x08, 0x48, 0x83, 0xe9, 0x08, 0x48,
        0x89, 0x4c, 0x24, 0x18, 0x48, 0x83, 0xc6, 0x08, 0x48, 0x89, 0x74, 0x24, 0x20, 0x31, 0xff,
        0xba, 0x03, 0x00, 0x00, 0x00, 0x41, 0xba, 0x22, 0x00, 0x00, 0x00, 0x49, 0xc7, 0xc0, 0xff,
        0xff, 0xff, 0xff, 0x45, 0x31, 0xc9, 0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85,
        0xc0,
    ]);
    let allocation_failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x28, 0x48, 0x8b, 0x4c,
        0x24, 0x18, 0x48, 0x3b, 0x4c, 0x24, 0x08,
    ]);
    let old_smaller = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x08]);
    let copy_start = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x74, 0x24, 0x00, 0x48, 0x8b, 0x7c, 0x24, 0x28, 0xf3, 0xa4,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0x48, 0x8b, 0x37, 0xb8, 0x0b, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x28, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let null_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, null, null_target);
    patch_short_jump(code, allocation_failed, failure);
    patch_short_jump(code, old_smaller, copy_start);
}

fn emit_alloc_copy_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08, 0x48, 0x89, 0x74, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x89, 0xf2]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[
        0x48, 0x89, 0x44, 0x24, 0x10, 0x48, 0x8b, 0x54, 0x24, 0x18, 0x48, 0x89, 0x10, 0x48, 0x83,
        0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x10,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x08]);
    code.extend_from_slice(&[0xf3, 0xa4]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let empty_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_near_jump(code, failed, failure);
    patch_short_jump(code, empty, empty_target);
}

fn emit_write_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x49, 0x89, 0xf0]);
    code.extend_from_slice(&[0x31, 0xd2]);
    let len_loop = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    let len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xff, 0xc2]);
    emit_short_jump_back(code, len_loop);
    let len_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x10]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc6, 0x41, 0x02, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0xa4, 0x01, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let open_failed = emit_short_jump_placeholder(code, 0x78);

    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]);
    code.push(0xc3);

    let open_failed_target = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28]);
    code.push(0xc3);

    patch_short_jump(code, len_done, len_target);
    patch_short_jump(code, open_failed, open_failed_target);
}

fn emit_copy_file_runtime(code: &mut Vec<u8>) {
    // Copy regular files through Linux open/read/write syscalls without a C runtime.
    code.extend_from_slice(&[0x48, 0x81, 0xec, 0x40, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24, 0x31, 0xf6, 0x31, 0xd2]);
    code.extend_from_slice(&[0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x10, 0x48, 0x85, 0xc0]);
    let source_open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc6, 0x41, 0x02, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0xa4, 0x01, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18, 0x48, 0x85, 0xc0]);
    let dest_open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    let read_loop = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8d, 0x74, 0x24, 0x40]);
    code.extend_from_slice(&[0xba, 0x00, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x31, 0xc0, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let read_done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8d, 0x44, 0x24, 0x40, 0x48, 0x89, 0x44, 0x24, 0x28]);

    let write_loop = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let write_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let zero_write = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x29, 0x44, 0x24, 0x20, 0x48, 0x01, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x20, 0x00]);
    let more_write = emit_near_jump_placeholder(code, 0x0f, 0x85);

    let back_to_read = emit_near_unconditional_placeholder(code);
    let success = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x18, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x18, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[
        0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3,
    ]);

    let dest_failure = code.len();
    code.extend_from_slice(&[
        0x48, 0x8b, 0x7c, 0x24, 0x10, 0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05,
    ]);
    code.extend_from_slice(&[
        0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3,
    ]);

    let source_failure = code.len();
    code.extend_from_slice(&[
        0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x81, 0xc4, 0x40, 0x10, 0x00, 0x00, 0xc3,
    ]);

    patch_near_jump(code, source_open_failed, source_failure);
    patch_near_jump(code, dest_open_failed, dest_failure);
    patch_near_jump(code, read_failed, failure);
    patch_near_jump(code, read_done, success);
    patch_near_jump(code, write_failed, failure);
    patch_near_jump(code, zero_write, failure);
    patch_near_jump(code, more_write, write_loop);
    patch_near_jump(code, back_to_read, read_loop);
}

fn emit_append_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_near_jump_placeholder(code, 0x0f, 0x84);

    code.extend_from_slice(&[0x49, 0x89, 0xf0]);
    code.extend_from_slice(&[0x31, 0xd2]);
    let len_loop = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    let len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xff, 0xc2]);
    emit_short_jump_back(code, len_loop);
    let len_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x10]);

    code.extend_from_slice(&[0xbf, 0x9c, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x8b, 0x34, 0x24]);
    code.extend_from_slice(&[0xba, 0x41, 0x04, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0xa4, 0x01, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x01, 0x01, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x3b, 0x44, 0x24, 0x10]);
    let write_failed = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x18]);
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let null_path_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x28, 0xc3]);

    patch_short_jump(code, len_done, len_target);
    patch_near_jump(code, null_path, null_path_target);
    patch_near_jump(code, open_failed, failure);
    patch_near_jump(code, write_failed, failure);
}

fn emit_touch_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x18]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0xbf, 0x9c, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x00]);
    code.extend_from_slice(&[0xba, 0x41, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0xa4, 0x01, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x01, 0x01, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x89, 0xc7]);
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let null_path_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    patch_near_jump(code, null_path, null_path_target);
    patch_near_jump(code, open_failed, failure);
}

fn emit_truncate_file_runtime(code: &mut Vec<u8>) {
    // Linux x86-64 truncate(const char *path, off_t length).
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xb8, 76, 0, 0, 0, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 1, 0, 0, 0, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_remove_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xb8, 0x57, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_rename_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_source = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_dest = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0xb8, 0x52, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null_source, failure);
    patch_short_jump(code, null_dest, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_create_dir_runtime(code: &mut Vec<u8>) {
    emit_directory_runtime(code, 83, true);
}

fn emit_create_dir_all_runtime(code: &mut Vec<u8>) {
    // Build and create each POSIX path prefix in a bounded stack buffer.
    // Linux mkdir returns -EEXIST for prefixes that are already present.
    code.extend_from_slice(&[0x48, 0x81, 0xec, 0x10, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[
        0x49, 0x89, 0xf9, 0x49, 0x89, 0xe0, 0x49, 0x81, 0xc0, 0x10, 0x00, 0x00, 0x00,
    ]);
    code.extend_from_slice(&[0x48, 0x31, 0xc9]);
    let copy_loop = code.len();
    code.extend_from_slice(&[0x48, 0x81, 0xf9, 0xf0, 0x0f, 0x00, 0x00]);
    let too_long = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[
        0x41, 0x8a, 0x04, 0x09, 0x41, 0x88, 0x04, 0x08, 0x48, 0xff, 0xc1,
    ]);
    code.extend_from_slice(&[0x84, 0xc0]);
    let copy_done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    emit_near_jump_back(code, copy_loop);

    let scan_start = code.len();
    code.extend_from_slice(&[0x4d, 0x89, 0xc1]);
    let scan_loop = code.len();
    code.extend_from_slice(&[0x41, 0x8a, 0x01, 0x84, 0xc0]);
    let final_path = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x3c, b'/']);
    let not_separator = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x4d, 0x39, 0xc1]);
    let leading_separator = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[
        0x41, 0xc6, 0x01, 0x00, 0x4c, 0x89, 0xc7, 0xbe, 0xed, 0x01, 0x00, 0x00,
    ]);
    code.extend_from_slice(&[0xb8, 83, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xef]);
    let prefix_exists = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let prefix_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let prefix_restore = code.len();
    code.extend_from_slice(&[0x41, 0xc6, 0x01, b'/']);
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_near_jump_back(code, scan_loop);

    let leading_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_near_jump_back(code, scan_loop);

    let not_separator_target = code.len();
    code.extend_from_slice(&[0x49, 0xff, 0xc1]);
    emit_near_jump_back(code, scan_loop);

    let final_target = code.len();
    code.extend_from_slice(&[0x4c, 0x89, 0xc7, 0xbe, 0xed, 0x01, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 83, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xef]);
    let final_exists = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let final_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    let success = code.len();
    code.extend_from_slice(&[0x48, 0x81, 0xc4, 0x10, 0x10, 0x00, 0x00, 0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[
        0x48, 0x81, 0xc4, 0x10, 0x10, 0x00, 0x00, 0xb8, 1, 0x00, 0x00, 0x00, 0xc3,
    ]);

    patch_near_jump(code, too_long, failure);
    patch_near_jump(code, copy_done, scan_start);
    patch_near_jump(code, final_path, final_target);
    patch_near_jump(code, not_separator, not_separator_target);
    patch_near_jump(code, leading_separator, leading_target);
    patch_near_jump(code, prefix_failed, failure);
    patch_near_jump(code, prefix_exists, prefix_restore);
    patch_near_jump(code, final_failed, failure);
    patch_near_jump(code, final_exists, success);
}

fn emit_remove_dir_runtime(code: &mut Vec<u8>) {
    emit_directory_runtime(code, 84, false);
}

fn emit_directory_runtime(code: &mut Vec<u8>, syscall: u32, create: bool) {
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_short_jump_placeholder(code, 0x74);
    if create {
        code.extend_from_slice(&[0xbe, 0xed, 0x01, 0x00, 0x00]);
    }
    code.extend_from_slice(&[0xb8]);
    code.extend_from_slice(&syscall.to_le_bytes());
    code.extend_from_slice(&[0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, null_path, failure);
    patch_short_jump(code, failed, failure);
}

fn emit_file_open_runtime(code: &mut Vec<u8>, flags: u32) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x18]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_path = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0xbf, 0x9c, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x8b, 0x34, 0x24]);
    code.push(0xba);
    code.extend_from_slice(&flags.to_le_bytes());
    if flags == 0 {
        code.extend_from_slice(&[0x45, 0x31, 0xd2]);
    } else {
        code.extend_from_slice(&[0x41, 0xba, 0xa4, 0x01, 0x00, 0x00]);
    }
    code.extend_from_slice(&[0xb8, 0x01, 0x01, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);
    patch_near_jump(code, null_path, failure);
}

fn emit_file_write_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x49, 0x89, 0xf0]);
    code.extend_from_slice(&[0x31, 0xd2]);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let null_data = emit_near_jump_placeholder(code, 0x0f, 0x84);
    let len_loop = code.len();
    code.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x08]);
    code.extend_from_slice(&[0x85, 0xc9]);
    let len_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x49, 0xff, 0xc0]);
    code.extend_from_slice(&[0xff, 0xc2]);
    emit_short_jump_back(code, len_loop);
    let len_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x3b, 0x44, 0x24, 0x10]);
    let write_failed = emit_near_jump_placeholder(code, 0x0f, 0x85);
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_near_jump(code, null_data, len_target);
    patch_near_jump(code, write_failed, failure);
    patch_short_jump(code, len_done, len_target);
}

fn emit_file_close_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_file_flush_runtime(code: &mut Vec<u8>) {
    // fsync(fd), returning zero on success and one on error.
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0xb8, 0x4a, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, invalid_handle, failure);
    patch_near_jump(code, failed, failure);
}

fn emit_file_seek_runtime(code: &mut Vec<u8>) {
    // lseek(fd, offset, SEEK_SET), returning zero on success and one on error.
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let invalid_handle = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0xba, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x08, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x83, 0xf8, 0xff]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_near_jump(code, invalid_handle, failure);
    patch_near_jump(code, failed, failure);
}

fn emit_file_read_to_string_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x31, 0xf6, 0xba, 0x02, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x08, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let seek_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc6]);
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08]);
    code.extend_from_slice(&[0x31, 0xff, 0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x10,
    ]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x31, 0xf6, 0x31, 0xd2]);
    code.extend_from_slice(&[0xb8, 0x08, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let rewind_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x08]);
    code.extend_from_slice(&[0x31, 0xc0, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let read_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x10]);
    code.extend_from_slice(&[0xc6, 0x04, 0x08, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, seek_failed, failure);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, rewind_failed, failure);
    patch_near_jump(code, read_failed, failure);
}

fn emit_read_file_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x31, 0xf6]);
    code.extend_from_slice(&[0x31, 0xd2]);
    code.extend_from_slice(&[0xb8, 0x02, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let open_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);

    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0x31, 0xf6]);
    code.extend_from_slice(&[0xba, 0x02, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xb8, 0x08, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0x31, 0xf6]);
    code.extend_from_slice(&[0x31, 0xd2]);
    code.extend_from_slice(&[0xb8, 0x08, 0x00, 0x00, 0x00, 0x0f, 0x05]);

    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc6]);
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x20]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x3d, 0x01, 0xf0, 0xff, 0xff]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x89, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xc0, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);

    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8b, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0x31, 0xc0, 0x0f, 0x05]);
    code.extend_from_slice(&[0x4c, 0x8b, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x49, 0x01, 0xc0]);
    code.extend_from_slice(&[0x41, 0xc6, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x08]);
    code.extend_from_slice(&[0xb8, 0x03, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38]);
    code.push(0xc3);

    let failure_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, open_failed, failure_target);
    patch_near_jump(code, allocation_failed, failure_target);
}

fn emit_read_file_or_runtime(code: &mut Vec<u8>, read_file_offset: usize) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x18]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.push(0xe8);
    let displacement = read_file_offset as isize - (code.len() as isize + 4);
    code.extend_from_slice(&(displacement as i32).to_le_bytes());
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let has_value = code.len();
    code.extend_from_slice(&[0x75, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x08]);
    let done = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);
    code[has_value + 1] = (done as isize - (has_value as isize + 2)) as i8 as u8;
}

fn emit_read_line_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xbe, 0x08, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x89, 0x04, 0x24]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0xc7, 0x00, 0x08, 0x10, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x04, 0x24]);

    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0x48, 0x8b, 0x34, 0x24]);
    code.extend_from_slice(&[0xba, 0xff, 0x0f, 0x00, 0x00]);
    code.extend_from_slice(&[0x31, 0xc0, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let read_empty = emit_short_jump_placeholder(code, 0x7e);
    code.extend_from_slice(&[0x49, 0x89, 0xc0]);
    code.extend_from_slice(&[0x31, 0xc9]);
    let scan = code.len();
    code.extend_from_slice(&[0x4c, 0x39, 0xc1]);
    let scan_done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[0x80, 0x3c, 0x0e, 0x0a]);
    let newline = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    emit_short_jump_back(code, scan);
    let newline_target = code.len();
    code.extend_from_slice(&[0x48, 0xff, 0xc1]);
    let terminate = code.len();
    code.extend_from_slice(&[0xc6, 0x04, 0x0e, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x04, 0x24]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let empty = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x34, 0x24]);
    code.extend_from_slice(&[0xc6, 0x06, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x04, 0x24]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x28, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);

    patch_near_jump(code, allocation_failed, failure);
    patch_short_jump(code, read_empty, empty);
    patch_short_jump(code, scan_done, terminate);
    patch_short_jump(code, newline, newline_target);
}

fn emit_mem_copy_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xd1]);
    code.extend_from_slice(&[0xf3, 0xa4]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_mem_zero_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xf1]);
    code.extend_from_slice(&[0x31, 0xc0]);
    code.extend_from_slice(&[0xf3, 0xaa]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_mem_move_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x39, 0xf7]);
    let forward = emit_short_jump_placeholder(code, 0x76);
    code.extend_from_slice(&[0x48, 0x8d, 0x3c, 0x17]);
    code.extend_from_slice(&[0x48, 0x8d, 0x34, 0x16]);
    code.extend_from_slice(&[0x48, 0xff, 0xcf]);
    code.extend_from_slice(&[0x48, 0xff, 0xce]);
    code.extend_from_slice(&[0x48, 0x89, 0xd1]);
    code.push(0xfd);
    code.extend_from_slice(&[0xf3, 0xa4]);
    code.push(0xfc);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let forward_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xd1]);
    code.extend_from_slice(&[0xf3, 0xa4]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, forward, forward_target);
}

fn emit_mem_fill_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xf9]);
    code.extend_from_slice(&[0x48, 0x89, 0xf1]);
    code.extend_from_slice(&[0x88, 0xd0]);
    code.extend_from_slice(&[0xf3, 0xaa]);
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
}

fn emit_mem_find_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x31, 0xc0]);
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x38, 0x14, 0x07]);
    let found = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0xff, 0xce]);
    emit_short_jump_back(code, loop_start);
    let found_target = code.len();
    code.push(0xc3);
    let not_found = code.len();
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    patch_short_jump(code, empty, not_found);
    patch_short_jump(code, found, found_target);
}

fn emit_mem_compare_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xd1]);
    code.extend_from_slice(&[0x48, 0x85, 0xc9]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x0f, 0xb6, 0x07]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x16]);
    code.extend_from_slice(&[0x39, 0xd0]);
    let less = emit_short_jump_placeholder(code, 0x72);
    let greater = emit_short_jump_placeholder(code, 0x77);
    code.extend_from_slice(&[0x48, 0xff, 0xc7]);
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    code.extend_from_slice(&[0x48, 0xff, 0xc9]);
    emit_short_jump_back(code, loop_start);
    let equal_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    let less_target = code.len();
    code.extend_from_slice(&[0xb8, 0xff, 0xff, 0xff, 0xff, 0xc3]);
    let greater_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    patch_short_jump(code, empty, equal_target);
    patch_short_jump(code, less, less_target);
    patch_short_jump(code, greater, greater_target);
}

fn emit_mem_equal_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xd2]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x0f, 0xb6, 0x07]);
    code.extend_from_slice(&[0x0f, 0xb6, 0x0e]);
    code.extend_from_slice(&[0x39, 0xc8]);
    let not_equal = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc7]);
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    code.extend_from_slice(&[0x48, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let equal_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, equal_target);
    patch_short_jump(code, not_equal, false_target);
}

fn emit_mem_is_zero_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    let loop_start = code.len();
    code.extend_from_slice(&[0x0f, 0xb6, 0x07]);
    code.extend_from_slice(&[0x85, 0xc0]);
    let not_zero = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xc7]);
    code.extend_from_slice(&[0x48, 0xff, 0xce]);
    emit_short_jump_back(code, loop_start);
    let zero_target = code.len();
    code.extend_from_slice(&[0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3]);
    let false_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, zero_target);
    patch_short_jump(code, not_zero, false_target);
}

fn emit_mem_reverse_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x85, 0xf6]);
    let empty = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0x8d, 0x54, 0x37, 0xff]);
    let loop_start = code.len();
    code.extend_from_slice(&[0x48, 0x39, 0xd7]);
    let done = emit_short_jump_placeholder(code, 0x73);
    code.extend_from_slice(&[0x8a, 0x07]);
    code.extend_from_slice(&[0x8a, 0x0a]);
    code.extend_from_slice(&[0x86, 0xc8]);
    code.extend_from_slice(&[0x88, 0x07]);
    code.extend_from_slice(&[0x88, 0x0a]);
    code.extend_from_slice(&[0x48, 0xff, 0xc7]);
    code.extend_from_slice(&[0x48, 0xff, 0xca]);
    emit_short_jump_back(code, loop_start);
    let done_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0xc3]);
    patch_short_jump(code, empty, done_target);
    patch_short_jump(code, done, done_target);
}

fn emit_string_from_byte_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x18]);
    code.extend_from_slice(&[0x40, 0x88, 0x3c, 0x24]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xbe, 0x0a, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x8a, 0x14, 0x24]);
    code.extend_from_slice(&[0x88, 0x10]);
    code.extend_from_slice(&[0xc6, 0x40, 0x01, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    patch_near_jump(code, failed, failure);
}

fn emit_string_from_utf8_codepoint_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x18, 0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let invalid_negative = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x81, 0xff, 0xff, 0xff, 0x10, 0x00]);
    let invalid_large = emit_near_jump_placeholder(code, 0x0f, 0x87);
    code.extend_from_slice(&[0x48, 0x81, 0xff, 0x00, 0xd8, 0x00, 0x00]);
    let valid = emit_near_jump_placeholder(code, 0x0f, 0x82);
    code.extend_from_slice(&[0x48, 0x81, 0xff, 0xff, 0xdf, 0x00, 0x00]);
    let invalid_surrogate = emit_near_jump_placeholder(code, 0x0f, 0x86);
    let valid_target = code.len();

    code.extend_from_slice(&[0x31, 0xff, 0xbe, 0x0d, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9, 0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0xc7, 0x00, 0x0d, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x83, 0xff, 0x7f]);
    let ascii = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[0x48, 0x81, 0xff, 0xff, 0x07, 0x00, 0x00]);
    let two_byte = emit_near_jump_placeholder(code, 0x0f, 0x86);
    code.extend_from_slice(&[0x48, 0x81, 0xff, 0xff, 0xff, 0x00, 0x00]);
    let three_byte = emit_near_jump_placeholder(code, 0x0f, 0x86);

    code.extend_from_slice(&[
        0x48, 0x89, 0xf9, 0x48, 0xc1, 0xe9, 0x12, 0x80, 0xc1, 0xf0, 0x88, 0x08,
    ]);
    code.extend_from_slice(&[
        0x48, 0x89, 0xf9, 0x48, 0xc1, 0xe9, 0x0c, 0x80, 0xe1, 0x3f, 0x80, 0xc9, 0x80, 0x88, 0x48,
        0x01,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x3c, 0x24, 0x48, 0xc1, 0xef, 0x06, 0x40, 0x80, 0xe7, 0x3f, 0x40, 0x80, 0xcf,
        0x80, 0x40, 0x88, 0x78, 0x02,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x3c, 0x24, 0x40, 0x80, 0xe7, 0x3f, 0x40, 0x80, 0xcf, 0x80, 0x40, 0x88, 0x78,
        0x03,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x08, 0xc6, 0x40, 0x04, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);

    let three_target = code.len();
    code.extend_from_slice(&[
        0x48, 0x89, 0xf9, 0x48, 0xc1, 0xe9, 0x0c, 0x80, 0xc1, 0xe0, 0x88, 0x08,
    ]);
    code.extend_from_slice(&[
        0x48, 0x89, 0xf9, 0x48, 0xc1, 0xe9, 0x06, 0x80, 0xe1, 0x3f, 0x80, 0xc9, 0x80, 0x88, 0x48,
        0x01,
    ]);
    code.extend_from_slice(&[
        0x40, 0x80, 0xe7, 0x3f, 0x40, 0x80, 0xcf, 0x80, 0x40, 0x88, 0x78, 0x02,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x08, 0xc6, 0x40, 0x03, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);

    let two_target = code.len();
    code.extend_from_slice(&[
        0x48, 0x89, 0xf9, 0x48, 0xc1, 0xe9, 0x06, 0x80, 0xc1, 0xc0, 0x88, 0x08,
    ]);
    code.extend_from_slice(&[
        0x40, 0x80, 0xe7, 0x3f, 0x40, 0x80, 0xcf, 0x80, 0x40, 0x88, 0x78, 0x01,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x08, 0xc6, 0x40, 0x02, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);

    let ascii_target = code.len();
    code.extend_from_slice(&[0x40, 0x88, 0x38, 0xc6, 0x40, 0x01, 0x00]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x18, 0xc3]);

    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x18, 0xc3]);
    patch_near_jump(code, invalid_negative, failure);
    patch_near_jump(code, invalid_large, failure);
    patch_near_jump(code, valid, valid_target);
    patch_near_jump(code, invalid_surrogate, failure);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, ascii, ascii_target);
    patch_near_jump(code, two_byte, two_target);
    patch_near_jump(code, three_byte, three_target);
}

fn emit_integer_to_string_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x50, 0x48, 0x89, 0xf8, 0x45, 0x31, 0xc0]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let positive = emit_near_jump_placeholder(code, 0x0f, 0x89);
    code.extend_from_slice(&[0x48, 0xf7, 0xd8, 0x41, 0xb0, 1]);
    let digits_start = code.len();
    code.extend_from_slice(&[0x48, 0x8d, 0x74, 0x24, 0x40, 0xc6, 0x06, 0x00]);
    code.extend_from_slice(&[0xb9, 0x0a, 0x00, 0x00, 0x00, 0x48, 0x85, 0xc0]);
    let nonzero = emit_short_jump_placeholder(code, 0x75);
    code.extend_from_slice(&[0x48, 0xff, 0xce, 0xc6, 0x06, b'0']);
    let sign = emit_short_jump_placeholder(code, 0xeb);
    let digit_loop = code.len();
    code.extend_from_slice(&[
        0x31, 0xd2, 0x48, 0xf7, 0xf1, 0x80, 0xc2, b'0', 0x48, 0xff, 0xce,
    ]);
    code.extend_from_slice(&[0x88, 0x16, 0x48, 0x85, 0xc0]);
    let more = emit_short_jump_placeholder(code, 0x75);
    let sign_target = code.len();
    code.extend_from_slice(&[0x45, 0x84, 0xc0]);
    let allocate = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xce, 0xc6, 0x06, b'-']);
    let allocate_target = code.len();
    code.extend_from_slice(&[
        0x48, 0x8d, 0x54, 0x24, 0x40, 0x48, 0x29, 0xf2, 0x48, 0xff, 0xc2,
    ]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08, 0x48, 0x89, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x89, 0xd7, 0x48, 0x83, 0xc7, 0x08, 0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[
        0x31, 0xff, 0xba, 0x03, 0x00, 0x00, 0x00, 0x41, 0xba, 0x22, 0x00, 0x00, 0x00,
    ]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05, 0x48, 0x85, 0xc0]);
    let failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x8b, 0x4c, 0x24, 0x10,
    ]);
    code.extend_from_slice(&[
        0x48, 0x8b, 0x74, 0x24, 0x08, 0x48, 0x89, 0xc7, 0xf3, 0xa4, 0x48, 0x83, 0xc4, 0x50, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x50, 0xc3]);
    patch_near_jump(code, positive, digits_start);
    patch_short_jump(code, nonzero, digit_loop);
    patch_short_jump(code, sign, sign_target);
    patch_short_jump(code, allocate, allocate_target);
    patch_short_jump(code, more, digit_loop);
    patch_near_jump(code, failed, failure);
}

fn emit_bool_to_string_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x28, 0x40, 0x84, 0xff]);
    let false_value = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[
        0xc7, 0x04, 0x24, 0x74, 0x72, 0x75, 0x65, 0xc6, 0x44, 0x24, 0x04, 0, 0xc6, 0x44, 0x24,
        0x05, 0,
    ]);
    let allocate = emit_short_jump_placeholder(code, 0xeb);
    let false_target = code.len();
    code.extend_from_slice(&[
        0xc7, 0x04, 0x24, 0x66, 0x61, 0x6c, 0x73, 0xc6, 0x44, 0x24, 0x04, 0x65, 0xc6, 0x44, 0x24,
        0x05, 0,
    ]);
    let allocate_target = code.len();
    code.extend_from_slice(&[
        0xbf, 0x06, 0, 0, 0, 0x48, 0x83, 0xc7, 0x08, 0x48, 0x89, 0xfe, 0x31, 0xff, 0xba, 0x03, 0,
        0, 0, 0x41, 0xba, 0x22, 0, 0, 0, 0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff, 0x45, 0x31,
        0xc9, 0xb8, 0x09, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0,
    ]);
    let failed = emit_short_jump_placeholder(code, 0x78);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0xc7, 0x48, 0x8d, 0x74, 0x24, 0x00,
        0xb9, 0x06, 0, 0, 0, 0xf3, 0xa4, 0x48, 0x83, 0xc4, 0x28, 0xc3,
    ]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x28, 0xc3]);
    patch_short_jump(code, false_value, false_target);
    patch_short_jump(code, allocate, allocate_target);
    patch_short_jump(code, failed, failure);
}

fn emit_string_clone_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0xfe]);
    code.extend_from_slice(&[0x31, 0xc0]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x06, 0x00]);
    let length_done = emit_short_jump_placeholder(code, 0x74);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, length_loop);
    let length_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    code.extend_from_slice(&[0x48, 0x89, 0xc6]);
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08]);
    code.extend_from_slice(&[0x31, 0xff]);
    code.extend_from_slice(&[0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9]);
    code.extend_from_slice(&[0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[
        0x48, 0x89, 0x30, 0x48, 0x83, 0xc0, 0x08, 0x48, 0x89, 0x44, 0x24, 0x10,
    ]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x00]);
    code.extend_from_slice(&[0x48, 0x8b, 0x7c, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x08]);
    code.extend_from_slice(&[0xf3, 0xa4]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_short_jump(code, length_done, length_target);
    patch_near_jump(code, allocation_failed, failure);
}

fn emit_string_slice_runtime(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x38]);
    code.extend_from_slice(&[0x48, 0x89, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x89, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x10]);
    code.extend_from_slice(&[0x48, 0x85, 0xff]);
    let null_source = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x31, 0xc0]);
    let length_loop = code.len();
    code.extend_from_slice(&[0x80, 0x3c, 0x07, 0x00]);
    let length_done = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0xff, 0xc0]);
    emit_short_jump_back(code, length_loop);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x39, 0xc1]);
    let start_out = emit_near_jump_placeholder(code, 0x0f, 0x83);
    code.extend_from_slice(&[0x48, 0x83, 0x7c, 0x24, 0x10, 0x00]);
    let zero_length = emit_near_jump_placeholder(code, 0x0f, 0x84);
    code.extend_from_slice(&[0x48, 0x29, 0xc8]);
    code.extend_from_slice(&[0x48, 0x39, 0xc2]);
    let clamp_length = emit_near_jump_placeholder(code, 0x0f, 0x87);
    let clamp_target = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0xc2]);
    let length_ready = code.len();
    code.extend_from_slice(&[0x48, 0x89, 0x54, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x89, 0xd6]);
    code.extend_from_slice(&[0x48, 0xff, 0xc6]);
    code.extend_from_slice(&[0x48, 0x83, 0xc6, 0x08]);
    code.extend_from_slice(&[0x31, 0xff, 0xba, 0x03, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x41, 0xba, 0x22, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x49, 0xc7, 0xc0, 0xff, 0xff, 0xff, 0xff]);
    code.extend_from_slice(&[0x45, 0x31, 0xc9, 0xb8, 0x09, 0x00, 0x00, 0x00, 0x0f, 0x05]);
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    let allocation_failed = emit_near_jump_placeholder(code, 0x0f, 0x88);
    code.extend_from_slice(&[0x48, 0x89, 0x30]);
    code.extend_from_slice(&[0x48, 0x83, 0xc0, 0x08]);
    code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x8b, 0x3c, 0x24]);
    code.extend_from_slice(&[0x48, 0x8b, 0x74, 0x24, 0x08]);
    code.extend_from_slice(&[0x48, 0x01, 0xfe]);
    code.extend_from_slice(&[0x48, 0x8b, 0x4c, 0x24, 0x20]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x89, 0xc7, 0xf3, 0xa4]);
    code.extend_from_slice(&[0x48, 0x8b, 0x44, 0x24, 0x28]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let zero_target = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x89, 0x44, 0x24, 0x20]);
    let zero_to_allocation = emit_near_unconditional_placeholder(code);
    let start_out_target = code.len();
    code.extend_from_slice(&[0x48, 0x8b, 0x04, 0x24, 0x48, 0x03, 0x44, 0x24, 0x18]);
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x38, 0xc3]);
    let failure = code.len();
    code.extend_from_slice(&[0x31, 0xc0, 0x48, 0x83, 0xc4, 0x38, 0xc3]);
    patch_near_jump(code, null_source, failure);
    patch_near_jump(code, length_done, length_ready);
    patch_near_jump(code, start_out, start_out_target);
    patch_near_jump(code, zero_length, zero_target);
    patch_near_jump(code, clamp_length, clamp_target);
    patch_near_jump(code, allocation_failed, failure);
    patch_near_jump(code, zero_to_allocation, length_ready);
}

fn emit_short_jump_placeholder(code: &mut Vec<u8>, opcode: u8) -> usize {
    code.push(opcode);
    let offset = code.len();
    code.push(0);
    offset
}

fn emit_near_jump_placeholder(code: &mut Vec<u8>, first: u8, second: u8) -> usize {
    code.extend_from_slice(&[first, second]);
    let offset = code.len();
    code.extend_from_slice(&[0, 0, 0, 0]);
    offset
}

fn patch_near_jump(code: &mut [u8], displacement: usize, target: usize) {
    let next = displacement + 4;
    let value = (target as isize - next as isize) as i32;
    code[displacement..displacement + 4].copy_from_slice(&value.to_le_bytes());
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
