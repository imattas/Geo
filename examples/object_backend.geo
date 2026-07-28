fn combine(a: int, b: int) -> int {
    return a + b
}

fn seventh(a: int, b: int, c: int, d: int, e: int, f: int, g: int) -> int {
    return g
}

fn main() -> int {
    var x: int = 0
    while x < 3 {
        x = x + 1
    }
    unsafe {
        let p: *int = &x
        *p = *p + 1
    }
    return seventh(1, 2, 3, 4, 5, 6, combine((25 / 4) + (25 % 4), (1 << 3 >> 1) + x))
}
