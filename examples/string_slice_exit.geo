import std.string

fn main() -> int {
    let prefix: string = string_slice("compiler.geo", 0usize, 8usize)
    if string_compare(prefix, "compiler") != 0 {
        return 1
    }
    let suffix: string = string_slice("compiler.geo", 9usize, 64usize)
    if string_compare(suffix, "geo") != 0 {
        return 2
    }
    let empty: string = string_slice("compiler.geo", 64usize, 4usize)
    if string_len(empty) != 0usize {
        return 3
    }
    let zero: string = string_slice("compiler.geo", 3usize, 0usize)
    if string_len(zero) != 0usize {
        return 4
    }
    return 0
}
