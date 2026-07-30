import std.string

struct FunctionNode {
    name_kind: int
    return_kind: int
    literal_value: int
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

fn parse_word(source: &string, start: usize, word: string) -> int {
    var cursor: usize = start
    var offset: usize = 0usize
    while offset < string_len(word) {
        if cursor >= string_len(*source) {
            return -1
        }
        if string_byte_at(*source, cursor) != string_byte_at(word, offset) {
            return -1
        }
        cursor += 1usize
        offset += 1usize
    }
    return cursor as int
}

fn parse_byte(source: &string, start: usize, expected: int) -> int {
    if start >= string_len(*source) || string_byte_at(*source, start) != expected {
        return -1
    }
    return (start + 1usize) as int
}

fn parse_number(source: &string, start: usize, expected: int) -> int {
    var cursor: usize = start
    var value: int = 0
    var digits: usize = 0usize
    while cursor < string_len(*source) {
        let byte: int = string_byte_at(*source, cursor)
        if byte < 48 || byte > 57 {
            break
        }
        value = value * 10 + byte - 48
        digits += 1usize
        cursor += 1usize
    }
    if digits == 0usize || value != expected {
        return -1
    }
    return cursor as int
}

fn parse_function(source: &string) -> bool {
    var cursor: int = 0
    var name_kind: int = 0
    var return_kind: int = 0
    var literal_value: int = 0

    cursor = parse_word(source, skip_space(source, cursor as usize), "fn")
    if cursor < 0 {
        return false
    }
    cursor = parse_word(source, skip_space(source, cursor as usize), "main")
    if cursor < 0 {
        return false
    }
    name_kind = 1
    cursor = parse_byte(source, skip_space(source, cursor as usize), 40)
    if cursor < 0 {
        return false
    }
    cursor = parse_byte(source, cursor as usize, 41)
    if cursor < 0 {
        return false
    }
    cursor = parse_word(source, skip_space(source, cursor as usize), "->")
    if cursor < 0 {
        return false
    }
    cursor = parse_word(source, skip_space(source, cursor as usize), "int")
    if cursor < 0 {
        return false
    }
    return_kind = 1
    cursor = parse_byte(source, skip_space(source, cursor as usize), 123)
    if cursor < 0 {
        return false
    }
    cursor = parse_word(source, skip_space(source, cursor as usize), "return")
    if cursor < 0 {
        return false
    }
    cursor = parse_number(source, skip_space(source, cursor as usize), 42)
    if cursor < 0 {
        return false
    }
    literal_value = 42
    cursor = parse_byte(source, skip_space(source, cursor as usize), 125)
    if cursor < 0 {
        return false
    }
    let node: FunctionNode = FunctionNode {
        name_kind: name_kind
        return_kind: return_kind
        literal_value: literal_value
    }
    return skip_space(source, cursor as usize) == string_len(*source)
        && node.name_kind == 1
        && node.return_kind == 1
        && node.literal_value == 42
}

fn main() -> int {
    let source: string = "fn main() -> int { return 42 }"
    if !parse_function(&source) {
        return 1
    }
    return 0
}
