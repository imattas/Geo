import std.string

fn main() -> int {
    if !string_ends_with("Geo compiler", "compiler") {
        return 1
    }
    if string_ends_with("Geo", "Rust") {
        return 2
    }
    return 0
}
