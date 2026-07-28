struct Buffer {
    len: usize
    capacity: usize
}

fn main() -> int {
    let empty: Buffer = Buffer { len: 0 capacity: 4 }
    let buffers: [Buffer] = [empty]
    let first_len: usize = buffers[0].len
    return 0
}
