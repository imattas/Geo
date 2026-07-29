import std.string

fn main() -> int {
    let prefix: string = string_utf8_slice("compiler.geo", 0usize, 8usize)
    if string_compare(prefix, "compiler") != 0 {
        return 1
    }

    let suffix: string = string_utf8_slice("compiler.geo", 9usize, 64usize)
    if string_compare(suffix, "geo") != 0 {
        return 2
    }

    let lambda: string = string_concat(string_from_byte(206), string_from_byte(187))
    let whole: string = string_utf8_slice(lambda, 0usize, 1usize)
    if string_len(whole) != 2usize {
        return 3
    }

    let empty: string = string_utf8_slice(string_concat(string_from_byte(206), string_from_byte(187)), 1usize, 2usize)
    if string_len(empty) != 0usize {
        return 4
    }

    let invalid: string = string_from_byte(255)
    let invalid_slice: string = string_utf8_slice(invalid, 0usize, 1usize)
    if string_len(invalid_slice) != 0usize {
        return 5
    }

    return 0
}
