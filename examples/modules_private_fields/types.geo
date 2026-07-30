pub struct Data {
    secret: int
    pub visible: int
}

pub fn make() -> Data {
    return Data { secret: 41 visible: 1 }
}
