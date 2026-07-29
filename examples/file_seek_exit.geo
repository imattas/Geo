import std.io

fn main() -> int {
    let handle = file_open_write("target/geo-seek-test.txt")
    if handle < 0 {
        return 1
    }

    if file_write(handle, "abcdef") != 0 {
        file_close(handle)
        return 2
    }
    if file_seek(handle, 2i64) != 0 {
        file_close(handle)
        return 3
    }
    if file_write(handle, "Z") != 0 {
        file_close(handle)
        return 4
    }
    return file_close(handle)
}
