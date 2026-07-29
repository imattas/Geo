import std.string

fn main() -> int {
    if string_utf8_len("Geo") != 3 {
        return 1
    }
    if string_utf8_len("λ") != 1 {
        return 2
    }
    if string_utf8_len("😀") != 1 {
        return 3
    }
    if string_utf8_len("λ😀") != 2 {
        return 4
    }
    return 0
}
