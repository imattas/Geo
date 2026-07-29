import std.array

fn main() -> int {
    var items: *u8 = array_new(1usize, 4usize)
    let first: u8 = 3
    let second: u8 = 4
    let third: u8 = 8
    unsafe {
        if array_push(items, &first) != 0 {
            return 1
        }
        if array_push(items, &second) != 0 {
            return 2
        }
        if array_push(items, &third) != 0 {
            return 3
        }

        if array_truncate(items, 2usize) != 0 || array_len(items) != 2usize {
            return 4
        }
        let last: *u8 = array_pop(items)
        if last == 0 as *u8 || *last != 4 || array_len(items) != 1usize {
            return 5
        }
        let first_value: *u8 = array_pop_first(items)
        if first_value == 0 as *u8 || *first_value != 3 || array_len(items) != 0usize {
            return 6
        }
        if array_pop(items) != 0 as *u8 || array_pop_first(items) != 0 as *u8 {
            return 7
        }
        if array_truncate(items, 1usize) != 1 {
            return 8
        }
        array_free(items)
    }
    return 0
}
