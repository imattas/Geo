fn main() -> int {
    var value: int = 1
    let slot: &mut int = &mut value
    *slot = 42
    value
}
