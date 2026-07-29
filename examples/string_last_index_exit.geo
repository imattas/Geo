import std.string

fn main() -> int {
    if string_last_index_of("compiler.geo.compiler.geo", ".geo") != 21 {
        return 1
    }
    if string_last_index_of("aaaa", "aa") != 2 {
        return 2
    }
    if string_last_index_of("geo", "rs") != -1 {
        return 3
    }
    if string_last_index_of("geo", "") != -1 {
        return 4
    }
    return 0
}
