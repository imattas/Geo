import std.io

fn main() -> int {
    write_file("target/geo-truncate-file.txt", "abcdef")
    if truncate_file("target/geo-truncate-file.txt", 3usize) != 0 {
        return 1
    }
    return file_size("target/geo-truncate-file.txt") as int
}
