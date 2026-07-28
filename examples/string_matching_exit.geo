import std.string

fn main() -> int {
    if !string_contains("Geo compiler", "compiler") {
        return 1
    }
    if string_contains("Geo", "Rust") {
        return 2
    }
    if !string_starts_with("Geo compiler", "Geo") {
        return 3
    }
    return 0
}
