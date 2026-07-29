import std.platform

fn main() -> int {
    if !path_is_absolute("/root") {
        return 1
    }
    if !path_is_absolute("C:\\root") {
        return 2
    }
    if path_is_absolute("relative/path") {
        return 3
    }
    return 0
}
