struct Token {
    kind: int
    start: usize
    len: usize
}

fn parse_function() -> int {
    let first: Token = Token { kind: 1 start: 0usize len: 2usize }
    let second: Token = Token { kind: 2 start: 3usize len: 4usize }
    let third: Token = Token { kind: 3 start: 7usize len: 1usize }
    let tokens: [Token] = [first, second, third]
    var pos: usize = 0usize
    var error: int = 0
    if tokens[0].kind != 1 {
        error = 1
    } else {
        pos += 1usize
    }
    if error != 0 {
        return 1
    }
    if tokens[1].kind != 2 {
        error = 1
    } else {
        pos += 1usize
    }
    if error != 0 {
        return 2
    }
    if tokens[2].kind != 3 {
        error = 1
    } else {
        pos += 1usize
    }
    if error != 0 {
        return 3
    }
    return pos as int
}

fn main() -> int {
    return parse_function() - 3
}
