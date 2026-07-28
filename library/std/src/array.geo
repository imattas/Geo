extern fn array_new(elem_size: usize) -> *u8
extern fn array_len(array: *u8) -> usize
extern fn array_capacity(array: *u8) -> usize
extern fn array_reserve(array: *u8, additional: usize) -> *u8
extern fn array_clear(array: *u8) -> int
extern fn array_free(array: *u8) -> int

