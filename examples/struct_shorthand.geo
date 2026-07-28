struct Token {
    kind: int,
    start: usize,
}

fn main() -> int {
    let kind: int = 42
    let start: usize = 0
    let token: Token = Token {
        kind,
        start,
    }

    return token.kind
}
