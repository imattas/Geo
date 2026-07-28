struct Parser {
    pos: int
    current: int
}

fn advance(pos: int) -> int {
    return pos + 1
}

fn main() -> int {
    let parser: Parser = Parser { pos: 0 current: 1 }
    let states: [Parser] = [parser]
    let next: int = advance(states[0].pos)
    if states[0].current == 1 {
        return 1
    } else {
        return 0
    }
}
