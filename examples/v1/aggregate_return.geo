struct Pair {
    left: int
    right: int
}

fn make_pair() -> Pair {
    return Pair { left: 7 right: 35 }
}

fn main() -> int {
    let pair: Pair = make_pair()
    return pair.left + pair.right
}
