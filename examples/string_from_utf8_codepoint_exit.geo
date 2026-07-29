import std.string

fn main() -> int {
    let ascii = string_from_utf8_codepoint(65)
    if string_compare(ascii, "A") != 0 {
        return 1
    }
    let greek = string_from_utf8_codepoint(955)
    if string_compare(greek, "λ") != 0 {
        return 2
    }
    let emoji = string_from_utf8_codepoint(128512)
    if string_compare(emoji, "😀") != 0 {
        return 3
    }
    let invalid = string_from_utf8_codepoint(55296)
    if string_len(invalid) != 0usize {
        return 4
    }
    var status = string_free(ascii)
    status = status + string_free(greek)
    status = status + string_free(emoji)
    if status != 0 {
        return 5
    }
    return 0
}
