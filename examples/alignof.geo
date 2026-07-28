struct Header {
    tag: u8
    next: *u8
}

fn main() -> usize {
    alignof(Header)
}
