import std.array
import std.mem

fn main() -> int {
    var items: *u8 = array_new(2usize, 4usize)
    let first: *u8 = alloc(2usize)
    let second: *u8 = alloc(2usize)
    let missing: *u8 = alloc(2usize)
    unsafe {
        if first == null || second == null || missing == null {
            return 1
        }
        let first16: *u16 = first as *u16
        let second16: *u16 = second as *u16
        let missing16: *u16 = missing as *u16
        *first16 = 0x0709
        *second16 = 0x0b0a
        *missing16 = 0x0f0e
        if array_push(items, first) != 0 || array_push(items, second) != 0 || array_push(items, first) != 0 {
            return 2
        }
        if array_index_of(items, second) != 1 {
            return 3
        }
        if array_last_index_of(items, first) != 2 {
            return 4
        }
        if !array_contains(items, second) || array_contains(items, missing) {
            return 5
        }
        if array_count(items, first) != 2usize || array_count(items, missing) != 0usize {
            return 6
        }
        array_free(items)
        free(first)
        free(second)
        free(missing)
    }
    return 0
}
