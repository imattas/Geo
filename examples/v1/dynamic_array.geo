struct Token {
    kind: int
    value: int
}

fn main() -> int {
    var index: usize = 1usize
    var tokens: [Token] = [
        Token { kind: 1 value: 10 },
        Token { kind: 2 value: 20 },
    ]

    tokens[index].value = 42
    tokens[index].kind += 3

    if tokens[index].value != 42 {
        return 1
    }
    return tokens[index].kind
}
