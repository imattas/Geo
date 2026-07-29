import std.io

fn main() -> int {
    if create_dir("target/geo-directory-test") != 0 {
        return 1
    }
    if !file_is_dir("target/geo-directory-test") {
        remove_dir("target/geo-directory-test")
        return 2
    }
    return remove_dir("target/geo-directory-test")
}
