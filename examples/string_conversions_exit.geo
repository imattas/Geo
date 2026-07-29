import std.string

fn main() -> int {
    let negative: string = int_to_string(-42)
    if string_compare(negative, "-42") != 0 {
        return 1
    }

    let zero: string = int_to_string(0)
    if string_compare(zero, "0") != 0 {
        return 2
    }

    let size: string = usize_to_string(42usize)
    if string_compare(size, "42") != 0 {
        return 3
    }

    let yes: string = bool_to_string(true)
    if string_compare(yes, "true") != 0 {
        return 4
    }

    let no: string = bool_to_string(false)
    if string_compare(no, "false") != 0 {
        return 5
    }

    return 0
}
