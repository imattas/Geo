fn main() -> int {
    unsafe {
        let first: *u32 = null
        let last: *u32 = first + 3
        if last - first == 3 {
            return 42
        }
    }

    return 1
}
