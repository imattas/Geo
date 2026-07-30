#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Const {
        dst: ValueId,
        value: i64,
    },
    StringConst {
        dst: ValueId,
        label: String,
        value: String,
    },
    And {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Or {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    BitAnd {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    BitOr {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    BitXor {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    ShiftLeft {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    ShiftRight {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Add {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Sub {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Mul {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Div {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Rem {
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    },
    Load {
        dst: ValueId,
        local: String,
    },
    AddressOf {
        dst: ValueId,
        local: String,
    },
    Deref {
        dst: ValueId,
        pointer: ValueId,
        width: u8,
    },
    BitNot {
        dst: ValueId,
        value: ValueId,
    },
    BoundsCheck {
        index: ValueId,
        len: usize,
    },
    Store {
        local: String,
        value: ValueId,
    },
    StoreDeref {
        pointer: ValueId,
        value: ValueId,
        width: u8,
    },
    Cmp {
        dst: ValueId,
        op: CmpOp,
        left: ValueId,
        right: ValueId,
    },
    Jump {
        label: String,
    },
    JumpIfZero {
        value: ValueId,
        label: String,
    },
    Label {
        name: String,
    },
    Call {
        dst: ValueId,
        function: String,
        args: Vec<ValueId>,
    },
    CallAggregate {
        dst: Vec<ValueId>,
        function: String,
        args: Vec<ValueId>,
        buffer: usize,
    },
    ReturnAggregate {
        values: Vec<ValueId>,
    },
    Return {
        value: ValueId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}
