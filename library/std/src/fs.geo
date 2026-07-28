extern fn file_exists(path: str) -> bool
extern fn file_is_file(path: str) -> bool
extern fn file_is_dir(path: str) -> bool
extern fn file_size(path: str) -> usize
extern fn copy_file(source: str, dest: str) -> int
extern fn rename_file(source: str, dest: str) -> int
extern fn remove_file(path: str) -> int
extern fn create_dir(path: str) -> int
extern fn create_dir_all(path: str) -> int
extern fn remove_dir(path: str) -> int
extern fn remove_dir_all(path: str) -> int
extern fn dir_entry_count(path: str) -> usize
extern fn dir_entry_name(path: str, index: usize) -> str

