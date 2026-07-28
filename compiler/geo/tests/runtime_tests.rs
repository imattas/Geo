use geo::ast::Type;
use geo::runtime::{c_runtime_path, functions_for_import, native_runtime};

#[test]
fn c_runtime_source_exists() {
    assert!(c_runtime_path().exists());
}

#[test]
fn native_runtime_is_compiler_owned_library_artifact() {
    let runtime = native_runtime();

    assert_eq!(runtime.name, "geo_native_runtime");
    assert!(runtime
        .source_path
        .ends_with("library/geo_runtime/geo_runtime.c"));
    assert!(runtime.source_path.exists());
}

#[test]
fn exposes_std_io_functions() {
    let path = vec!["std".to_string(), "io".to_string()];
    let functions = functions_for_import(&path).unwrap();

    assert!(functions.iter().any(|function| function.name == "println"));
    assert!(functions.iter().any(|function| {
        function.name == "read_line"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    for name in ["file_open_write", "file_open_append"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::Int);
    }
    assert!(functions.iter().any(|function| {
        function.name == "read_file"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "read_file_or"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "write_file"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "append_file"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "touch_file"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "truncate_file"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_exists"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_is_file"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_is_empty"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_is_dir"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "remove_file"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_size"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_modified_time"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_accessed_time"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "file_created_time"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "copy_file"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "copy_dir_all"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "rename_file"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "dir_exists"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "dir_entry_count"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "dir_entry_name"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "dir_entry_path"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "create_dir"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "create_dir_all"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "remove_dir"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "remove_dir_all"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
    let print = functions
        .iter()
        .find(|function| function.name == "print")
        .expect("print signature should exist");
    assert_eq!(print.params[0].ty, Type::String);
    assert_eq!(print.return_type, Type::Unit);
}

#[test]
fn exposes_runtime_symbols_for_libc_conflicting_functions() {
    let mem_path = vec!["std".to_string(), "mem".to_string()];
    let process_path = vec!["std".to_string(), "process".to_string()];

    let mem = functions_for_import(&mem_path).unwrap();
    let process = functions_for_import(&process_path).unwrap();

    assert_eq!(
        mem.iter()
            .find(|function| function.name == "free")
            .expect("free signature should exist")
            .symbol,
        "free_geo"
    );
    let alloc_zeroed = mem
        .iter()
        .find(|function| function.name == "alloc_zeroed")
        .expect("alloc_zeroed signature should exist");
    assert_eq!(alloc_zeroed.params.len(), 1);
    assert_eq!(alloc_zeroed.params[0].ty, Type::Usize);
    assert_eq!(alloc_zeroed.return_type, Type::Pointer(Box::new(Type::U8)));
    let alloc_array = mem
        .iter()
        .find(|function| function.name == "alloc_array")
        .expect("alloc_array signature should exist");
    assert_eq!(alloc_array.params.len(), 2);
    assert_eq!(alloc_array.params[0].ty, Type::Usize);
    assert_eq!(alloc_array.params[1].ty, Type::Usize);
    assert_eq!(alloc_array.return_type, Type::Pointer(Box::new(Type::U8)));
    let alloc_copy = mem
        .iter()
        .find(|function| function.name == "alloc_copy")
        .expect("alloc_copy signature should exist");
    assert_eq!(alloc_copy.params.len(), 2);
    assert_eq!(alloc_copy.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(alloc_copy.params[1].ty, Type::Usize);
    assert_eq!(alloc_copy.return_type, Type::Pointer(Box::new(Type::U8)));
    let realloc_array = mem
        .iter()
        .find(|function| function.name == "realloc_array")
        .expect("realloc_array signature should exist");
    assert_eq!(realloc_array.params.len(), 3);
    assert_eq!(
        realloc_array.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(realloc_array.params[1].ty, Type::Usize);
    assert_eq!(realloc_array.params[2].ty, Type::Usize);
    assert_eq!(realloc_array.return_type, Type::Pointer(Box::new(Type::U8)));
    let align_up = mem
        .iter()
        .find(|function| function.name == "align_up")
        .expect("align_up signature should exist");
    assert_eq!(align_up.params.len(), 2);
    assert_eq!(align_up.params[0].ty, Type::Usize);
    assert_eq!(align_up.params[1].ty, Type::Usize);
    assert_eq!(align_up.return_type, Type::Usize);
    let align_down = mem
        .iter()
        .find(|function| function.name == "align_down")
        .expect("align_down signature should exist");
    assert_eq!(align_down.params.len(), 2);
    assert_eq!(align_down.params[0].ty, Type::Usize);
    assert_eq!(align_down.params[1].ty, Type::Usize);
    assert_eq!(align_down.return_type, Type::Usize);
    let is_aligned = mem
        .iter()
        .find(|function| function.name == "is_aligned")
        .expect("is_aligned signature should exist");
    assert_eq!(is_aligned.params.len(), 2);
    assert_eq!(is_aligned.params[0].ty, Type::Usize);
    assert_eq!(is_aligned.params[1].ty, Type::Usize);
    assert_eq!(is_aligned.return_type, Type::Bool);
    let mem_compare = mem
        .iter()
        .find(|function| function.name == "mem_compare")
        .expect("mem_compare signature should exist");
    assert_eq!(mem_compare.params.len(), 3);
    assert_eq!(mem_compare.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_compare.params[1].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_compare.params[2].ty, Type::Usize);
    assert_eq!(mem_compare.return_type, Type::Int);
    let mem_equal = mem
        .iter()
        .find(|function| function.name == "mem_equal")
        .expect("mem_equal signature should exist");
    assert_eq!(mem_equal.params.len(), 3);
    assert_eq!(mem_equal.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_equal.params[1].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_equal.params[2].ty, Type::Usize);
    assert_eq!(mem_equal.return_type, Type::Bool);
    let mem_is_zero = mem
        .iter()
        .find(|function| function.name == "mem_is_zero")
        .expect("mem_is_zero signature should exist");
    assert_eq!(mem_is_zero.params.len(), 2);
    assert_eq!(mem_is_zero.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_is_zero.params[1].ty, Type::Usize);
    assert_eq!(mem_is_zero.return_type, Type::Bool);
    let mem_swap = mem
        .iter()
        .find(|function| function.name == "mem_swap")
        .expect("mem_swap signature should exist");
    assert_eq!(mem_swap.params.len(), 3);
    assert_eq!(mem_swap.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_swap.params[1].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_swap.params[2].ty, Type::Usize);
    assert_eq!(mem_swap.return_type, Type::Int);
    let mem_reverse = mem
        .iter()
        .find(|function| function.name == "mem_reverse")
        .expect("mem_reverse signature should exist");
    assert_eq!(mem_reverse.params.len(), 2);
    assert_eq!(mem_reverse.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_reverse.params[1].ty, Type::Usize);
    assert_eq!(mem_reverse.return_type, Type::Int);
    let mem_fill = mem
        .iter()
        .find(|function| function.name == "mem_fill")
        .expect("mem_fill signature should exist");
    assert_eq!(mem_fill.params.len(), 3);
    assert_eq!(mem_fill.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_fill.params[1].ty, Type::Usize);
    assert_eq!(mem_fill.params[2].ty, Type::U8);
    assert_eq!(mem_fill.return_type, Type::Int);
    let mem_replace_byte = mem
        .iter()
        .find(|function| function.name == "mem_replace_byte")
        .expect("mem_replace_byte signature should exist");
    assert_eq!(mem_replace_byte.params.len(), 4);
    assert_eq!(
        mem_replace_byte.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_replace_byte.params[1].ty, Type::Usize);
    assert_eq!(mem_replace_byte.params[2].ty, Type::U8);
    assert_eq!(mem_replace_byte.params[3].ty, Type::U8);
    assert_eq!(mem_replace_byte.return_type, Type::Usize);
    let mem_replace_pattern = mem
        .iter()
        .find(|function| function.name == "mem_replace_pattern")
        .expect("mem_replace_pattern signature should exist");
    assert_eq!(mem_replace_pattern.params.len(), 6);
    assert_eq!(
        mem_replace_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_replace_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_replace_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_replace_pattern.params[3].ty, Type::Usize);
    assert_eq!(
        mem_replace_pattern.params[4].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_replace_pattern.params[5].ty, Type::Usize);
    assert_eq!(mem_replace_pattern.return_type, Type::Usize);
    let mem_xor_byte = mem
        .iter()
        .find(|function| function.name == "mem_xor_byte")
        .expect("mem_xor_byte signature should exist");
    assert_eq!(mem_xor_byte.params.len(), 3);
    assert_eq!(mem_xor_byte.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_xor_byte.params[1].ty, Type::Usize);
    assert_eq!(mem_xor_byte.params[2].ty, Type::U8);
    assert_eq!(mem_xor_byte.return_type, Type::Int);
    let mem_repeat_pattern = mem
        .iter()
        .find(|function| function.name == "mem_repeat_pattern")
        .expect("mem_repeat_pattern signature should exist");
    assert_eq!(mem_repeat_pattern.params.len(), 4);
    assert_eq!(
        mem_repeat_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_repeat_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_repeat_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_repeat_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_repeat_pattern.return_type, Type::Int);
    let mem_rotate_left = mem
        .iter()
        .find(|function| function.name == "mem_rotate_left")
        .expect("mem_rotate_left signature should exist");
    assert_eq!(mem_rotate_left.params.len(), 3);
    assert_eq!(
        mem_rotate_left.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_rotate_left.params[1].ty, Type::Usize);
    assert_eq!(mem_rotate_left.params[2].ty, Type::Usize);
    assert_eq!(mem_rotate_left.return_type, Type::Int);
    let mem_rotate_right = mem
        .iter()
        .find(|function| function.name == "mem_rotate_right")
        .expect("mem_rotate_right signature should exist");
    assert_eq!(mem_rotate_right.params.len(), 3);
    assert_eq!(
        mem_rotate_right.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_rotate_right.params[1].ty, Type::Usize);
    assert_eq!(mem_rotate_right.params[2].ty, Type::Usize);
    assert_eq!(mem_rotate_right.return_type, Type::Int);
    let mem_move = mem
        .iter()
        .find(|function| function.name == "mem_move")
        .expect("mem_move signature should exist");
    assert_eq!(mem_move.params.len(), 3);
    assert_eq!(mem_move.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_move.params[1].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_move.params[2].ty, Type::Usize);
    assert_eq!(mem_move.return_type, Type::Int);
    let mem_copy = mem
        .iter()
        .find(|function| function.name == "mem_copy")
        .expect("mem_copy signature should exist");
    assert_eq!(mem_copy.params.len(), 3);
    assert_eq!(mem_copy.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_copy.params[1].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_copy.params[2].ty, Type::Usize);
    assert_eq!(mem_copy.return_type, Type::Int);
    let mem_find = mem
        .iter()
        .find(|function| function.name == "mem_find")
        .expect("mem_find signature should exist");
    assert_eq!(mem_find.params.len(), 3);
    assert_eq!(mem_find.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_find.params[1].ty, Type::Usize);
    assert_eq!(mem_find.params[2].ty, Type::U8);
    assert_eq!(mem_find.return_type, Type::Int);
    let mem_find_pattern = mem
        .iter()
        .find(|function| function.name == "mem_find_pattern")
        .expect("mem_find_pattern signature should exist");
    assert_eq!(mem_find_pattern.params.len(), 4);
    assert_eq!(
        mem_find_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_find_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_find_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_find_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_find_pattern.return_type, Type::Int);
    let mem_last_find = mem
        .iter()
        .find(|function| function.name == "mem_last_find")
        .expect("mem_last_find signature should exist");
    assert_eq!(mem_last_find.params.len(), 3);
    assert_eq!(
        mem_last_find.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_last_find.params[1].ty, Type::Usize);
    assert_eq!(mem_last_find.params[2].ty, Type::U8);
    assert_eq!(mem_last_find.return_type, Type::Int);
    let mem_last_find_pattern = mem
        .iter()
        .find(|function| function.name == "mem_last_find_pattern")
        .expect("mem_last_find_pattern signature should exist");
    assert_eq!(mem_last_find_pattern.params.len(), 4);
    assert_eq!(
        mem_last_find_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_last_find_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_last_find_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_last_find_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_last_find_pattern.return_type, Type::Int);
    let mem_count = mem
        .iter()
        .find(|function| function.name == "mem_count")
        .expect("mem_count signature should exist");
    assert_eq!(mem_count.params.len(), 3);
    assert_eq!(mem_count.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_count.params[1].ty, Type::Usize);
    assert_eq!(mem_count.params[2].ty, Type::U8);
    assert_eq!(mem_count.return_type, Type::Usize);
    let mem_count_pattern = mem
        .iter()
        .find(|function| function.name == "mem_count_pattern")
        .expect("mem_count_pattern signature should exist");
    assert_eq!(mem_count_pattern.params.len(), 4);
    assert_eq!(
        mem_count_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_count_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_count_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_count_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_count_pattern.return_type, Type::Usize);
    let mem_split_count = mem
        .iter()
        .find(|function| function.name == "mem_split_count")
        .expect("mem_split_count signature should exist");
    assert_eq!(mem_split_count.params.len(), 3);
    assert_eq!(
        mem_split_count.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_count.params[1].ty, Type::Usize);
    assert_eq!(mem_split_count.params[2].ty, Type::U8);
    assert_eq!(mem_split_count.return_type, Type::Usize);
    let mem_split_count_pattern = mem
        .iter()
        .find(|function| function.name == "mem_split_count_pattern")
        .expect("mem_split_count_pattern signature should exist");
    assert_eq!(mem_split_count_pattern.params.len(), 4);
    assert_eq!(
        mem_split_count_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_count_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_split_count_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_count_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_split_count_pattern.return_type, Type::Usize);
    let mem_split_field_start = mem
        .iter()
        .find(|function| function.name == "mem_split_field_start")
        .expect("mem_split_field_start signature should exist");
    assert_eq!(mem_split_field_start.params.len(), 4);
    assert_eq!(
        mem_split_field_start.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_start.params[1].ty, Type::Usize);
    assert_eq!(mem_split_field_start.params[2].ty, Type::U8);
    assert_eq!(mem_split_field_start.params[3].ty, Type::Usize);
    assert_eq!(mem_split_field_start.return_type, Type::Int);
    let mem_split_field_len = mem
        .iter()
        .find(|function| function.name == "mem_split_field_len")
        .expect("mem_split_field_len signature should exist");
    assert_eq!(mem_split_field_len.params.len(), 4);
    assert_eq!(
        mem_split_field_len.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_len.params[1].ty, Type::Usize);
    assert_eq!(mem_split_field_len.params[2].ty, Type::U8);
    assert_eq!(mem_split_field_len.params[3].ty, Type::Usize);
    assert_eq!(mem_split_field_len.return_type, Type::Usize);
    let mem_split_field_start_pattern = mem
        .iter()
        .find(|function| function.name == "mem_split_field_start_pattern")
        .expect("mem_split_field_start_pattern signature should exist");
    assert_eq!(mem_split_field_start_pattern.params.len(), 5);
    assert_eq!(
        mem_split_field_start_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_start_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_split_field_start_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_start_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_split_field_start_pattern.params[4].ty, Type::Usize);
    assert_eq!(mem_split_field_start_pattern.return_type, Type::Int);
    let mem_split_field_len_pattern = mem
        .iter()
        .find(|function| function.name == "mem_split_field_len_pattern")
        .expect("mem_split_field_len_pattern signature should exist");
    assert_eq!(mem_split_field_len_pattern.params.len(), 5);
    assert_eq!(
        mem_split_field_len_pattern.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_len_pattern.params[1].ty, Type::Usize);
    assert_eq!(
        mem_split_field_len_pattern.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_split_field_len_pattern.params[3].ty, Type::Usize);
    assert_eq!(mem_split_field_len_pattern.params[4].ty, Type::Usize);
    assert_eq!(mem_split_field_len_pattern.return_type, Type::Usize);
    let mem_line_count = mem
        .iter()
        .find(|function| function.name == "mem_line_count")
        .expect("mem_line_count signature should exist");
    assert_eq!(mem_line_count.params.len(), 2);
    assert_eq!(
        mem_line_count.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_line_count.params[1].ty, Type::Usize);
    assert_eq!(mem_line_count.return_type, Type::Usize);
    let mem_line_start = mem
        .iter()
        .find(|function| function.name == "mem_line_start")
        .expect("mem_line_start signature should exist");
    assert_eq!(mem_line_start.params.len(), 3);
    assert_eq!(
        mem_line_start.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_line_start.params[1].ty, Type::Usize);
    assert_eq!(mem_line_start.params[2].ty, Type::Usize);
    assert_eq!(mem_line_start.return_type, Type::Int);
    let mem_line_len = mem
        .iter()
        .find(|function| function.name == "mem_line_len")
        .expect("mem_line_len signature should exist");
    assert_eq!(mem_line_len.params.len(), 3);
    assert_eq!(mem_line_len.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_line_len.params[1].ty, Type::Usize);
    assert_eq!(mem_line_len.params[2].ty, Type::Usize);
    assert_eq!(mem_line_len.return_type, Type::Usize);
    let mem_line_index_at = mem
        .iter()
        .find(|function| function.name == "mem_line_index_at")
        .expect("mem_line_index_at signature should exist");
    assert_eq!(mem_line_index_at.params.len(), 3);
    assert_eq!(
        mem_line_index_at.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_line_index_at.params[1].ty, Type::Usize);
    assert_eq!(mem_line_index_at.params[2].ty, Type::Usize);
    assert_eq!(mem_line_index_at.return_type, Type::Int);
    let mem_column_at = mem
        .iter()
        .find(|function| function.name == "mem_column_at")
        .expect("mem_column_at signature should exist");
    assert_eq!(mem_column_at.params.len(), 3);
    assert_eq!(
        mem_column_at.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_column_at.params[1].ty, Type::Usize);
    assert_eq!(mem_column_at.params[2].ty, Type::Usize);
    assert_eq!(mem_column_at.return_type, Type::Int);
    let mem_offset_at_line_column = mem
        .iter()
        .find(|function| function.name == "mem_offset_at_line_column")
        .expect("mem_offset_at_line_column signature should exist");
    assert_eq!(mem_offset_at_line_column.params.len(), 4);
    assert_eq!(
        mem_offset_at_line_column.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_offset_at_line_column.params[1].ty, Type::Usize);
    assert_eq!(mem_offset_at_line_column.params[2].ty, Type::Usize);
    assert_eq!(mem_offset_at_line_column.params[3].ty, Type::Usize);
    assert_eq!(mem_offset_at_line_column.return_type, Type::Int);
    let mem_hash = mem
        .iter()
        .find(|function| function.name == "mem_hash")
        .expect("mem_hash signature should exist");
    assert_eq!(mem_hash.params.len(), 2);
    assert_eq!(mem_hash.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_hash.params[1].ty, Type::Usize);
    assert_eq!(mem_hash.return_type, Type::Usize);
    let mem_hash_seed = mem
        .iter()
        .find(|function| function.name == "mem_hash_seed")
        .expect("mem_hash_seed signature should exist");
    assert_eq!(mem_hash_seed.params.len(), 3);
    assert_eq!(
        mem_hash_seed.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_hash_seed.params[1].ty, Type::Usize);
    assert_eq!(mem_hash_seed.params[2].ty, Type::Usize);
    assert_eq!(mem_hash_seed.return_type, Type::Usize);
    let mem_contains = mem
        .iter()
        .find(|function| function.name == "mem_contains")
        .expect("mem_contains signature should exist");
    assert_eq!(mem_contains.params.len(), 3);
    assert_eq!(mem_contains.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_contains.params[1].ty, Type::Usize);
    assert_eq!(mem_contains.params[2].ty, Type::U8);
    assert_eq!(mem_contains.return_type, Type::Bool);
    let mem_all = mem
        .iter()
        .find(|function| function.name == "mem_all")
        .expect("mem_all signature should exist");
    assert_eq!(mem_all.params.len(), 3);
    assert_eq!(mem_all.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_all.params[1].ty, Type::Usize);
    assert_eq!(mem_all.params[2].ty, Type::U8);
    assert_eq!(mem_all.return_type, Type::Bool);
    let mem_any = mem
        .iter()
        .find(|function| function.name == "mem_any")
        .expect("mem_any signature should exist");
    assert_eq!(mem_any.params.len(), 3);
    assert_eq!(mem_any.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(mem_any.params[1].ty, Type::Usize);
    assert_eq!(mem_any.params[2].ty, Type::U8);
    assert_eq!(mem_any.return_type, Type::Bool);
    let mem_leading_count = mem
        .iter()
        .find(|function| function.name == "mem_leading_count")
        .expect("mem_leading_count signature should exist");
    assert_eq!(mem_leading_count.params.len(), 3);
    assert_eq!(
        mem_leading_count.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_leading_count.params[1].ty, Type::Usize);
    assert_eq!(mem_leading_count.params[2].ty, Type::U8);
    assert_eq!(mem_leading_count.return_type, Type::Usize);
    let mem_trailing_count = mem
        .iter()
        .find(|function| function.name == "mem_trailing_count")
        .expect("mem_trailing_count signature should exist");
    assert_eq!(mem_trailing_count.params.len(), 3);
    assert_eq!(
        mem_trailing_count.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_trailing_count.params[1].ty, Type::Usize);
    assert_eq!(mem_trailing_count.params[2].ty, Type::U8);
    assert_eq!(mem_trailing_count.return_type, Type::Usize);
    let mem_trimmed_len = mem
        .iter()
        .find(|function| function.name == "mem_trimmed_len")
        .expect("mem_trimmed_len signature should exist");
    assert_eq!(mem_trimmed_len.params.len(), 3);
    assert_eq!(
        mem_trimmed_len.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_trimmed_len.params[1].ty, Type::Usize);
    assert_eq!(mem_trimmed_len.params[2].ty, Type::U8);
    assert_eq!(mem_trimmed_len.return_type, Type::Usize);
    let mem_starts_with = mem
        .iter()
        .find(|function| function.name == "mem_starts_with")
        .expect("mem_starts_with signature should exist");
    assert_eq!(mem_starts_with.params.len(), 4);
    assert_eq!(
        mem_starts_with.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_starts_with.params[1].ty, Type::Usize);
    assert_eq!(
        mem_starts_with.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_starts_with.params[3].ty, Type::Usize);
    assert_eq!(mem_starts_with.return_type, Type::Bool);
    let mem_ends_with = mem
        .iter()
        .find(|function| function.name == "mem_ends_with")
        .expect("mem_ends_with signature should exist");
    assert_eq!(mem_ends_with.params.len(), 4);
    assert_eq!(
        mem_ends_with.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_ends_with.params[1].ty, Type::Usize);
    assert_eq!(
        mem_ends_with.params[2].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(mem_ends_with.params[3].ty, Type::Usize);
    assert_eq!(mem_ends_with.return_type, Type::Bool);
    assert_eq!(
        process
            .iter()
            .find(|function| function.name == "exit")
            .expect("exit signature should exist")
            .symbol,
        "exit_geo"
    );
    assert!(process
        .iter()
        .any(|function| function.name == "env_get" && function.return_type == Type::String));
    let env_get_or = process
        .iter()
        .find(|function| function.name == "env_get_or")
        .expect("env_get_or signature should exist");
    assert_eq!(env_get_or.params.len(), 2);
    assert_eq!(env_get_or.params[0].ty, Type::String);
    assert_eq!(env_get_or.params[1].ty, Type::String);
    assert_eq!(env_get_or.return_type, Type::String);
    assert!(process.iter().any(|function| {
        function.name == "env_exists"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(process.iter().any(|function| {
        function.name == "env_count"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(process.iter().any(|function| {
        function.name == "env_name"
            && function.params.len() == 1
            && function.params[0].ty == Type::Usize
            && function.return_type == Type::String
    }));
    assert!(process.iter().any(|function| {
        function.name == "env_value"
            && function.params.len() == 1
            && function.params[0].ty == Type::Usize
            && function.return_type == Type::String
    }));
    for name in ["env_set", "env_remove"] {
        let function = process
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.return_type, Type::Int);
    }
    let env_set = process
        .iter()
        .find(|function| function.name == "env_set")
        .expect("env_set signature should exist");
    assert_eq!(env_set.params.len(), 2);
    assert_eq!(env_set.params[0].ty, Type::String);
    assert_eq!(env_set.params[1].ty, Type::String);
    let env_remove = process
        .iter()
        .find(|function| function.name == "env_remove")
        .expect("env_remove signature should exist");
    assert_eq!(env_remove.params.len(), 1);
    assert_eq!(env_remove.params[0].ty, Type::String);
    assert!(process.iter().any(|function| {
        function.name == "arg_exists"
            && function.params.len() == 1
            && function.params[0].ty == Type::Int
            && function.return_type == Type::Bool
    }));
    let arg_or = process
        .iter()
        .find(|function| function.name == "arg_or")
        .expect("arg_or signature should exist");
    assert_eq!(arg_or.params.len(), 2);
    assert_eq!(arg_or.params[0].ty, Type::Int);
    assert_eq!(arg_or.params[1].ty, Type::String);
    assert_eq!(arg_or.return_type, Type::String);
    assert!(process.iter().any(|function| {
        function.name == "current_exe"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    assert!(process.iter().any(|function| {
        function.name == "process_id"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(process.iter().any(|function| {
        function.name == "run_command"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
}

#[test]
fn exposes_std_platform_functions() {
    let path = vec!["std".to_string(), "platform".to_string()];
    let functions = functions_for_import(&path).unwrap();

    assert!(functions
        .iter()
        .any(|function| function.name == "platform_os" && function.return_type == Type::String));
    assert!(functions.iter().any(|function| {
        function.name == "platform_arch" && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "platform_path_separator" && function.return_type == Type::Char
    }));
    assert!(functions.iter().any(|function| {
        function.name == "platform_newline" && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "temp_dir"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "home_dir"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "user_name"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "cpu_count"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_join"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_file_name"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_parent"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_extension"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_stem"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_is_absolute"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_without_extension"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_with_extension"
            && function.params.len() == 2
            && function.params[0].ty == Type::String
            && function.params[1].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "path_normalize"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    for name in ["path_to_unix", "path_to_windows"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }
    assert!(functions.iter().any(|function| {
        function.name == "path_absolute"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "current_dir"
            && function.params.is_empty()
            && function.return_type == Type::String
    }));
    assert!(functions.iter().any(|function| {
        function.name == "change_dir"
            && function.params.len() == 1
            && function.params[0].ty == Type::String
            && function.return_type == Type::Int
    }));
}

#[test]
fn exposes_std_array_functions() {
    let path = vec!["std".to_string(), "array".to_string()];
    let functions = functions_for_import(&path).unwrap();

    assert!(functions.iter().any(|function| {
        function.name == "array_new"
            && function.return_type == Type::Pointer(Box::new(Type::U8))
            && function.params[0].ty == Type::Usize
            && function.params[1].ty == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_clone"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_reserve"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions
        .iter()
        .any(|function| function.name == "array_len" && function.return_type == Type::Usize));
    assert!(functions.iter().any(|function| {
        function.name == "array_is_empty"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_capacity" && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_get"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_first"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_last"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_index_of"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_last_index_of"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_contains"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Bool
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_count"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_set"
            && function.params.len() == 3
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.params[2].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_fill"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_extend"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_copy"
            && function.params.len() == 5
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.params[2].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[3].ty == Type::Usize
            && function.params[4].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_resize"
            && function.params.len() == 3
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.params[2].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_insert"
            && function.params.len() == 3
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.params[2].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_swap"
            && function.params.len() == 3
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.params[2].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_reverse"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_truncate"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_remove"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_swap_remove"
            && function.params.len() == 2
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.params[1].ty == Type::Usize
            && function.return_type == Type::Int
    }));
    assert!(functions.iter().any(|function| {
        function.name == "array_pop_first"
            && function.params.len() == 1
            && function.params[0].ty == Type::Pointer(Box::new(Type::U8))
            && function.return_type == Type::Pointer(Box::new(Type::U8))
    }));
    assert!(functions
        .iter()
        .any(|function| function.name == "array_clear" && function.return_type == Type::Int));
    assert!(functions
        .iter()
        .any(|function| function.name == "array_free" && function.return_type == Type::Int));
}

#[test]
fn exposes_std_time_functions() {
    let path = vec!["std".to_string(), "time".to_string()];
    let functions = functions_for_import(&path).unwrap();

    assert!(functions.iter().any(|function| {
        function.name == "unix_time_secs"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "unix_time_millis"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "unix_time_micros"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "unix_time_nanos"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "monotonic_millis"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "monotonic_micros"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "monotonic_nanos"
            && function.params.is_empty()
            && function.return_type == Type::Usize
    }));
    assert!(functions.iter().any(|function| {
        function.name == "sleep_millis"
            && function.params.len() == 1
            && function.params[0].ty == Type::Usize
            && function.return_type == Type::Int
    }));
}

#[test]
fn exposes_std_math_integer_functions() {
    let path = vec!["std".to_string(), "math".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let abs = functions
        .iter()
        .find(|function| function.name == "int_abs")
        .expect("int_abs signature should exist");
    assert_eq!(abs.params.len(), 1);
    assert_eq!(abs.params[0].ty, Type::Int);
    assert_eq!(abs.return_type, Type::Int);

    let abs_diff = functions
        .iter()
        .find(|function| function.name == "int_abs_diff")
        .expect("int_abs_diff signature should exist");
    assert_eq!(abs_diff.params.len(), 2);
    assert_eq!(abs_diff.params[0].ty, Type::Int);
    assert_eq!(abs_diff.params[1].ty, Type::Int);
    assert_eq!(abs_diff.return_type, Type::Usize);

    for name in ["int_min", "int_max"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    let clamp = functions
        .iter()
        .find(|function| function.name == "int_clamp")
        .expect("int_clamp signature should exist");
    assert_eq!(clamp.params.len(), 3);
    assert_eq!(clamp.params[0].ty, Type::Int);
    assert_eq!(clamp.params[1].ty, Type::Int);
    assert_eq!(clamp.params[2].ty, Type::Int);
    assert_eq!(clamp.return_type, Type::Int);

    let int_div_floor = functions
        .iter()
        .find(|function| function.name == "int_div_floor")
        .expect("int_div_floor signature should exist");
    assert_eq!(int_div_floor.params.len(), 2);
    assert_eq!(int_div_floor.params[0].ty, Type::Int);
    assert_eq!(int_div_floor.params[1].ty, Type::Int);
    assert_eq!(int_div_floor.return_type, Type::Int);

    let int_div_ceil = functions
        .iter()
        .find(|function| function.name == "int_div_ceil")
        .expect("int_div_ceil signature should exist");
    assert_eq!(int_div_ceil.params.len(), 2);
    assert_eq!(int_div_ceil.params[0].ty, Type::Int);
    assert_eq!(int_div_ceil.params[1].ty, Type::Int);
    assert_eq!(int_div_ceil.return_type, Type::Int);

    let int_div_euclid = functions
        .iter()
        .find(|function| function.name == "int_div_euclid")
        .expect("int_div_euclid signature should exist");
    assert_eq!(int_div_euclid.params.len(), 2);
    assert_eq!(int_div_euclid.params[0].ty, Type::Int);
    assert_eq!(int_div_euclid.params[1].ty, Type::Int);
    assert_eq!(int_div_euclid.return_type, Type::Int);

    let int_rem_floor = functions
        .iter()
        .find(|function| function.name == "int_rem_floor")
        .expect("int_rem_floor signature should exist");
    assert_eq!(int_rem_floor.params.len(), 2);
    assert_eq!(int_rem_floor.params[0].ty, Type::Int);
    assert_eq!(int_rem_floor.params[1].ty, Type::Int);
    assert_eq!(int_rem_floor.return_type, Type::Int);

    let int_rem_euclid = functions
        .iter()
        .find(|function| function.name == "int_rem_euclid")
        .expect("int_rem_euclid signature should exist");
    assert_eq!(int_rem_euclid.params.len(), 2);
    assert_eq!(int_rem_euclid.params[0].ty, Type::Int);
    assert_eq!(int_rem_euclid.params[1].ty, Type::Int);
    assert_eq!(int_rem_euclid.return_type, Type::Int);

    let int_checked_add = functions
        .iter()
        .find(|function| function.name == "int_checked_add")
        .expect("int_checked_add signature should exist");
    assert_eq!(int_checked_add.params.len(), 2);
    assert_eq!(int_checked_add.params[0].ty, Type::Int);
    assert_eq!(int_checked_add.params[1].ty, Type::Int);
    assert_eq!(int_checked_add.return_type, Type::Int);

    let int_checked_sub = functions
        .iter()
        .find(|function| function.name == "int_checked_sub")
        .expect("int_checked_sub signature should exist");
    assert_eq!(int_checked_sub.params.len(), 2);
    assert_eq!(int_checked_sub.params[0].ty, Type::Int);
    assert_eq!(int_checked_sub.params[1].ty, Type::Int);
    assert_eq!(int_checked_sub.return_type, Type::Int);

    let int_checked_mul = functions
        .iter()
        .find(|function| function.name == "int_checked_mul")
        .expect("int_checked_mul signature should exist");
    assert_eq!(int_checked_mul.params.len(), 2);
    assert_eq!(int_checked_mul.params[0].ty, Type::Int);
    assert_eq!(int_checked_mul.params[1].ty, Type::Int);
    assert_eq!(int_checked_mul.return_type, Type::Int);

    let int_checked_div = functions
        .iter()
        .find(|function| function.name == "int_checked_div")
        .expect("int_checked_div signature should exist");
    assert_eq!(int_checked_div.params.len(), 2);
    assert_eq!(int_checked_div.params[0].ty, Type::Int);
    assert_eq!(int_checked_div.params[1].ty, Type::Int);
    assert_eq!(int_checked_div.return_type, Type::Int);

    let int_checked_rem = functions
        .iter()
        .find(|function| function.name == "int_checked_rem")
        .expect("int_checked_rem signature should exist");
    assert_eq!(int_checked_rem.params.len(), 2);
    assert_eq!(int_checked_rem.params[0].ty, Type::Int);
    assert_eq!(int_checked_rem.params[1].ty, Type::Int);
    assert_eq!(int_checked_rem.return_type, Type::Int);

    let int_checked_neg = functions
        .iter()
        .find(|function| function.name == "int_checked_neg")
        .expect("int_checked_neg signature should exist");
    assert_eq!(int_checked_neg.params.len(), 1);
    assert_eq!(int_checked_neg.params[0].ty, Type::Int);
    assert_eq!(int_checked_neg.return_type, Type::Int);

    let int_checked_abs = functions
        .iter()
        .find(|function| function.name == "int_checked_abs")
        .expect("int_checked_abs signature should exist");
    assert_eq!(int_checked_abs.params.len(), 1);
    assert_eq!(int_checked_abs.params[0].ty, Type::Int);
    assert_eq!(int_checked_abs.return_type, Type::Int);

    for name in [
        "int_saturating_add",
        "int_saturating_sub",
        "int_saturating_mul",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    let int_saturating_abs = functions
        .iter()
        .find(|function| function.name == "int_saturating_abs")
        .expect("int_saturating_abs signature should exist");
    assert_eq!(int_saturating_abs.params.len(), 1);
    assert_eq!(int_saturating_abs.params[0].ty, Type::Int);
    assert_eq!(int_saturating_abs.return_type, Type::Int);

    let int_saturating_neg = functions
        .iter()
        .find(|function| function.name == "int_saturating_neg")
        .expect("int_saturating_neg signature should exist");
    assert_eq!(int_saturating_neg.params.len(), 1);
    assert_eq!(int_saturating_neg.params[0].ty, Type::Int);
    assert_eq!(int_saturating_neg.return_type, Type::Int);

    for name in ["int_wrapping_add", "int_wrapping_sub", "int_wrapping_mul"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    let int_wrapping_neg = functions
        .iter()
        .find(|function| function.name == "int_wrapping_neg")
        .expect("int_wrapping_neg signature should exist");
    assert_eq!(int_wrapping_neg.params.len(), 1);
    assert_eq!(int_wrapping_neg.params[0].ty, Type::Int);
    assert_eq!(int_wrapping_neg.return_type, Type::Int);

    let int_wrapping_abs = functions
        .iter()
        .find(|function| function.name == "int_wrapping_abs")
        .expect("int_wrapping_abs signature should exist");
    assert_eq!(int_wrapping_abs.params.len(), 1);
    assert_eq!(int_wrapping_abs.params[0].ty, Type::Int);
    assert_eq!(int_wrapping_abs.return_type, Type::Int);

    for name in ["usize_min", "usize_max"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    let usize_clamp = functions
        .iter()
        .find(|function| function.name == "usize_clamp")
        .expect("usize_clamp signature should exist");
    assert_eq!(usize_clamp.params.len(), 3);
    assert_eq!(usize_clamp.params[0].ty, Type::Usize);
    assert_eq!(usize_clamp.params[1].ty, Type::Usize);
    assert_eq!(usize_clamp.params[2].ty, Type::Usize);
    assert_eq!(usize_clamp.return_type, Type::Usize);

    let usize_abs_diff = functions
        .iter()
        .find(|function| function.name == "usize_abs_diff")
        .expect("usize_abs_diff signature should exist");
    assert_eq!(usize_abs_diff.params.len(), 2);
    assert_eq!(usize_abs_diff.params[0].ty, Type::Usize);
    assert_eq!(usize_abs_diff.params[1].ty, Type::Usize);
    assert_eq!(usize_abs_diff.return_type, Type::Usize);

    for name in [
        "usize_checked_add",
        "usize_checked_sub",
        "usize_checked_mul",
        "usize_checked_div",
        "usize_checked_rem",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    for name in [
        "usize_saturating_add",
        "usize_saturating_sub",
        "usize_saturating_mul",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    for name in [
        "usize_wrapping_add",
        "usize_wrapping_sub",
        "usize_wrapping_mul",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    let int_pow = functions
        .iter()
        .find(|function| function.name == "int_pow")
        .expect("int_pow signature should exist");
    assert_eq!(int_pow.params.len(), 2);
    assert_eq!(int_pow.params[0].ty, Type::Int);
    assert_eq!(int_pow.params[1].ty, Type::Usize);
    assert_eq!(int_pow.return_type, Type::Int);

    let int_checked_pow = functions
        .iter()
        .find(|function| function.name == "int_checked_pow")
        .expect("int_checked_pow signature should exist");
    assert_eq!(int_checked_pow.params.len(), 2);
    assert_eq!(int_checked_pow.params[0].ty, Type::Int);
    assert_eq!(int_checked_pow.params[1].ty, Type::Usize);
    assert_eq!(int_checked_pow.return_type, Type::Int);

    let int_saturating_pow = functions
        .iter()
        .find(|function| function.name == "int_saturating_pow")
        .expect("int_saturating_pow signature should exist");
    assert_eq!(int_saturating_pow.params.len(), 2);
    assert_eq!(int_saturating_pow.params[0].ty, Type::Int);
    assert_eq!(int_saturating_pow.params[1].ty, Type::Usize);
    assert_eq!(int_saturating_pow.return_type, Type::Int);

    let int_wrapping_pow = functions
        .iter()
        .find(|function| function.name == "int_wrapping_pow")
        .expect("int_wrapping_pow signature should exist");
    assert_eq!(int_wrapping_pow.params.len(), 2);
    assert_eq!(int_wrapping_pow.params[0].ty, Type::Int);
    assert_eq!(int_wrapping_pow.params[1].ty, Type::Usize);
    assert_eq!(int_wrapping_pow.return_type, Type::Int);

    let usize_pow = functions
        .iter()
        .find(|function| function.name == "usize_pow")
        .expect("usize_pow signature should exist");
    assert_eq!(usize_pow.params.len(), 2);
    assert_eq!(usize_pow.params[0].ty, Type::Usize);
    assert_eq!(usize_pow.params[1].ty, Type::Usize);
    assert_eq!(usize_pow.return_type, Type::Usize);

    let usize_checked_pow = functions
        .iter()
        .find(|function| function.name == "usize_checked_pow")
        .expect("usize_checked_pow signature should exist");
    assert_eq!(usize_checked_pow.params.len(), 2);
    assert_eq!(usize_checked_pow.params[0].ty, Type::Usize);
    assert_eq!(usize_checked_pow.params[1].ty, Type::Usize);
    assert_eq!(usize_checked_pow.return_type, Type::Usize);

    let usize_saturating_pow = functions
        .iter()
        .find(|function| function.name == "usize_saturating_pow")
        .expect("usize_saturating_pow signature should exist");
    assert_eq!(usize_saturating_pow.params.len(), 2);
    assert_eq!(usize_saturating_pow.params[0].ty, Type::Usize);
    assert_eq!(usize_saturating_pow.params[1].ty, Type::Usize);
    assert_eq!(usize_saturating_pow.return_type, Type::Usize);

    let usize_wrapping_pow = functions
        .iter()
        .find(|function| function.name == "usize_wrapping_pow")
        .expect("usize_wrapping_pow signature should exist");
    assert_eq!(usize_wrapping_pow.params.len(), 2);
    assert_eq!(usize_wrapping_pow.params[0].ty, Type::Usize);
    assert_eq!(usize_wrapping_pow.params[1].ty, Type::Usize);
    assert_eq!(usize_wrapping_pow.return_type, Type::Usize);

    for name in ["int_gcd", "int_lcm"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    for name in ["usize_gcd", "usize_lcm"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    for name in ["int_is_even", "int_is_odd"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Bool);
    }

    let int_is_power_of_two = functions
        .iter()
        .find(|function| function.name == "int_is_power_of_two")
        .expect("int_is_power_of_two signature should exist");
    assert_eq!(int_is_power_of_two.params.len(), 1);
    assert_eq!(int_is_power_of_two.params[0].ty, Type::Int);
    assert_eq!(int_is_power_of_two.return_type, Type::Bool);

    let int_prev_power_of_two = functions
        .iter()
        .find(|function| function.name == "int_prev_power_of_two")
        .expect("int_prev_power_of_two signature should exist");
    assert_eq!(int_prev_power_of_two.params.len(), 1);
    assert_eq!(int_prev_power_of_two.params[0].ty, Type::Int);
    assert_eq!(int_prev_power_of_two.return_type, Type::Int);

    let int_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "int_next_power_of_two")
        .expect("int_next_power_of_two signature should exist");
    assert_eq!(int_next_power_of_two.params.len(), 1);
    assert_eq!(int_next_power_of_two.params[0].ty, Type::Int);
    assert_eq!(int_next_power_of_two.return_type, Type::Int);

    let int_checked_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "int_checked_next_power_of_two")
        .expect("int_checked_next_power_of_two signature should exist");
    assert_eq!(int_checked_next_power_of_two.params.len(), 1);
    assert_eq!(int_checked_next_power_of_two.params[0].ty, Type::Int);
    assert_eq!(int_checked_next_power_of_two.return_type, Type::Int);

    let int_saturating_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "int_saturating_next_power_of_two")
        .expect("int_saturating_next_power_of_two signature should exist");
    assert_eq!(int_saturating_next_power_of_two.params.len(), 1);
    assert_eq!(int_saturating_next_power_of_two.params[0].ty, Type::Int);
    assert_eq!(int_saturating_next_power_of_two.return_type, Type::Int);

    for name in ["usize_is_even", "usize_is_odd"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Bool);
    }

    let usize_is_power_of_two = functions
        .iter()
        .find(|function| function.name == "usize_is_power_of_two")
        .expect("usize_is_power_of_two signature should exist");
    assert_eq!(usize_is_power_of_two.params.len(), 1);
    assert_eq!(usize_is_power_of_two.params[0].ty, Type::Usize);
    assert_eq!(usize_is_power_of_two.return_type, Type::Bool);

    let usize_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "usize_next_power_of_two")
        .expect("usize_next_power_of_two signature should exist");
    assert_eq!(usize_next_power_of_two.params.len(), 1);
    assert_eq!(usize_next_power_of_two.params[0].ty, Type::Usize);
    assert_eq!(usize_next_power_of_two.return_type, Type::Usize);

    let usize_checked_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "usize_checked_next_power_of_two")
        .expect("usize_checked_next_power_of_two signature should exist");
    assert_eq!(usize_checked_next_power_of_two.params.len(), 1);
    assert_eq!(usize_checked_next_power_of_two.params[0].ty, Type::Usize);
    assert_eq!(usize_checked_next_power_of_two.return_type, Type::Usize);

    let usize_saturating_next_power_of_two = functions
        .iter()
        .find(|function| function.name == "usize_saturating_next_power_of_two")
        .expect("usize_saturating_next_power_of_two signature should exist");
    assert_eq!(usize_saturating_next_power_of_two.params.len(), 1);
    assert_eq!(usize_saturating_next_power_of_two.params[0].ty, Type::Usize);
    assert_eq!(usize_saturating_next_power_of_two.return_type, Type::Usize);

    let usize_prev_power_of_two = functions
        .iter()
        .find(|function| function.name == "usize_prev_power_of_two")
        .expect("usize_prev_power_of_two signature should exist");
    assert_eq!(usize_prev_power_of_two.params.len(), 1);
    assert_eq!(usize_prev_power_of_two.params[0].ty, Type::Usize);
    assert_eq!(usize_prev_power_of_two.return_type, Type::Usize);

    let usize_align_up = functions
        .iter()
        .find(|function| function.name == "usize_align_up")
        .expect("usize_align_up signature should exist");
    assert_eq!(usize_align_up.params.len(), 2);
    assert_eq!(usize_align_up.params[0].ty, Type::Usize);
    assert_eq!(usize_align_up.params[1].ty, Type::Usize);
    assert_eq!(usize_align_up.return_type, Type::Usize);

    let int_align_up = functions
        .iter()
        .find(|function| function.name == "int_align_up")
        .expect("int_align_up signature should exist");
    assert_eq!(int_align_up.params.len(), 2);
    assert_eq!(int_align_up.params[0].ty, Type::Int);
    assert_eq!(int_align_up.params[1].ty, Type::Int);
    assert_eq!(int_align_up.return_type, Type::Int);

    let usize_align_down = functions
        .iter()
        .find(|function| function.name == "usize_align_down")
        .expect("usize_align_down signature should exist");
    assert_eq!(usize_align_down.params.len(), 2);
    assert_eq!(usize_align_down.params[0].ty, Type::Usize);
    assert_eq!(usize_align_down.params[1].ty, Type::Usize);
    assert_eq!(usize_align_down.return_type, Type::Usize);

    let int_align_down = functions
        .iter()
        .find(|function| function.name == "int_align_down")
        .expect("int_align_down signature should exist");
    assert_eq!(int_align_down.params.len(), 2);
    assert_eq!(int_align_down.params[0].ty, Type::Int);
    assert_eq!(int_align_down.params[1].ty, Type::Int);
    assert_eq!(int_align_down.return_type, Type::Int);

    let int_align_up_saturating = functions
        .iter()
        .find(|function| function.name == "int_align_up_saturating")
        .expect("int_align_up_saturating signature should exist");
    assert_eq!(int_align_up_saturating.params.len(), 2);
    assert_eq!(int_align_up_saturating.params[0].ty, Type::Int);
    assert_eq!(int_align_up_saturating.params[1].ty, Type::Int);
    assert_eq!(int_align_up_saturating.return_type, Type::Int);

    let usize_align_up_saturating = functions
        .iter()
        .find(|function| function.name == "usize_align_up_saturating")
        .expect("usize_align_up_saturating signature should exist");
    assert_eq!(usize_align_up_saturating.params.len(), 2);
    assert_eq!(usize_align_up_saturating.params[0].ty, Type::Usize);
    assert_eq!(usize_align_up_saturating.params[1].ty, Type::Usize);
    assert_eq!(usize_align_up_saturating.return_type, Type::Usize);

    let usize_div_ceil = functions
        .iter()
        .find(|function| function.name == "usize_div_ceil")
        .expect("usize_div_ceil signature should exist");
    assert_eq!(usize_div_ceil.params.len(), 2);
    assert_eq!(usize_div_ceil.params[0].ty, Type::Usize);
    assert_eq!(usize_div_ceil.params[1].ty, Type::Usize);
    assert_eq!(usize_div_ceil.return_type, Type::Usize);

    let signum = functions
        .iter()
        .find(|function| function.name == "int_signum")
        .expect("int_signum signature should exist");
    assert_eq!(signum.params.len(), 1);
    assert_eq!(signum.params[0].ty, Type::Int);
    assert_eq!(signum.return_type, Type::Int);

    for name in ["int_is_positive", "int_is_negative"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Bool);
    }
}

#[test]
fn exposes_std_bits_functions() {
    let path = vec!["std".to_string(), "bits".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let int_popcount = functions
        .iter()
        .find(|function| function.name == "int_popcount")
        .expect("int_popcount signature should exist");
    assert_eq!(int_popcount.params.len(), 1);
    assert_eq!(int_popcount.params[0].ty, Type::Int);
    assert_eq!(int_popcount.return_type, Type::Usize);

    let int_count_ones = functions
        .iter()
        .find(|function| function.name == "int_count_ones")
        .expect("int_count_ones signature should exist");
    assert_eq!(int_count_ones.params.len(), 1);
    assert_eq!(int_count_ones.params[0].ty, Type::Int);
    assert_eq!(int_count_ones.return_type, Type::Usize);

    let int_parity = functions
        .iter()
        .find(|function| function.name == "int_parity")
        .expect("int_parity signature should exist");
    assert_eq!(int_parity.params.len(), 1);
    assert_eq!(int_parity.params[0].ty, Type::Int);
    assert_eq!(int_parity.return_type, Type::Bool);

    let int_count_zeros = functions
        .iter()
        .find(|function| function.name == "int_count_zeros")
        .expect("int_count_zeros signature should exist");
    assert_eq!(int_count_zeros.params.len(), 1);
    assert_eq!(int_count_zeros.params[0].ty, Type::Int);
    assert_eq!(int_count_zeros.return_type, Type::Usize);

    let int_leading_zeros = functions
        .iter()
        .find(|function| function.name == "int_leading_zeros")
        .expect("int_leading_zeros signature should exist");
    assert_eq!(int_leading_zeros.params.len(), 1);
    assert_eq!(int_leading_zeros.params[0].ty, Type::Int);
    assert_eq!(int_leading_zeros.return_type, Type::Usize);

    let int_leading_ones = functions
        .iter()
        .find(|function| function.name == "int_leading_ones")
        .expect("int_leading_ones signature should exist");
    assert_eq!(int_leading_ones.params.len(), 1);
    assert_eq!(int_leading_ones.params[0].ty, Type::Int);
    assert_eq!(int_leading_ones.return_type, Type::Usize);

    let int_trailing_zeros = functions
        .iter()
        .find(|function| function.name == "int_trailing_zeros")
        .expect("int_trailing_zeros signature should exist");
    assert_eq!(int_trailing_zeros.params.len(), 1);
    assert_eq!(int_trailing_zeros.params[0].ty, Type::Int);
    assert_eq!(int_trailing_zeros.return_type, Type::Usize);

    let int_trailing_ones = functions
        .iter()
        .find(|function| function.name == "int_trailing_ones")
        .expect("int_trailing_ones signature should exist");
    assert_eq!(int_trailing_ones.params.len(), 1);
    assert_eq!(int_trailing_ones.params[0].ty, Type::Int);
    assert_eq!(int_trailing_ones.return_type, Type::Usize);

    let int_reverse_bits = functions
        .iter()
        .find(|function| function.name == "int_reverse_bits")
        .expect("int_reverse_bits signature should exist");
    assert_eq!(int_reverse_bits.params.len(), 1);
    assert_eq!(int_reverse_bits.params[0].ty, Type::Int);
    assert_eq!(int_reverse_bits.return_type, Type::Int);

    let int_swap_bytes = functions
        .iter()
        .find(|function| function.name == "int_swap_bytes")
        .expect("int_swap_bytes signature should exist");
    assert_eq!(int_swap_bytes.params.len(), 1);
    assert_eq!(int_swap_bytes.params[0].ty, Type::Int);
    assert_eq!(int_swap_bytes.return_type, Type::Int);

    for name in ["int_from_be", "int_from_le", "int_to_be", "int_to_le"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    let int_bit_width = functions
        .iter()
        .find(|function| function.name == "int_bit_width")
        .expect("int_bit_width signature should exist");
    assert_eq!(int_bit_width.params.len(), 1);
    assert_eq!(int_bit_width.params[0].ty, Type::Int);
    assert_eq!(int_bit_width.return_type, Type::Usize);

    for name in [
        "int_lowest_one",
        "int_highest_one",
        "int_clear_lowest_one",
        "int_clear_highest_one",
        "int_fill_ones_below",
        "int_fill_ones_above",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    let int_bit_is_set = functions
        .iter()
        .find(|function| function.name == "int_bit_is_set")
        .expect("int_bit_is_set signature should exist");
    assert_eq!(int_bit_is_set.params.len(), 2);
    assert_eq!(int_bit_is_set.params[0].ty, Type::Int);
    assert_eq!(int_bit_is_set.params[1].ty, Type::Usize);
    assert_eq!(int_bit_is_set.return_type, Type::Bool);

    for name in ["int_bits_contains_all", "int_bits_disjoint"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Int);
        assert_eq!(function.return_type, Type::Bool);
    }

    let int_bit_set = functions
        .iter()
        .find(|function| function.name == "int_bit_set")
        .expect("int_bit_set signature should exist");
    assert_eq!(int_bit_set.params.len(), 2);
    assert_eq!(int_bit_set.params[0].ty, Type::Int);
    assert_eq!(int_bit_set.params[1].ty, Type::Usize);
    assert_eq!(int_bit_set.return_type, Type::Int);

    let int_low_mask = functions
        .iter()
        .find(|function| function.name == "int_low_mask")
        .expect("int_low_mask signature should exist");
    assert_eq!(int_low_mask.params.len(), 1);
    assert_eq!(int_low_mask.params[0].ty, Type::Usize);
    assert_eq!(int_low_mask.return_type, Type::Int);

    let int_range_mask = functions
        .iter()
        .find(|function| function.name == "int_range_mask")
        .expect("int_range_mask signature should exist");
    assert_eq!(int_range_mask.params.len(), 2);
    assert_eq!(int_range_mask.params[0].ty, Type::Usize);
    assert_eq!(int_range_mask.params[1].ty, Type::Usize);
    assert_eq!(int_range_mask.return_type, Type::Int);

    let int_sign_extend = functions
        .iter()
        .find(|function| function.name == "int_sign_extend")
        .expect("int_sign_extend signature should exist");
    assert_eq!(int_sign_extend.params.len(), 2);
    assert_eq!(int_sign_extend.params[0].ty, Type::Int);
    assert_eq!(int_sign_extend.params[1].ty, Type::Usize);
    assert_eq!(int_sign_extend.return_type, Type::Int);

    let int_extract_bits = functions
        .iter()
        .find(|function| function.name == "int_extract_bits")
        .expect("int_extract_bits signature should exist");
    assert_eq!(int_extract_bits.params.len(), 3);
    assert_eq!(int_extract_bits.params[0].ty, Type::Int);
    assert_eq!(int_extract_bits.params[1].ty, Type::Usize);
    assert_eq!(int_extract_bits.params[2].ty, Type::Usize);
    assert_eq!(int_extract_bits.return_type, Type::Int);

    let int_insert_bits = functions
        .iter()
        .find(|function| function.name == "int_insert_bits")
        .expect("int_insert_bits signature should exist");
    assert_eq!(int_insert_bits.params.len(), 4);
    assert_eq!(int_insert_bits.params[0].ty, Type::Int);
    assert_eq!(int_insert_bits.params[1].ty, Type::Int);
    assert_eq!(int_insert_bits.params[2].ty, Type::Usize);
    assert_eq!(int_insert_bits.params[3].ty, Type::Usize);
    assert_eq!(int_insert_bits.return_type, Type::Int);

    let int_byte_at = functions
        .iter()
        .find(|function| function.name == "int_byte_at")
        .expect("int_byte_at signature should exist");
    assert_eq!(int_byte_at.params.len(), 2);
    assert_eq!(int_byte_at.params[0].ty, Type::Int);
    assert_eq!(int_byte_at.params[1].ty, Type::Usize);
    assert_eq!(int_byte_at.return_type, Type::U8);

    let int_with_byte = functions
        .iter()
        .find(|function| function.name == "int_with_byte")
        .expect("int_with_byte signature should exist");
    assert_eq!(int_with_byte.params.len(), 3);
    assert_eq!(int_with_byte.params[0].ty, Type::Int);
    assert_eq!(int_with_byte.params[1].ty, Type::Usize);
    assert_eq!(int_with_byte.params[2].ty, Type::U8);
    assert_eq!(int_with_byte.return_type, Type::Int);

    let int_bit_clear = functions
        .iter()
        .find(|function| function.name == "int_bit_clear")
        .expect("int_bit_clear signature should exist");
    assert_eq!(int_bit_clear.params.len(), 2);
    assert_eq!(int_bit_clear.params[0].ty, Type::Int);
    assert_eq!(int_bit_clear.params[1].ty, Type::Usize);
    assert_eq!(int_bit_clear.return_type, Type::Int);

    let int_bit_toggle = functions
        .iter()
        .find(|function| function.name == "int_bit_toggle")
        .expect("int_bit_toggle signature should exist");
    assert_eq!(int_bit_toggle.params.len(), 2);
    assert_eq!(int_bit_toggle.params[0].ty, Type::Int);
    assert_eq!(int_bit_toggle.params[1].ty, Type::Usize);
    assert_eq!(int_bit_toggle.return_type, Type::Int);

    for name in [
        "usize_popcount",
        "usize_count_ones",
        "usize_count_zeros",
        "usize_leading_zeros",
        "usize_leading_ones",
        "usize_trailing_zeros",
        "usize_trailing_ones",
        "usize_reverse_bits",
        "usize_swap_bytes",
        "usize_from_be",
        "usize_from_le",
        "usize_to_be",
        "usize_to_le",
        "usize_bit_width",
        "usize_lowest_one",
        "usize_highest_one",
        "usize_clear_lowest_one",
        "usize_clear_highest_one",
        "usize_fill_ones_below",
        "usize_fill_ones_above",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    let usize_parity = functions
        .iter()
        .find(|function| function.name == "usize_parity")
        .expect("usize_parity signature should exist");
    assert_eq!(usize_parity.params.len(), 1);
    assert_eq!(usize_parity.params[0].ty, Type::Usize);
    assert_eq!(usize_parity.return_type, Type::Bool);

    for name in [
        "usize_rotate_left",
        "usize_rotate_right",
        "usize_checked_shl",
        "usize_checked_shr",
        "usize_wrapping_shl",
        "usize_wrapping_shr",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Usize);
    }

    let usize_bit_is_set = functions
        .iter()
        .find(|function| function.name == "usize_bit_is_set")
        .expect("usize_bit_is_set signature should exist");
    assert_eq!(usize_bit_is_set.params.len(), 2);
    assert_eq!(usize_bit_is_set.params[0].ty, Type::Usize);
    assert_eq!(usize_bit_is_set.params[1].ty, Type::Usize);
    assert_eq!(usize_bit_is_set.return_type, Type::Bool);

    for name in ["usize_bits_contains_all", "usize_bits_disjoint"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Usize);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Bool);
    }

    let usize_bit_set = functions
        .iter()
        .find(|function| function.name == "usize_bit_set")
        .expect("usize_bit_set signature should exist");
    assert_eq!(usize_bit_set.params.len(), 2);
    assert_eq!(usize_bit_set.params[0].ty, Type::Usize);
    assert_eq!(usize_bit_set.params[1].ty, Type::Usize);
    assert_eq!(usize_bit_set.return_type, Type::Usize);

    let usize_low_mask = functions
        .iter()
        .find(|function| function.name == "usize_low_mask")
        .expect("usize_low_mask signature should exist");
    assert_eq!(usize_low_mask.params.len(), 1);
    assert_eq!(usize_low_mask.params[0].ty, Type::Usize);
    assert_eq!(usize_low_mask.return_type, Type::Usize);

    let usize_range_mask = functions
        .iter()
        .find(|function| function.name == "usize_range_mask")
        .expect("usize_range_mask signature should exist");
    assert_eq!(usize_range_mask.params.len(), 2);
    assert_eq!(usize_range_mask.params[0].ty, Type::Usize);
    assert_eq!(usize_range_mask.params[1].ty, Type::Usize);
    assert_eq!(usize_range_mask.return_type, Type::Usize);

    let usize_extract_bits = functions
        .iter()
        .find(|function| function.name == "usize_extract_bits")
        .expect("usize_extract_bits signature should exist");
    assert_eq!(usize_extract_bits.params.len(), 3);
    assert_eq!(usize_extract_bits.params[0].ty, Type::Usize);
    assert_eq!(usize_extract_bits.params[1].ty, Type::Usize);
    assert_eq!(usize_extract_bits.params[2].ty, Type::Usize);
    assert_eq!(usize_extract_bits.return_type, Type::Usize);

    let usize_insert_bits = functions
        .iter()
        .find(|function| function.name == "usize_insert_bits")
        .expect("usize_insert_bits signature should exist");
    assert_eq!(usize_insert_bits.params.len(), 4);
    assert_eq!(usize_insert_bits.params[0].ty, Type::Usize);
    assert_eq!(usize_insert_bits.params[1].ty, Type::Usize);
    assert_eq!(usize_insert_bits.params[2].ty, Type::Usize);
    assert_eq!(usize_insert_bits.params[3].ty, Type::Usize);
    assert_eq!(usize_insert_bits.return_type, Type::Usize);

    let usize_byte_at = functions
        .iter()
        .find(|function| function.name == "usize_byte_at")
        .expect("usize_byte_at signature should exist");
    assert_eq!(usize_byte_at.params.len(), 2);
    assert_eq!(usize_byte_at.params[0].ty, Type::Usize);
    assert_eq!(usize_byte_at.params[1].ty, Type::Usize);
    assert_eq!(usize_byte_at.return_type, Type::U8);

    let usize_with_byte = functions
        .iter()
        .find(|function| function.name == "usize_with_byte")
        .expect("usize_with_byte signature should exist");
    assert_eq!(usize_with_byte.params.len(), 3);
    assert_eq!(usize_with_byte.params[0].ty, Type::Usize);
    assert_eq!(usize_with_byte.params[1].ty, Type::Usize);
    assert_eq!(usize_with_byte.params[2].ty, Type::U8);
    assert_eq!(usize_with_byte.return_type, Type::Usize);

    let usize_bit_clear = functions
        .iter()
        .find(|function| function.name == "usize_bit_clear")
        .expect("usize_bit_clear signature should exist");
    assert_eq!(usize_bit_clear.params.len(), 2);
    assert_eq!(usize_bit_clear.params[0].ty, Type::Usize);
    assert_eq!(usize_bit_clear.params[1].ty, Type::Usize);
    assert_eq!(usize_bit_clear.return_type, Type::Usize);

    let usize_bit_toggle = functions
        .iter()
        .find(|function| function.name == "usize_bit_toggle")
        .expect("usize_bit_toggle signature should exist");
    assert_eq!(usize_bit_toggle.params.len(), 2);
    assert_eq!(usize_bit_toggle.params[0].ty, Type::Usize);
    assert_eq!(usize_bit_toggle.params[1].ty, Type::Usize);
    assert_eq!(usize_bit_toggle.return_type, Type::Usize);

    let int_rotate_left = functions
        .iter()
        .find(|function| function.name == "int_rotate_left")
        .expect("int_rotate_left signature should exist");
    assert_eq!(int_rotate_left.params.len(), 2);
    assert_eq!(int_rotate_left.params[0].ty, Type::Int);
    assert_eq!(int_rotate_left.params[1].ty, Type::Usize);
    assert_eq!(int_rotate_left.return_type, Type::Int);

    let int_rotate_right = functions
        .iter()
        .find(|function| function.name == "int_rotate_right")
        .expect("int_rotate_right signature should exist");
    assert_eq!(int_rotate_right.params.len(), 2);
    assert_eq!(int_rotate_right.params[0].ty, Type::Int);
    assert_eq!(int_rotate_right.params[1].ty, Type::Usize);
    assert_eq!(int_rotate_right.return_type, Type::Int);

    for name in [
        "int_checked_shl",
        "int_checked_shr",
        "int_wrapping_shl",
        "int_wrapping_shr",
        "int_arithmetic_shr",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Int);
    }
}

#[test]
fn exposes_std_string_search_functions() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let byte_at = functions
        .iter()
        .find(|function| function.name == "string_byte_at")
        .expect("string_byte_at signature should exist");
    assert_eq!(byte_at.params.len(), 2);
    assert_eq!(byte_at.params[0].ty, Type::String);
    assert_eq!(byte_at.params[1].ty, Type::Usize);
    assert_eq!(byte_at.return_type, Type::Int);

    let from_byte = functions
        .iter()
        .find(|function| function.name == "string_from_byte")
        .expect("string_from_byte signature should exist");
    assert_eq!(from_byte.params.len(), 1);
    assert_eq!(from_byte.params[0].ty, Type::Int);
    assert_eq!(from_byte.return_type, Type::String);

    let from_utf8_codepoint = functions
        .iter()
        .find(|function| function.name == "string_from_utf8_codepoint")
        .expect("string_from_utf8_codepoint signature should exist");
    assert_eq!(from_utf8_codepoint.params.len(), 1);
    assert_eq!(from_utf8_codepoint.params[0].ty, Type::Int);
    assert_eq!(from_utf8_codepoint.return_type, Type::String);

    let find_byte = functions
        .iter()
        .find(|function| function.name == "string_find_byte")
        .expect("string_find_byte signature should exist");
    assert_eq!(find_byte.params.len(), 2);
    assert_eq!(find_byte.params[0].ty, Type::String);
    assert_eq!(find_byte.params[1].ty, Type::Int);
    assert_eq!(find_byte.return_type, Type::Int);

    let utf8_find_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_find_codepoint")
        .expect("string_utf8_find_codepoint signature should exist");
    assert_eq!(utf8_find_codepoint.params.len(), 2);
    assert_eq!(utf8_find_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_find_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_find_codepoint.return_type, Type::Int);

    let utf8_contains_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_contains_codepoint")
        .expect("string_utf8_contains_codepoint signature should exist");
    assert_eq!(utf8_contains_codepoint.params.len(), 2);
    assert_eq!(utf8_contains_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_contains_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_contains_codepoint.return_type, Type::Bool);

    let utf8_starts_with_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_starts_with_codepoint")
        .expect("string_utf8_starts_with_codepoint signature should exist");
    assert_eq!(utf8_starts_with_codepoint.params.len(), 2);
    assert_eq!(utf8_starts_with_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_starts_with_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_starts_with_codepoint.return_type, Type::Bool);

    let utf8_ends_with_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_ends_with_codepoint")
        .expect("string_utf8_ends_with_codepoint signature should exist");
    assert_eq!(utf8_ends_with_codepoint.params.len(), 2);
    assert_eq!(utf8_ends_with_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_ends_with_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_ends_with_codepoint.return_type, Type::Bool);

    let utf8_count_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_count_codepoint")
        .expect("string_utf8_count_codepoint signature should exist");
    assert_eq!(utf8_count_codepoint.params.len(), 2);
    assert_eq!(utf8_count_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_count_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_count_codepoint.return_type, Type::Usize);

    let utf8_last_find_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_last_find_codepoint")
        .expect("string_utf8_last_find_codepoint signature should exist");
    assert_eq!(utf8_last_find_codepoint.params.len(), 2);
    assert_eq!(utf8_last_find_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_last_find_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_last_find_codepoint.return_type, Type::Int);

    let last_find_byte = functions
        .iter()
        .find(|function| function.name == "string_last_find_byte")
        .expect("string_last_find_byte signature should exist");
    assert_eq!(last_find_byte.params.len(), 2);
    assert_eq!(last_find_byte.params[0].ty, Type::String);
    assert_eq!(last_find_byte.params[1].ty, Type::Int);
    assert_eq!(last_find_byte.return_type, Type::Int);

    let is_empty = functions
        .iter()
        .find(|function| function.name == "string_is_empty")
        .expect("string_is_empty signature should exist");
    assert_eq!(is_empty.params.len(), 1);
    assert_eq!(is_empty.params[0].ty, Type::String);
    assert_eq!(is_empty.return_type, Type::Bool);

    let is_ascii = functions
        .iter()
        .find(|function| function.name == "string_is_ascii")
        .expect("string_is_ascii signature should exist");
    assert_eq!(is_ascii.params.len(), 1);
    assert_eq!(is_ascii.params[0].ty, Type::String);
    assert_eq!(is_ascii.return_type, Type::Bool);

    let is_utf8 = functions
        .iter()
        .find(|function| function.name == "string_is_utf8")
        .expect("string_is_utf8 signature should exist");
    assert_eq!(is_utf8.params.len(), 1);
    assert_eq!(is_utf8.params[0].ty, Type::String);
    assert_eq!(is_utf8.return_type, Type::Bool);

    let utf8_is_valid = functions
        .iter()
        .find(|function| function.name == "string_utf8_is_valid")
        .expect("string_utf8_is_valid signature should exist");
    assert_eq!(utf8_is_valid.params.len(), 1);
    assert_eq!(utf8_is_valid.params[0].ty, Type::String);
    assert_eq!(utf8_is_valid.return_type, Type::Bool);

    let utf8_len = functions
        .iter()
        .find(|function| function.name == "string_utf8_len")
        .expect("string_utf8_len signature should exist");
    assert_eq!(utf8_len.params.len(), 1);
    assert_eq!(utf8_len.params[0].ty, Type::String);
    assert_eq!(utf8_len.return_type, Type::Int);

    let utf8_char_at = functions
        .iter()
        .find(|function| function.name == "string_utf8_char_at")
        .expect("string_utf8_char_at signature should exist");
    assert_eq!(utf8_char_at.params.len(), 2);
    assert_eq!(utf8_char_at.params[0].ty, Type::String);
    assert_eq!(utf8_char_at.params[1].ty, Type::Usize);
    assert_eq!(utf8_char_at.return_type, Type::String);

    let utf8_codepoint_at = functions
        .iter()
        .find(|function| function.name == "string_utf8_codepoint_at")
        .expect("string_utf8_codepoint_at signature should exist");
    assert_eq!(utf8_codepoint_at.params.len(), 2);
    assert_eq!(utf8_codepoint_at.params[0].ty, Type::String);
    assert_eq!(utf8_codepoint_at.params[1].ty, Type::Usize);
    assert_eq!(utf8_codepoint_at.return_type, Type::Int);

    let utf8_byte_offset = functions
        .iter()
        .find(|function| function.name == "string_utf8_byte_offset")
        .expect("string_utf8_byte_offset signature should exist");
    assert_eq!(utf8_byte_offset.params.len(), 2);
    assert_eq!(utf8_byte_offset.params[0].ty, Type::String);
    assert_eq!(utf8_byte_offset.params[1].ty, Type::Usize);
    assert_eq!(utf8_byte_offset.return_type, Type::Int);

    let utf8_next_offset = functions
        .iter()
        .find(|function| function.name == "string_utf8_next_offset")
        .expect("string_utf8_next_offset signature should exist");
    assert_eq!(utf8_next_offset.params.len(), 2);
    assert_eq!(utf8_next_offset.params[0].ty, Type::String);
    assert_eq!(utf8_next_offset.params[1].ty, Type::Usize);
    assert_eq!(utf8_next_offset.return_type, Type::Int);

    let utf8_prev_offset = functions
        .iter()
        .find(|function| function.name == "string_utf8_prev_offset")
        .expect("string_utf8_prev_offset signature should exist");
    assert_eq!(utf8_prev_offset.params.len(), 2);
    assert_eq!(utf8_prev_offset.params[0].ty, Type::String);
    assert_eq!(utf8_prev_offset.params[1].ty, Type::Usize);
    assert_eq!(utf8_prev_offset.return_type, Type::Int);

    let utf8_index_at = functions
        .iter()
        .find(|function| function.name == "string_utf8_index_at")
        .expect("string_utf8_index_at signature should exist");
    assert_eq!(utf8_index_at.params.len(), 2);
    assert_eq!(utf8_index_at.params[0].ty, Type::String);
    assert_eq!(utf8_index_at.params[1].ty, Type::Usize);
    assert_eq!(utf8_index_at.return_type, Type::Int);

    let utf8_is_boundary = functions
        .iter()
        .find(|function| function.name == "string_utf8_is_boundary")
        .expect("string_utf8_is_boundary signature should exist");
    assert_eq!(utf8_is_boundary.params.len(), 2);
    assert_eq!(utf8_is_boundary.params[0].ty, Type::String);
    assert_eq!(utf8_is_boundary.params[1].ty, Type::Usize);
    assert_eq!(utf8_is_boundary.return_type, Type::Bool);

    let is_ascii_digit = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_digit")
        .expect("string_is_ascii_digit signature should exist");
    assert_eq!(is_ascii_digit.params.len(), 1);
    assert_eq!(is_ascii_digit.params[0].ty, Type::String);
    assert_eq!(is_ascii_digit.return_type, Type::Bool);

    let is_ascii_hex_digit = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_hex_digit")
        .expect("string_is_ascii_hex_digit signature should exist");
    assert_eq!(is_ascii_hex_digit.params.len(), 1);
    assert_eq!(is_ascii_hex_digit.params[0].ty, Type::String);
    assert_eq!(is_ascii_hex_digit.return_type, Type::Bool);

    let is_ascii_alpha = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_alpha")
        .expect("string_is_ascii_alpha signature should exist");
    assert_eq!(is_ascii_alpha.params.len(), 1);
    assert_eq!(is_ascii_alpha.params[0].ty, Type::String);
    assert_eq!(is_ascii_alpha.return_type, Type::Bool);

    for name in ["string_is_ascii_lower", "string_is_ascii_upper"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::Bool);
    }

    let is_ascii_alnum = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_alnum")
        .expect("string_is_ascii_alnum signature should exist");
    assert_eq!(is_ascii_alnum.params.len(), 1);
    assert_eq!(is_ascii_alnum.params[0].ty, Type::String);
    assert_eq!(is_ascii_alnum.return_type, Type::Bool);

    let is_ascii_identifier = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_identifier")
        .expect("string_is_ascii_identifier signature should exist");
    assert_eq!(is_ascii_identifier.params.len(), 1);
    assert_eq!(is_ascii_identifier.params[0].ty, Type::String);
    assert_eq!(is_ascii_identifier.return_type, Type::Bool);

    let is_ascii_whitespace = functions
        .iter()
        .find(|function| function.name == "string_is_ascii_whitespace")
        .expect("string_is_ascii_whitespace signature should exist");
    assert_eq!(is_ascii_whitespace.params.len(), 1);
    assert_eq!(is_ascii_whitespace.params[0].ty, Type::String);
    assert_eq!(is_ascii_whitespace.return_type, Type::Bool);

    for name in ["string_contains", "string_starts_with", "string_ends_with"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::String);
        assert_eq!(function.return_type, Type::Bool);
    }

    for name in [
        "string_eq",
        "string_not_eq",
        "string_less",
        "string_less_or_equal",
        "string_greater",
        "string_greater_or_equal",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::String);
        assert_eq!(function.return_type, Type::Bool);
    }

    let index_of = functions
        .iter()
        .find(|function| function.name == "string_index_of")
        .expect("string_index_of signature should exist");
    assert_eq!(index_of.params.len(), 2);
    assert_eq!(index_of.params[0].ty, Type::String);
    assert_eq!(index_of.params[1].ty, Type::String);
    assert_eq!(index_of.return_type, Type::Int);

    let last_index_of = functions
        .iter()
        .find(|function| function.name == "string_last_index_of")
        .expect("string_last_index_of signature should exist");
    assert_eq!(last_index_of.params.len(), 2);
    assert_eq!(last_index_of.params[0].ty, Type::String);
    assert_eq!(last_index_of.params[1].ty, Type::String);
    assert_eq!(last_index_of.return_type, Type::Int);

    let count = functions
        .iter()
        .find(|function| function.name == "string_count")
        .expect("string_count signature should exist");
    assert_eq!(count.params.len(), 2);
    assert_eq!(count.params[0].ty, Type::String);
    assert_eq!(count.params[1].ty, Type::String);
    assert_eq!(count.return_type, Type::Usize);

    let compare_ignore_case = functions
        .iter()
        .find(|function| function.name == "string_compare_ignore_case")
        .expect("string_compare_ignore_case signature should exist");
    assert_eq!(compare_ignore_case.params.len(), 2);
    assert_eq!(compare_ignore_case.params[0].ty, Type::String);
    assert_eq!(compare_ignore_case.params[1].ty, Type::String);
    assert_eq!(compare_ignore_case.return_type, Type::Int);

    for name in [
        "string_eq_ignore_case",
        "string_not_eq_ignore_case",
        "string_less_ignore_case",
        "string_less_or_equal_ignore_case",
        "string_greater_ignore_case",
        "string_greater_or_equal_ignore_case",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::String);
        assert_eq!(function.return_type, Type::Bool);
    }

    for name in [
        "string_before",
        "string_after",
        "string_before_last",
        "string_after_last",
        "string_strip_prefix",
        "string_strip_suffix",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }

    let between = functions
        .iter()
        .find(|function| function.name == "string_between")
        .expect("string_between signature should exist");
    assert_eq!(between.params.len(), 3);
    assert_eq!(between.params[0].ty, Type::String);
    assert_eq!(between.params[1].ty, Type::String);
    assert_eq!(between.params[2].ty, Type::String);
    assert_eq!(between.return_type, Type::String);

    let between_last = functions
        .iter()
        .find(|function| function.name == "string_between_last")
        .expect("string_between_last signature should exist");
    assert_eq!(between_last.params.len(), 3);
    assert_eq!(between_last.params[0].ty, Type::String);
    assert_eq!(between_last.params[1].ty, Type::String);
    assert_eq!(between_last.params[2].ty, Type::String);
    assert_eq!(between_last.return_type, Type::String);
}

#[test]
fn exposes_std_string_trim_functions() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    for name in [
        "string_trim",
        "string_trim_start",
        "string_trim_end",
        "string_strip_ascii_line_comment",
        "string_strip_ascii_block_comment",
        "string_collapse_ascii_whitespace",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }

    let trim_start_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_trim_start_codepoint")
        .expect("string_utf8_trim_start_codepoint signature should exist");
    assert_eq!(trim_start_codepoint.params.len(), 2);
    assert_eq!(trim_start_codepoint.params[0].ty, Type::String);
    assert_eq!(trim_start_codepoint.params[1].ty, Type::Int);
    assert_eq!(trim_start_codepoint.return_type, Type::String);

    let trim_end_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_trim_end_codepoint")
        .expect("string_utf8_trim_end_codepoint signature should exist");
    assert_eq!(trim_end_codepoint.params.len(), 2);
    assert_eq!(trim_end_codepoint.params[0].ty, Type::String);
    assert_eq!(trim_end_codepoint.params[1].ty, Type::Int);
    assert_eq!(trim_end_codepoint.return_type, Type::String);

    let trim_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_trim_codepoint")
        .expect("string_utf8_trim_codepoint signature should exist");
    assert_eq!(trim_codepoint.params.len(), 2);
    assert_eq!(trim_codepoint.params[0].ty, Type::String);
    assert_eq!(trim_codepoint.params[1].ty, Type::Int);
    assert_eq!(trim_codepoint.return_type, Type::String);
}

#[test]
fn exposes_std_string_line_functions() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let line_count = functions
        .iter()
        .find(|function| function.name == "string_line_count")
        .expect("string_line_count signature should exist");
    assert_eq!(line_count.params.len(), 1);
    assert_eq!(line_count.params[0].ty, Type::String);
    assert_eq!(line_count.return_type, Type::Usize);

    let line_at = functions
        .iter()
        .find(|function| function.name == "string_line_at")
        .expect("string_line_at signature should exist");
    assert_eq!(line_at.params.len(), 2);
    assert_eq!(line_at.params[0].ty, Type::String);
    assert_eq!(line_at.params[1].ty, Type::Usize);
    assert_eq!(line_at.return_type, Type::String);

    let indent = functions
        .iter()
        .find(|function| function.name == "string_indent")
        .expect("string_indent signature should exist");
    assert_eq!(indent.params.len(), 2);
    assert_eq!(indent.params[0].ty, Type::String);
    assert_eq!(indent.params[1].ty, Type::String);
    assert_eq!(indent.return_type, Type::String);

    let prefix_lines = functions
        .iter()
        .find(|function| function.name == "string_prefix_lines")
        .expect("string_prefix_lines signature should exist");
    assert_eq!(prefix_lines.params.len(), 2);
    assert_eq!(prefix_lines.params[0].ty, Type::String);
    assert_eq!(prefix_lines.params[1].ty, Type::String);
    assert_eq!(prefix_lines.return_type, Type::String);

    let suffix_lines = functions
        .iter()
        .find(|function| function.name == "string_suffix_lines")
        .expect("string_suffix_lines signature should exist");
    assert_eq!(suffix_lines.params.len(), 2);
    assert_eq!(suffix_lines.params[0].ty, Type::String);
    assert_eq!(suffix_lines.params[1].ty, Type::String);
    assert_eq!(suffix_lines.return_type, Type::String);

    let dedent = functions
        .iter()
        .find(|function| function.name == "string_dedent")
        .expect("string_dedent signature should exist");
    assert_eq!(dedent.params.len(), 2);
    assert_eq!(dedent.params[0].ty, Type::String);
    assert_eq!(dedent.params[1].ty, Type::String);
    assert_eq!(dedent.return_type, Type::String);

    for name in ["string_line_index_at", "string_column_at"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::Int);
    }

    let offset_at = functions
        .iter()
        .find(|function| function.name == "string_offset_at_line_column")
        .expect("string_offset_at_line_column signature should exist");
    assert_eq!(offset_at.params.len(), 3);
    assert_eq!(offset_at.params[0].ty, Type::String);
    assert_eq!(offset_at.params[1].ty, Type::Usize);
    assert_eq!(offset_at.params[2].ty, Type::Usize);
    assert_eq!(offset_at.return_type, Type::Int);
}

#[test]
fn exposes_std_string_slice_function() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let function = functions
        .iter()
        .find(|function| function.name == "string_slice")
        .expect("string_slice signature should exist");
    assert_eq!(function.params.len(), 3);
    assert_eq!(function.params[0].ty, Type::String);
    assert_eq!(function.params[1].ty, Type::Usize);
    assert_eq!(function.params[2].ty, Type::Usize);
    assert_eq!(function.return_type, Type::String);

    let utf8_slice = functions
        .iter()
        .find(|function| function.name == "string_utf8_slice")
        .expect("string_utf8_slice signature should exist");
    assert_eq!(utf8_slice.params.len(), 3);
    assert_eq!(utf8_slice.params[0].ty, Type::String);
    assert_eq!(utf8_slice.params[1].ty, Type::Usize);
    assert_eq!(utf8_slice.params[2].ty, Type::Usize);
    assert_eq!(utf8_slice.return_type, Type::String);

    let utf8_take_while_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_take_while_codepoint")
        .expect("string_utf8_take_while_codepoint signature should exist");
    assert_eq!(utf8_take_while_codepoint.params.len(), 2);
    assert_eq!(utf8_take_while_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_take_while_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_take_while_codepoint.return_type, Type::String);

    let utf8_take_until_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_take_until_codepoint")
        .expect("string_utf8_take_until_codepoint signature should exist");
    assert_eq!(utf8_take_until_codepoint.params.len(), 2);
    assert_eq!(utf8_take_until_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_take_until_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_take_until_codepoint.return_type, Type::String);

    let utf8_through_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_through_codepoint")
        .expect("string_utf8_through_codepoint signature should exist");
    assert_eq!(utf8_through_codepoint.params.len(), 2);
    assert_eq!(utf8_through_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_through_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_through_codepoint.return_type, Type::String);

    let utf8_through_last_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_through_last_codepoint")
        .expect("string_utf8_through_last_codepoint signature should exist");
    assert_eq!(utf8_through_last_codepoint.params.len(), 2);
    assert_eq!(utf8_through_last_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_through_last_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_through_last_codepoint.return_type, Type::String);

    let utf8_between_codepoints = functions
        .iter()
        .find(|function| function.name == "string_utf8_between_codepoints")
        .expect("string_utf8_between_codepoints signature should exist");
    assert_eq!(utf8_between_codepoints.params.len(), 3);
    assert_eq!(utf8_between_codepoints.params[0].ty, Type::String);
    assert_eq!(utf8_between_codepoints.params[1].ty, Type::Int);
    assert_eq!(utf8_between_codepoints.params[2].ty, Type::Int);
    assert_eq!(utf8_between_codepoints.return_type, Type::String);

    let utf8_between_last_codepoints = functions
        .iter()
        .find(|function| function.name == "string_utf8_between_last_codepoints")
        .expect("string_utf8_between_last_codepoints signature should exist");
    assert_eq!(utf8_between_last_codepoints.params.len(), 3);
    assert_eq!(utf8_between_last_codepoints.params[0].ty, Type::String);
    assert_eq!(utf8_between_last_codepoints.params[1].ty, Type::Int);
    assert_eq!(utf8_between_last_codepoints.params[2].ty, Type::Int);
    assert_eq!(utf8_between_last_codepoints.return_type, Type::String);

    let utf8_between_outer_codepoints = functions
        .iter()
        .find(|function| function.name == "string_utf8_between_outer_codepoints")
        .expect("string_utf8_between_outer_codepoints signature should exist");
    assert_eq!(utf8_between_outer_codepoints.params.len(), 3);
    assert_eq!(utf8_between_outer_codepoints.params[0].ty, Type::String);
    assert_eq!(utf8_between_outer_codepoints.params[1].ty, Type::Int);
    assert_eq!(utf8_between_outer_codepoints.params[2].ty, Type::Int);
    assert_eq!(utf8_between_outer_codepoints.return_type, Type::String);

    let utf8_before_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_before_codepoint")
        .expect("string_utf8_before_codepoint signature should exist");
    assert_eq!(utf8_before_codepoint.params.len(), 2);
    assert_eq!(utf8_before_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_before_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_before_codepoint.return_type, Type::String);

    let utf8_before_last_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_before_last_codepoint")
        .expect("string_utf8_before_last_codepoint signature should exist");
    assert_eq!(utf8_before_last_codepoint.params.len(), 2);
    assert_eq!(utf8_before_last_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_before_last_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_before_last_codepoint.return_type, Type::String);

    let utf8_drop_until_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_drop_until_codepoint")
        .expect("string_utf8_drop_until_codepoint signature should exist");
    assert_eq!(utf8_drop_until_codepoint.params.len(), 2);
    assert_eq!(utf8_drop_until_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_drop_until_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_drop_until_codepoint.return_type, Type::String);

    let utf8_after_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_after_codepoint")
        .expect("string_utf8_after_codepoint signature should exist");
    assert_eq!(utf8_after_codepoint.params.len(), 2);
    assert_eq!(utf8_after_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_after_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_after_codepoint.return_type, Type::String);

    let utf8_after_last_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_after_last_codepoint")
        .expect("string_utf8_after_last_codepoint signature should exist");
    assert_eq!(utf8_after_last_codepoint.params.len(), 2);
    assert_eq!(utf8_after_last_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_after_last_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_after_last_codepoint.return_type, Type::String);

    let utf8_drop_while_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_drop_while_codepoint")
        .expect("string_utf8_drop_while_codepoint signature should exist");
    assert_eq!(utf8_drop_while_codepoint.params.len(), 2);
    assert_eq!(utf8_drop_while_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_drop_while_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_drop_while_codepoint.return_type, Type::String);

    let utf8_strip_prefix_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_strip_prefix_codepoint")
        .expect("string_utf8_strip_prefix_codepoint signature should exist");
    assert_eq!(utf8_strip_prefix_codepoint.params.len(), 2);
    assert_eq!(utf8_strip_prefix_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_strip_prefix_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_strip_prefix_codepoint.return_type, Type::String);

    let utf8_strip_suffix_codepoint = functions
        .iter()
        .find(|function| function.name == "string_utf8_strip_suffix_codepoint")
        .expect("string_utf8_strip_suffix_codepoint signature should exist");
    assert_eq!(utf8_strip_suffix_codepoint.params.len(), 2);
    assert_eq!(utf8_strip_suffix_codepoint.params[0].ty, Type::String);
    assert_eq!(utf8_strip_suffix_codepoint.params[1].ty, Type::Int);
    assert_eq!(utf8_strip_suffix_codepoint.return_type, Type::String);

    for name in [
        "string_take",
        "string_drop",
        "string_take_last",
        "string_drop_last",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.return_type, Type::String);
    }
}

#[test]
fn exposes_std_string_case_functions() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    for name in ["string_to_lower", "string_to_upper"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }

    for name in [
        "ascii_to_lower",
        "ascii_to_upper",
        "unicode_ascii_to_lower",
        "unicode_ascii_to_upper",
        "ascii_digit_value",
        "ascii_hex_value",
        "unicode_ascii_digit_value",
        "unicode_ascii_hex_value",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Int);
    }

    for name in [
        "ascii_is_digit",
        "ascii_is_hex_digit",
        "ascii_is_identifier_start",
        "ascii_is_identifier_continue",
        "ascii_is_alpha",
        "ascii_is_alnum",
        "ascii_is_whitespace",
        "unicode_is_ascii_digit",
        "unicode_is_ascii_hex_digit",
        "unicode_is_ascii_identifier_start",
        "unicode_is_ascii_identifier_continue",
        "unicode_is_ascii_alpha",
        "unicode_is_ascii_alnum",
        "unicode_is_ascii_whitespace",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::Int);
        assert_eq!(function.return_type, Type::Bool);
    }
}

#[test]
fn exposes_std_string_reverse_function() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let function = functions
        .iter()
        .find(|function| function.name == "string_reverse")
        .expect("string_reverse signature should exist");
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].ty, Type::String);
    assert_eq!(function.return_type, Type::String);
}

#[test]
fn exposes_std_string_replace_function() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    for name in ["string_replace", "string_replace_all"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 3);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::String);
        assert_eq!(function.params[2].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }

    for name in [
        "string_escape",
        "string_escape_ascii",
        "string_unescape",
        "string_unescape_hex",
        "string_unescape_unicode",
    ] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 1);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }
}

#[test]
fn exposes_std_string_repeat_function() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let function = functions
        .iter()
        .find(|function| function.name == "string_repeat")
        .expect("string_repeat signature should exist");
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].ty, Type::String);
    assert_eq!(function.params[1].ty, Type::Usize);
    assert_eq!(function.return_type, Type::String);

    for name in ["string_pad_start", "string_pad_end"] {
        let function = functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("{name} signature should exist"));
        assert_eq!(function.params.len(), 3);
        assert_eq!(function.params[0].ty, Type::String);
        assert_eq!(function.params[1].ty, Type::Usize);
        assert_eq!(function.params[2].ty, Type::String);
        assert_eq!(function.return_type, Type::String);
    }
}

#[test]
fn exposes_std_string_parse_int_function() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();
    let function = functions
        .iter()
        .find(|function| function.name == "string_parse_int")
        .expect("string_parse_int signature should exist");

    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].ty, Type::String);
    assert_eq!(function.return_type, Type::Int);

    let usize_function = functions
        .iter()
        .find(|function| function.name == "string_parse_usize")
        .expect("string_parse_usize signature should exist");
    assert_eq!(usize_function.params.len(), 1);
    assert_eq!(usize_function.params[0].ty, Type::String);
    assert_eq!(usize_function.return_type, Type::Usize);

    let try_int = functions
        .iter()
        .find(|function| function.name == "string_try_parse_int")
        .expect("string_try_parse_int signature should exist");
    assert_eq!(try_int.params.len(), 2);
    assert_eq!(try_int.params[0].ty, Type::String);
    assert_eq!(try_int.params[1].ty, Type::Pointer(Box::new(Type::Int)));
    assert_eq!(try_int.return_type, Type::Bool);

    let try_usize = functions
        .iter()
        .find(|function| function.name == "string_try_parse_usize")
        .expect("string_try_parse_usize signature should exist");
    assert_eq!(try_usize.params.len(), 2);
    assert_eq!(try_usize.params[0].ty, Type::String);
    assert_eq!(try_usize.params[1].ty, Type::Pointer(Box::new(Type::Usize)));
    assert_eq!(try_usize.return_type, Type::Bool);
}

#[test]
fn exposes_std_string_integer_format_functions() {
    let path = vec!["std".to_string(), "string".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let int_function = functions
        .iter()
        .find(|function| function.name == "int_to_string")
        .expect("int_to_string signature should exist");
    assert_eq!(int_function.params.len(), 1);
    assert_eq!(int_function.params[0].ty, Type::Int);
    assert_eq!(int_function.return_type, Type::String);

    let usize_function = functions
        .iter()
        .find(|function| function.name == "usize_to_string")
        .expect("usize_to_string signature should exist");
    assert_eq!(usize_function.params.len(), 1);
    assert_eq!(usize_function.params[0].ty, Type::Usize);
    assert_eq!(usize_function.return_type, Type::String);

    let bool_function = functions
        .iter()
        .find(|function| function.name == "bool_to_string")
        .expect("bool_to_string signature should exist");
    assert_eq!(bool_function.params.len(), 1);
    assert_eq!(bool_function.params[0].ty, Type::Bool);
    assert_eq!(bool_function.return_type, Type::String);
}

#[test]
fn exposes_std_random_functions() {
    let path = vec!["std".to_string(), "random".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let seed = functions
        .iter()
        .find(|function| function.name == "random_seed")
        .expect("random_seed signature should exist");
    assert_eq!(seed.params.len(), 1);
    assert_eq!(seed.params[0].ty, Type::Usize);
    assert_eq!(seed.return_type, Type::Int);

    let random = functions
        .iter()
        .find(|function| function.name == "random_usize")
        .expect("random_usize signature should exist");
    assert!(random.params.is_empty());
    assert_eq!(random.return_type, Type::Usize);

    let range = functions
        .iter()
        .find(|function| function.name == "random_range")
        .expect("random_range signature should exist");
    assert_eq!(range.params.len(), 1);
    assert_eq!(range.params[0].ty, Type::Usize);
    assert_eq!(range.return_type, Type::Usize);

    let range_inclusive = functions
        .iter()
        .find(|function| function.name == "random_range_inclusive")
        .expect("random_range_inclusive signature should exist");
    assert_eq!(range_inclusive.params.len(), 1);
    assert_eq!(range_inclusive.params[0].ty, Type::Usize);
    assert_eq!(range_inclusive.return_type, Type::Usize);

    let bool_fn = functions
        .iter()
        .find(|function| function.name == "random_bool")
        .expect("random_bool signature should exist");
    assert!(bool_fn.params.is_empty());
    assert_eq!(bool_fn.return_type, Type::Bool);

    let int_range = functions
        .iter()
        .find(|function| function.name == "random_int_range")
        .expect("random_int_range signature should exist");
    assert_eq!(int_range.params.len(), 2);
    assert_eq!(int_range.params[0].ty, Type::Int);
    assert_eq!(int_range.params[1].ty, Type::Int);
    assert_eq!(int_range.return_type, Type::Int);
}

#[test]
fn exposes_std_hash_functions() {
    let path = vec!["std".to_string(), "hash".to_string()];
    let functions = functions_for_import(&path).unwrap();

    let hash_string = functions
        .iter()
        .find(|function| function.name == "hash_string")
        .expect("hash_string signature should exist");
    assert_eq!(hash_string.params.len(), 1);
    assert_eq!(hash_string.params[0].ty, Type::String);
    assert_eq!(hash_string.return_type, Type::Usize);

    let hash_usize = functions
        .iter()
        .find(|function| function.name == "hash_usize")
        .expect("hash_usize signature should exist");
    assert_eq!(hash_usize.params.len(), 1);
    assert_eq!(hash_usize.params[0].ty, Type::Usize);
    assert_eq!(hash_usize.return_type, Type::Usize);

    let hash_bytes = functions
        .iter()
        .find(|function| function.name == "hash_bytes")
        .expect("hash_bytes signature should exist");
    assert_eq!(hash_bytes.params.len(), 2);
    assert_eq!(hash_bytes.params[0].ty, Type::Pointer(Box::new(Type::U8)));
    assert_eq!(hash_bytes.params[1].ty, Type::Usize);
    assert_eq!(hash_bytes.return_type, Type::Usize);

    let hash_bytes_seed = functions
        .iter()
        .find(|function| function.name == "hash_bytes_seed")
        .expect("hash_bytes_seed signature should exist");
    assert_eq!(hash_bytes_seed.params.len(), 3);
    assert_eq!(
        hash_bytes_seed.params[0].ty,
        Type::Pointer(Box::new(Type::U8))
    );
    assert_eq!(hash_bytes_seed.params[1].ty, Type::Usize);
    assert_eq!(hash_bytes_seed.params[2].ty, Type::Usize);
    assert_eq!(hash_bytes_seed.return_type, Type::Usize);

    let hash_combine = functions
        .iter()
        .find(|function| function.name == "hash_combine")
        .expect("hash_combine signature should exist");
    assert_eq!(hash_combine.params.len(), 2);
    assert_eq!(hash_combine.params[0].ty, Type::Usize);
    assert_eq!(hash_combine.params[1].ty, Type::Usize);
    assert_eq!(hash_combine.return_type, Type::Usize);
}

#[test]
fn rejects_unknown_std_module() {
    let path = vec!["std".to_string(), "net".to_string()];
    let err = functions_for_import(&path).unwrap_err();
    assert!(err
        .message
        .contains("unknown standard library module 'std.net'"));
}
