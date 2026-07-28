import std.io
import std.string

fn main() -> int {
    return string_len(read_file("/tmp/geo-read-file-example")) as int
}
