import std.string

fn main() -> int {
    if string_utf8_codepoint_at("Geo", 1usize) != 101 {
        return 1
    }
    if string_utf8_codepoint_at("λ", 0usize) != 955 {
        return 2
    }
    if string_utf8_codepoint_at("😀", 0usize) != 128512 {
        return 3
    }
    if string_utf8_codepoint_at("λ😀", 1usize) != 128512 {
        return 4
    }
    if string_utf8_codepoint_at("λ😀", 2usize) != -1 {
        return 5
    }
    return 0
}
