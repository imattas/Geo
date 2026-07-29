import std.string

fn main() -> int {
    if string_last_find_byte("banana", 97) != 5 {
        return 1
    }
    if string_last_find_byte("Geo", 120) != -1 {
        return 2
    }
    if string_last_find_byte("Geo", -1) != -1 {
        return 3
    }
    if string_last_find_byte("Geo", 256) != -1 {
        return 4
    }
    return 0
}
