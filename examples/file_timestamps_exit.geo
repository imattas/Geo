import std.io

fn main() -> int {
    if write_file("target/geo-file-timestamps.txt", "timestamps") < 0 {
        return 1
    }

    let accessed = file_accessed_time("target/geo-file-timestamps.txt")
    let modified = file_modified_time("target/geo-file-timestamps.txt")
    let created = file_created_time("target/geo-file-timestamps.txt")
    let status = remove_file("target/geo-file-timestamps.txt")
    if accessed == 0 || modified == 0 || created == 0 {
        return 2
    }
    return status
}
