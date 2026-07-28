import std.mem

fn main() -> int {
    let buffer: *u8 = alloc(8)
    mem_fill(buffer, 8, 65)
    return mem_find(buffer, 8, 65) + 42
}
