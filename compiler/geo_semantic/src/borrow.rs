use crate::ast::{Expr, Function, Program, Stmt, Type, UnaryOp};
use crate::diagnostics::Diagnostic;
use std::collections::{HashMap, HashSet};

pub fn check(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for function in &program.functions {
        check_function(function, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_function(function: &Function, diagnostics: &mut Vec<Diagnostic>) {
    let diagnostic_start = diagnostics.len();
    let mut ctx = BorrowCtx {
        locals: HashMap::new(),
        moved: HashSet::new(),
        borrows: HashMap::new(),
        reference_origins: HashMap::new(),
        diagnostics,
    };

    for param in &function.params {
        ctx.locals.insert(param.name.clone(), param.ty.clone());
    }

    for (index, statement) in function.body.iter().enumerate() {
        let statement_diagnostic_start = ctx.diagnostics.len();
        ctx.check_stmts(std::slice::from_ref(statement));
        let expression_span = function
            .statement_expression_ranges
            .get(index)
            .and_then(|(_, end)| end.checked_sub(1))
            .and_then(|expression| function.expression_spans.get(expression))
            .or_else(|| function.statement_spans.get(index));
        if let Some(span) = expression_span {
            for diagnostic in ctx.diagnostics.iter_mut().skip(statement_diagnostic_start) {
                if diagnostic.span.is_none() {
                    diagnostic.span = Some(crate::diagnostics::DiagnosticSpan {
                        offset: span.offset,
                        len: span.len,
                    });
                }
            }
        }
    }

    for diagnostic in diagnostics.iter_mut().skip(diagnostic_start) {
        if diagnostic.span.is_none() && function.span.len > 0 {
            diagnostic.span = Some(crate::diagnostics::DiagnosticSpan {
                offset: function.span.offset,
                len: function.span.len,
            });
        }
        if diagnostic.source_path.is_none() {
            diagnostic.source_path = function.source_path.clone();
        }
    }
}

struct BorrowCtx<'a> {
    locals: HashMap<String, Type>,
    moved: HashSet<String>,
    borrows: HashMap<String, BorrowState>,
    reference_origins: HashMap<String, String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, Default)]
struct BorrowState {
    shared: usize,
    mutable: bool,
    temporary_shared: usize,
    temporary_mutable: bool,
}

impl BorrowCtx<'_> {
    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.check_stmt(stmt);
            self.expire_temporary_borrows();
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.reject_escaping_borrow(expr);
                    self.consume_expr(expr);
                }
            }
            Stmt::Expr(expr) => {
                self.consume_expr(expr);
            }
            Stmt::Let {
                name, ty, value, ..
            } => {
                self.consume_expr(value);
                let ty = ty
                    .clone()
                    .or_else(|| self.infer_expr_type(value))
                    .unwrap_or(Type::Int);
                if matches!(ty, Type::Reference { .. }) {
                    self.promote_temporary_borrows();
                    if let Some(source) = borrowed_source(value) {
                        self.reference_origins.insert(name.clone(), source);
                    }
                }
                self.locals.insert(name.clone(), ty);
                self.moved.remove(name);
            }
            Stmt::Assign { name, op, value } => {
                self.ensure_not_borrowed_for_assignment(name);
                if op.is_some() {
                    self.ensure_not_moved(name);
                }
                self.consume_expr(value);
                self.moved.remove(name);
            }
            Stmt::PointerAssign { pointer, value, .. } => {
                self.read_expr(pointer);
                self.consume_expr(value);
            }
            Stmt::PlaceAssign { target, op, value } => {
                if let Some(name) = borrowed_local(target) {
                    self.ensure_not_borrowed_for_assignment(&name);
                }
                if op.is_some() {
                    self.read_expr(target);
                } else if let Expr::Index { index, .. } = target {
                    self.read_expr(index);
                }
                self.consume_expr(value);
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                self.read_expr(condition);
                let before = self.moved.clone();
                self.check_stmts(then_body);
                let then_moved = self.moved.clone();
                self.moved = before.clone();
                self.check_stmts(else_body);
                self.moved.extend(then_moved);
            }
            Stmt::While { condition, body } => {
                self.read_expr(condition);
                self.check_stmts(body);
            }
            Stmt::For {
                name,
                start,
                end,
                inclusive: _,
                body,
            } => {
                self.read_expr(start);
                self.read_expr(end);
                let previous = self.locals.insert(name.clone(), Type::Int);
                self.moved.remove(name);
                self.check_stmts(body);
                if let Some(previous) = previous {
                    self.locals.insert(name.clone(), previous);
                } else {
                    self.locals.remove(name);
                }
            }
            Stmt::Loop(body) => {
                self.check_stmts(body);
            }
            Stmt::Unsafe(body) => {
                self.check_stmts(body);
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }

    fn consume_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) if self.is_owned_local(name) => {
                self.ensure_not_moved(name);
                self.ensure_not_borrowed_for_move(name);
                self.moved.insert(name.clone());
            }
            Expr::Var(name) => self.ensure_not_moved(name),
            Expr::Struct { fields, .. } => {
                for (_, value) in fields {
                    self.consume_expr(value);
                }
            }
            Expr::Array(values) => {
                for value in values {
                    self.consume_expr(value);
                }
            }
            Expr::Call { name, args } => {
                for arg in args {
                    if non_consuming_string_call(name) {
                        self.read_expr(arg);
                    } else {
                        self.consume_expr(arg);
                    }
                }
            }
            Expr::Unary { op, expr } => match op {
                UnaryOp::AddressOf => self.borrow_expr(expr, false),
                UnaryOp::MutableAddressOf => self.borrow_expr(expr, true),
                UnaryOp::Deref | UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot => {
                    self.read_expr(expr)
                }
            },
            Expr::Cast { expr, .. } => self.read_expr(expr),
            Expr::SizeOf(_) => {}
            Expr::AlignOf(_) => {}
            Expr::OffsetOf { .. } => {}
            Expr::Binary { left, right, .. } => {
                self.read_expr(left);
                self.read_expr(right);
            }
            Expr::Match { value, arms } => {
                self.read_expr(value);
                for arm in arms {
                    self.consume_expr(&arm.value);
                }
            }
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                self.read_expr(condition);
                self.consume_expr(then_value);
                self.consume_expr(else_value);
            }
            Expr::Block { statements, value } => {
                for stmt in statements {
                    self.check_stmt(stmt);
                }
                self.consume_expr(value);
            }
            Expr::Field { base, .. } => self.read_expr(base),
            Expr::Index { base, index } => {
                self.read_expr(base);
                self.read_expr(index);
            }
            Expr::Int(_)
            | Expr::TypedInt { .. }
            | Expr::Bool(_)
            | Expr::Char(_)
            | Expr::String(_)
            | Expr::Null => {}
        }
    }

    fn read_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) => self.ensure_not_moved(name),
            Expr::Unary { op, expr } => match op {
                UnaryOp::AddressOf => self.borrow_expr(expr, false),
                UnaryOp::MutableAddressOf => self.borrow_expr(expr, true),
                UnaryOp::Deref | UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot => {
                    self.read_expr(expr)
                }
            },
            Expr::Cast { expr, .. } => self.read_expr(expr),
            Expr::SizeOf(_) => {}
            Expr::AlignOf(_) => {}
            Expr::OffsetOf { .. } => {}
            Expr::Binary { left, right, .. } => {
                self.read_expr(left);
                self.read_expr(right);
            }
            Expr::Match { value, arms } => {
                self.read_expr(value);
                for arm in arms {
                    self.read_expr(&arm.value);
                }
            }
            Expr::If {
                condition,
                then_value,
                else_value,
            } => {
                self.read_expr(condition);
                self.read_expr(then_value);
                self.read_expr(else_value);
            }
            Expr::Block { statements, value } => {
                for stmt in statements {
                    self.check_stmt(stmt);
                }
                self.read_expr(value);
            }
            Expr::Call { name, args } => {
                for arg in args {
                    if non_consuming_string_call(name) {
                        self.read_expr(arg);
                    } else {
                        self.consume_expr(arg);
                    }
                }
            }
            Expr::Struct { fields, .. } => {
                for (_, value) in fields {
                    self.consume_expr(value);
                }
            }
            Expr::Array(values) => {
                for value in values {
                    self.consume_expr(value);
                }
            }
            Expr::Field { base, .. } => self.read_expr(base),
            Expr::Index { base, index } => {
                self.read_expr(base);
                self.read_expr(index);
            }
            Expr::Int(_)
            | Expr::TypedInt { .. }
            | Expr::Bool(_)
            | Expr::Char(_)
            | Expr::String(_)
            | Expr::Null => {}
        }
    }

    fn ensure_not_moved(&mut self, name: &str) {
        if self.moved.contains(name) {
            self.diagnostics
                .push(Diagnostic::error(format!("use of moved value '{name}'")));
        }
    }

    fn ensure_not_borrowed_for_move(&mut self, name: &str) {
        if self
            .borrows
            .get(name)
            .is_some_and(|state| state.shared > 0 || state.mutable)
        {
            self.diagnostics.push(Diagnostic::error(format!(
                "cannot move borrowed value '{name}'"
            )));
        }
    }

    fn ensure_not_borrowed_for_assignment(&mut self, name: &str) {
        if self
            .borrows
            .get(name)
            .is_some_and(|state| state.shared > 0 || state.mutable)
        {
            self.diagnostics.push(Diagnostic::error(format!(
                "cannot assign to borrowed value '{name}'"
            )));
        }
    }

    fn borrow_expr(&mut self, expr: &Expr, mutable: bool) {
        self.read_expr(expr);
        let Some(name) = borrowed_local(expr) else {
            return;
        };
        let state = self.borrows.entry(name.clone()).or_default();
        if mutable {
            if state.shared > 0 || state.mutable {
                self.diagnostics.push(Diagnostic::error(format!(
                    "cannot mutably borrow '{name}' while it is already borrowed"
                )));
            }
            state.mutable = true;
            state.temporary_mutable = true;
        } else if state.mutable {
            self.diagnostics.push(Diagnostic::error(format!(
                "cannot borrow '{name}' while it is mutably borrowed"
            )));
        } else {
            state.shared += 1;
            state.temporary_shared += 1;
        }
    }

    fn reject_escaping_borrow(&mut self, expr: &Expr) {
        if let Expr::Unary {
            op: UnaryOp::AddressOf | UnaryOp::MutableAddressOf,
            expr,
        } = expr
        {
            if let Some(name) = borrowed_local(expr) {
                self.diagnostics
                    .push(Diagnostic::error(format!("borrow of '{name}' escapes")));
            }
        } else if let Expr::Var(name) = expr {
            if let Some(source) = self.reference_origins.get(name) {
                self.diagnostics.push(Diagnostic::error(format!(
                    "borrow of '{source}' escapes through reference '{name}'"
                )));
            }
        }
    }

    fn expire_temporary_borrows(&mut self) {
        self.borrows.retain(|_, state| {
            state.shared = state.shared.saturating_sub(state.temporary_shared);
            state.temporary_shared = 0;
            if state.temporary_mutable {
                state.mutable = false;
                state.temporary_mutable = false;
            }
            state.shared > 0 || state.mutable
        });
    }

    fn promote_temporary_borrows(&mut self) {
        for state in self.borrows.values_mut() {
            state.temporary_shared = 0;
            state.temporary_mutable = false;
        }
    }

    fn is_owned_local(&self, name: &str) -> bool {
        self.locals.get(name).is_some_and(is_owned_type)
    }

    fn infer_expr_type(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Int(_) => Some(Type::Int),
            Expr::TypedInt { ty, .. } => Some(ty.clone()),
            Expr::Bool(_) => Some(Type::Bool),
            Expr::Char(_) => Some(Type::Char),
            Expr::String(_) => Some(Type::String),
            Expr::Var(name) => self.locals.get(name).cloned(),
            Expr::Struct { name, .. } => Some(Type::Named(name.clone())),
            Expr::Array(values) => values
                .first()
                .and_then(|value| self.infer_expr_type(value))
                .map(|ty| Type::Array(Box::new(ty))),
            Expr::Unary { op, expr } => match op {
                UnaryOp::AddressOf => self.infer_expr_type(expr).map(|inner| Type::Reference {
                    mutable: false,
                    inner: Box::new(inner),
                }),
                UnaryOp::MutableAddressOf => {
                    self.infer_expr_type(expr).map(|inner| Type::Reference {
                        mutable: true,
                        inner: Box::new(inner),
                    })
                }
                UnaryOp::Deref => self.infer_expr_type(expr).and_then(|ty| match ty {
                    Type::Reference { inner, .. } => Some(*inner),
                    _ => None,
                }),
                UnaryOp::Neg | UnaryOp::Not | UnaryOp::BitNot => self.infer_expr_type(expr),
            },
            Expr::Cast { ty, .. } => Some(ty.clone()),
            Expr::SizeOf(_) => Some(Type::Usize),
            Expr::AlignOf(_) => Some(Type::Usize),
            Expr::OffsetOf { .. } => Some(Type::Usize),
            Expr::Null => None,
            Expr::Binary { left, .. } => self.infer_expr_type(left),
            Expr::Match { arms, .. } => arms
                .first()
                .and_then(|arm| self.infer_expr_type(&arm.value)),
            Expr::If { then_value, .. } => self.infer_expr_type(then_value),
            Expr::Block { value, .. } => self.infer_expr_type(value),
            Expr::Field { .. } | Expr::Index { .. } | Expr::Call { .. } => None,
        }
    }
}

fn borrowed_local(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::Field { base, .. } | Expr::Index { base, .. } => borrowed_local(base),
        _ => None,
    }
}

fn borrowed_source(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Unary {
            op: UnaryOp::AddressOf | UnaryOp::MutableAddressOf,
            expr,
        } => borrowed_local(expr),
        _ => borrowed_local(expr),
    }
}

fn non_consuming_string_call(function: &str) -> bool {
    matches!(function, "string_len" | "string_byte_at")
}

fn is_owned_type(ty: &Type) -> bool {
    matches!(ty, Type::String | Type::Array(_) | Type::Named(_))
}
