import std.platform
import std.string

fn main() -> int {
    let name = path_file_name("a/b\\only.txt")
    let valid = string_compare(name, "only.txt") == 0
    string_free(name)

    if valid {
        return 0
    }

    return 1
}
