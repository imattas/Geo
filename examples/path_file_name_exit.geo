import std.platform
import std.string

fn main() -> int {
    let name = path_file_name("a/b\\only.txt")
    let parent = path_parent("a/b\\only.txt")
    let valid = string_compare(name, "only.txt") == 0
    let parent_valid = string_compare(parent, "a/b") == 0
    string_free(name)
    string_free(parent)

    if valid && parent_valid {
        return 0
    }

    return 1
}
