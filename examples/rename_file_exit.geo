import std.io

fn main() -> int {
    if write_file("target/geo-rename-source.txt", "renamed") < 0 {
        return 1
    }
    if rename_file("target/geo-rename-source.txt", "target/geo-rename-dest.txt") != 0 {
        remove_file("target/geo-rename-source.txt")
        return 2
    }
    if !file_exists("target/geo-rename-dest.txt") {
        return 3
    }
    let status = remove_file("target/geo-rename-dest.txt")
    if file_exists("target/geo-rename-source.txt") {
        return 4
    }
    return status
}
