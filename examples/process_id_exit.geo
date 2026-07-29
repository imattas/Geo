import std.process

fn main() -> int {
    if process_id() > 0 {
        return 0
    }
    return 1
}
