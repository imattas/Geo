extern fn print(value: str) -> int
extern fn println(value: str) -> int
extern fn eprint(value: str) -> int
extern fn read_line() -> str

extern fn file_open(path: str) -> int
extern fn file_open_write(path: str) -> int
extern fn file_open_append(path: str) -> int
extern fn file_close(handle: int) -> int
extern fn file_seek(handle: int, offset: i64) -> int
extern fn file_read_to_string(handle: int) -> str
extern fn file_write(handle: int, data: str) -> int

extern fn read_file(path: str) -> str
extern fn write_file(path: str, data: str) -> int
extern fn append_file(path: str, data: str) -> int
