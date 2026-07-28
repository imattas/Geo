import std.io

fn main() -> int {
    let write_status = write_file("geo-file-exists-test.txt", "Geo")
    if write_status < 0 {
        return write_status
    }
    if !file_exists("geo-file-exists-test.txt") {
        return 1
    }

    return remove_file("geo-file-exists-test.txt")
}
