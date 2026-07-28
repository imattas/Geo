extern fn alloc(size: usize) -> *u8
extern fn realloc(ptr: *u8, size: usize) -> *u8
extern fn free(ptr: *u8) -> int
extern fn mem_copy(dest: *u8, source: *u8, len: usize) -> int
extern fn mem_zero(dest: *u8, len: usize) -> int

