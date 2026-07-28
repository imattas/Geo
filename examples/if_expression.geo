fn choose(enabled: bool) -> int {
    return if enabled { 42 } else { 7 }
}

fn main() -> int {
    let enabled: bool = true
    return choose(enabled)
}
