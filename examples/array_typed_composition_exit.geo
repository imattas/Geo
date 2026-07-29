import std.array

fn main() -> int {
    var left: *u8 = array_new(2usize, 2usize)
    var right: *u8 = array_new(2usize, 2usize)
    let value: u8 = 7
    unsafe {
        if array_push(left, &value) != 0 || array_push(right, &value) != 0 {
            return 1
        }
        if array_extend(left, right) != 0 || array_len(left) != 2usize {
            return 2
        }
        let result: *u8 = array_get(left, 1usize)
        if result == 0 as *u8 || *result != 7 {
            return 3
        }
        if array_copy(left, 0usize, right, 0usize, 1usize) != 0 {
            return 4
        }
        array_free(left)
        array_free(right)
    }
    return 0
}
