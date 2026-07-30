use crate::ast::{BinaryOp, Expr, Field, MatchPattern, Program, Stmt, StructDecl, Type, UnaryOp};
use crate::ir::{CmpOp, Instruction, IrFunction, IrProgram, ValueId};
use crate::runtime;
use std::collections::{HashMap, HashSet};

pub fn lower(program: &Program) -> IrProgram {
    let program = crate::typecheck::expand_type_aliases_for_codegen(program);
    let program = &program;
    IrProgram {
        functions: program
            .functions
            .iter()
            .map(|function| lower_function(program, function))
            .collect(),
    }
}

fn lower_function(program: &Program, function: &crate::ast::Function) -> IrFunction {
    let mut ctx = LowerCtx {
        function_name: function.name.clone(),
        structs: program
            .structs
            .iter()
            .map(|decl| (decl.name.clone(), decl.clone()))
            .collect(),
        enum_discriminants: enum_discriminants(program),
        enum_types: program
            .enums
            .iter()
            .map(|decl| (decl.name.clone(), Type::Named(decl.name.clone())))
            .collect(),
        locals: function
            .params
            .iter()
            .map(|param| (param.name.clone(), param.ty.clone()))
            .collect(),
        const_values: program
            .consts
            .iter()
            .map(|decl| (decl.name.clone(), decl.value.clone()))
            .collect(),
        const_types: program
            .consts
            .iter()
            .map(|decl| (decl.name.clone(), decl.ty.clone()))
            .collect(),
        array_lengths: HashMap::new(),
        runtime_symbols: runtime_symbols(program),
        function_returns: function_returns(program),
        next_value: 0,
        next_label: 0,
        next_string: 0,
        loop_stack: Vec::new(),
        instructions: Vec::new(),
    };

    if let Some((Stmt::Expr(expr), prefix)) = function
        .body
        .split_last()
        .filter(|_| function.return_type != Type::Unit)
    {
        ctx.lower_stmts(prefix);
        let value = ctx.lower_expr(expr);
        ctx.instructions.push(Instruction::Return { value });
    } else {
        ctx.lower_stmts(&function.body);
    }
    if !matches!(ctx.instructions.last(), Some(Instruction::Return { .. })) {
        let value = ctx.fresh();
        ctx.instructions.push(Instruction::Const {
            dst: value,
            value: 0,
        });
        ctx.instructions.push(Instruction::Return { value });
    }

    IrFunction {
        name: function.name.clone(),
        params: function
            .params
            .iter()
            .map(|param| param.name.clone())
            .collect(),
        instructions: ctx.instructions,
    }
}

struct LowerCtx {
    function_name: String,
    structs: HashMap<String, StructDecl>,
    enum_discriminants: HashMap<(String, String), i64>,
    enum_types: HashMap<String, Type>,
    locals: HashMap<String, Type>,
    const_values: HashMap<String, Expr>,
    const_types: HashMap<String, Type>,
    array_lengths: HashMap<String, usize>,
    runtime_symbols: HashMap<String, String>,
    function_returns: HashMap<String, Type>,
    next_value: usize,
    next_label: usize,
    next_string: usize,
    loop_stack: Vec<(String, String)>,
    instructions: Vec<Instruction>,
}

impl LowerCtx {
    fn fresh(&mut self) -> ValueId {
        let value = ValueId(self.next_value);
        self.next_value += 1;
        value
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let label = format!(".L{prefix}_{}", self.next_label);
        self.next_label += 1;
        label
    }

    fn fresh_string_label(&mut self) -> String {
        let label = format!("__geo_str_{}_{}", self.function_name, self.next_string);
        self.next_string += 1;
        label
    }

    fn lower_const_scalar(&mut self, value: i64) -> ValueId {
        let dst = self.fresh();
        self.instructions.push(Instruction::Const { dst, value });
        dst
    }

    fn binary_instruction(
        &self,
        op: BinaryOp,
        dst: ValueId,
        left: ValueId,
        right: ValueId,
    ) -> Instruction {
        match op {
            BinaryOp::And => Instruction::And { dst, left, right },
            BinaryOp::Or => Instruction::Or { dst, left, right },
            BinaryOp::BitAnd => Instruction::BitAnd { dst, left, right },
            BinaryOp::BitOr => Instruction::BitOr { dst, left, right },
            BinaryOp::BitXor => Instruction::BitXor { dst, left, right },
            BinaryOp::ShiftLeft => Instruction::ShiftLeft { dst, left, right },
            BinaryOp::ShiftRight => Instruction::ShiftRight { dst, left, right },
            BinaryOp::Add => Instruction::Add { dst, left, right },
            BinaryOp::Sub => Instruction::Sub { dst, left, right },
            BinaryOp::Mul => Instruction::Mul { dst, left, right },
            BinaryOp::Div => Instruction::Div { dst, left, right },
            BinaryOp::Rem => Instruction::Rem { dst, left, right },
            BinaryOp::Equal => Instruction::Cmp {
                dst,
                op: CmpOp::Equal,
                left,
                right,
            },
            BinaryOp::NotEqual => Instruction::Cmp {
                dst,
                op: CmpOp::NotEqual,
                left,
                right,
            },
            BinaryOp::Less => Instruction::Cmp {
                dst,
                op: CmpOp::Less,
                left,
                right,
            },
            BinaryOp::LessEqual => Instruction::Cmp {
                dst,
                op: CmpOp::LessEqual,
                left,
                right,
            },
            BinaryOp::Greater => Instruction::Cmp {
                dst,
                op: CmpOp::Greater,
                left,
                right,
            },
            BinaryOp::GreaterEqual => Instruction::Cmp {
                dst,
                op: CmpOp::GreaterEqual,
                left,
                right,
            },
        }
    }

    fn lower_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.lower_stmt(stmt);
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Return(expr) => {
                let value = if let Some(expr) = expr {
                    self.lower_expr(expr)
                } else {
                    let value = self.fresh();
                    self.instructions.push(Instruction::Const {
                        dst: value,
                        value: 0,
                    });
                    value
                };
                self.instructions.push(Instruction::Return { value });
            }
            Stmt::Let {
                name, ty, value, ..
            } if self
                .let_type(ty, value)
                .is_some_and(|ty| self.is_aggregate_type(&ty)) =>
            {
                let ty = self
                    .let_type(ty, value)
                    .expect("guard checked aggregate type");
                self.locals.insert(name.clone(), ty.clone());
                self.lower_aggregate_into(name, &ty, value);
            }
            Stmt::Let {
                name, ty, value, ..
            } => {
                let ty = self.let_type(ty, value).unwrap_or(Type::Int);
                self.locals.insert(name.clone(), ty);
                let value = self.lower_expr(value);
                self.instructions.push(Instruction::Store {
                    local: name.clone(),
                    value,
                });
            }
            Stmt::Assign {
                name,
                op: None,
                value,
            } if is_aggregate_expr(value) => {
                let ty = self
                    .locals
                    .get(name)
                    .unwrap_or_else(|| panic!("assignment to unknown aggregate local '{name}'"))
                    .clone();
                self.lower_aggregate_into(name, &ty, value);
            }
            Stmt::Assign { name, op, value } => {
                let value = if let Some(op) = op {
                    let left = self.fresh();
                    self.instructions.push(Instruction::Load {
                        dst: left,
                        local: name.clone(),
                    });
                    if let Some(Type::Pointer(inner)) = self.locals.get(name).cloned() {
                        if matches!(op, BinaryOp::Add | BinaryOp::Sub)
                            && self.expr_type(value).is_some_and(|ty| is_integer_type(&ty))
                        {
                            let right = self.lower_expr(value);
                            let scale = self.pointer_scale(&inner);
                            let scaled_right = self.scale_value(right, scale);
                            let dst = self.fresh();
                            let instruction = match op {
                                BinaryOp::Add => Instruction::Add {
                                    dst,
                                    left,
                                    right: scaled_right,
                                },
                                BinaryOp::Sub => Instruction::Sub {
                                    dst,
                                    left,
                                    right: scaled_right,
                                },
                                _ => unreachable!("guarded by matches"),
                            };
                            self.instructions.push(instruction);
                            dst
                        } else {
                            let right = self.lower_expr(value);
                            let dst = self.fresh();
                            let instruction = self.binary_instruction(*op, dst, left, right);
                            self.instructions.push(instruction);
                            dst
                        }
                    } else {
                        let right = self.lower_expr(value);
                        let dst = self.fresh();
                        let instruction = self.binary_instruction(*op, dst, left, right);
                        self.instructions.push(instruction);
                        dst
                    }
                } else {
                    self.lower_expr(value)
                };
                self.instructions.push(Instruction::Store {
                    local: name.clone(),
                    value,
                });
            }
            Stmt::PointerAssign { pointer, op, value } => {
                let deref_width = self.pointer_deref_width(pointer);
                let pointer = self.lower_expr(pointer);
                let value = if let Some(op) = op {
                    let left = self.fresh();
                    self.instructions.push(Instruction::Deref {
                        dst: left,
                        pointer,
                        width: deref_width,
                    });
                    let right = self.lower_expr(value);
                    let dst = self.fresh();
                    let instruction = self.binary_instruction(*op, dst, left, right);
                    self.instructions.push(instruction);
                    dst
                } else {
                    self.lower_expr(value)
                };
                self.instructions.push(Instruction::StoreDeref {
                    pointer,
                    value,
                    width: deref_width,
                });
            }
            Stmt::PlaceAssign { target, op, value } => {
                let local = self
                    .aggregate_place(target)
                    .unwrap_or_else(|| panic!("unsupported assignment place"));
                let value = if let Some(op) = op {
                    let left = self.fresh();
                    self.instructions.push(Instruction::Load {
                        dst: left,
                        local: local.clone(),
                    });
                    let right = self.lower_expr(value);
                    let dst = self.fresh();
                    let instruction = self.binary_instruction(*op, dst, left, right);
                    self.instructions.push(instruction);
                    dst
                } else {
                    self.lower_expr(value)
                };
                self.instructions.push(Instruction::Store { local, value });
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let else_label = self.fresh_label("else");
                let end_label = self.fresh_label("endif");
                let condition = self.lower_expr(condition);
                self.instructions.push(Instruction::JumpIfZero {
                    value: condition,
                    label: else_label.clone(),
                });
                self.lower_stmts(then_body);
                self.instructions.push(Instruction::Jump {
                    label: end_label.clone(),
                });
                self.instructions
                    .push(Instruction::Label { name: else_label });
                self.lower_stmts(else_body);
                self.instructions
                    .push(Instruction::Label { name: end_label });
            }
            Stmt::While { condition, body } => {
                let start_label = self.fresh_label("while");
                let end_label = self.fresh_label("endwhile");
                self.loop_stack
                    .push((start_label.clone(), end_label.clone()));
                self.instructions.push(Instruction::Label {
                    name: start_label.clone(),
                });
                let condition = self.lower_expr(condition);
                self.instructions.push(Instruction::JumpIfZero {
                    value: condition,
                    label: end_label.clone(),
                });
                self.lower_stmts(body);
                self.loop_stack.pop();
                self.instructions
                    .push(Instruction::Jump { label: start_label });
                self.instructions
                    .push(Instruction::Label { name: end_label });
            }
            Stmt::For {
                name,
                start,
                end,
                inclusive,
                body,
            } => {
                self.locals
                    .insert(name.clone(), self.expr_type(start).unwrap_or(Type::Int));
                let start_value = self.lower_expr(start);
                self.instructions.push(Instruction::Store {
                    local: name.clone(),
                    value: start_value,
                });
                let end_value = self.lower_expr(end);
                let end_local = format!("__geo_for_end_{}_{}", name, self.next_label);
                self.instructions.push(Instruction::Store {
                    local: end_local.clone(),
                    value: end_value,
                });

                let start_label = self.fresh_label("for");
                let continue_label = self.fresh_label("for_next");
                let end_label = self.fresh_label("endfor");
                self.loop_stack
                    .push((continue_label.clone(), end_label.clone()));
                self.instructions.push(Instruction::Label {
                    name: start_label.clone(),
                });
                let current = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst: current,
                    local: name.clone(),
                });
                let end = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst: end,
                    local: end_local,
                });
                let condition = self.fresh();
                self.instructions.push(Instruction::Cmp {
                    dst: condition,
                    op: if *inclusive {
                        CmpOp::LessEqual
                    } else {
                        CmpOp::Less
                    },
                    left: current,
                    right: end,
                });
                self.instructions.push(Instruction::JumpIfZero {
                    value: condition,
                    label: end_label.clone(),
                });

                self.lower_stmts(body);
                self.instructions.push(Instruction::Label {
                    name: continue_label,
                });
                let current = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst: current,
                    local: name.clone(),
                });
                let one = self.fresh();
                self.instructions
                    .push(Instruction::Const { dst: one, value: 1 });
                let next = self.fresh();
                self.instructions.push(Instruction::Add {
                    dst: next,
                    left: current,
                    right: one,
                });
                self.instructions.push(Instruction::Store {
                    local: name.clone(),
                    value: next,
                });
                self.loop_stack.pop();
                self.instructions
                    .push(Instruction::Jump { label: start_label });
                self.instructions
                    .push(Instruction::Label { name: end_label });
            }
            Stmt::Loop(body) => {
                let start_label = self.fresh_label("loop");
                let end_label = self.fresh_label("endloop");
                self.loop_stack
                    .push((start_label.clone(), end_label.clone()));
                self.instructions.push(Instruction::Label {
                    name: start_label.clone(),
                });
                self.lower_stmts(body);
                self.loop_stack.pop();
                self.instructions
                    .push(Instruction::Jump { label: start_label });
                self.instructions
                    .push(Instruction::Label { name: end_label });
            }
            Stmt::Unsafe(body) => {
                self.lower_stmts(body);
            }
            Stmt::Break => {
                let (_, end_label) = self
                    .loop_stack
                    .last()
                    .expect("break should have been validated by type checker");
                self.instructions.push(Instruction::Jump {
                    label: end_label.clone(),
                });
            }
            Stmt::Continue => {
                let (start_label, _) = self
                    .loop_stack
                    .last()
                    .expect("continue should have been validated by type checker");
                self.instructions.push(Instruction::Jump {
                    label: start_label.clone(),
                });
            }
            Stmt::Expr(expr) => {
                self.lower_expr(expr);
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> ValueId {
        if self.contains_const_ref(expr) {
            if let Some(value) = self.const_scalar_value(expr) {
                return self.lower_const_scalar(value);
            }
        }
        match expr {
            Expr::Int(value) | Expr::TypedInt { value, .. } => {
                let dst = self.fresh();
                self.instructions
                    .push(Instruction::Const { dst, value: *value });
                dst
            }
            Expr::Bool(value) => {
                let dst = self.fresh();
                self.instructions.push(Instruction::Const {
                    dst,
                    value: i64::from(*value),
                });
                dst
            }
            Expr::Null => {
                let dst = self.fresh();
                self.instructions.push(Instruction::Const { dst, value: 0 });
                dst
            }
            Expr::Char(value) => {
                let dst = self.fresh();
                self.instructions.push(Instruction::Const {
                    dst,
                    value: i64::from(u32::from(*value)),
                });
                dst
            }
            Expr::String(value) => {
                let dst = self.fresh();
                let label = self.fresh_string_label();
                self.instructions.push(Instruction::StringConst {
                    dst,
                    label,
                    value: value.clone(),
                });
                dst
            }
            Expr::Var(name) => {
                if !self.locals.contains_key(name) {
                    if let Some(value) = self.const_values.get(name).cloned() {
                        return self.lower_expr(&value);
                    }
                }
                let dst = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst,
                    local: name.clone(),
                });
                dst
            }
            Expr::Binary { op, left, right } => {
                if *op == BinaryOp::Add
                    && self.expr_type(left) == Some(Type::String)
                    && self.expr_type(right) == Some(Type::String)
                {
                    let left = self.lower_expr(left);
                    let right = self.lower_expr(right);
                    let dst = self.fresh();
                    self.instructions.push(Instruction::Call {
                        dst,
                        function: "string_concat".to_string(),
                        args: vec![left, right],
                    });
                    return dst;
                }
                if let Some(value) = self.lower_pointer_binary(*op, left, right) {
                    return value;
                }
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let dst = self.fresh();
                let instruction = self.binary_instruction(*op, dst, left, right);
                self.instructions.push(instruction);
                dst
            }
            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => {
                    let zero = self.fresh();
                    self.instructions.push(Instruction::Const {
                        dst: zero,
                        value: 0,
                    });
                    let value = self.lower_expr(expr);
                    let dst = self.fresh();
                    self.instructions.push(Instruction::Sub {
                        dst,
                        left: zero,
                        right: value,
                    });
                    dst
                }
                UnaryOp::Not => {
                    let value = self.lower_expr(expr);
                    let zero = self.fresh();
                    self.instructions.push(Instruction::Const {
                        dst: zero,
                        value: 0,
                    });
                    let dst = self.fresh();
                    self.instructions.push(Instruction::Cmp {
                        dst,
                        op: CmpOp::Equal,
                        left: value,
                        right: zero,
                    });
                    dst
                }
                UnaryOp::BitNot => {
                    let value = self.lower_expr(expr);
                    let dst = self.fresh();
                    self.instructions.push(Instruction::BitNot { dst, value });
                    dst
                }
                UnaryOp::AddressOf | UnaryOp::MutableAddressOf => {
                    let local = self
                        .aggregate_place(expr)
                        .unwrap_or_else(|| panic!("address-of requires a local place"));
                    let dst = self.fresh();
                    self.instructions
                        .push(Instruction::AddressOf { dst, local });
                    dst
                }
                UnaryOp::Deref => {
                    let width = self.pointer_deref_width(expr);
                    let pointer = self.lower_expr(expr);
                    let dst = self.fresh();
                    self.instructions.push(Instruction::Deref {
                        dst,
                        pointer,
                        width,
                    });
                    dst
                }
            },
            Expr::Cast { expr, .. } => self.lower_expr(expr),
            Expr::SizeOf(ty) => {
                let dst = self.fresh();
                let value = self.type_size(ty);
                self.instructions.push(Instruction::Const { dst, value });
                dst
            }
            Expr::AlignOf(ty) => {
                let dst = self.fresh();
                let value = self.type_align(ty);
                self.instructions.push(Instruction::Const { dst, value });
                dst
            }
            Expr::OffsetOf { ty, field } => {
                let dst = self.fresh();
                let value = self.field_offset(ty, field);
                self.instructions.push(Instruction::Const { dst, value });
                dst
            }
            Expr::Match { value, arms } => {
                let scrutinee = self.lower_expr(value);
                let result_local =
                    format!("__geo_match_{}_{}", self.function_name, self.next_label);
                if let Some(result_ty) = self.expr_type(expr) {
                    self.locals.insert(result_local.clone(), result_ty);
                }
                let end_label = self.fresh_label("match_end");

                for arm in arms {
                    match self.lower_match_pattern_value(&arm.pattern) {
                        Some(pattern_value) => {
                            let arm_label = self.fresh_label("match_arm");
                            let next_label = self.fresh_label("match_next");
                            let not_equal = self.fresh();
                            self.instructions.push(Instruction::Cmp {
                                dst: not_equal,
                                op: CmpOp::NotEqual,
                                left: scrutinee,
                                right: pattern_value,
                            });
                            self.instructions.push(Instruction::JumpIfZero {
                                value: not_equal,
                                label: arm_label.clone(),
                            });
                            self.instructions.push(Instruction::Jump {
                                label: next_label.clone(),
                            });
                            self.instructions
                                .push(Instruction::Label { name: arm_label });
                            let value = self.lower_expr(&arm.value);
                            self.instructions.push(Instruction::Store {
                                local: result_local.clone(),
                                value,
                            });
                            self.instructions.push(Instruction::Jump {
                                label: end_label.clone(),
                            });
                            self.instructions
                                .push(Instruction::Label { name: next_label });
                        }
                        None => {
                            let value = self.lower_expr(&arm.value);
                            self.instructions.push(Instruction::Store {
                                local: result_local.clone(),
                                value,
                            });
                            self.instructions.push(Instruction::Jump {
                                label: end_label.clone(),
                            });
                            break;
                        }
                    }
                }

                let fallback = self.fresh();
                self.instructions.push(Instruction::Const {
                    dst: fallback,
                    value: 0,
                });
                self.instructions.push(Instruction::Store {
                    local: result_local.clone(),
                    value: fallback,
                });
                self.instructions
                    .push(Instruction::Label { name: end_label });
                let dst = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst,
                    local: result_local,
                });
                dst
            }
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                let result_local = format!("__geo_if_{}_{}", self.function_name, self.next_label);
                if let Some(result_ty) = self.expr_type(expr) {
                    self.locals.insert(result_local.clone(), result_ty);
                }
                let else_label = self.fresh_label("if_else");
                let end_label = self.fresh_label("if_end");
                let condition = self.lower_expr(condition);
                self.instructions.push(Instruction::JumpIfZero {
                    value: condition,
                    label: else_label.clone(),
                });
                let then_value = self.lower_expr(then_value);
                self.instructions.push(Instruction::Store {
                    local: result_local.clone(),
                    value: then_value,
                });
                self.instructions.push(Instruction::Jump {
                    label: end_label.clone(),
                });
                self.instructions
                    .push(Instruction::Label { name: else_label });
                let else_value = self.lower_expr(else_value);
                self.instructions.push(Instruction::Store {
                    local: result_local.clone(),
                    value: else_value,
                });
                self.instructions
                    .push(Instruction::Label { name: end_label });
                let dst = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst,
                    local: result_local,
                });
                dst
            }
            Expr::Block { statements, value } => {
                self.lower_stmts(statements);
                self.lower_expr(value)
            }
            Expr::Call { name, args } => {
                let args = args.iter().map(|arg| self.lower_expr(arg)).collect();
                let dst = self.fresh();
                let function = self
                    .runtime_symbols
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone());
                self.instructions.push(Instruction::Call {
                    dst,
                    function,
                    args,
                });
                dst
            }
            Expr::Index { base, index } if self.expr_type(base) == Some(Type::String) => {
                let base = self.lower_expr(base);
                let index = self.lower_expr(index);
                let dst = self.fresh();
                self.instructions.push(Instruction::Call {
                    dst,
                    function: "__geo_string_get".to_string(),
                    args: vec![base, index],
                });
                dst
            }
            Expr::Field { base, name } => {
                if let Expr::Var(enum_name) = base.as_ref() {
                    if let Some(discriminant) = self
                        .enum_discriminants
                        .get(&(enum_name.clone(), name.clone()))
                        .copied()
                    {
                        let dst = self.fresh();
                        self.instructions.push(Instruction::Const {
                            dst,
                            value: discriminant,
                        });
                        return dst;
                    }
                }
                let local = self
                    .aggregate_place(expr)
                    .unwrap_or_else(|| panic!("unsupported aggregate place expression"));
                let dst = self.fresh();
                self.instructions.push(Instruction::Load { dst, local });
                dst
            }
            Expr::Index { .. } => {
                let local = self
                    .aggregate_place(expr)
                    .unwrap_or_else(|| panic!("unsupported aggregate place expression"));
                let dst = self.fresh();
                self.instructions.push(Instruction::Load { dst, local });
                dst
            }
            Expr::Struct { .. } | Expr::Array(_) => {
                panic!("aggregate literals must be assigned to a local before use")
            }
        }
    }

    fn const_scalar_value(&self, expr: &Expr) -> Option<i64> {
        self.const_scalar_value_inner(expr, &mut HashSet::new())
    }

    fn contains_const_ref(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Var(name) => {
                !self.locals.contains_key(name) && self.const_values.contains_key(name)
            }
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => self.contains_const_ref(expr),
            Expr::Binary { left, right, .. } => {
                self.contains_const_ref(left) || self.contains_const_ref(right)
            }
            Expr::Call { args, .. } | Expr::Array(args) => {
                args.iter().any(|arg| self.contains_const_ref(arg))
            }
            Expr::Struct { fields, .. } => fields
                .iter()
                .any(|(_, value)| self.contains_const_ref(value)),
            Expr::Field { base, .. } => self.contains_const_ref(base),
            Expr::Index { base, index } => {
                self.contains_const_ref(base) || self.contains_const_ref(index)
            }
            Expr::Match { value, arms } => {
                self.contains_const_ref(value)
                    || arms.iter().any(|arm| self.contains_const_ref(&arm.value))
            }
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                self.contains_const_ref(condition)
                    || self.contains_const_ref(then_value)
                    || self.contains_const_ref(else_value)
            }
            Expr::Block { statements, value } => {
                statements
                    .iter()
                    .any(|stmt| self.stmt_contains_const_ref(stmt))
                    || self.contains_const_ref(value)
            }
            Expr::Int(_)
            | Expr::TypedInt { .. }
            | Expr::Bool(_)
            | Expr::Char(_)
            | Expr::String(_)
            | Expr::Null
            | Expr::SizeOf(_)
            | Expr::AlignOf(_)
            | Expr::OffsetOf { .. } => false,
        }
    }

    fn stmt_contains_const_ref(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Let { value, .. }
            | Stmt::Assign { value, .. }
            | Stmt::PointerAssign { value, .. }
            | Stmt::PlaceAssign { value, .. }
            | Stmt::Return(Some(value))
            | Stmt::Expr(value) => self.contains_const_ref(value),
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                self.contains_const_ref(condition)
                    || then_body
                        .iter()
                        .any(|stmt| self.stmt_contains_const_ref(stmt))
                    || else_body
                        .iter()
                        .any(|stmt| self.stmt_contains_const_ref(stmt))
            }
            Stmt::While { condition, body } => {
                self.contains_const_ref(condition)
                    || body.iter().any(|stmt| self.stmt_contains_const_ref(stmt))
            }
            Stmt::For {
                start, end, body, ..
            } => {
                self.contains_const_ref(start)
                    || self.contains_const_ref(end)
                    || body.iter().any(|stmt| self.stmt_contains_const_ref(stmt))
            }
            Stmt::Unsafe(body) | Stmt::Loop(body) => {
                body.iter().any(|stmt| self.stmt_contains_const_ref(stmt))
            }
            Stmt::Return(None) | Stmt::Break | Stmt::Continue => false,
        }
    }

    fn const_scalar_value_inner(&self, expr: &Expr, visiting: &mut HashSet<String>) -> Option<i64> {
        match expr {
            Expr::Int(value) | Expr::TypedInt { value, .. } => Some(*value),
            Expr::Bool(value) => Some(i64::from(*value)),
            Expr::Char(value) => Some(i64::from(u32::from(*value))),
            Expr::Null => Some(0),
            Expr::Var(name) if !self.locals.contains_key(name) => {
                if !visiting.insert(name.clone()) {
                    return None;
                }
                let value = self
                    .const_values
                    .get(name)
                    .and_then(|expr| self.const_scalar_value_inner(expr, visiting));
                visiting.remove(name);
                value
            }
            Expr::Unary { op, expr } => {
                let value = self.const_scalar_value_inner(expr, visiting)?;
                match op {
                    UnaryOp::Neg => Some(-value),
                    UnaryOp::Not => Some(i64::from(value == 0)),
                    UnaryOp::BitNot => Some(!value),
                    UnaryOp::AddressOf | UnaryOp::MutableAddressOf | UnaryOp::Deref => None,
                }
            }
            Expr::Binary { op, left, right } => {
                let left = self.const_scalar_value_inner(left, visiting)?;
                let right = self.const_scalar_value_inner(right, visiting)?;
                match op {
                    BinaryOp::And => Some(i64::from(left != 0 && right != 0)),
                    BinaryOp::Or => Some(i64::from(left != 0 || right != 0)),
                    BinaryOp::BitAnd => Some(left & right),
                    BinaryOp::BitOr => Some(left | right),
                    BinaryOp::BitXor => Some(left ^ right),
                    BinaryOp::ShiftLeft => Some(left << right),
                    BinaryOp::ShiftRight => Some(left >> right),
                    BinaryOp::Add => Some(left + right),
                    BinaryOp::Sub => Some(left - right),
                    BinaryOp::Mul => Some(left * right),
                    BinaryOp::Div if right != 0 => Some(left / right),
                    BinaryOp::Rem if right != 0 => Some(left % right),
                    BinaryOp::Equal => Some(i64::from(left == right)),
                    BinaryOp::NotEqual => Some(i64::from(left != right)),
                    BinaryOp::Less => Some(i64::from(left < right)),
                    BinaryOp::LessEqual => Some(i64::from(left <= right)),
                    BinaryOp::Greater => Some(i64::from(left > right)),
                    BinaryOp::GreaterEqual => Some(i64::from(left >= right)),
                    BinaryOp::Div | BinaryOp::Rem => None,
                }
            }
            Expr::Cast { expr, .. } => self.const_scalar_value_inner(expr, visiting),
            Expr::SizeOf(ty) => Some(self.type_size(ty)),
            Expr::AlignOf(ty) => Some(self.type_align(ty)),
            Expr::OffsetOf { ty, field } => Some(self.field_offset(ty, field)),
            _ => None,
        }
    }

    fn lower_aggregate_into(&mut self, prefix: &str, ty: &Type, value: &Expr) {
        match ty {
            Type::Named(name) if self.structs.contains_key(name) => {
                self.lower_struct_into(prefix, name, value)
            }
            Type::Array(element_ty) => self.lower_array_into(prefix, element_ty, value),
            _ => {
                let value = self.lower_expr(value);
                self.instructions.push(Instruction::Store {
                    local: prefix.to_string(),
                    value,
                });
            }
        }
    }

    fn lower_struct_into(&mut self, prefix: &str, name: &str, value: &Expr) {
        let fields = self.struct_fields(name);
        match value {
            Expr::Struct {
                name: literal_name,
                fields: values,
            } => {
                assert_eq!(
                    name, literal_name,
                    "type checker should validate struct names"
                );
                for field in fields {
                    let field_value = values
                        .iter()
                        .find(|(field_name, _)| field_name == &field.name)
                        .unwrap_or_else(|| {
                            panic!("type checker should validate field '{}'", field.name)
                        })
                        .1
                        .clone();
                    let slot = format!("{prefix}.{}", field.name);
                    self.lower_aggregate_into(&slot, &field.ty, &field_value);
                }
            }
            Expr::Var(source) => {
                for field in fields {
                    let source_slot = format!("{source}.{}", field.name);
                    let target_slot = format!("{prefix}.{}", field.name);
                    self.copy_aggregate_slot(&target_slot, &field.ty, &source_slot);
                }
            }
            Expr::Field { .. } | Expr::Index { .. } => {
                let source = self
                    .aggregate_place(value)
                    .unwrap_or_else(|| panic!("unsupported aggregate source expression"));
                for field in fields {
                    let source_slot = format!("{source}.{}", field.name);
                    let target_slot = format!("{prefix}.{}", field.name);
                    self.copy_aggregate_slot(&target_slot, &field.ty, &source_slot);
                }
            }
            _ => panic!("unsupported struct aggregate source"),
        }
    }

    fn lower_array_into(&mut self, prefix: &str, element_ty: &Type, value: &Expr) {
        match value {
            Expr::Array(elements) => {
                self.array_lengths
                    .insert(prefix.to_string(), elements.len());
                for (index, element) in elements.iter().enumerate() {
                    let slot = format!("{prefix}[{index}]");
                    self.lower_aggregate_into(&slot, element_ty, element);
                }
            }
            Expr::Var(_) | Expr::Field { .. } | Expr::Index { .. } => {
                let source = self
                    .aggregate_place(value)
                    .unwrap_or_else(|| panic!("unsupported array aggregate source"));
                self.copy_array_slots(prefix, element_ty, &source);
            }
            _ => panic!("unsupported array aggregate source"),
        }
    }

    fn copy_array_slots(&mut self, target_prefix: &str, element_ty: &Type, source_prefix: &str) {
        let length = self
            .array_lengths
            .get(source_prefix)
            .copied()
            .unwrap_or_else(|| panic!("unknown fixed array length for '{source_prefix}'"));
        self.array_lengths.insert(target_prefix.to_string(), length);
        for index in 0..length {
            self.copy_aggregate_slot(
                &format!("{target_prefix}[{index}]"),
                element_ty,
                &format!("{source_prefix}[{index}]"),
            );
        }
    }

    fn copy_aggregate_slot(&mut self, target_slot: &str, ty: &Type, source_slot: &str) {
        match ty {
            Type::Named(name) => {
                for field in self.struct_fields(name) {
                    self.copy_aggregate_slot(
                        &format!("{target_slot}.{}", field.name),
                        &field.ty,
                        &format!("{source_slot}.{}", field.name),
                    );
                }
            }
            Type::Array(element_ty) => {
                self.copy_array_slots(target_slot, element_ty, source_slot);
            }
            _ => {
                let value = self.fresh();
                self.instructions.push(Instruction::Load {
                    dst: value,
                    local: source_slot.to_string(),
                });
                self.instructions.push(Instruction::Store {
                    local: target_slot.to_string(),
                    value,
                });
            }
        }
    }

    fn aggregate_place(&mut self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Var(name) => Some(name.clone()),
            Expr::Field { base, name } => self
                .aggregate_place(base)
                .map(|base| format!("{base}.{name}")),
            Expr::Index { base, index } => {
                let Expr::Int(index) = index.as_ref() else {
                    return None;
                };
                self.aggregate_place(base).map(|base| {
                    if let Some(len) = self.array_lengths.get(&base).copied() {
                        let index_value = self.fresh();
                        self.instructions.push(Instruction::Const {
                            dst: index_value,
                            value: *index,
                        });
                        self.instructions.push(Instruction::BoundsCheck {
                            index: index_value,
                            len,
                        });
                    }
                    format!("{base}[{index}]")
                })
            }
            _ => None,
        }
    }

    fn struct_fields(&self, name: &str) -> Vec<Field> {
        self.structs
            .get(name)
            .unwrap_or_else(|| panic!("unknown struct '{name}'"))
            .fields
            .clone()
    }

    fn expr_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Int(_) => Some(Type::Int),
            Expr::TypedInt { ty, .. } => Some(ty.clone()),
            Expr::Bool(_) => Some(Type::Bool),
            Expr::Char(_) => Some(Type::Char),
            Expr::String(_) => Some(Type::String),
            Expr::Null => None,
            Expr::Var(name) => self
                .locals
                .get(name)
                .cloned()
                .or_else(|| self.enum_types.get(name).cloned())
                .or_else(|| self.const_types.get(name).cloned()),
            Expr::Unary { op, expr } => match op {
                UnaryOp::AddressOf | UnaryOp::MutableAddressOf => {
                    self.expr_type(expr).map(|ty| Type::Reference {
                        mutable: matches!(op, UnaryOp::MutableAddressOf),
                        inner: Box::new(ty),
                    })
                }
                UnaryOp::Deref => match self.expr_type(expr) {
                    Some(Type::Pointer(inner)) | Some(Type::Reference { inner, .. }) => {
                        Some(*inner)
                    }
                    _ => None,
                },
                UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot => self.expr_type(expr),
            },
            Expr::Cast { ty, .. } => Some(ty.clone()),
            Expr::SizeOf(_) => Some(Type::Usize),
            Expr::AlignOf(_) => Some(Type::Usize),
            Expr::OffsetOf { .. } => Some(Type::Usize),
            Expr::Binary { left, .. } => self.expr_type(left),
            Expr::Call { name, .. } => self.function_returns.get(name).cloned(),
            Expr::Struct { name, .. } => Some(Type::Named(name.clone())),
            Expr::Array(values) => values
                .first()
                .and_then(|value| self.expr_type(value))
                .map(|ty| Type::Array(Box::new(ty))),
            Expr::Field { base, name } => {
                if let Expr::Var(enum_name) = base.as_ref() {
                    if self
                        .enum_discriminants
                        .contains_key(&(enum_name.clone(), name.clone()))
                    {
                        return Some(Type::Named(enum_name.clone()));
                    }
                }
                let Type::Named(struct_name) = self.expr_type(base)? else {
                    return None;
                };
                self.structs
                    .get(&struct_name)?
                    .fields
                    .iter()
                    .find(|field| field.name == *name)
                    .map(|field| field.ty.clone())
            }
            Expr::Index { base, .. } => match self.expr_type(base) {
                Some(Type::Array(inner)) | Some(Type::Slice(inner)) => Some(*inner),
                Some(Type::String) => Some(Type::Char),
                _ => None,
            },
            Expr::Match { arms, .. } => arms.first().and_then(|arm| self.expr_type(&arm.value)),
            Expr::If { then_value, .. } => self.expr_type(then_value),
            Expr::Block { value, .. } => self.expr_type(value),
        }
    }

    fn lower_match_pattern_value(&mut self, pattern: &MatchPattern) -> Option<ValueId> {
        let value = match pattern {
            MatchPattern::Wildcard => return None,
            MatchPattern::Int(value) => *value,
            MatchPattern::Bool(value) => i64::from(*value),
            MatchPattern::EnumVariant { enum_name, variant } => self
                .enum_discriminants
                .get(&(enum_name.clone(), variant.clone()))
                .copied()
                .unwrap_or(0),
        };
        let dst = self.fresh();
        self.instructions.push(Instruction::Const { dst, value });
        Some(dst)
    }

    fn let_type(&self, annotated: &Option<Type>, value: &Expr) -> Option<Type> {
        annotated.clone().or_else(|| self.expr_type(value))
    }

    fn is_aggregate_type(&self, ty: &Type) -> bool {
        match ty {
            Type::Named(name) => self.structs.contains_key(name),
            Type::Array(_) => true,
            _ => false,
        }
    }

    fn pointer_deref_width(&self, expr: &Expr) -> u8 {
        match self.expr_type(expr) {
            Some(Type::Pointer(inner)) | Some(Type::Reference { inner, .. }) => {
                self.type_size(&inner).clamp(1, 8) as u8
            }
            _ => 8,
        }
    }

    fn type_size(&self, ty: &Type) -> i64 {
        match ty {
            Type::Unit => 0,
            Type::Bool | Type::I8 | Type::U8 => 1,
            Type::I16 | Type::U16 => 2,
            Type::Char | Type::I32 | Type::U32 => 4,
            Type::Int
            | Type::String
            | Type::Usize
            | Type::I64
            | Type::U64
            | Type::Slice(_)
            | Type::Reference { .. }
            | Type::Pointer(_) => 8,
            Type::Array(inner) => self.type_size(inner),
            Type::Named(name) => {
                if let Some(struct_decl) = self.structs.get(name) {
                    self.struct_layout(struct_decl).0
                } else {
                    8
                }
            }
        }
    }

    fn type_align(&self, ty: &Type) -> i64 {
        match ty {
            Type::Unit => 1,
            Type::Bool | Type::I8 | Type::U8 => 1,
            Type::I16 | Type::U16 => 2,
            Type::Char | Type::I32 | Type::U32 => 4,
            Type::Int
            | Type::String
            | Type::Usize
            | Type::I64
            | Type::U64
            | Type::Slice(_)
            | Type::Reference { .. }
            | Type::Pointer(_) => 8,
            Type::Array(inner) => self.type_align(inner),
            Type::Named(name) => {
                if let Some(struct_decl) = self.structs.get(name) {
                    struct_decl
                        .fields
                        .iter()
                        .map(|field| self.type_align(&field.ty))
                        .max()
                        .unwrap_or(1)
                } else {
                    8
                }
            }
        }
    }

    fn field_offset(&self, ty: &Type, field: &str) -> i64 {
        let Type::Named(name) = ty else {
            return 0;
        };
        let Some(struct_decl) = self.structs.get(name) else {
            return 0;
        };
        self.struct_layout(struct_decl)
            .1
            .into_iter()
            .find(|(candidate, _)| candidate == field)
            .map(|(_, offset)| offset)
            .unwrap_or(0)
    }

    fn struct_layout(&self, struct_decl: &StructDecl) -> (i64, Vec<(String, i64)>) {
        let mut offset = 0;
        let mut max_align = 1;
        let mut fields = Vec::new();

        for field in &struct_decl.fields {
            let align = self.type_align(&field.ty);
            max_align = max_align.max(align);
            offset = align_to_i64(offset, align);
            fields.push((field.name.clone(), offset));
            offset += self.type_size(&field.ty);
        }

        (align_to_i64(offset, max_align), fields)
    }

    fn lower_pointer_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Option<ValueId> {
        match (op, self.expr_type(left), self.expr_type(right)) {
            (BinaryOp::Add | BinaryOp::Sub, Some(Type::Pointer(inner)), Some(right_ty))
                if is_integer_type(&right_ty) =>
            {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let scale = self.pointer_scale(&inner);
                let scaled_right = self.scale_value(right, scale);
                let dst = self.fresh();
                let instruction = match op {
                    BinaryOp::Add => Instruction::Add {
                        dst,
                        left,
                        right: scaled_right,
                    },
                    BinaryOp::Sub => Instruction::Sub {
                        dst,
                        left,
                        right: scaled_right,
                    },
                    _ => unreachable!("guarded by match"),
                };
                self.instructions.push(instruction);
                Some(dst)
            }
            (BinaryOp::Sub, Some(Type::Pointer(inner)), Some(Type::Pointer(right_inner)))
                if inner == right_inner =>
            {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let diff = self.fresh();
                self.instructions.push(Instruction::Sub {
                    dst: diff,
                    left,
                    right,
                });
                let scale = self.pointer_scale(&inner);
                let scale_value = self.fresh();
                self.instructions.push(Instruction::Const {
                    dst: scale_value,
                    value: scale,
                });
                let dst = self.fresh();
                self.instructions.push(Instruction::Div {
                    dst,
                    left: diff,
                    right: scale_value,
                });
                Some(dst)
            }
            _ => None,
        }
    }

    fn scale_value(&mut self, value: ValueId, scale: i64) -> ValueId {
        if scale == 1 {
            return value;
        }
        let scale_value = self.fresh();
        self.instructions.push(Instruction::Const {
            dst: scale_value,
            value: scale,
        });
        let dst = self.fresh();
        self.instructions.push(Instruction::Mul {
            dst,
            left: value,
            right: scale_value,
        });
        dst
    }

    fn pointer_scale(&self, inner: &Type) -> i64 {
        self.type_size(inner).max(1)
    }
}

fn align_to_i64(value: i64, align: i64) -> i64 {
    if align <= 1 {
        value
    } else {
        ((value + align - 1) / align) * align
    }
}

fn is_integer_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Int
            | Type::Usize
            | Type::I8
            | Type::I16
            | Type::I32
            | Type::I64
            | Type::U8
            | Type::U16
            | Type::U32
            | Type::U64
    )
}

fn is_aggregate_expr(value: &Expr) -> bool {
    matches!(value, Expr::Struct { .. } | Expr::Array(_))
}

fn runtime_symbols(program: &Program) -> HashMap<String, String> {
    let mut symbols = HashMap::new();
    for import in &program.imports {
        if let Ok(functions) = runtime::functions_for_import(&import.path) {
            for function in functions {
                symbols.insert(function.name, function.symbol);
            }
        }
    }
    symbols
}

fn function_returns(program: &Program) -> HashMap<String, Type> {
    let mut returns = HashMap::new();
    for import in &program.imports {
        if let Ok(functions) = runtime::functions_for_import(&import.path) {
            for function in functions {
                returns.insert(function.name, function.return_type);
            }
        }
    }
    for extern_function in &program.externs {
        returns.insert(
            extern_function.name.clone(),
            extern_function.return_type.clone(),
        );
    }
    for function in &program.functions {
        returns.insert(function.name.clone(), function.return_type.clone());
    }
    returns
}

fn enum_discriminants(program: &Program) -> HashMap<(String, String), i64> {
    let mut values = HashMap::new();
    for enum_decl in &program.enums {
        let mut next_discriminant = 0;
        for variant in &enum_decl.variants {
            let value = variant.value.unwrap_or(next_discriminant);
            values.insert((enum_decl.name.clone(), variant.name.clone()), value);
            next_discriminant = value + 1;
        }
    }
    values
}
