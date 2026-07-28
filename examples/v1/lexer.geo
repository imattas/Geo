struct Token {
    kind: int
    start: usize
    len: usize
}

fn classify(ch: char) -> int {
    if ch == 'f' {
        return 1
    } else {
        if ch == 'n' {
            return 2
        } else {
            return 0
        }
    }
}

fn main() -> int {
    let first: Token = Token { kind: classify('f') start: 0 len: 2 }
    let second: Token = Token { kind: classify('n') start: 3 len: 4 }
    let tokens: [Token] = [first, second]
    return tokens[1].kind
}
