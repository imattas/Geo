import std.array

fn main() -> int {
    var items: *u8 = array_new(1usize, 2usize)
    var first: u8 = 3
    var second: u8 = 8
    var replacement: u8 = 13
    unsafe {
        if array_push(items, &first) != 0 {
            return 1
        }
        if array_push(items, &second) != 0 {
            return 2
        }
        let grown: *u8 = array_reserve(items, 4usize)
        if grown == null || array_capacity(grown) != 4usize {
            return 3
        }
        let grown_first: u8 = *array_get(grown, 0usize)
        if grown_first != 3 {
            return 4
        }
        let grown_second: u8 = *array_get(grown, 1usize)
        if grown_second != 8 {
            return 4
        }
        let cloned: *u8 = array_clone(grown)
        if cloned == null || array_len(cloned) != 2usize {
            return 5
        }
        if array_set(grown, 0usize, &replacement) != 0 {
            return 6
        }
        let cloned_first: u8 = *array_get(cloned, 0usize)
        if cloned_first != 3 {
            return 7
        }
        if array_clear(grown) != 0 || array_len(grown) != 0usize {
            return 8
        }
        if array_capacity(grown) != 4usize {
            return 9
        }
        if array_free(cloned) != 0 || array_free(grown) != 0 {
            return 10
        }
    }
    return 0
}
