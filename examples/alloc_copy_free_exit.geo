import std.mem

fn main() -> int {
    let source: *u8 = alloc(8usize)
    if source == null {
        return 1
    }
    unsafe {
        *source = 55
        let copy: *u8 = alloc_copy(source, 8usize)
        if copy == null || *copy != 55 {
            return 2
        }
        if free(copy) != 0 || free(source) != 0 {
            return 3
        }
    }
    return 0
}
