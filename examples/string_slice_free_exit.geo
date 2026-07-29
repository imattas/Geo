import std.string

fn main() -> int {
    return string_free(string_slice("compiler.geo", 0usize, 8usize))
}
