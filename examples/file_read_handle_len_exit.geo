import std.io
import std.string

fn main() -> int {
    let writer = file_open_write("geo-handle-read-test.txt")
    if writer < 0 {
        return 1
    }
    let write_status = file_write(writer, "Geo")
    file_close(writer)
    if write_status != 0 {
        return write_status
    }

    let reader = file_open("geo-handle-read-test.txt")
    if reader < 0 {
        return 1
    }
    let contents = file_read_to_string(reader)
    let close_status = file_close(reader)
    if close_status != 0 {
        return close_status
    }

    return string_len(contents) as int
}
