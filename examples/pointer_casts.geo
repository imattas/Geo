fn main() -> int {
    let ptr: *u8 = null
    let addr: usize = ptr as usize

    unsafe {
        let roundtrip: *u8 = addr as *u8
        if roundtrip == null {
            return 42
        }
    }

    return 1
}
