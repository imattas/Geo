fn main() -> usize {
    unsafe {
        var ptr: *u32 = null
        ptr += 3
        ptr -= 1
        return ptr as usize
    }
}
