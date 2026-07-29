import std.platform
import std.string

fn main() -> int {
    let name = path_file_name("a/b\\only.txt")
    let parent = path_parent("a/b\\only.txt")
    let extension = path_extension("a/b\\only.txt")
    let dotfile_extension = path_extension(".gitignore")
    let valid = string_compare(name, "only.txt") == 0
    let parent_valid = string_compare(parent, "a/b") == 0
    let extension_valid = string_compare(extension, "txt") == 0
    let dotfile_valid = string_len(dotfile_extension) == 0
    string_free(name)
    string_free(parent)
    string_free(extension)
    string_free(dotfile_extension)

    if valid && parent_valid && extension_valid && dotfile_valid {
        return 0
    }

    return 1
}
