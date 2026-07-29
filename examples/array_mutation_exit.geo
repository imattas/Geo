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
        if array_swap(items, 0usize, 2usize) != 0 {
            return 4
        }
        let swapped: *u8 = array_get(items, 0usize)
        if swapped == 0 as *u8 || *swapped != 8 {
            return 5
        }
        if array_swap(items, 0usize, 9usize) != 1 {
            return 6
        }
        if array_swap_remove(items, 1usize) != 0 || array_len(items) != 2usize {
            return 7
        }
        let removed: *u8 = array_get(items, 1usize)
        if removed == 0 as *u8 || *removed != 3 {
            return 8
        }
        if array_insert(items, 1usize, &first) != 0 {
            return 16
        }
        let inserted: *u8 = array_get(items, 1usize)
        if inserted == 0 as *u8 || *inserted != 3 || array_len(items) != 3usize {
            return 17
        }
        if array_remove(items, 1usize) != 0 || array_len(items) != 2usize {
            return 18
        }
        if array_remove(items, 9usize) != 1 {
            return 19
        }
        let cloned: *u8 = array_clone(items)
        if cloned == 0 as *u8 {
            return 20
        }
        if array_copy(items, 0usize, cloned, 0usize, 2usize) != 0 {
            return 22
        }
        if array_copy(items, 1usize, cloned, 0usize, 2usize) != 1 {
            return 31
        }
        if array_extend(items, cloned) != 0 || array_len(items) != 4usize {
            return 21
        }
        let extended: *u8 = array_get(items, 3usize)
        if extended == 0 as *u8 {
            return 28
        }
        if *extended != 3 {
            return 29
        }
        array_free(cloned)
        if array_truncate(items, 2usize) != 0 {
            return 23
        }
        if array_truncate(items, 1usize) != 0 || array_len(items) != 1usize {
            return 24
        }
        let last: *u8 = array_pop(items)
        if last == 0 as *u8 || *last != 8 || array_len(items) != 0usize {
            return 10
        }
        if array_pop(items) != 0 as *u8 || array_pop_first(items) != 0 as *u8 {
            return 11
        }
        if array_push(items, &first) != 0 || array_push(items, &second) != 0 {
            return 12
        }
        let first_value: *u8 = array_pop_first(items)
        if first_value == 0 as *u8 || *first_value != 4 || array_len(items) != 1usize {
            return 13
        }
        let final_value: *u8 = array_pop(items)
        if final_value == 0 as *u8 || *final_value != 4 || array_len(items) != 0usize {
            return 14
        }
        if array_truncate(items, 1usize) != 1 {
            return 15
        }
        array_free(items)
    }
    return 0
}
