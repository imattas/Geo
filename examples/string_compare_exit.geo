import std.string

fn main() -> int {
    if string_compare("Geo", "Geo") != 0 {
        return 1
    }
    if !string_less("Geo", "Rust") {
        return 2
    }
    if !string_greater_or_equal("Rust", "Rust") {
        return 3
    }
    if !string_not_eq("Geo", "Rust") {
        return 4
    }
    return 0
}
