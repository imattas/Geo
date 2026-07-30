import std.process
import std.string

fn main() -> int {
    if arg_count() < 2 {
        return 1
    }

    if !arg_exists(1) {
        return 2
    }

    let value: string = arg(1)
    if string_compare(value, "alpha") != 0 {
        return 3
    }

    let fallback: string = arg_or(99, "fallback")
    if string_compare(fallback, "fallback") != 0 {
        return 4
    }

    return 0
}
