import std.string

fn main() -> int {
    if string_utf8_byte_offset("Geo", 0usize) != 0 { return 11 }
    if string_utf8_byte_offset("Geo", 2usize) != 2 { return 12 }
    if string_utf8_byte_offset("Geo", 3usize) != 3 { return 13 }
    if string_utf8_byte_offset("Geo", 4usize) != -1 { return 14 }
    if string_utf8_next_offset("Geo", 0usize) != 1 || string_utf8_next_offset("Geo", 2usize) != 3 || string_utf8_next_offset("Geo", 3usize) != 3 || string_utf8_next_offset("Geo", 4usize) != -1 {
        return 2
    }
    if string_utf8_prev_offset("Geo", 0usize) != 0 || string_utf8_prev_offset("Geo", 1usize) != 0 || string_utf8_prev_offset("Geo", 3usize) != 2 || string_utf8_prev_offset("Geo", 4usize) != -1 {
        return 3
    }
    if string_utf8_index_at("Geo", 0usize) != 0 || string_utf8_index_at("Geo", 2usize) != 2 || string_utf8_index_at("Geo", 3usize) != 3 || string_utf8_index_at("Geo", 4usize) != -1 {
        return 4
    }
    if !string_utf8_is_boundary("Geo", 0usize) || !string_utf8_is_boundary("Geo", 3usize) || string_utf8_is_boundary("Geo", 4usize) {
        return 5
    }

    if string_utf8_byte_offset(string_concat(string_from_byte(206), string_from_byte(187)), 1usize) != 2 {
        return 6
    }
    if string_utf8_next_offset(string_concat(string_from_byte(206), string_from_byte(187)), 0usize) != 2 || string_utf8_next_offset(string_concat(string_from_byte(206), string_from_byte(187)), 1usize) != -1 || string_utf8_next_offset(string_concat(string_from_byte(206), string_from_byte(187)), 2usize) != 2 {
        return 7
    }
    if string_utf8_prev_offset(string_concat(string_from_byte(206), string_from_byte(187)), 0usize) != 0 || string_utf8_prev_offset(string_concat(string_from_byte(206), string_from_byte(187)), 1usize) != -1 || string_utf8_prev_offset(string_concat(string_from_byte(206), string_from_byte(187)), 2usize) != 0 {
        return 8
    }
    if string_utf8_index_at(string_concat(string_from_byte(206), string_from_byte(187)), 0usize) != 0 || string_utf8_index_at(string_concat(string_from_byte(206), string_from_byte(187)), 1usize) != -1 || string_utf8_index_at(string_concat(string_from_byte(206), string_from_byte(187)), 2usize) != 1 {
        return 9
    }
    if !string_utf8_is_boundary(string_concat(string_from_byte(206), string_from_byte(187)), 0usize) || string_utf8_is_boundary(string_concat(string_from_byte(206), string_from_byte(187)), 1usize) || !string_utf8_is_boundary(string_concat(string_from_byte(206), string_from_byte(187)), 2usize) {
        return 10
    }
    return 0
}
