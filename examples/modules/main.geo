import math

fn main() -> int {
    let value: math.Word = math.ANSWER
    let answer = math.Answer { value: value }
    if math.add(40, 2) == answer.value {
        return 0
    }
    return 1
}
