extern fn platform_os() -> str
extern fn platform_arch() -> str
extern fn platform_path_separator() -> int
extern fn platform_newline() -> str
extern fn cpu_count() -> usize
extern fn unix_time_secs() -> usize
extern fn monotonic_millis() -> usize
extern fn sleep_millis(ms: usize) -> int
extern fn temp_dir() -> str
extern fn home_dir() -> str
extern fn current_dir() -> str
extern fn change_dir(path: str) -> int
extern fn path_file_name(path: str) -> str
extern fn path_parent(path: str) -> str
extern fn path_extension(path: str) -> str
extern fn path_stem(path: str) -> str
extern fn path_without_extension(path: str) -> str
extern fn path_is_absolute(path: str) -> bool
