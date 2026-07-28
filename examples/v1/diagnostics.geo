import std.io

struct SourceError {
    line: usize
    column: usize
    code: int
}

fn main() -> int {
    let error: SourceError = SourceError { line: 3 column: 8 code: 1001 }
    let errors: [SourceError] = [error]
    println("error: unexpected token")
    return errors[0].code
}
