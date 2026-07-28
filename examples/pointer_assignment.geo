fn main() -> int {
    var x: int = 1
    unsafe {
        let p: *int = &x
        *p = 42
    }
    return x
}
