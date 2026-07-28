import std.mem

fn main() -> int {
    let memory: *u8 = alloc(1)

    if memory != null {
        return 42
    }

    return 1
}
