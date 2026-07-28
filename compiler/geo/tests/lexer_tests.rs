use geo::lexer::lex;
use geo::token::TokenKind;

#[test]
fn tokenizes_return_42() {
    let tokens = lex("fn main() -> int {\n    return 42\n}\n").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Ident("main".to_string()),
            TokenKind::LeftParen,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::Int,
            TokenKind::LeftBrace,
            TokenKind::Return,
            TokenKind::IntLiteral(42),
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn distinguishes_single_and_double_character_operators() {
    let tokens = lex("= == != < <= > >= ->").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Equal,
            TokenKind::EqualEqual,
            TokenKind::BangEqual,
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::Arrow,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn accepts_remainder_operator() {
    let tokens = lex("10 % 4").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(10),
            TokenKind::Percent,
            TokenKind::IntLiteral(4),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_prefixed_and_underscored_integer_literals() {
    let tokens = lex("0xff 0b1010 0o755 1_000").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(255),
            TokenKind::IntLiteral(10),
            TokenKind::IntLiteral(493),
            TokenKind::IntLiteral(1000),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_typed_integer_literal_suffixes() {
    let tokens = lex("255u8 16usize").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::TypedIntLiteral(255, "u8".to_string()),
            TokenKind::TypedIntLiteral(16, "usize".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn rejects_invalid_binary_integer_literal() {
    let err = lex("0b102").unwrap_err();

    assert!(err[0].message.contains("invalid binary integer literal"));
}

#[test]
fn rejects_invalid_octal_integer_literal() {
    let err = lex("0o789").unwrap_err();

    assert!(err[0].message.contains("invalid octal integer literal"));
}

#[test]
fn tokenizes_sizeof_keyword() {
    let tokens = lex("sizeof(int)").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Sizeof,
            TokenKind::LeftParen,
            TokenKind::Int,
            TokenKind::RightParen,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_alignof_keyword() {
    let tokens = lex("alignof(*u8)").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Alignof,
            TokenKind::LeftParen,
            TokenKind::Star,
            TokenKind::U8,
            TokenKind::RightParen,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_offsetof_keyword() {
    let tokens = lex("offsetof(Header, next)").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Offsetof,
            TokenKind::LeftParen,
            TokenKind::Ident("Header".to_string()),
            TokenKind::Comma,
            TokenKind::Ident("next".to_string()),
            TokenKind::RightParen,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_null_keyword() {
    let tokens = lex("let p: *u8 = null").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Let,
            TokenKind::Ident("p".to_string()),
            TokenKind::Colon,
            TokenKind::Star,
            TokenKind::U8,
            TokenKind::Equal,
            TokenKind::Null,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_compound_assignment_operators() {
    let tokens = lex("+= -= *= /= %= &= |= ^= <<= >>=").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::PlusEqual,
            TokenKind::MinusEqual,
            TokenKind::StarEqual,
            TokenKind::SlashEqual,
            TokenKind::PercentEqual,
            TokenKind::AmpersandEqual,
            TokenKind::PipeEqual,
            TokenKind::CaretEqual,
            TokenKind::ShiftLeftEqual,
            TokenKind::ShiftRightEqual,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_boolean_logic_operators() {
    let tokens = lex("true && false || true").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::True,
            TokenKind::AmpersandAmpersand,
            TokenKind::False,
            TokenKind::PipePipe,
            TokenKind::True,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_bitwise_operators() {
    let tokens = lex("10 & 6 | 1 ^ 3").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(10),
            TokenKind::Ampersand,
            TokenKind::IntLiteral(6),
            TokenKind::Pipe,
            TokenKind::IntLiteral(1),
            TokenKind::Caret,
            TokenKind::IntLiteral(3),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_shift_operators() {
    let tokens = lex("1 << 3 >> 1").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::IntLiteral(1),
            TokenKind::ShiftLeft,
            TokenKind::IntLiteral(3),
            TokenKind::ShiftRight,
            TokenKind::IntLiteral(1),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_bitwise_not_operator() {
    let tokens = lex("~10").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![TokenKind::Tilde, TokenKind::IntLiteral(10), TokenKind::Eof]
    );
}

#[test]
fn tokenizes_as_cast_keyword() {
    let tokens = lex("value as i32").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Ident("value".to_string()),
            TokenKind::As,
            TokenKind::I32,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_for_in_range_syntax() {
    let tokens = lex("for i in 0..10").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::For,
            TokenKind::Ident("i".to_string()),
            TokenKind::In,
            TokenKind::IntLiteral(0),
            TokenKind::DotDot,
            TokenKind::IntLiteral(10),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_inclusive_for_range_syntax() {
    let tokens = lex("for i in 0..=10").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::For,
            TokenKind::Ident("i".to_string()),
            TokenKind::In,
            TokenKind::IntLiteral(0),
            TokenKind::DotDotEqual,
            TokenKind::IntLiteral(10),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_loop_keyword() {
    let tokens = lex("loop { break }").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Loop,
            TokenKind::LeftBrace,
            TokenKind::Break,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_const_keyword() {
    let tokens = lex("const LIMIT: int = 42").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Const,
            TokenKind::Ident("LIMIT".to_string()),
            TokenKind::Colon,
            TokenKind::Int,
            TokenKind::Equal,
            TokenKind::IntLiteral(42),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_type_alias_keyword() {
    let tokens = lex("type Byte = u8").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Type,
            TokenKind::Ident("Byte".to_string()),
            TokenKind::Equal,
            TokenKind::U8,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_enum_keyword() {
    let tokens = lex("enum TokenKind { Eof Ident Number }").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Enum,
            TokenKind::Ident("TokenKind".to_string()),
            TokenKind::LeftBrace,
            TokenKind::Ident("Eof".to_string()),
            TokenKind::Ident("Ident".to_string()),
            TokenKind::Ident("Number".to_string()),
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_match_expression_syntax() {
    let tokens = lex("match kind { TokenKind.Eof => 0 _ => 1 }").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Match,
            TokenKind::Ident("kind".to_string()),
            TokenKind::LeftBrace,
            TokenKind::Ident("TokenKind".to_string()),
            TokenKind::Dot,
            TokenKind::Ident("Eof".to_string()),
            TokenKind::FatArrow,
            TokenKind::IntLiteral(0),
            TokenKind::Ident("_".to_string()),
            TokenKind::FatArrow,
            TokenKind::IntLiteral(1),
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tracks_line_and_column() {
    let tokens = lex("fn\n  main").unwrap();
    assert_eq!(tokens[1].span.line, 2);
    assert_eq!(tokens[1].span.column, 3);
}

#[test]
fn rejects_unknown_characters() {
    let err = lex("@").unwrap_err();
    assert!(err[0].message.contains("unexpected character '@'"));
}

#[test]
fn skips_comments_and_reads_v1_literals() {
    let tokens = lex(
        "// hi\n/* block */ string char usize i32 u8 \"Geo\\n\" 'G' ! & mut unsafe break continue",
    )
    .unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::String,
            TokenKind::Char,
            TokenKind::Usize,
            TokenKind::I32,
            TokenKind::U8,
            TokenKind::StringLiteral("Geo\n".to_string()),
            TokenKind::CharLiteral('G'),
            TokenKind::Bang,
            TokenKind::Ampersand,
            TokenKind::Mut,
            TokenKind::Unsafe,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn skips_nested_block_comments() {
    let tokens = lex("fn /* outer /* inner */ still outer */ main").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Ident("main".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_carriage_return_and_nul_escapes() {
    let tokens = lex("\"A\\r\\0B\" '\\r' '\\0'").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral("A\r\0B".to_string()),
            TokenKind::CharLiteral('\r'),
            TokenKind::CharLiteral('\0'),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_hex_byte_escapes() {
    let tokens = lex("\"A\\x0d\\x00B\" '\\x41'").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral("A\r\0B".to_string()),
            TokenKind::CharLiteral('A'),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_unicode_escapes() {
    let tokens = lex("\"lambda: \\u{03bb}\" '\\u{1f600}'").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral("lambda: \u{03bb}".to_string()),
            TokenKind::CharLiteral('\u{1f600}'),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_raw_string_literals() {
    let tokens = lex(r#"r"C:\temp\n.txt" r"slashes \\ stay""#).unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral(r"C:\temp\n.txt".to_string()),
            TokenKind::StringLiteral(r"slashes \\ stay".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_hash_raw_string_literals_with_quotes() {
    let tokens = lex(r##"r#"quote: " and slash: \"#"##).unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::StringLiteral(r#"quote: " and slash: \"#.to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_aggregate_syntax() {
    let tokens = lex("struct Token { values: [int] view: []int } x.y a[0]").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Struct,
            TokenKind::Ident("Token".to_string()),
            TokenKind::LeftBrace,
            TokenKind::Ident("values".to_string()),
            TokenKind::Colon,
            TokenKind::LeftBracket,
            TokenKind::Int,
            TokenKind::RightBracket,
            TokenKind::Ident("view".to_string()),
            TokenKind::Colon,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::Int,
            TokenKind::RightBrace,
            TokenKind::Ident("x".to_string()),
            TokenKind::Dot,
            TokenKind::Ident("y".to_string()),
            TokenKind::Ident("a".to_string()),
            TokenKind::LeftBracket,
            TokenKind::IntLiteral(0),
            TokenKind::RightBracket,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_import_and_extern_keywords() {
    let tokens = lex("import std.io extern fn puts(message: *u8) -> int").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Import,
            TokenKind::Ident("std".to_string()),
            TokenKind::Dot,
            TokenKind::Ident("io".to_string()),
            TokenKind::Extern,
            TokenKind::Fn,
            TokenKind::Ident("puts".to_string()),
            TokenKind::LeftParen,
            TokenKind::Ident("message".to_string()),
            TokenKind::Colon,
            TokenKind::Star,
            TokenKind::U8,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::Int,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_clean_core_syntax_tokens() {
    let tokens = lex("import std.io; fn main() { var name: str = \"Geo\"; }").unwrap();
    let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Import,
            TokenKind::Ident("std".to_string()),
            TokenKind::Dot,
            TokenKind::Ident("io".to_string()),
            TokenKind::Semicolon,
            TokenKind::Fn,
            TokenKind::Ident("main".to_string()),
            TokenKind::LeftParen,
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::Var,
            TokenKind::Ident("name".to_string()),
            TokenKind::Colon,
            TokenKind::Str,
            TokenKind::Equal,
            TokenKind::StringLiteral("Geo".to_string()),
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
}
