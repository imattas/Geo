import std.io

fn main() -> int {
    if create_dir("target/geo-dir-count-test") != 0 {
        return 1
    }
    if write_file("target/geo-dir-count-test/one.txt", "one") < 0 {
        remove_dir("target/geo-dir-count-test")
        return 2
    }
    if write_file("target/geo-dir-count-test/two.txt", "two") < 0 {
        remove_file("target/geo-dir-count-test/one.txt")
        remove_dir("target/geo-dir-count-test")
        return 3
    }

    let count = dir_entry_count("target/geo-dir-count-test")
    let first_status = remove_file("target/geo-dir-count-test/one.txt")
    let second_status = remove_file("target/geo-dir-count-test/two.txt")
    let dir_status = remove_dir("target/geo-dir-count-test")
    if count != 2 {
        return 4
    }
    if first_status != 0 || second_status != 0 {
        return 5
    }
    return dir_status
}
