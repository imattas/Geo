import std.mem

fn main() -> int {
    let buffer: *u8 = alloc(8)
    return mem_fill(buffer, 8, 65) + 42
}
