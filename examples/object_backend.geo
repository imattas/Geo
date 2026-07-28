fn main() -> int {
    var x: int = 0
    while x < 3 {
        x = x + 1
    }
    unsafe {
        let p: *int = &x
        *p = *p + 1
    }
    return (25 / 4) + (25 % 4) + (1 << 3 >> 1) + x
}
