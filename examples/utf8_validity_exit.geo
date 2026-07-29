import std.string

fn main() -> int {
    if !string_is_utf8("Geo") {
        return 1
    }
    if !string_utf8_is_valid("Geo") {
        return 2
    }
    let overlong: string = string_concat(string_from_byte(192), string_from_byte(128))
    if string_is_utf8(overlong) {
        return 3
    }
    let surrogate: string = string_concat(string_from_byte(237), string_concat(string_from_byte(160), string_from_byte(128)))
    if string_utf8_is_valid(surrogate) {
        return 4
    }
    let too_large: string = string_concat(string_from_byte(244), string_concat(string_from_byte(144), string_concat(string_from_byte(128), string_from_byte(128))))
    if string_utf8_is_valid(too_large) {
        return 5
    }
    return 0
}
