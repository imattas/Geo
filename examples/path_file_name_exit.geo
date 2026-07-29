import std.platform
import std.string

fn main() -> int {
    let name = path_file_name("a/b\\only.txt")
    let parent = path_parent("a/b\\only.txt")
    let extension = path_extension("a/b\\only.txt")
    let dotfile_extension = path_extension(".gitignore")
    let stem = path_stem("a/b\\only.tar.txt")
    let dotfile_stem = path_stem(".gitignore")
    let without_extension = path_without_extension("a/b\\only.tar.txt")
    let dotfile_without_extension = path_without_extension(".gitignore")
    let valid = string_compare(name, "only.txt") == 0
    let parent_valid = string_compare(parent, "a/b") == 0
    let extension_valid = string_compare(extension, "txt") == 0
    let dotfile_valid = string_len(dotfile_extension) == 0
    let stem_valid = string_compare(stem, "only.tar") == 0
    let dotfile_stem_valid = string_compare(dotfile_stem, ".gitignore") == 0
    let without_extension_valid = string_compare(without_extension, "a/b\\only.tar") == 0
    let dotfile_without_extension_valid = string_compare(dotfile_without_extension, ".gitignore") == 0
    string_free(name)
    string_free(parent)
    string_free(extension)
    string_free(dotfile_extension)
    string_free(stem)
    string_free(dotfile_stem)
    string_free(without_extension)
    string_free(dotfile_without_extension)

    if valid && parent_valid && extension_valid && dotfile_valid && stem_valid && dotfile_stem_valid && without_extension_valid && dotfile_without_extension_valid {
        return 0
    }

    return 1
}
