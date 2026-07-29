import std.string

struct Token {
    kind: int
    start: usize
    len: usize
}

fn classify(byte: int) -> int {
    if byte == 102 {
        return 1
    } else {
        if byte == 110 {
            return 2
        } else {
            if byte == 109 {
                return 3
            } else {
                return 0
            }
        }
    }
}

fn lex(source: string) -> int {
    var token_count: int = 0
    var checksum: int = 0
    var in_token: bool = false

    for index in 0usize..string_len(source) {
        let byte: int = string_byte_at(source, index)
        let separator: bool = byte == 32 || byte == 9 || byte == 10
        if separator {
            in_token = false
        } else {
            if !in_token {
                token_count += 1
                in_token = true
            }
            checksum += classify(byte)
        }
    }

    return token_count * 10 + checksum
}

fn main() -> int {
    let first: Token = Token { kind: 1 start: 0usize len: 2usize }
    let second: Token = Token { kind: 2 start: 3usize len: 4usize }
    let lexed: int = lex("fn main")
    if first.kind != 1 || second.start != 3usize || lexed != 28 {
        return 1
    }
    return 0
}
