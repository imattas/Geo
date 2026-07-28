import std.mem

fn main() -> int {
    let memory: *u8 = alloc(8)
    mem_zero(memory, 8)
    mem_copy(memory, memory, 8)
    return mem_move(memory, memory, 8)
}
