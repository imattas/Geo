struct Token {
    kind: int
    value: int
}

fn read_value(token: Token) -> int {
    return token.value
}

fn main() -> int {
    let token: Token = Token { kind: 7 value: 42 }
    return read_value(token)
}
