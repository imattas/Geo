extern fn string_len(value: str) -> usize
extern fn string_clone(value: str) -> str
extern fn string_free(value: str) -> int
extern fn string_concat(left: str, right: str) -> str
extern fn string_from_byte(value: int) -> str
extern fn string_from_utf8_codepoint(value: int) -> str
extern fn string_equals(left: str, right: str) -> bool
extern fn string_compare(left: str, right: str) -> int
extern fn string_contains(value: str, needle: str) -> bool
extern fn string_starts_with(value: str, prefix: str) -> bool
extern fn string_ends_with(value: str, suffix: str) -> bool
extern fn string_substring(value: str, start: usize, len: usize) -> str
extern fn string_is_utf8(value: str) -> bool
extern fn string_utf8_is_valid(value: str) -> bool
extern fn string_utf8_char_at(value: str, index: usize) -> str
extern fn string_utf8_find_codepoint(value: str, codepoint: int) -> int
extern fn string_utf8_byte_offset(value: str, index: usize) -> int
extern fn string_utf8_next_offset(value: str, offset: usize) -> int
extern fn string_utf8_prev_offset(value: str, offset: usize) -> int
extern fn string_utf8_index_at(value: str, offset: usize) -> int
extern fn string_utf8_is_boundary(value: str, offset: usize) -> bool
extern fn string_utf8_slice(value: str, start: usize, end: usize) -> str
extern fn int_to_string(value: int) -> str
extern fn bool_to_string(value: bool) -> str
