import std.mem

fn main() -> int {
    let source: *u8 = alloc(8)
    let copy: *u8 = alloc_copy(source, 8)
    if copy != null {
        return 42
    }
    return 1
}
