import std.io

fn main() -> int {
    let handle = file_open_write("geo-handle-test.txt")
    if handle < 0 {
        return 1
    }

    let write_status = file_write(handle, "Geo")
    let close_status = file_close(handle)
    if write_status != 0 {
        return write_status
    }

    return close_status
}
