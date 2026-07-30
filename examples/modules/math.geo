pub type Word = int
pub const ANSWER: int = 42

pub struct Answer {
    value: Word
}

fn identity(value: int) -> int {
    return value
}

pub fn add(left: int, right: int) -> int {
    return identity(left) + right
}
