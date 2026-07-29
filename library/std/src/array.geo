extern fn array_new(elem_size: usize, capacity: usize) -> *u8
extern fn array_clone(array: *u8) -> *u8
extern fn array_reserve(array: *u8, capacity: usize) -> *u8
extern fn array_len(array: *u8) -> usize
extern fn array_is_empty(array: *u8) -> bool
extern fn array_capacity(array: *u8) -> usize
extern fn array_push(array: *u8, value: *u8) -> int
extern fn array_get(array: *u8, index: usize) -> *u8
extern fn array_first(array: *u8) -> *u8
extern fn array_last(array: *u8) -> *u8
extern fn array_index_of(array: *u8, value: *u8) -> int
extern fn array_last_index_of(array: *u8, value: *u8) -> int
extern fn array_contains(array: *u8, value: *u8) -> bool
extern fn array_count(array: *u8, value: *u8) -> usize
extern fn array_set(array: *u8, index: usize, value: *u8) -> int
extern fn array_fill(array: *u8, value: *u8) -> int
extern fn array_extend(array: *u8, other: *u8) -> int
extern fn array_copy(dst: *u8, dst_index: usize, src: *u8, src_index: usize, count: usize) -> int
extern fn array_resize(array: *u8, length: usize, value: *u8) -> int
extern fn array_insert(array: *u8, index: usize, value: *u8) -> int
extern fn array_swap(array: *u8, left: usize, right: usize) -> int
extern fn array_reverse(array: *u8) -> int
extern fn array_truncate(array: *u8, length: usize) -> int
extern fn array_remove(array: *u8, index: usize) -> int
extern fn array_swap_remove(array: *u8, index: usize) -> int
extern fn array_pop(array: *u8) -> *u8
extern fn array_pop_first(array: *u8) -> *u8
extern fn array_clear(array: *u8) -> int
extern fn array_free(array: *u8) -> int
