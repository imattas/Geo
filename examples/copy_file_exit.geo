import std.io
import std.string

fn main() -> int {
    if write_file("target/geo-copy-source.txt", "copied") < 0 {
        return 1
    }

    if copy_file("target/geo-copy-source.txt", "target/geo-copy-dest.txt") != 0 {
        remove_file("target/geo-copy-source.txt")
        return 2
    }
    if !file_exists("target/geo-copy-dest.txt") {
        remove_file("target/geo-copy-source.txt")
        return 3
    }
    if string_compare(read_file("target/geo-copy-dest.txt"), "copied") != 0 {
        remove_file("target/geo-copy-source.txt")
        remove_file("target/geo-copy-dest.txt")
        return 4
    }

    let source_status = remove_file("target/geo-copy-source.txt")
    let dest_status = remove_file("target/geo-copy-dest.txt")
    if source_status != 0 {
        return 5
    }
    return dest_status
}
