import std.array

fn main() -> int {
    var items: *u8 = array_new(1usize, 4usize)
    let value: u8 = 8
    unsafe {
        if array_push(items, &value) != 0 || array_push(items, &value) != 0 {
            return 1
        }
        if array_resize(items, 4usize, &value) != 0 || array_len(items) != 4usize {
            return 2
        }
        let result: *u8 = array_get(items, 3usize)
        if result == 0 as *u8 || *result != 8 {
            return 3
        }
        if array_resize(items, 2usize, &value) != 0 || array_len(items) != 2usize {
            return 4
        }
        if array_resize(items, 5usize, &value) != 1 {
            return 5
        }
        array_free(items)
    }
    return 0
}
