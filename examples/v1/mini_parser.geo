import std.string
import std.array
import std.mem

struct FunctionNode {
    name_kind: int
    return_kind: int
    literal_value: int
}

struct Token {
    kind: int
    start: usize
    len: usize
}

fn is_space(byte: int) -> bool {
    return byte == 32 || byte == 9 || byte == 10 || byte == 13
}

fn skip_space(source: &string, start: usize) -> usize {
    var cursor: usize = start
    while cursor < string_len(*source) && is_space(string_byte_at(*source, cursor)) {
        cursor += 1usize
    }
    return cursor
}

fn token_kind(source: &string, start: usize, len: usize) -> int {
    let first: int = string_byte_at(*source, start)
    if len == 1usize {
        let byte: int = first
        if byte == 40 {
            return 7
        }
        if byte == 41 {
            return 8
        }
        if byte == 123 {
            return 9
        }
        if byte == 125 {
            return 10
        }
    }
    if first >= 48 && first <= 57 {
        return 6
    }
    if len == 2usize && first == 102 {
        return 1
    }
    if len == 2usize && first == 45 {
        return 3
    }
    if len == 3usize && first == 105 {
        return 4
    }
    if len == 4usize && first == 109 {
        return 2
    }
    if len == 6usize && first == 114 {
        return 5
    }
    return 0
}

fn make_token(kind: int, start: usize, len: usize) -> *u8 {
    let token: *u8 = alloc(sizeof(Token))
    unsafe {
        let kind_ptr: *int = token as *int
        let start_ptr: *usize = (token + 8usize) as *usize
        let len_ptr: *usize = (token + 16usize) as *usize
        *kind_ptr = kind
        *start_ptr = start
        *len_ptr = len
    }
    return token
}

fn token_kind_from_ptr(token: *u8) -> int {
    unsafe {
        let kind_ptr: *int = token as *int
        return *kind_ptr
    }
}

fn token_start(token: *u8) -> usize {
    unsafe {
        let start_ptr: *usize = (token + 8usize) as *usize
        return *start_ptr
    }
}

fn token_len(token: *u8) -> usize {
    unsafe {
        let len_ptr: *usize = (token + 16usize) as *usize
        return *len_ptr
    }
}

fn token_value(source: &string, token: *u8) -> int {
    var cursor: usize = token_start(token)
    var value: int = 0
    var offset: usize = 0usize
    while offset < token_len(token) {
        value = value * 10 + string_byte_at(*source, cursor) - 48
        cursor += 1usize
        offset += 1usize
    }
    return value
}

fn parse_function(source: &string) -> bool {
    unsafe {
        var tokens: *u8 = array_new(sizeof(Token), 16usize)
        if tokens == null {
            return false
        }
        var token_count: usize = 0usize
        var cursor: usize = 0usize
        var token_start_value: usize = 0usize
        var in_token: bool = false

        while cursor < string_len(*source) {
            let byte: int = string_byte_at(*source, cursor)
            let punctuation: bool = byte == 40 || byte == 41 || byte == 123 || byte == 125
            if is_space(byte) || punctuation {
                if in_token {
                    if token_count >= 16usize {
                        array_free(tokens)
                        return false
                    }
                    let token: *u8 = make_token(
                        token_kind(source, token_start_value, cursor - token_start_value),
                        token_start_value,
                        cursor - token_start_value,
                    )
                    if array_push(tokens, token) != 0 {
                        free(token)
                        array_free(tokens)
                        return false
                    }
                    free(token)
                    token_count += 1usize
                    in_token = false
                }
                if punctuation {
                    if token_count >= 16usize {
                        array_free(tokens)
                        return false
                    }
                    let token: *u8 = make_token(token_kind(source, cursor, 1usize), cursor, 1usize)
                    if array_push(tokens, token) != 0 {
                        free(token)
                        array_free(tokens)
                        return false
                    }
                    free(token)
                    token_count += 1usize
                }
            } else if !in_token {
                token_start_value = cursor
                in_token = true
            }
            cursor += 1usize
        }
        if in_token {
            if token_count >= 16usize {
                array_free(tokens)
                return false
            }
            let token: *u8 = make_token(
                token_kind(source, token_start_value, cursor - token_start_value),
                token_start_value,
                cursor - token_start_value,
            )
            if array_push(tokens, token) != 0 {
                free(token)
                array_free(tokens)
                return false
            }
            free(token)
            token_count += 1usize
        }
        if token_count != 10usize {
            array_free(tokens)
            return false
        }

        let first: *u8 = array_get(tokens, 0usize)
        let second: *u8 = array_get(tokens, 1usize)
        let open: *u8 = array_get(tokens, 2usize)
        let close: *u8 = array_get(tokens, 3usize)
        let arrow: *u8 = array_get(tokens, 4usize)
        let return_type: *u8 = array_get(tokens, 5usize)
        let body_open: *u8 = array_get(tokens, 6usize)
        let return_word: *u8 = array_get(tokens, 7usize)
        let literal: *u8 = array_get(tokens, 8usize)
        let body_close: *u8 = array_get(tokens, 9usize)
        if first == null || second == null || open == null || close == null || arrow == null {
            array_free(tokens)
            return false
        }
        if return_type == null || body_open == null || return_word == null {
            array_free(tokens)
            return false
        }
        if literal == null || body_close == null {
            array_free(tokens)
            return false
        }

        var name_kind: int = 0
        var return_kind: int = 0
        var literal_value: int = 0
        if token_kind_from_ptr(first) == 1 && token_kind_from_ptr(second) == 2 {
            name_kind = 1
        }
        if token_kind_from_ptr(open) != 7
            || token_kind_from_ptr(close) != 8
            || token_kind_from_ptr(arrow) != 3
        {
            array_free(tokens)
            return false
        }
        if token_kind_from_ptr(return_type) == 4 && token_kind_from_ptr(body_open) == 9 {
            return_kind = 1
        }
        if token_kind_from_ptr(return_word) != 5
            || token_kind_from_ptr(literal) != 6
            || token_kind_from_ptr(body_close) != 10
        {
            array_free(tokens)
            return false
        }
        literal_value = token_value(source, literal)
        let node: FunctionNode = FunctionNode {
            name_kind: name_kind
            return_kind: return_kind
            literal_value: literal_value
        }
        let valid: bool = node.name_kind == 1
            && node.return_kind == 1
            && node.literal_value == 42
        array_free(tokens)
        return valid
    }
}

fn main() -> int {
    let source: string = "fn main() -> int { return 42 }"
    if !parse_function(&source) {
        return 1
    }
    return 0
}
