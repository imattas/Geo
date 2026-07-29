import std.mem

fn main() -> int {
    let buffer: *u8 = alloc(16usize)
    if buffer == null {
        return 1
    }
    unsafe {
        *buffer = 42
        if *buffer != 42 {
            return 2
        }
        if free(buffer) != 0 {
            return 3
        }
    }
    return 0
}
