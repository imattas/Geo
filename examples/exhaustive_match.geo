enum TokenKind {
    Eof
    Number
}

fn classify(kind: TokenKind) -> int {
    match kind {
        TokenKind.Eof => 0
        TokenKind.Number => 42
    }
}

fn main() -> int {
    classify(TokenKind.Number)
}
