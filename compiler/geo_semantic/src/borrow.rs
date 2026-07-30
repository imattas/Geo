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
        suspended_references: HashMap::new(),
        reborrow_parents: HashMap::new(),
        temporary_reborrow_parents: Vec::new(),
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
    reference_origins: HashMap<String, ReferenceOrigin>,
    suspended_references: HashMap<String, ReferenceOrigin>,
    reborrow_parents: HashMap<String, String>,
    temporary_reborrow_parents: Vec<String>,
    diagnostics: &'a mut Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, Default)]
struct BorrowState {
    shared: usize,
    mutable: bool,
    temporary_shared: usize,
    temporary_mutable: bool,
    retained_shared: usize,
    retained_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReferenceOrigin {
    sources: Vec<String>,
    mutable: bool,
}

impl BorrowCtx<'_> {
    fn check_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.check_stmt(stmt);
            self.expire_temporary_borrows();
        }
    }

    fn check_scoped_stmts(&mut self, stmts: &[Stmt]) {
        let saved_locals = self.locals.clone();
        let saved_origins = self.reference_origins.clone();
        self.check_stmts(stmts);
        self.restore_scope(saved_locals, saved_origins);
    }

    fn restore_scope(
        &mut self,
        saved_locals: HashMap<String, Type>,
        saved_origins: HashMap<String, ReferenceOrigin>,
    ) {
        let current_locals = self.locals.clone();
        let inner_references = self
            .reference_origins
            .iter()
            .filter(|(name, origin)| {
                !saved_locals.contains_key(name.as_str())
                    || (saved_origins.get(name.as_str()) != Some(*origin)
                        && current_locals
                            .get(name.as_str())
                            .zip(saved_locals.get(name.as_str()))
                            .is_some_and(|(current, saved)| local_changed(current, saved)))
            })
            .map(|(name, origin)| (name.clone(), origin.clone()))
            .collect::<Vec<_>>();
        let mut inner_references = inner_references;
        inner_references.sort_by_key(|(name, _)| !self.reborrow_parents.contains_key(name));
        for (name, origin) in inner_references {
            self.release_reference_binding(&name, &origin);
        }
        for (name, origin) in &saved_origins {
            if current_locals
                .get(name)
                .zip(saved_locals.get(name))
                .is_some_and(|(current, saved)| local_changed(current, saved))
            {
                self.reference_origins.insert(name.clone(), origin.clone());
            }
        }
        self.locals = saved_locals;
        self.reference_origins
            .retain(|name, _| self.locals.contains_key(name));
        self.moved.retain(|name| self.locals.contains_key(name));
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
                    if let Some(source) = self.borrow_source(value) {
                        self.reference_origins.insert(
                            name.clone(),
                            ReferenceOrigin {
                                sources: vec![source],
                                mutable: matches!(ty, Type::Reference { mutable: true, .. }),
                            },
                        );
                        if let Some(parent) = self.mutable_reborrow_parent_from_borrow(value) {
                            self.temporary_reborrow_parents
                                .retain(|candidate| candidate != &parent);
                            self.reborrow_parents.insert(name.clone(), parent);
                        }
                    }
                } else {
                    self.reference_origins.remove(name);
                }
                self.locals.insert(name.clone(), ty);
                self.moved.remove(name);
            }
            Stmt::Assign { name, op, value } => {
                self.ensure_not_borrowed_for_assignment(name);
                if op.is_some() {
                    self.ensure_not_moved(name);
                }
                let old_origin = self.reference_origins.get(name).cloned();
                self.consume_expr(value);
                if let Some(origin) = old_origin {
                    self.release_reference_binding(name, &origin);
                }
                self.moved.remove(name);
                self.reference_origins.remove(name);
                if let Some(Type::Reference { mutable, .. }) = self.locals.get(name).cloned() {
                    self.promote_temporary_borrows();
                    if let Some(source) = self.borrow_source(value) {
                        self.reference_origins.insert(
                            name.clone(),
                            ReferenceOrigin {
                                sources: vec![source],
                                mutable,
                            },
                        );
                        if let Some(parent) = self.mutable_reborrow_parent_from_borrow(value) {
                            self.temporary_reborrow_parents
                                .retain(|candidate| candidate != &parent);
                            self.reborrow_parents.insert(name.clone(), parent);
                        }
                    }
                }
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
                let before_borrows = self.borrows.clone();
                let before_origins = self.reference_origins.clone();
                self.check_scoped_stmts(then_body);
                let then_moved = self.moved.clone();
                let then_borrows = self.borrows.clone();
                let then_origins = self.reference_origins.clone();
                self.moved = before.clone();
                self.borrows = before_borrows.clone();
                self.reference_origins = before_origins.clone();
                self.check_scoped_stmts(else_body);
                let else_moved = self.moved.clone();
                let else_borrows = self.borrows.clone();
                let else_origins = self.reference_origins.clone();
                self.moved = definitely_moved_after_branches(&then_moved, &else_moved);
                self.borrows = merge_borrows(&then_borrows, &else_borrows);
                self.reference_origins = merge_reference_origins(&then_origins, &else_origins);
            }
            Stmt::While { condition, body } => {
                self.read_expr(condition);
                let before = self.moved.clone();
                let before_borrows = self.borrows.clone();
                let before_origins = self.reference_origins.clone();
                self.check_scoped_stmts(body);
                self.moved = before;
                self.borrows = before_borrows;
                self.reference_origins = before_origins;
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
                let before_moved = self.moved.clone();
                let before_borrows = self.borrows.clone();
                let before_origins = self.reference_origins.clone();
                let previous = self.locals.insert(name.clone(), Type::Int);
                self.moved.remove(name);
                self.check_scoped_stmts(body);
                if let Some(previous) = previous {
                    self.locals.insert(name.clone(), previous);
                } else {
                    self.locals.remove(name);
                }
                self.moved = before_moved;
                self.borrows = before_borrows;
                self.reference_origins = before_origins;
            }
            Stmt::Loop(body) => {
                self.check_scoped_stmts(body);
            }
            Stmt::Unsafe(body) => {
                self.check_scoped_stmts(body);
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }

    fn consume_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name) if self.is_owned_local(name) => {
                self.ensure_not_moved(name);
                self.ensure_not_borrowed_for_move(name);
                self.ensure_not_reborrowed(name);
                self.moved.insert(name.clone());
            }
            Expr::Var(name) => {
                self.ensure_not_moved(name);
                self.ensure_not_reborrowed(name);
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
                self.check_expr_block(statements, value, true);
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
            Expr::Var(name) => {
                self.ensure_not_moved(name);
                self.ensure_not_reborrowed(name);
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
                self.check_expr_block(statements, value, false);
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

    fn ensure_not_reborrowed(&mut self, name: &str) {
        if self.suspended_references.contains_key(name) {
            self.diagnostics.push(Diagnostic::error(format!(
                "cannot use reborrowed reference '{name}' while nested borrow is active"
            )));
        }
    }

    fn borrow_expr(&mut self, expr: &Expr, mutable: bool) {
        self.read_expr(expr);
        let reborrow_parent = if mutable {
            self.mutable_reborrow_parent(expr)
        } else {
            None
        };
        if let Some(parent) = reborrow_parent.as_ref() {
            if self.suspended_references.contains_key(parent) {
                return;
            }
            if let Some(origin) = self.reference_origins.get(parent).cloned() {
                self.release_reference_origin(&origin);
                self.suspended_references.insert(parent.clone(), origin);
                self.temporary_reborrow_parents.push(parent.clone());
            }
        }
        let Some(name) = self.borrow_target(expr) else {
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
            if let Some(name) = self.borrow_target(expr) {
                self.diagnostics
                    .push(Diagnostic::error(format!("borrow of '{name}' escapes")));
            }
        } else if let Expr::Var(name) = expr {
            if let Some(source) = self.reference_origins.get(name) {
                self.diagnostics.push(Diagnostic::error(format!(
                    "borrow of '{}' escapes through reference '{name}'",
                    self.root_reference_source(&source.sources[0])
                )));
            }
        }
    }

    fn check_expr_block(&mut self, statements: &[Stmt], value: &Expr, consume: bool) {
        let saved_locals = self.locals.clone();
        let saved_origins = self.reference_origins.clone();
        self.check_stmts(statements);
        if consume {
            self.consume_expr(value);
        } else {
            self.read_expr(value);
        }
        self.restore_scope(saved_locals, saved_origins);
    }

    fn expire_temporary_borrows(&mut self) {
        self.borrows.retain(|_, state| {
            state.shared = state.shared.saturating_sub(state.temporary_shared);
            state.temporary_shared = 0;
            state.temporary_mutable = false;
            state.mutable = state.retained_mutable;
            state.shared > 0 || state.mutable
        });
        let parents = std::mem::take(&mut self.temporary_reborrow_parents);
        for parent in parents {
            self.restore_suspended_reference(&parent);
        }
    }

    fn promote_temporary_borrows(&mut self) {
        for state in self.borrows.values_mut() {
            state.retained_shared += state.temporary_shared;
            state.retained_mutable |= state.temporary_mutable;
            state.temporary_shared = 0;
            state.temporary_mutable = false;
            state.mutable = state.retained_mutable;
        }
    }

    fn release_reference_binding(&mut self, name: &str, origin: &ReferenceOrigin) {
        self.release_reference_origin(origin);
        if let Some(parent) = self.reborrow_parents.remove(name) {
            self.restore_suspended_reference(&parent);
        }
    }

    fn release_reference_origin(&mut self, origin: &ReferenceOrigin) {
        for source in &origin.sources {
            self.release_retained_borrow(source, origin.mutable);
        }
    }

    fn restore_suspended_reference(&mut self, parent: &str) {
        let Some(origin) = self.suspended_references.remove(parent) else {
            return;
        };
        for source in origin.sources {
            self.retain_borrow(&source, origin.mutable);
        }
    }

    fn retain_borrow(&mut self, source: &str, mutable: bool) {
        let state = self.borrows.entry(source.to_string()).or_default();
        if mutable {
            state.retained_mutable = true;
            state.mutable = true;
        } else {
            state.retained_shared += 1;
            state.shared += 1;
        }
    }

    fn release_retained_borrow(&mut self, source: &str, mutable: bool) {
        let Some(state) = self.borrows.get_mut(source) else {
            return;
        };
        if mutable {
            state.retained_mutable = false;
            state.mutable = state.retained_mutable;
        } else {
            state.retained_shared = state.retained_shared.saturating_sub(1);
            state.shared = state.shared.saturating_sub(1);
        }
        if state.shared == 0 && !state.mutable {
            self.borrows.remove(source);
        }
    }

    fn borrow_target(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Unary {
                op: UnaryOp::Deref,
                expr,
            } => borrowed_local(expr).map(|name| self.root_reference_source(&name)),
            _ => borrowed_local(expr),
        }
    }

    fn borrow_source(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Unary {
                op: UnaryOp::AddressOf | UnaryOp::MutableAddressOf,
                expr,
            } => self.borrow_target(expr),
            _ => borrowed_local(expr),
        }
    }

    fn mutable_reborrow_parent(&self, expr: &Expr) -> Option<String> {
        let Expr::Unary {
            op: UnaryOp::Deref,
            expr,
        } = expr
        else {
            return None;
        };
        let parent = borrowed_local(expr)?;
        self.reference_origins
            .contains_key(&parent)
            .then_some(parent)
    }

    fn mutable_reborrow_parent_from_borrow(&self, expr: &Expr) -> Option<String> {
        let Expr::Unary {
            op: UnaryOp::MutableAddressOf,
            expr,
        } = expr
        else {
            return None;
        };
        self.mutable_reborrow_parent(expr)
    }

    fn root_reference_source(&self, name: &str) -> String {
        let mut current = name.to_string();
        let mut seen = HashSet::new();
        while seen.insert(current.clone()) {
            let Some(origin) = self.reference_origins.get(&current) else {
                break;
            };
            current = origin.sources[0].clone();
        }
        current
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

fn local_changed(current: &Type, saved: &Type) -> bool {
    current != saved
}

fn definitely_moved_after_branches(
    then_moved: &HashSet<String>,
    else_moved: &HashSet<String>,
) -> HashSet<String> {
    then_moved.intersection(else_moved).cloned().collect()
}

fn merge_borrows(
    then_borrows: &HashMap<String, BorrowState>,
    else_borrows: &HashMap<String, BorrowState>,
) -> HashMap<String, BorrowState> {
    let mut merged = then_borrows.clone();
    for (name, else_state) in else_borrows {
        let state = merged.entry(name.clone()).or_default();
        state.shared = state.shared.max(else_state.shared);
        state.mutable |= else_state.mutable;
        state.temporary_shared = state.temporary_shared.max(else_state.temporary_shared);
        state.temporary_mutable |= else_state.temporary_mutable;
        state.retained_shared = state.retained_shared.max(else_state.retained_shared);
        state.retained_mutable |= else_state.retained_mutable;
    }
    merged
}

fn merge_reference_origins(
    then_origins: &HashMap<String, ReferenceOrigin>,
    else_origins: &HashMap<String, ReferenceOrigin>,
) -> HashMap<String, ReferenceOrigin> {
    let mut merged = then_origins.clone();
    for (name, else_origin) in else_origins {
        let Some(origin) = merged.get_mut(name) else {
            merged.insert(name.clone(), else_origin.clone());
            continue;
        };
        origin.mutable |= else_origin.mutable;
        for source in &else_origin.sources {
            if !origin.sources.contains(source) {
                origin.sources.push(source.clone());
            }
        }
    }
    merged
}

fn borrowed_local(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::Field { base, .. } | Expr::Index { base, .. } => borrowed_local(base),
        _ => None,
    }
}

fn non_consuming_string_call(function: &str) -> bool {
    matches!(function, "string_len" | "string_byte_at")
}

fn is_owned_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::String | Type::Array(_) | Type::ArrayFixed(_, _) | Type::Named(_)
    )
}
