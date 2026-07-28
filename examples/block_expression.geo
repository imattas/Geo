fn main() -> int {
    let enabled: bool = true
    return if enabled {
        let base: int = 40
        base + 2
    } else {
        let fallback: int = 7
        fallback
    }
}
