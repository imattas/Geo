use crate::token::{Span, Token, TokenKind};
use geo_diagnostics::Diagnostic;

pub fn lex(source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer::new(source);
    lexer.lex_all()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,
    index: usize,
    offset: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            index: 0,
            offset: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex_all(&mut self) -> Result<Vec<Token>, Vec<Diagnostic>> {
        while let Some(ch) = self.peek() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => self.advance_newline(),
                '0'..='9' => self.lex_number(),
                'r' if self.is_raw_string_start() => self.lex_raw_string(),
                'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
                '[' => self.single(TokenKind::LeftBracket),
                ']' => self.single(TokenKind::RightBracket),
                ':' => self.single(TokenKind::Colon),
                ',' => self.single(TokenKind::Comma),
                '.' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('.') {
                        if self.match_char('=') {
                            self.push(TokenKind::DotDotEqual, span, 3);
                        } else {
                            self.push(TokenKind::DotDot, span, 2);
                        }
                    } else {
                        self.push(TokenKind::Dot, span, 1);
                    }
                }
                ';' => self.single(TokenKind::Semicolon),
                '&' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('&') {
                        self.push(TokenKind::AmpersandAmpersand, span, 2);
                    } else if self.match_char('=') {
                        self.push(TokenKind::AmpersandEqual, span, 2);
                    } else {
                        self.push(TokenKind::Ampersand, span, 1);
                    }
                }
                '|' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('|') {
                        self.push(TokenKind::PipePipe, span, 2);
                    } else if self.match_char('=') {
                        self.push(TokenKind::PipeEqual, span, 2);
                    } else {
                        self.push(TokenKind::Pipe, span, 1);
                    }
                }
                '^' => self.two_char(TokenKind::Caret, '=', TokenKind::CaretEqual),
                '+' => self.two_char(TokenKind::Plus, '=', TokenKind::PlusEqual),
                '*' => self.two_char(TokenKind::Star, '=', TokenKind::StarEqual),
                '%' => self.two_char(TokenKind::Percent, '=', TokenKind::PercentEqual),
                '/' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('/') {
                        self.skip_line_comment();
                    } else if self.match_char('*') {
                        self.skip_block_comment();
                    } else if self.match_char('=') {
                        self.push(TokenKind::SlashEqual, span, 2);
                    } else {
                        self.push(TokenKind::Slash, span, 1);
                    }
                }
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                '-' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('>') {
                        self.push(TokenKind::Arrow, span, 2);
                    } else if self.match_char('=') {
                        self.push(TokenKind::MinusEqual, span, 2);
                    } else {
                        self.push(TokenKind::Minus, span, 1);
                    }
                }
                '=' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('=') {
                        self.push(TokenKind::EqualEqual, span, 2);
                    } else if self.match_char('>') {
                        self.push(TokenKind::FatArrow, span, 2);
                    } else {
                        self.push(TokenKind::Equal, span, 1);
                    }
                }
                '!' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('=') {
                        self.push(TokenKind::BangEqual, span, 2);
                    } else {
                        self.push(TokenKind::Bang, span, 1);
                    }
                }
                '~' => self.single(TokenKind::Tilde),
                '<' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('<') {
                        if self.match_char('=') {
                            self.push(TokenKind::ShiftLeftEqual, span, 3);
                        } else {
                            self.push(TokenKind::ShiftLeft, span, 2);
                        }
                    } else if self.match_char('=') {
                        self.push(TokenKind::LessEqual, span, 2);
                    } else {
                        self.push(TokenKind::Less, span, 1);
                    }
                }
                '>' => {
                    let span = self.start_span();
                    self.advance();
                    if self.match_char('>') {
                        if self.match_char('=') {
                            self.push(TokenKind::ShiftRightEqual, span, 3);
                        } else {
                            self.push(TokenKind::ShiftRight, span, 2);
                        }
                    } else if self.match_char('=') {
                        self.push(TokenKind::GreaterEqual, span, 2);
                    } else {
                        self.push(TokenKind::Greater, span, 1);
                    }
                }
                other => {
                    self.diagnostics.push(
                        Diagnostic::error(format!("unexpected character '{other}'"))
                            .with_span(self.offset, other.len_utf8()),
                    );
                    self.advance();
                }
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: self.start_span(),
        });

        if self.diagnostics.is_empty() {
            Ok(std::mem::take(&mut self.tokens))
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        self.offset += ch.len_utf8();
        self.column += 1;
        Some(ch)
    }

    fn advance_newline(&mut self) {
        self.index += 1;
        self.offset += 1;
        self.line += 1;
        self.column = 1;
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn is_raw_string_start(&self) -> bool {
        let mut index = self.index + 1;
        while self.chars.get(index) == Some(&'#') {
            index += 1;
        }
        self.chars.get(index) == Some(&'"')
    }

    fn start_span(&self) -> Span {
        Span {
            line: self.line,
            column: self.column,
            offset: self.offset,
            len: 1,
        }
    }

    fn push(&mut self, kind: TokenKind, mut span: Span, len: usize) {
        span.len = len;
        self.tokens.push(Token { kind, span });
    }

    fn single(&mut self, kind: TokenKind) {
        let span = self.start_span();
        self.advance();
        self.push(kind, span, 1);
    }

    fn two_char(&mut self, single: TokenKind, second: char, double: TokenKind) {
        let span = self.start_span();
        self.advance();
        if self.match_char(second) {
            self.push(double, span, 2);
        } else {
            self.push(single, span, 1);
        }
    }

    fn lex_number(&mut self) {
        let span = self.start_span();
        let start = self.offset;
        let mut base = 10;
        let mut prefix = "";

        if self.peek() == Some('0') {
            self.advance();
            match self.peek() {
                Some('x' | 'X') => {
                    self.advance();
                    base = 16;
                    prefix = "hexadecimal ";
                }
                Some('b' | 'B') => {
                    self.advance();
                    base = 2;
                    prefix = "binary ";
                }
                Some('o' | 'O') => {
                    self.advance();
                    base = 8;
                    prefix = "octal ";
                }
                _ => {}
            }
        }

        while self.peek().is_some_and(|ch| {
            ch == '_'
                || match base {
                    16 => ch.is_ascii_hexdigit(),
                    8 => matches!(ch, '0'..='7'),
                    2 => matches!(ch, '0' | '1'),
                    _ => ch.is_ascii_digit(),
                }
        }) {
            self.advance();
        }

        let digits_end = self.offset;
        let mut suffix = None;
        if self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            let suffix_start = self.offset;
            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                self.advance();
            }
            suffix = Some(self.source[suffix_start..self.offset].to_string());
        } else if self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            while self.peek().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
                self.advance();
            }
        }

        let text = &self.source[start..self.offset];
        let digit_text = &self.source[start..digits_end];
        let digits = match base {
            16 => &digit_text[2..],
            8 => &digit_text[2..],
            2 => &digit_text[2..],
            _ => digit_text,
        };
        let cleaned: String = digits.chars().filter(|ch| *ch != '_').collect();
        if cleaned.is_empty()
            || !cleaned.chars().all(|ch| ch.to_digit(base).is_some())
            || (suffix.is_none()
                && text[digits_end - start..]
                    .chars()
                    .any(|ch| ch.is_ascii_alphanumeric()))
        {
            self.diagnostics.push(
                Diagnostic::error(format!("invalid {prefix}integer literal '{text}'"))
                    .with_span(span.offset, span.len),
            );
            return;
        }
        let Ok(value) = i64::from_str_radix(&cleaned, base) else {
            self.diagnostics.push(
                Diagnostic::error(format!("integer literal '{text}' is out of range"))
                    .with_span(span.offset, span.len),
            );
            return;
        };
        let kind = match suffix {
            Some(suffix) => TokenKind::TypedIntLiteral(value, suffix),
            None => TokenKind::IntLiteral(value),
        };
        self.push(kind, span, text.len());
    }

    fn lex_identifier(&mut self) {
        let span = self.start_span();
        let start = self.offset;
        while matches!(self.peek(), Some('a'..='z' | 'A'..='Z' | '0'..='9' | '_')) {
            self.advance();
        }
        let text = &self.source[start..self.offset];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "pub" => TokenKind::Pub,
            "import" => TokenKind::Import,
            "extern" => TokenKind::Extern,
            "return" => TokenKind::Return,
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "const" => TokenKind::Const,
            "type" => TokenKind::Type,
            "mut" => TokenKind::Mut,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "loop" => TokenKind::Loop,
            "match" => TokenKind::Match,
            "unsafe" => TokenKind::Unsafe,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "as" => TokenKind::As,
            "sizeof" => TokenKind::Sizeof,
            "alignof" => TokenKind::Alignof,
            "offsetof" => TokenKind::Offsetof,
            "null" => TokenKind::Null,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "int" => TokenKind::Int,
            "bool" => TokenKind::Bool,
            "char" => TokenKind::Char,
            "string" => TokenKind::String,
            "str" => TokenKind::Str,
            "usize" => TokenKind::Usize,
            "i8" => TokenKind::I8,
            "i16" => TokenKind::I16,
            "i32" => TokenKind::I32,
            "i64" => TokenKind::I64,
            "u8" => TokenKind::U8,
            "u16" => TokenKind::U16,
            "u32" => TokenKind::U32,
            "u64" => TokenKind::U64,
            _ => TokenKind::Ident(text.to_string()),
        };
        self.push(kind, span, text.len());
    }

    fn skip_line_comment(&mut self) {
        while !matches!(self.peek(), None | Some('\n')) {
            self.advance();
        }
    }

    fn skip_block_comment(&mut self) {
        let mut depth = 1usize;
        loop {
            match self.peek() {
                None => {
                    self.diagnostics
                        .push(Diagnostic::error("unterminated block comment"));
                    break;
                }
                Some('/') if self.peek_next() == Some('*') => {
                    self.advance();
                    self.advance();
                    depth += 1;
                }
                Some('*') => {
                    self.advance();
                    if self.match_char('/') {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                Some('\n') => self.advance_newline(),
                Some(_) => {
                    self.advance();
                }
            }
        }
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn lex_string(&mut self) {
        let span = self.start_span();
        self.advance();
        let mut value = String::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    self.diagnostics
                        .push(Diagnostic::error("unterminated string literal"));
                    break;
                }
                Some('"') => {
                    self.advance();
                    let len = self.offset - span.offset;
                    self.push(TokenKind::StringLiteral(value), span, len);
                    break;
                }
                Some('\\') => {
                    self.advance();
                    match self.peek() {
                        Some('n') => {
                            value.push('\n');
                            self.advance();
                        }
                        Some('t') => {
                            value.push('\t');
                            self.advance();
                        }
                        Some('r') => {
                            value.push('\r');
                            self.advance();
                        }
                        Some('0') => {
                            value.push('\0');
                            self.advance();
                        }
                        Some('x') => {
                            self.advance();
                            if let Some(ch) = self.lex_hex_escape("string") {
                                value.push(ch);
                            }
                        }
                        Some('u') => {
                            self.advance();
                            if let Some(ch) = self.lex_unicode_escape("string") {
                                value.push(ch);
                            }
                        }
                        Some('"') => {
                            value.push('"');
                            self.advance();
                        }
                        Some('\\') => {
                            value.push('\\');
                            self.advance();
                        }
                        Some(other) => {
                            self.diagnostics.push(Diagnostic::error(format!(
                                "unsupported string escape '\\{other}'"
                            )));
                            self.advance();
                        }
                        None => {
                            self.diagnostics
                                .push(Diagnostic::error("unterminated string literal"));
                            break;
                        }
                    }
                }
                Some(ch) => {
                    value.push(ch);
                    self.advance();
                }
            }
        }
    }

    fn lex_raw_string(&mut self) {
        let span = self.start_span();
        self.advance();
        let mut hash_count = 0usize;
        while self.peek() == Some('#') {
            self.advance();
            hash_count += 1;
        }
        self.advance();
        let mut value = String::new();

        loop {
            match self.peek() {
                None => {
                    self.diagnostics
                        .push(Diagnostic::error("unterminated raw string literal"));
                    break;
                }
                Some('"') => {
                    if self.raw_string_close_matches(hash_count) {
                        self.advance();
                        for _ in 0..hash_count {
                            self.advance();
                        }
                        let len = self.offset - span.offset;
                        self.push(TokenKind::StringLiteral(value), span, len);
                        break;
                    }
                    value.push('"');
                    self.advance();
                }
                Some('\n') => {
                    value.push('\n');
                    self.advance_newline();
                }
                Some(ch) => {
                    value.push(ch);
                    self.advance();
                }
            }
        }
    }

    fn raw_string_close_matches(&self, hash_count: usize) -> bool {
        for offset in 1..=hash_count {
            if self.chars.get(self.index + offset) != Some(&'#') {
                return false;
            }
        }
        true
    }

    fn lex_char(&mut self) {
        let span = self.start_span();
        self.advance();
        let value = match self.peek() {
            Some('\\') => {
                self.advance();
                match self.peek() {
                    Some('n') => {
                        self.advance();
                        Some('\n')
                    }
                    Some('t') => {
                        self.advance();
                        Some('\t')
                    }
                    Some('r') => {
                        self.advance();
                        Some('\r')
                    }
                    Some('0') => {
                        self.advance();
                        Some('\0')
                    }
                    Some('x') => {
                        self.advance();
                        self.lex_hex_escape("char")
                    }
                    Some('u') => {
                        self.advance();
                        self.lex_unicode_escape("char")
                    }
                    Some('\'') => {
                        self.advance();
                        Some('\'')
                    }
                    Some('\\') => {
                        self.advance();
                        Some('\\')
                    }
                    Some(other) => {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "unsupported char escape '\\{other}'"
                        )));
                        self.advance();
                        None
                    }
                    None => None,
                }
            }
            Some(ch) if ch != '\'' && ch != '\n' => {
                self.advance();
                Some(ch)
            }
            _ => None,
        };

        if !self.match_char('\'') {
            self.diagnostics
                .push(Diagnostic::error("unterminated char literal"));
            return;
        }

        if let Some(value) = value {
            let len = self.offset - span.offset;
            self.push(TokenKind::CharLiteral(value), span, len);
        }
    }

    fn lex_hex_escape(&mut self, literal_kind: &str) -> Option<char> {
        let Some(high) = self.peek() else {
            self.diagnostics.push(Diagnostic::error(format!(
                "incomplete {literal_kind} hex escape"
            )));
            return None;
        };
        self.advance();

        let Some(low) = self.peek() else {
            self.diagnostics.push(Diagnostic::error(format!(
                "incomplete {literal_kind} hex escape"
            )));
            return None;
        };
        self.advance();

        let Some(high) = high.to_digit(16) else {
            self.diagnostics.push(Diagnostic::error(format!(
                "invalid {literal_kind} hex escape"
            )));
            return None;
        };
        let Some(low) = low.to_digit(16) else {
            self.diagnostics.push(Diagnostic::error(format!(
                "invalid {literal_kind} hex escape"
            )));
            return None;
        };

        Some(char::from(((high << 4) | low) as u8))
    }

    fn lex_unicode_escape(&mut self, literal_kind: &str) -> Option<char> {
        if !self.match_char('{') {
            self.diagnostics.push(Diagnostic::error(format!(
                "unicode {literal_kind} escape requires '{{'"
            )));
            return None;
        }

        let mut value = 0u32;
        let mut digits = 0usize;
        loop {
            match self.peek() {
                Some('}') => {
                    self.advance();
                    break;
                }
                Some(ch) if ch.is_ascii_hexdigit() => {
                    if digits == 6 {
                        self.diagnostics.push(Diagnostic::error(format!(
                            "unicode {literal_kind} escape has too many digits"
                        )));
                        self.advance();
                        return None;
                    }
                    value = (value << 4) | ch.to_digit(16).expect("hex digit checked");
                    digits += 1;
                    self.advance();
                }
                Some(_) => {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "invalid unicode {literal_kind} escape"
                    )));
                    self.advance();
                    return None;
                }
                None => {
                    self.diagnostics.push(Diagnostic::error(format!(
                        "unterminated unicode {literal_kind} escape"
                    )));
                    return None;
                }
            }
        }

        if digits == 0 {
            self.diagnostics.push(Diagnostic::error(format!(
                "unicode {literal_kind} escape requires hex digits"
            )));
            return None;
        }

        let Some(ch) = char::from_u32(value) else {
            self.diagnostics.push(Diagnostic::error(format!(
                "unicode {literal_kind} escape is not a valid scalar value"
            )));
            return None;
        };
        Some(ch)
    }
}
