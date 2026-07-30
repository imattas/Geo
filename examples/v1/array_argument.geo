fn read_value(values: [int; 2]) -> int {
    return values[1]
}

fn main() -> int {
    let values: [int; 2] = [10, 42]
    return read_value(values)
}
