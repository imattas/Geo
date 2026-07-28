import std.io

fn main() -> int {
    let handle: int = file_open("examples/v1/file_echo.geo")
    let data: string = file_read_to_string(handle)
    print(data)
    return file_close(handle)
}
