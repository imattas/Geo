import std.io
import std.string

fn main() -> int {
    let reader = file_open("examples/read_file_fixture.txt")
    if reader < 0 {
        return 1
    }
    let contents = file_read_to_string(reader)
    let close_status = file_close(reader)
    let free_status = string_free(contents)
    if close_status != 0 {
        return close_status
    }
    return free_status
}
