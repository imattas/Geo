import std.array

fn main() -> int {
    var items: *u8 = array_new(1usize, 4usize)
    var first: u8 = 3
    var second: u8 = 8
    var replacement: u8 = 13
    unsafe {
        if !array_is_empty(items) {
            return 1
        }
        if array_push(items, &first) != 0 {
            return 2
        }
        if array_push(items, &second) != 0 {
            return 3
        }
        if array_len(items) != 2usize || array_capacity(items) != 4usize {
            return 4
        }
        let value: *u8 = array_get(items, 1usize)
        if value == null {
            return 8
        }
        if *value != 8 {
            return 5
        }
        if array_set(items, 1usize, &replacement) != 0 {
            return 6
        }
        let updated: *u8 = array_get(items, 1usize)
        if *updated != 13 {
            return 7
        }
    }
    return 0
}
