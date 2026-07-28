fn main() -> int {
    let min_i8: i8 = -128
    let max_i8: i8 = 127
    let max_u8: u8 = 255
    let max_i16: i16 = 32_767
    let max_u16: u16 = 65_535

    return min_i8 as int + max_i8 as int + max_u8 as int + max_i16 as int + max_u16 as int
}
