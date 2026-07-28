import std.io

fn main() -> int {
    let touched = touch_file("geo-touch-test.txt")
    if touched != 0 {
        return touched
    }

    return remove_file("geo-touch-test.txt")
}
