import std.io
import std.string

fn main() -> int {
    if create_dir("target/geo-dir-path-test") != 0 {
        return 1
    }
    if write_file("target/geo-dir-path-test/only.txt", "path") < 0 {
        remove_dir("target/geo-dir-path-test")
        return 2
    }

    let child = dir_entry_path("target/geo-dir-path-test", 0)
    let valid = file_is_file(child)
    string_free(child)
    let file_status = remove_file("target/geo-dir-path-test/only.txt")
    let dir_status = remove_dir("target/geo-dir-path-test")
    if !valid {
        return 3
    }
    if file_status != 0 {
        return 4
    }
    return dir_status
}
