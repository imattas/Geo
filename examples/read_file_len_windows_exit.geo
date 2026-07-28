import std.io
import std.string

fn main() -> int {
    return string_len(read_file("examples\\read_file_fixture.txt")) as int
}
