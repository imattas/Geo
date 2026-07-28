import std.io
import std.string

fn main() -> int {
    return string_len(read_file_or("C:\\geo-file-that-does-not-exist", "fallback")) as int
}
