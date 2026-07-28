import std.string

fn main() -> int {
    if string_byte_at("Geo", 1) != 101 {
        return 1
    }
    if string_is_empty("Geo") || !string_is_empty("") {
        return 2
    }
    if !string_is_ascii("Geo") {
        return 3
    }
    return string_find_byte("Geo", 111)
}
