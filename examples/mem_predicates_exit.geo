import std.mem

fn main() -> int {
    if mem_equal(null, null, 0) {
        if mem_is_zero(null, 0) {
            return 42
        }
    }
    return 1
}
