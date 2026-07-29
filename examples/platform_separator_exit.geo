import std.platform

fn main() -> int {
    let separator: char = platform_path_separator()
    if separator == '/' || separator == '\\' {
        return 0
    }
    return 1
}
