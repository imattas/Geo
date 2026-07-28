import std.io

fn main() -> int {
    let handle: int = file_open("examples/v1/file_echo.geo")
    let data: string = file_read(handle)
    file_write(1, data)
    return file_close(handle)
}
