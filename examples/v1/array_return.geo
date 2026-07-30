fn make_values() -> [int; 2] {
    return [7, 35]
}

fn main() -> int {
    let values: [int; 2] = make_values()
    return values[0] + values[1]
}
