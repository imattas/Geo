import std.io

fn main() -> int {
    let handle = file_open_write("target/geo-flush-test.txt")
    if handle < 0 {
        return 1
    }
    if file_write(handle, "flushed") != 0 {
        file_close(handle)
        return 2
    }
    if file_flush(handle) != 0 {
        file_close(handle)
        return 3
    }
    return file_close(handle)
}
