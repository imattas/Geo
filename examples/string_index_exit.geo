import std.string

fn main() -> int {
    if string_index_of("Geo compiler", "compiler") != 4 {
        return 1
    }
    if string_index_of("Geo", "Rust") != -1 {
        return 2
    }
    return 0
}
