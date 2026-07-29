import std.io
import std.string

fn main() -> int {
    if create_dir("target/geo-dir-name-test") != 0 {
        return 1
    }
    if write_file("target/geo-dir-name-test/only.txt", "name") < 0 {
        remove_dir("target/geo-dir-name-test")
        return 2
    }

    let name = dir_entry_name("target/geo-dir-name-test", 0)
    let comparison = string_compare(name, "only.txt")
    string_free(name)
    let file_status = remove_file("target/geo-dir-name-test/only.txt")
    let dir_status = remove_dir("target/geo-dir-name-test")
    if comparison != 0 {
        return 3
    }
    if file_status != 0 {
        return 4
    }
    return dir_status
}
