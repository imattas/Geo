import std.string

fn main() -> int {
    if string_count("aaaa", "aa") != 2usize {
        return 1
    }
    if string_count("Geo compiler", "o") != 2usize {
        return 2
    }
    return 0
}
