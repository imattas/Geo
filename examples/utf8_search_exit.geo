import std.string

fn main() -> int {
    if string_utf8_find_codepoint("Geo", 111) != 2 {
        return 1
    }
    if string_utf8_find_codepoint("Geo", 120) != -1 {
        return 2
    }

    let lambda: string = string_concat(string_from_byte(206), string_from_byte(187))
    if string_utf8_find_codepoint(lambda, 955) != 0 {
        return 3
    }
    let lambda_char: string = string_utf8_char_at(string_concat(string_from_byte(206), string_from_byte(187)), 0usize)
    if string_len(lambda_char) != 2usize {
        return 4
    }
    let empty: string = string_utf8_char_at("Geo", 3usize)
    if string_len(empty) != 0usize {
        return 5
    }
    return 0
}
