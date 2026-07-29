import std.array
import std.mem

fn main() -> int {
    var left: *u8 = array_new(2usize, 4usize)
    var right: *u8 = array_new(2usize, 2usize)
    let value: *u8 = alloc(2usize)
    let replacement: *u8 = alloc(2usize)
    unsafe {
        if value == 0 as *u8 || replacement == 0 as *u8 {
            return 1
        }
        let value16: *u16 = value as *u16
        let replacement16: *u16 = replacement as *u16
        *value16 = 0x0709
        *replacement16 = 0x0b0a
        if *value != 9 {
            return 10
        }
        if array_push(left, value) != 0 || array_push(right, value) != 0 {
            return 1
        }
        let pushed: *u8 = array_get(left, 0usize)
        if pushed == 0 as *u8 || *pushed != 9 {
            return 11
        }
        if array_extend(left, right) != 0 || array_len(left) != 2usize {
            return 2
        }
        let result: *u8 = array_get(left, 1usize)
        let result16: *u16 = result as *u16
        if result == 0 as *u8 {
            return 3
        }
        if *result != 9 {
            return 4
        }
        if result16 == 0 as *u16 || *result16 != 0x0709 {
            return 5
        }
        if array_set(left, 0usize, replacement) != 0 {
            return 5
        }
        let set_value: *u8 = array_get(left, 0usize)
        let set16: *u16 = set_value as *u16
        if set_value == 0 as *u8 || set16 == 0 as *u16 || *set16 != 0x0b0a {
            return 6
        }
        if array_resize(left, 3usize, replacement) != 0 {
            return 14
        }
        let resized: *u8 = array_get(left, 2usize)
        let resized16: *u16 = resized as *u16
        if resized == 0 as *u8 || resized16 == 0 as *u16 || *resized16 != 0x0b0a {
            return 15
        }
        if array_fill(right, replacement) != 0 {
            return 7
        }
        let filled: *u8 = array_get(right, 0usize)
        let filled16: *u16 = filled as *u16
        if filled == 0 as *u8 {
            return 8
        }
        if *filled != 10 {
            return 12
        }
        if filled16 == 0 as *u16 || *filled16 != 0x0b0a {
            return 13
        }
        if array_copy(left, 0usize, right, 0usize, 1usize) != 0 {
            return 9
        }
        array_free(left)
        array_free(right)
    }
    return 0
}
