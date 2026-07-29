extern fn string_len(value: str) -> usize
extern fn string_clone(value: str) -> str
extern fn string_concat(left: str, right: str) -> str
extern fn string_from_byte(value: int) -> str
extern fn string_equals(left: str, right: str) -> bool
extern fn string_contains(value: str, needle: str) -> bool
extern fn string_starts_with(value: str, prefix: str) -> bool
extern fn string_ends_with(value: str, suffix: str) -> bool
extern fn string_substring(value: str, start: usize, len: usize) -> str
extern fn string_is_utf8(value: str) -> bool
extern fn string_utf8_is_valid(value: str) -> bool
extern fn int_to_string(value: int) -> str
extern fn bool_to_string(value: bool) -> str
