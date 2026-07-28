import std.io

fn main() -> int {
    let write_status = write_file("geo-file-metadata-test.txt", "Geo")
    if write_status < 0 {
        return 1
    }
    if !file_is_file("geo-file-metadata-test.txt") {
        return 2
    }
    if file_is_dir("geo-file-metadata-test.txt") {
        return 3
    }
    if file_is_empty("geo-file-metadata-test.txt") {
        return 4
    }
    if file_size("geo-file-metadata-test.txt") != 3usize {
        return 5
    }
    return remove_file("geo-file-metadata-test.txt")
}
