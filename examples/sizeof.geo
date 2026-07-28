struct Buffer {
    ptr: *u8
    len: usize
}

fn main() -> usize {
    sizeof(Buffer)
}
