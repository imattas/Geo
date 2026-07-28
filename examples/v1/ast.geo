struct ExprNode {
    kind: int
    left: int
    right: int
}

fn main() -> int {
    let literal: ExprNode = ExprNode { kind: 1 left: 0 right: 0 }
    let binary: ExprNode = ExprNode { kind: 2 left: 0 right: 1 }
    let nodes: [ExprNode] = [literal, binary]
    return nodes[1].kind
}
