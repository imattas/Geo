extern fn exit(code: int) -> int
extern fn arg_count() -> usize
extern fn arg(index: usize) -> str
extern fn arg_exists(index: usize) -> bool
extern fn process_id() -> usize
extern fn env_get(name: str) -> str
extern fn env_exists(name: str) -> bool
extern fn env_set(name: str, value: str) -> int
extern fn env_remove(name: str) -> int
extern fn current_exe() -> str
