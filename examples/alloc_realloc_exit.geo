import std.mem

fn main() -> int {
    let source: *u8 = alloc(4usize)
    if source == null {
        return 1
    }
    unsafe {
        *source = 41
        let second: *u8 = source + 1usize
        *second = 42
        let grown: *u8 = realloc(source, 8usize)
        if grown == null {
            return 2
        }
        if *grown != 41 {
            return 3
        }
        let grown_second: *u8 = grown + 1usize
        if *grown_second != 42 {
            return 4
        }
        let shrunk: *u8 = realloc(grown, 2usize)
        if shrunk == null || *shrunk != 41 {
            return 5
        }
        let shrunk_second: *u8 = shrunk + 1usize
        if *shrunk_second != 42 {
            return 6
        }
        if free(shrunk) != 0 {
            return 7
        }
    }
    return 0
}
