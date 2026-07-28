fn main() -> int {
    unsafe {
        let first: *u32 = null
        let last: *u32 = first + 3

        if first < last && first <= last && last > first && last >= first {
            return 42
        }
    }

    return 1
}
