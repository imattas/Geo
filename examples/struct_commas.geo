struct Token {
    kind: int,
    start: usize,
}

fn main() -> int {
    let token: Token = Token {
        kind: 42,
        start: 0,
    }

    return token.kind
}
