enum TokenKind {
    Eof
    Ident
    Number
}

fn classify(kind: TokenKind) -> int {
    return match kind {
        TokenKind.Eof => 0
        TokenKind.Ident => 1
        TokenKind.Number => 2
        _ => 9
    }
}

fn main() -> int {
    return classify(TokenKind.Number) + 40
}
