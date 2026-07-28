import std.mem

fn main() -> int {
    let left: *u8 = alloc(4)
    let right: *u8 = alloc(4)
    mem_fill(left, 4, 65)
    mem_fill(right, 4, 66)
    return mem_compare(left, right, 4) + 42
}
