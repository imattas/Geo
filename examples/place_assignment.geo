struct Token {
    kind: int
}

fn main() -> int {
    var token: Token = Token { kind: 1 }
    var values: [int] = [1]
    token.kind = 2
    values[0] += token.kind
    return values[0] + 39
}
