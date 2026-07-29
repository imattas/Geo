use crate::ast::{
    BinaryOp, ConstDecl, EnumDecl, Expr, MatchPattern, Param, Program, Stmt, StructDecl, Type,
    UnaryOp,
};
use crate::diagnostics::Diagnostic;
use crate::runtime;
use std::collections::{HashMap, HashSet};

pub fn check(program: &Program) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let raw_structs = struct_map(&program.structs);
    let raw_enums = enum_map(&program.enums);
    let aliases = type_alias_map(program, &raw_structs, &raw_enums, &mut diagnostics);
    for alias in &program.type_aliases {
        let expanded = expand_type_alias(&alias.ty, &aliases, &mut Vec::new(), &mut diagnostics);
        validate_type(&expanded, &raw_structs, &raw_enums, &mut diagnostics);
    }
    let program = expand_program_type_aliases(program, &aliases, &mut diagnostics);
    let program = &program;

    let mut functions = HashMap::new();
    let mut structs = HashMap::new();
    let mut enums = HashMap::new();
    let mut const_names = HashMap::new();

    for struct_decl in &program.structs {
        if structs
            .insert(struct_decl.name.as_str(), struct_decl)
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate struct '{}'",
                struct_decl.name
            )));
        }
        let mut fields = HashMap::new();
        for field in &struct_decl.fields {
            if fields.insert(field.name.as_str(), ()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate field '{}' in struct '{}'",
                    field.name, struct_decl.name
                )));
            }
            validate_type(&field.ty, &structs, &enums, &mut diagnostics);
        }
    }

    for enum_decl in &program.enums {
        if structs.contains_key(enum_decl.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate type '{}'",
                enum_decl.name
            )));
        }
        if enums.insert(enum_decl.name.as_str(), enum_decl).is_some() {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate enum '{}'",
                enum_decl.name
            )));
        }
        let mut variants = HashMap::new();
        let mut discriminants = HashMap::new();
        let mut next_discriminant = 0;
        for variant in &enum_decl.variants {
            if variants.insert(variant.name.as_str(), ()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate variant '{}' in enum '{}'",
                    variant.name, enum_decl.name
                )));
            }
            let value = variant.value.unwrap_or(next_discriminant);
            if discriminants.insert(value, variant.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate discriminant '{}' in enum '{}'",
                    value, enum_decl.name
                )));
            }
            next_discriminant = value + 1;
        }
    }

    for import in &program.imports {
        match runtime::functions_for_import(&import.path) {
            Ok(runtime_functions) => {
                for runtime_function in runtime_functions {
                    if functions
                        .insert(
                            runtime_function.name.clone(),
                            Callable {
                                params: runtime_function.params,
                                return_type: runtime_function.return_type,
                            },
                        )
                        .is_some()
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "duplicate function '{}'",
                            runtime_function.name
                        )));
                    }
                }
            }
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    for const_decl in &program.consts {
        validate_type(&const_decl.ty, &structs, &enums, &mut diagnostics);
        if const_names
            .insert(const_decl.name.as_str(), const_decl)
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate constant '{}'",
                const_decl.name
            )));
        }
        if functions.contains_key(const_decl.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function or constant '{}'",
                const_decl.name
            )));
        }
    }

    for extern_function in &program.externs {
        if const_names.contains_key(extern_function.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function or constant '{}'",
                extern_function.name
            )));
        }
        if functions
            .insert(
                extern_function.name.clone(),
                Callable {
                    params: extern_function.params.clone(),
                    return_type: extern_function.return_type.clone(),
                },
            )
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function '{}'",
                extern_function.name
            )));
        }
        validate_type(
            &extern_function.return_type,
            &structs,
            &enums,
            &mut diagnostics,
        );
        for param in &extern_function.params {
            validate_type(&param.ty, &structs, &enums, &mut diagnostics);
        }
    }

    for function in &program.functions {
        if const_names.contains_key(function.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function or constant '{}'",
                function.name
            )));
        }
        if functions
            .insert(
                function.name.clone(),
                Callable {
                    params: function.params.clone(),
                    return_type: function.return_type.clone(),
                },
            )
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate function '{}'",
                function.name
            )));
        }
        validate_type(&function.return_type, &structs, &enums, &mut diagnostics);
        for param in &function.params {
            validate_type(&param.ty, &structs, &enums, &mut diagnostics);
        }
    }

    if !functions.contains_key("main") {
        diagnostics.push(Diagnostic::error("missing main function"));
    }

    let const_locals = const_locals(&program.consts);
    for const_decl in &program.consts {
        let actual = expr_type(
            &const_decl.value,
            Some(&const_decl.ty),
            &const_locals,
            &functions,
            &structs,
            &enums,
            &mut diagnostics,
            0,
        );
        if !type_matches_expr(&const_decl.ty, actual.as_ref(), &const_decl.value) {
            diagnostics.push(Diagnostic::error("const initializer type mismatch"));
        }
    }
    detect_const_cycles(&program.consts, &mut diagnostics);

    for function in &program.functions {
        check_function(
            function,
            &program.consts,
            &functions,
            &structs,
            &enums,
            &mut diagnostics,
        );
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn expand_type_aliases_for_codegen(program: &Program) -> Program {
    let mut diagnostics = Vec::new();
    let raw_structs = struct_map(&program.structs);
    let raw_enums = enum_map(&program.enums);
    let aliases = type_alias_map(program, &raw_structs, &raw_enums, &mut diagnostics);
    expand_program_type_aliases(program, &aliases, &mut diagnostics)
}

#[derive(Clone)]
struct Callable {
    params: Vec<Param>,
    return_type: Type,
}

#[derive(Clone)]
struct Local {
    ty: Type,
    mutable: bool,
}

fn struct_map<'a>(structs: &'a [StructDecl]) -> HashMap<&'a str, &'a StructDecl> {
    structs
        .iter()
        .map(|decl| (decl.name.as_str(), decl))
        .collect()
}

fn enum_map<'a>(enums: &'a [EnumDecl]) -> HashMap<&'a str, &'a EnumDecl> {
    enums
        .iter()
        .map(|decl| (decl.name.as_str(), decl))
        .collect()
}

fn type_alias_map(
    program: &Program,
    structs: &HashMap<&str, &StructDecl>,
    enums: &HashMap<&str, &EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) -> HashMap<String, Type> {
    let mut aliases = HashMap::new();
    for alias in &program.type_aliases {
        if structs.contains_key(alias.name.as_str()) || enums.contains_key(alias.name.as_str()) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate type '{}'",
                alias.name
            )));
        }
        if aliases
            .insert(alias.name.clone(), alias.ty.clone())
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate type alias '{}'",
                alias.name
            )));
        }
    }
    aliases
}

fn expand_program_type_aliases(
    program: &Program,
    aliases: &HashMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Program {
    Program {
        imports: program.imports.clone(),
        type_aliases: program.type_aliases.clone(),
        consts: program
            .consts
            .iter()
            .map(|decl| ConstDecl {
                name: decl.name.clone(),
                ty: expand_type_alias(&decl.ty, aliases, &mut Vec::new(), diagnostics),
                value: expand_expr_type_aliases(&decl.value, aliases, diagnostics),
            })
            .collect(),
        structs: program
            .structs
            .iter()
            .map(|decl| StructDecl {
                name: decl.name.clone(),
                fields: decl
                    .fields
                    .iter()
                    .map(|field| crate::ast::Field {
                        name: field.name.clone(),
                        ty: expand_type_alias(&field.ty, aliases, &mut Vec::new(), diagnostics),
                    })
                    .collect(),
            })
            .collect(),
        enums: program.enums.clone(),
        externs: program
            .externs
            .iter()
            .map(|function| crate::ast::ExternFunction {
                name: function.name.clone(),
                params: expand_params_type_aliases(&function.params, aliases, diagnostics),
                return_type: expand_type_alias(
                    &function.return_type,
                    aliases,
                    &mut Vec::new(),
                    diagnostics,
                ),
            })
            .collect(),
        functions: program
            .functions
            .iter()
            .map(|function| crate::ast::Function {
                name: function.name.clone(),
                params: expand_params_type_aliases(&function.params, aliases, diagnostics),
                return_type: expand_type_alias(
                    &function.return_type,
                    aliases,
                    &mut Vec::new(),
                    diagnostics,
                ),
                body: function
                    .body
                    .iter()
                    .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                    .collect(),
                span: function.span,
                statement_spans: function.statement_spans.clone(),
                expression_spans: function.expression_spans.clone(),
                statement_expression_ranges: function.statement_expression_ranges.clone(),
                source_path: function.source_path.clone(),
            })
            .collect(),
    }
}

fn expand_params_type_aliases(
    params: &[Param],
    aliases: &HashMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Param> {
    params
        .iter()
        .map(|param| Param {
            name: param.name.clone(),
            ty: expand_type_alias(&param.ty, aliases, &mut Vec::new(), diagnostics),
        })
        .collect()
}

fn expand_stmt_type_aliases(
    stmt: &Stmt,
    aliases: &HashMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Stmt {
    match stmt {
        Stmt::Return(value) => Stmt::Return(
            value
                .as_ref()
                .map(|expr| expand_expr_type_aliases(expr, aliases, diagnostics)),
        ),
        Stmt::Let {
            name,
            ty,
            mutable,
            value,
        } => Stmt::Let {
            name: name.clone(),
            ty: ty
                .as_ref()
                .map(|ty| expand_type_alias(ty, aliases, &mut Vec::new(), diagnostics)),
            mutable: *mutable,
            value: expand_expr_type_aliases(value, aliases, diagnostics),
        },
        Stmt::Assign { name, op, value } => Stmt::Assign {
            name: name.clone(),
            op: *op,
            value: expand_expr_type_aliases(value, aliases, diagnostics),
        },
        Stmt::PointerAssign { pointer, op, value } => Stmt::PointerAssign {
            pointer: expand_expr_type_aliases(pointer, aliases, diagnostics),
            op: *op,
            value: expand_expr_type_aliases(value, aliases, diagnostics),
        },
        Stmt::PlaceAssign { target, op, value } => Stmt::PlaceAssign {
            target: expand_expr_type_aliases(target, aliases, diagnostics),
            op: *op,
            value: expand_expr_type_aliases(value, aliases, diagnostics),
        },
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => Stmt::If {
            condition: expand_expr_type_aliases(condition, aliases, diagnostics),
            then_body: then_body
                .iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
            else_body: else_body
                .iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
        },
        Stmt::While { condition, body } => Stmt::While {
            condition: expand_expr_type_aliases(condition, aliases, diagnostics),
            body: body
                .iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
        },
        Stmt::For {
            name,
            start,
            end,
            inclusive,
            body,
        } => Stmt::For {
            name: name.clone(),
            start: expand_expr_type_aliases(start, aliases, diagnostics),
            end: expand_expr_type_aliases(end, aliases, diagnostics),
            inclusive: *inclusive,
            body: body
                .iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
        },
        Stmt::Loop(body) => Stmt::Loop(
            body.iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
        ),
        Stmt::Unsafe(body) => Stmt::Unsafe(
            body.iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
        ),
        Stmt::Break => Stmt::Break,
        Stmt::Continue => Stmt::Continue,
        Stmt::Expr(expr) => Stmt::Expr(expand_expr_type_aliases(expr, aliases, diagnostics)),
    }
}

fn expand_expr_type_aliases(
    expr: &Expr,
    aliases: &HashMap<String, Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Expr {
    match expr {
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(expand_expr_type_aliases(expr, aliases, diagnostics)),
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(expand_expr_type_aliases(expr, aliases, diagnostics)),
            ty: expand_type_alias(ty, aliases, &mut Vec::new(), diagnostics),
        },
        Expr::SizeOf(ty) => {
            Expr::SizeOf(expand_type_alias(ty, aliases, &mut Vec::new(), diagnostics))
        }
        Expr::AlignOf(ty) => {
            Expr::AlignOf(expand_type_alias(ty, aliases, &mut Vec::new(), diagnostics))
        }
        Expr::OffsetOf { ty, field } => Expr::OffsetOf {
            ty: expand_type_alias(ty, aliases, &mut Vec::new(), diagnostics),
            field: field.clone(),
        },
        Expr::Match { value, arms } => Expr::Match {
            value: Box::new(expand_expr_type_aliases(value, aliases, diagnostics)),
            arms: arms
                .iter()
                .map(|arm| crate::ast::MatchArm {
                    pattern: arm.pattern.clone(),
                    value: expand_expr_type_aliases(&arm.value, aliases, diagnostics),
                })
                .collect(),
        },
        Expr::If {
            condition,
            then_value,
            else_value,
        } => Expr::If {
            condition: Box::new(expand_expr_type_aliases(condition, aliases, diagnostics)),
            then_value: Box::new(expand_expr_type_aliases(then_value, aliases, diagnostics)),
            else_value: Box::new(expand_expr_type_aliases(else_value, aliases, diagnostics)),
        },
        Expr::Block { statements, value } => Expr::Block {
            statements: statements
                .iter()
                .map(|stmt| expand_stmt_type_aliases(stmt, aliases, diagnostics))
                .collect(),
            value: Box::new(expand_expr_type_aliases(value, aliases, diagnostics)),
        },
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(expand_expr_type_aliases(left, aliases, diagnostics)),
            right: Box::new(expand_expr_type_aliases(right, aliases, diagnostics)),
        },
        Expr::Call { name, args } => Expr::Call {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| expand_expr_type_aliases(arg, aliases, diagnostics))
                .collect(),
        },
        Expr::Struct { name, fields } => Expr::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(name, expr)| {
                    (
                        name.clone(),
                        expand_expr_type_aliases(expr, aliases, diagnostics),
                    )
                })
                .collect(),
        },
        Expr::Array(values) => Expr::Array(
            values
                .iter()
                .map(|value| expand_expr_type_aliases(value, aliases, diagnostics))
                .collect(),
        ),
        Expr::Field { base, name } => Expr::Field {
            base: Box::new(expand_expr_type_aliases(base, aliases, diagnostics)),
            name: name.clone(),
        },
        Expr::Index { base, index } => Expr::Index {
            base: Box::new(expand_expr_type_aliases(base, aliases, diagnostics)),
            index: Box::new(expand_expr_type_aliases(index, aliases, diagnostics)),
        },
        Expr::Int(_)
        | Expr::TypedInt { .. }
        | Expr::Bool(_)
        | Expr::Char(_)
        | Expr::String(_)
        | Expr::Null
        | Expr::Var(_) => expr.clone(),
    }
}

fn expand_type_alias(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    visiting: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Type {
    match ty {
        Type::Named(name) if aliases.contains_key(name) => {
            if visiting.contains(name) {
                diagnostics.push(Diagnostic::error(format!(
                    "circular type alias involving '{name}'"
                )));
                return ty.clone();
            }
            visiting.push(name.clone());
            let expanded = expand_type_alias(
                aliases.get(name).expect("contains_key checked"),
                aliases,
                visiting,
                diagnostics,
            );
            visiting.pop();
            expanded
        }
        Type::Array(inner) => Type::Array(Box::new(expand_type_alias(
            inner,
            aliases,
            visiting,
            diagnostics,
        ))),
        Type::Slice(inner) => Type::Slice(Box::new(expand_type_alias(
            inner,
            aliases,
            visiting,
            diagnostics,
        ))),
        Type::Reference { mutable, inner } => Type::Reference {
            mutable: *mutable,
            inner: Box::new(expand_type_alias(inner, aliases, visiting, diagnostics)),
        },
        Type::Pointer(inner) => Type::Pointer(Box::new(expand_type_alias(
            inner,
            aliases,
            visiting,
            diagnostics,
        ))),
        _ => ty.clone(),
    }
}

fn check_function<'a>(
    function: &'a crate::ast::Function,
    consts: &'a [ConstDecl],
    functions: &HashMap<String, Callable>,
    structs: &HashMap<&'a str, &'a StructDecl>,
    enums: &HashMap<&'a str, &'a EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let diagnostic_start = diagnostics.len();
    let mut locals = const_locals(consts);
    for param in &function.params {
        if locals
            .insert(
                param.name.as_str(),
                Local {
                    ty: param.ty.clone(),
                    mutable: true,
                },
            )
            .is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate local '{}'",
                param.name
            )));
        }
    }
    for (index, statement) in function.body.iter().enumerate() {
        let statement_diagnostic_start = diagnostics.len();
        check_stmts(
            std::slice::from_ref(statement),
            &function.return_type,
            &mut locals,
            functions,
            structs,
            enums,
            diagnostics,
            0,
            0,
            index + 1 == function.body.len(),
        );
        let expression_span = function
            .statement_expression_ranges
            .get(index)
            .and_then(|(_, end)| end.checked_sub(1))
            .and_then(|expression| function.expression_spans.get(expression))
            .or_else(|| function.statement_spans.get(index));
        if let Some(span) = expression_span {
            for diagnostic in diagnostics.iter_mut().skip(statement_diagnostic_start) {
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

fn const_locals<'a>(consts: &'a [ConstDecl]) -> HashMap<&'a str, Local> {
    consts
        .iter()
        .map(|decl| {
            (
                decl.name.as_str(),
                Local {
                    ty: decl.ty.clone(),
                    mutable: false,
                },
            )
        })
        .collect()
}

fn detect_const_cycles(consts: &[ConstDecl], diagnostics: &mut Vec<Diagnostic>) {
    let values: HashMap<&str, &Expr> = consts
        .iter()
        .map(|decl| (decl.name.as_str(), &decl.value))
        .collect();
    let mut states = HashMap::new();
    let mut reported = HashSet::new();

    for decl in consts {
        visit_const(
            decl.name.as_str(),
            &values,
            &mut states,
            &mut Vec::new(),
            &mut reported,
            diagnostics,
        );
    }
}

fn visit_const<'a>(
    name: &'a str,
    values: &HashMap<&'a str, &'a Expr>,
    states: &mut HashMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
    reported: &mut HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match states.get(name).copied() {
        Some(2) => return,
        Some(1) => {
            report_const_cycle(name, reported, diagnostics);
            return;
        }
        _ => {}
    }

    let Some(value) = values.get(name) else {
        return;
    };
    states.insert(name, 1);
    stack.push(name);
    for dependency in const_dependencies(value, values) {
        if stack.contains(&dependency) {
            report_const_cycle(dependency, reported, diagnostics);
        } else {
            visit_const(dependency, values, states, stack, reported, diagnostics);
        }
    }
    stack.pop();
    states.insert(name, 2);
}

fn report_const_cycle<'a>(
    name: &'a str,
    reported: &mut HashSet<&'a str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if reported.insert(name) {
        diagnostics.push(Diagnostic::error(format!(
            "circular constant dependency involving '{name}'"
        )));
    }
}

fn const_dependencies<'a>(expr: &'a Expr, values: &HashMap<&'a str, &'a Expr>) -> Vec<&'a str> {
    let mut dependencies = Vec::new();
    collect_const_dependencies(expr, values, &mut dependencies);
    dependencies
}

fn collect_const_dependencies<'a>(
    expr: &'a Expr,
    values: &HashMap<&'a str, &'a Expr>,
    dependencies: &mut Vec<&'a str>,
) {
    match expr {
        Expr::Var(name) => {
            if let Some((dependency, _)) = values.get_key_value(name.as_str()) {
                dependencies.push(*dependency);
            }
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_const_dependencies(expr, values, dependencies);
        }
        Expr::Binary { left, right, .. } => {
            collect_const_dependencies(left, values, dependencies);
            collect_const_dependencies(right, values, dependencies);
        }
        Expr::Call { args, .. } | Expr::Array(args) => {
            for arg in args {
                collect_const_dependencies(arg, values, dependencies);
            }
        }
        Expr::Struct { fields, .. } => {
            for (_, value) in fields {
                collect_const_dependencies(value, values, dependencies);
            }
        }
        Expr::Field { base, .. } => collect_const_dependencies(base, values, dependencies),
        Expr::Index { base, index } => {
            collect_const_dependencies(base, values, dependencies);
            collect_const_dependencies(index, values, dependencies);
        }
        Expr::Match { value, arms } => {
            collect_const_dependencies(value, values, dependencies);
            for arm in arms {
                collect_const_dependencies(&arm.value, values, dependencies);
            }
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            collect_const_dependencies(condition, values, dependencies);
            collect_const_dependencies(then_value, values, dependencies);
            collect_const_dependencies(else_value, values, dependencies);
        }
        Expr::Block { statements, value } => {
            for statement in statements {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
            collect_const_dependencies(value, values, dependencies);
        }
        Expr::Int(_)
        | Expr::TypedInt { .. }
        | Expr::Bool(_)
        | Expr::Char(_)
        | Expr::String(_)
        | Expr::Null
        | Expr::SizeOf(_)
        | Expr::AlignOf(_)
        | Expr::OffsetOf { .. } => {}
    }
}

fn collect_stmt_const_dependencies<'a>(
    statement: &'a Stmt,
    values: &HashMap<&'a str, &'a Expr>,
    dependencies: &mut Vec<&'a str>,
) {
    match statement {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::PointerAssign { value, .. }
        | Stmt::PlaceAssign { value, .. }
        | Stmt::Return(Some(value))
        | Stmt::Expr(value) => collect_const_dependencies(value, values, dependencies),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            collect_const_dependencies(condition, values, dependencies);
            for statement in then_body {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
            for statement in else_body {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
        }
        Stmt::While { condition, body } => {
            collect_const_dependencies(condition, values, dependencies);
            for statement in body {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
        }
        Stmt::For {
            start, end, body, ..
        } => {
            collect_const_dependencies(start, values, dependencies);
            collect_const_dependencies(end, values, dependencies);
            for statement in body {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
        }
        Stmt::Unsafe(body) | Stmt::Loop(body) => {
            for statement in body {
                collect_stmt_const_dependencies(statement, values, dependencies);
            }
        }
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
    }
}

fn check_stmts<'a>(
    stmts: &'a [Stmt],
    return_type: &Type,
    locals: &mut HashMap<&'a str, Local>,
    functions: &HashMap<String, Callable>,
    structs: &HashMap<&'a str, &'a StructDecl>,
    enums: &HashMap<&'a str, &'a EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    loop_depth: usize,
    unsafe_depth: usize,
    allow_tail_return: bool,
) {
    for (index, stmt) in stmts.iter().enumerate() {
        let is_tail_stmt = allow_tail_return && index + 1 == stmts.len();
        match stmt {
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    let actual = expr_type(
                        expr,
                        Some(return_type),
                        locals,
                        functions,
                        structs,
                        enums,
                        diagnostics,
                        unsafe_depth,
                    );
                    if !type_matches_expr(return_type, actual.as_ref(), expr) {
                        diagnostics.push(Diagnostic::error("return type mismatch"));
                    }
                } else if return_type != &Type::Unit {
                    diagnostics.push(Diagnostic::error(
                        "return statement requires a value for non-unit function",
                    ));
                }
            }
            Stmt::Let {
                name,
                ty,
                mutable,
                value,
            } => {
                if let Some(ty) = ty {
                    validate_type(ty, structs, enums, diagnostics);
                }
                let actual = expr_type(
                    value,
                    ty.as_ref(),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let Some(local_ty) = ty.clone().or(actual.clone()) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot infer type for local '{name}'"
                    )));
                    continue;
                };
                if let Some(ty) = ty {
                    if !type_matches_expr(ty, actual.as_ref(), value) {
                        diagnostics.push(Diagnostic::error("let initializer type mismatch"));
                    }
                }
                if locals
                    .insert(
                        name.as_str(),
                        Local {
                            ty: local_ty,
                            mutable: *mutable,
                        },
                    )
                    .is_some()
                {
                    diagnostics.push(Diagnostic::error(format!("duplicate local '{name}'")));
                }
            }
            Stmt::Assign { name, op, value } => {
                let expected = locals.get(name.as_str()).cloned();
                let actual = expr_type(
                    value,
                    if op.is_some() {
                        None
                    } else {
                        expected.as_ref().map(|local| &local.ty)
                    },
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if expected.is_none() {
                    diagnostics.push(Diagnostic::error(format!("unknown variable '{name}'")));
                } else if !expected.as_ref().expect("checked above").mutable {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot assign to immutable local '{name}'"
                    )));
                } else if let Some(op) = op {
                    let expected_ty = expected.as_ref().expect("checked above").ty.clone();
                    let result = if let Some(pointer_result) = pointer_binary_expr_type(
                        *op,
                        Some(&expected_ty),
                        actual.as_ref(),
                        diagnostics,
                    ) {
                        if unsafe_depth == 0 {
                            diagnostics
                                .push(Diagnostic::error("raw pointer arithmetic requires unsafe"));
                        }
                        pointer_result
                    } else {
                        binary_expr_type(*op, Some(expected_ty.clone()), actual, diagnostics)
                    };
                    if result.is_some() && result != Some(expected_ty) {
                        diagnostics.push(Diagnostic::error("assignment type mismatch"));
                    }
                } else if !type_matches_expr(
                    &expected.as_ref().expect("checked above").ty,
                    actual.as_ref(),
                    value,
                ) {
                    diagnostics.push(Diagnostic::error("assignment type mismatch"));
                }
            }
            Stmt::PointerAssign { pointer, op, value } => {
                let pointer_ty = expr_type(
                    pointer,
                    None,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let inner = match pointer_ty {
                    Some(Type::Pointer(inner)) => {
                        if unsafe_depth == 0 {
                            diagnostics
                                .push(Diagnostic::error("pointer assignment requires unsafe"));
                        }
                        inner
                    }
                    Some(Type::Reference {
                        mutable: true,
                        inner,
                    }) => inner,
                    Some(Type::Reference { mutable: false, .. }) => {
                        diagnostics.push(Diagnostic::error(
                            "pointer assignment target must be a mutable reference",
                        ));
                        continue;
                    }
                    _ => {
                        diagnostics.push(Diagnostic::error(
                            "pointer assignment target must be a raw pointer or mutable reference",
                        ));
                        continue;
                    }
                };
                let actual = expr_type(
                    value,
                    Some(&inner),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if let Some(op) = op {
                    let result = binary_expr_type(*op, Some((*inner).clone()), actual, diagnostics);
                    if result.is_some() && result != Some((*inner).clone()) {
                        diagnostics.push(Diagnostic::error("pointer assignment type mismatch"));
                    }
                } else if !type_matches_expr(&inner, actual.as_ref(), value) {
                    diagnostics.push(Diagnostic::error("pointer assignment type mismatch"));
                }
            }
            Stmt::PlaceAssign { target, op, value } => {
                let Some(root) = assigned_local(target) else {
                    diagnostics.push(Diagnostic::error(
                        "assignment target must be a local field or index place",
                    ));
                    continue;
                };
                if locals
                    .get(root.as_str())
                    .is_some_and(|local| !local.mutable)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot assign to immutable local '{root}'"
                    )));
                }
                let target_ty = expr_type(
                    target,
                    None,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let actual = expr_type(
                    value,
                    if op.is_some() {
                        None
                    } else {
                        target_ty.as_ref()
                    },
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if let Some(op) = op {
                    let result = binary_expr_type(*op, target_ty.clone(), actual, diagnostics);
                    if result.is_some() && result != target_ty {
                        diagnostics.push(Diagnostic::error("place assignment type mismatch"));
                    }
                } else if let Some(target_ty) = target_ty {
                    if !type_matches_expr(&target_ty, actual.as_ref(), value) {
                        diagnostics.push(Diagnostic::error("place assignment type mismatch"));
                    }
                }
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                if expr_type(
                    condition,
                    Some(&Type::Bool),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                ) != Some(Type::Bool)
                {
                    diagnostics.push(Diagnostic::error("if condition must be bool"));
                }
                check_stmts(
                    then_body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth,
                    unsafe_depth,
                    false,
                );
                check_stmts(
                    else_body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth,
                    unsafe_depth,
                    false,
                );
            }
            Stmt::While { condition, body } => {
                if expr_type(
                    condition,
                    Some(&Type::Bool),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                ) != Some(Type::Bool)
                {
                    diagnostics.push(Diagnostic::error("while condition must be bool"));
                }
                check_stmts(
                    body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth + 1,
                    unsafe_depth,
                    false,
                );
            }
            Stmt::For {
                name,
                start,
                end,
                inclusive: _,
                body,
            } => {
                let start_ty = expr_type(
                    start,
                    None,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let end_ty = expr_type(
                    end,
                    start_ty.as_ref(),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if !(start_ty.as_ref().is_some_and(is_integer_type) && start_ty == end_ty) {
                    diagnostics.push(Diagnostic::error(
                        "for range bounds must be matching integer types",
                    ));
                }
                let loop_ty = start_ty.filter(is_integer_type).unwrap_or(Type::Int);
                let previous = locals.insert(
                    name.as_str(),
                    Local {
                        ty: loop_ty,
                        mutable: false,
                    },
                );
                check_stmts(
                    body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth + 1,
                    unsafe_depth,
                    false,
                );
                if let Some(previous) = previous {
                    locals.insert(name.as_str(), previous);
                } else {
                    locals.remove(name.as_str());
                }
            }
            Stmt::Loop(body) => {
                check_stmts(
                    body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth + 1,
                    unsafe_depth,
                    false,
                );
            }
            Stmt::Break => {
                if loop_depth == 0 {
                    diagnostics.push(Diagnostic::error("break outside loop"));
                }
            }
            Stmt::Continue => {
                if loop_depth == 0 {
                    diagnostics.push(Diagnostic::error("continue outside loop"));
                }
            }
            Stmt::Unsafe(body) => {
                check_stmts(
                    body,
                    return_type,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    loop_depth,
                    unsafe_depth + 1,
                    false,
                );
            }
            Stmt::Expr(expr) => {
                let expected = if is_tail_stmt && return_type != &Type::Unit {
                    Some(return_type)
                } else {
                    None
                };
                let actual = expr_type(
                    expr,
                    expected,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if let Some(expected) = expected {
                    if !type_matches_expr(expected, actual.as_ref(), expr) {
                        diagnostics.push(Diagnostic::error("tail expression type mismatch"));
                    }
                }
            }
        }
    }
}

fn expr_type<'a>(
    expr: &'a Expr,
    expected: Option<&Type>,
    locals: &HashMap<&'a str, Local>,
    functions: &HashMap<String, Callable>,
    structs: &HashMap<&'a str, &'a StructDecl>,
    enums: &HashMap<&'a str, &'a EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    unsafe_depth: usize,
) -> Option<Type> {
    if let Some(ty) = typed_integer_literal_type(expr, expected, diagnostics) {
        return Some(ty);
    }

    match expr {
        Expr::Int(_) => Some(Type::Int),
        Expr::TypedInt { value, ty } => {
            if !integer_value_fits_type(*value, ty) {
                diagnostics.push(Diagnostic::error(format!(
                    "integer literal {value} does not fit in type {}",
                    type_name(ty)
                )));
            }
            Some(ty.clone())
        }
        Expr::Bool(_) => Some(Type::Bool),
        Expr::Char(_) => Some(Type::Char),
        Expr::String(_) => Some(Type::String),
        Expr::Null => match expected {
            Some(Type::Pointer(inner)) => Some(Type::Pointer(inner.clone())),
            _ => {
                diagnostics.push(Diagnostic::error("null requires raw pointer type context"));
                None
            }
        },
        Expr::Var(name) => locals
            .get(name.as_str())
            .map(|local| local.ty.clone())
            .or_else(|| {
                diagnostics.push(Diagnostic::error(format!("unknown variable '{name}'")));
                None
            }),
        Expr::Unary { op, expr } => {
            let ty = expr_type(
                expr,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            match op {
                UnaryOp::Neg => {
                    if ty.as_ref().is_some_and(is_integer_type) {
                        ty
                    } else {
                        diagnostics.push(Diagnostic::error("unary '-' operand must be an integer"));
                        None
                    }
                }
                UnaryOp::Not => {
                    if ty == Some(Type::Bool) {
                        Some(Type::Bool)
                    } else {
                        diagnostics.push(Diagnostic::error("unary '!' operand must be bool"));
                        None
                    }
                }
                UnaryOp::BitNot => {
                    if ty.as_ref().is_some_and(is_integer_type) {
                        ty
                    } else {
                        diagnostics
                            .push(Diagnostic::error("bitwise NOT operand must be an integer"));
                        None
                    }
                }
                UnaryOp::AddressOf | UnaryOp::MutableAddressOf => {
                    let mutable = matches!(op, UnaryOp::MutableAddressOf);
                    if matches!(expected, Some(Type::Pointer(_))) && unsafe_depth == 0 {
                        diagnostics.push(Diagnostic::error(
                            "address-of raw pointer operation requires unsafe",
                        ));
                    }
                    ty.map(|ty| {
                        if matches!(expected, Some(Type::Pointer(_))) {
                            Type::Pointer(Box::new(ty))
                        } else {
                            Type::Reference {
                                mutable,
                                inner: Box::new(ty),
                            }
                        }
                    })
                }
                UnaryOp::Deref => match ty {
                    Some(Type::Pointer(inner)) => Some(*inner),
                    Some(Type::Reference { inner, .. }) => Some(*inner),
                    _ => {
                        diagnostics.push(Diagnostic::error(
                            "dereference operand must be a pointer or reference",
                        ));
                        None
                    }
                },
            }
        }
        Expr::Cast { expr, ty } => {
            validate_type(ty, structs, enums, diagnostics);
            let source_ty = expr_type(
                expr,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            match (source_ty.as_ref(), ty) {
                (Some(source), target) if is_integer_type(source) && is_integer_type(target) => {
                    Some(target.clone())
                }
                (Some(Type::Pointer(_)), target) if is_integer_type(target) => Some(target.clone()),
                (Some(source), Type::Pointer(_)) if is_integer_type(source) => {
                    if unsafe_depth == 0 {
                        diagnostics
                            .push(Diagnostic::error("integer to pointer cast requires unsafe"));
                        None
                    } else {
                        Some(ty.clone())
                    }
                }
                (Some(Type::Pointer(_)), Type::Pointer(_)) => Some(ty.clone()),
                _ => {
                    diagnostics.push(Diagnostic::error(
                        "casts require integer or raw pointer source and target types",
                    ));
                    None
                }
            }
        }
        Expr::SizeOf(ty) => {
            validate_type(ty, structs, enums, diagnostics);
            Some(Type::Usize)
        }
        Expr::AlignOf(ty) => {
            validate_type(ty, structs, enums, diagnostics);
            Some(Type::Usize)
        }
        Expr::OffsetOf { ty, field } => {
            validate_type(ty, structs, enums, diagnostics);
            match ty {
                Type::Named(name) => match structs.get(name.as_str()) {
                    Some(struct_decl) => {
                        if !struct_decl.fields.iter().any(|decl| decl.name == *field) {
                            diagnostics.push(Diagnostic::error(format!(
                                "unknown field '{field}' on struct '{name}'"
                            )));
                        }
                    }
                    None => diagnostics.push(Diagnostic::error("offsetof requires a struct type")),
                },
                _ => diagnostics.push(Diagnostic::error("offsetof requires a struct type")),
            }
            Some(Type::Usize)
        }
        Expr::Binary { op, left, right } => {
            let (left_ty, right_ty) = if matches!(left.as_ref(), Expr::Null)
                && matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
            {
                let right_ty = expr_type(
                    right,
                    None,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let left_ty = expr_type(
                    left,
                    right_ty.as_ref(),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                (left_ty, right_ty)
            } else {
                let left_ty = expr_type(
                    left,
                    None,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                let right_ty = expr_type(
                    right,
                    left_ty.as_ref(),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                (left_ty, right_ty)
            };
            if let Some(pointer_result) =
                pointer_binary_expr_type(*op, left_ty.as_ref(), right_ty.as_ref(), diagnostics)
            {
                if unsafe_depth == 0 {
                    let message = if matches!(
                        op,
                        BinaryOp::Less
                            | BinaryOp::LessEqual
                            | BinaryOp::Greater
                            | BinaryOp::GreaterEqual
                    ) {
                        "raw pointer comparison requires unsafe"
                    } else {
                        "raw pointer arithmetic requires unsafe"
                    };
                    diagnostics.push(Diagnostic::error(message));
                }
                pointer_result
            } else {
                binary_expr_type(*op, left_ty, right_ty, diagnostics)
            }
        }
        Expr::Call { name, args } => {
            let Some(function) = functions.get(name) else {
                diagnostics.push(Diagnostic::error(format!("unknown function '{name}'")));
                return None;
            };
            if args.len() != function.params.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "function '{name}' expected {} arguments but got {}",
                    function.params.len(),
                    args.len()
                )));
                return None;
            }
            for (arg, param) in args.iter().zip(function.params.iter()) {
                let arg_ty = expr_type(
                    arg,
                    Some(&param.ty),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if !type_matches_expr(&param.ty, arg_ty.as_ref(), arg) {
                    diagnostics.push(Diagnostic::error(format!(
                        "argument '{}' type mismatch",
                        param.name
                    )));
                }
            }
            Some(function.return_type.clone())
        }
        Expr::Struct { name, fields } => {
            let Some(struct_decl) = structs.get(name.as_str()) else {
                diagnostics.push(Diagnostic::error(format!("unknown struct '{name}'")));
                return None;
            };
            let mut seen = HashMap::new();
            for (field_name, field_expr) in fields {
                if seen.insert(field_name.as_str(), ()).is_some() {
                    diagnostics.push(Diagnostic::error(format!(
                        "duplicate field '{field_name}' in struct literal"
                    )));
                    continue;
                }
                let Some(field) = struct_decl
                    .fields
                    .iter()
                    .find(|field| field.name == *field_name)
                else {
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown field '{field_name}' on struct '{name}'"
                    )));
                    continue;
                };
                let actual = expr_type(
                    field_expr,
                    Some(&field.ty),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if !type_matches_expr(&field.ty, actual.as_ref(), field_expr) {
                    diagnostics.push(Diagnostic::error(format!(
                        "field '{field_name}' type mismatch"
                    )));
                }
            }
            for field in &struct_decl.fields {
                if !seen.contains_key(field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "missing field '{}' in struct literal '{}'",
                        field.name, name
                    )));
                }
            }
            Some(Type::Named(name.clone()))
        }
        Expr::Array(values) => {
            let expected_inner = match expected {
                Some(Type::Array(inner)) | Some(Type::Slice(inner)) => Some(inner.as_ref()),
                _ => None,
            };
            if values.is_empty() {
                return expected_inner.map(|inner| Type::Array(Box::new(inner.clone())));
            }

            let mut element_ty = None;
            for value in values {
                let actual = expr_type(
                    value,
                    expected_inner,
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if let Some(expected_inner) = expected_inner {
                    if !type_matches_expr(expected_inner, actual.as_ref(), value) {
                        diagnostics.push(Diagnostic::error("array element type mismatch"));
                    }
                    element_ty = Some(expected_inner.clone());
                } else if element_ty.is_none() {
                    element_ty = actual;
                } else if !type_matches_expr(
                    element_ty.as_ref().expect("set"),
                    actual.as_ref(),
                    value,
                ) {
                    diagnostics.push(Diagnostic::error(
                        "array literal elements must have one type",
                    ));
                }
            }
            element_ty.map(|ty| Type::Array(Box::new(ty)))
        }
        Expr::Match { value, arms } => {
            let value_ty = expr_type(
                value,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            if arms.is_empty() {
                diagnostics.push(Diagnostic::error(
                    "match expression requires at least one arm",
                ));
                return None;
            }
            check_unreachable_match_arms(arms, diagnostics);
            check_match_exhaustive(value_ty.as_ref(), arms, enums, diagnostics);

            let mut result_ty = None;
            for arm in arms {
                check_match_pattern(&arm.pattern, value_ty.as_ref(), enums, diagnostics);
                let arm_ty = expr_type(
                    &arm.value,
                    result_ty.as_ref().or(expected),
                    locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
                if let Some(existing) = result_ty.as_ref() {
                    if !type_matches_expr(existing, arm_ty.as_ref(), &arm.value) {
                        diagnostics.push(Diagnostic::error("match arm type mismatch"));
                    }
                } else {
                    result_ty = arm_ty;
                }
            }
            result_ty
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            let condition_ty = expr_type(
                condition,
                Some(&Type::Bool),
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            if condition_ty != Some(Type::Bool) {
                diagnostics.push(Diagnostic::error("if expression condition must be bool"));
            }
            let then_ty = expr_type(
                then_value,
                expected,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            let else_ty = expr_type(
                else_value,
                then_ty.as_ref().or(expected),
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            if !type_matches_expr(
                then_ty.as_ref().unwrap_or(&Type::Unit),
                else_ty.as_ref(),
                else_value,
            ) {
                diagnostics.push(Diagnostic::error("if expression branch type mismatch"));
                return None;
            }
            then_ty
        }
        Expr::Block { statements, value } => {
            let mut block_locals = locals.clone();
            for stmt in statements {
                check_block_expr_stmt(
                    stmt,
                    &mut block_locals,
                    functions,
                    structs,
                    enums,
                    diagnostics,
                    unsafe_depth,
                );
            }
            expr_type(
                value,
                expected,
                &block_locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            )
        }
        Expr::Field { base, name } => {
            if let Expr::Var(enum_name) = base.as_ref() {
                if let Some(enum_decl) = enums.get(enum_name.as_str()) {
                    if enum_decl
                        .variants
                        .iter()
                        .any(|variant| variant.name == *name)
                    {
                        return Some(Type::Named(enum_name.clone()));
                    }
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown variant '{name}' on enum '{enum_name}'"
                    )));
                    return None;
                }
            }
            let base_ty = expr_type(
                base,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            let Some(Type::Named(struct_name)) = base_ty else {
                diagnostics.push(Diagnostic::error("field access requires a struct value"));
                return None;
            };
            let Some(struct_decl) = structs.get(struct_name.as_str()) else {
                diagnostics.push(Diagnostic::error(format!("unknown struct '{struct_name}'")));
                return None;
            };
            struct_decl
                .fields
                .iter()
                .find(|field| field.name == *name)
                .map(|field| field.ty.clone())
                .or_else(|| {
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown field '{name}' on struct '{struct_name}'"
                    )));
                    None
                })
        }
        Expr::Index { base, index } => {
            let base_ty = expr_type(
                base,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            let index_ty = expr_type(
                index,
                Some(&Type::Usize),
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            if !index_ty.as_ref().is_some_and(is_integer_type) {
                diagnostics.push(Diagnostic::error("index expression must be an integer"));
            }
            match base_ty {
                Some(Type::Array(inner)) | Some(Type::Slice(inner)) => Some(*inner),
                Some(Type::String) => Some(Type::Char),
                _ => {
                    diagnostics.push(Diagnostic::error(
                        "indexing requires an array, slice, or string",
                    ));
                    None
                }
            }
        }
    }
}

fn check_block_expr_stmt<'a>(
    stmt: &'a Stmt,
    locals: &mut HashMap<&'a str, Local>,
    functions: &HashMap<String, Callable>,
    structs: &HashMap<&'a str, &'a StructDecl>,
    enums: &HashMap<&'a str, &'a EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    unsafe_depth: usize,
) {
    match stmt {
        Stmt::Let {
            name,
            ty,
            mutable,
            value,
        } => {
            if let Some(ty) = ty {
                validate_type(ty, structs, enums, diagnostics);
            }
            let actual = expr_type(
                value,
                ty.as_ref(),
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            let Some(local_ty) = ty.clone().or(actual.clone()) else {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot infer type for local '{name}'"
                )));
                return;
            };
            if let Some(ty) = ty {
                if !type_matches_expr(ty, actual.as_ref(), value) {
                    diagnostics.push(Diagnostic::error("let initializer type mismatch"));
                }
            }
            if locals
                .insert(
                    name.as_str(),
                    Local {
                        ty: local_ty,
                        mutable: *mutable,
                    },
                )
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!("duplicate local '{name}'")));
            }
        }
        Stmt::Assign { name, op, value } => {
            let expected = locals.get(name.as_str()).cloned();
            let actual = expr_type(
                value,
                if op.is_some() {
                    None
                } else {
                    expected.as_ref().map(|local| &local.ty)
                },
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
            if expected.is_none() {
                diagnostics.push(Diagnostic::error(format!("unknown variable '{name}'")));
            } else if !expected.as_ref().expect("checked above").mutable {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot assign to immutable local '{name}'"
                )));
            } else if let Some(op) = op {
                let expected_ty = expected.as_ref().expect("checked above").ty.clone();
                let result = binary_expr_type(*op, Some(expected_ty.clone()), actual, diagnostics);
                if result.is_some() && result != Some(expected_ty) {
                    diagnostics.push(Diagnostic::error("assignment type mismatch"));
                }
            } else if !type_matches_expr(
                &expected.as_ref().expect("checked above").ty,
                actual.as_ref(),
                value,
            ) {
                diagnostics.push(Diagnostic::error("assignment type mismatch"));
            }
        }
        Stmt::Expr(expr) => {
            expr_type(
                expr,
                None,
                locals,
                functions,
                structs,
                enums,
                diagnostics,
                unsafe_depth,
            );
        }
        _ => diagnostics.push(Diagnostic::error(
            "unsupported statement in block expression",
        )),
    }
}

fn check_unreachable_match_arms(arms: &[crate::ast::MatchArm], diagnostics: &mut Vec<Diagnostic>) {
    let mut wildcard_seen = false;
    let mut seen_ints = Vec::new();
    let mut seen_bools = Vec::new();
    let mut seen_enum_variants = Vec::new();

    for arm in arms {
        if wildcard_seen {
            diagnostics.push(Diagnostic::error("unreachable match arm"));
            continue;
        }

        match &arm.pattern {
            MatchPattern::Wildcard => {
                wildcard_seen = true;
            }
            MatchPattern::Int(value) => {
                if seen_ints.contains(value) {
                    diagnostics.push(Diagnostic::error("unreachable match arm"));
                } else {
                    seen_ints.push(*value);
                }
            }
            MatchPattern::Bool(value) => {
                if seen_bools.contains(value) {
                    diagnostics.push(Diagnostic::error("unreachable match arm"));
                } else {
                    seen_bools.push(*value);
                }
            }
            MatchPattern::EnumVariant { enum_name, variant } => {
                let key = (enum_name.as_str(), variant.as_str());
                if seen_enum_variants.contains(&key) {
                    diagnostics.push(Diagnostic::error("unreachable match arm"));
                } else {
                    seen_enum_variants.push(key);
                }
            }
        }
    }
}

fn check_match_exhaustive(
    value_ty: Option<&Type>,
    arms: &[crate::ast::MatchArm],
    enums: &HashMap<&str, &EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if arms
        .iter()
        .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard))
    {
        return;
    }

    match value_ty {
        Some(Type::Bool) => {
            let has_true = arms
                .iter()
                .any(|arm| matches!(arm.pattern, MatchPattern::Bool(true)));
            let has_false = arms
                .iter()
                .any(|arm| matches!(arm.pattern, MatchPattern::Bool(false)));
            if !(has_true && has_false) {
                diagnostics.push(Diagnostic::error(
                    "non-exhaustive match expression; add missing bool arms or '_'",
                ));
            }
        }
        Some(Type::Named(enum_name)) => {
            let Some(enum_decl) = enums.get(enum_name.as_str()) else {
                return;
            };
            for variant in &enum_decl.variants {
                let covered = arms.iter().any(|arm| {
                    matches!(
                        &arm.pattern,
                        MatchPattern::EnumVariant {
                            enum_name: pattern_enum,
                            variant: pattern_variant,
                        } if pattern_enum == enum_name && pattern_variant == &variant.name
                    )
                });
                if !covered {
                    diagnostics.push(Diagnostic::error(format!(
                        "non-exhaustive match expression; missing '{enum_name}.{}' or '_'",
                        variant.name
                    )));
                    return;
                }
            }
        }
        Some(ty) if is_integer_type(ty) => {
            diagnostics.push(Diagnostic::error(
                "non-exhaustive match expression; integer matches require '_'",
            ));
        }
        _ => {}
    }
}

fn check_match_pattern(
    pattern: &MatchPattern,
    value_ty: Option<&Type>,
    enums: &HashMap<&str, &EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match pattern {
        MatchPattern::Wildcard => {}
        MatchPattern::Int(_) => {
            if !value_ty.is_some_and(is_integer_type) {
                diagnostics.push(Diagnostic::error(
                    "integer match pattern requires an integer scrutinee",
                ));
            }
        }
        MatchPattern::Bool(_) => {
            if value_ty != Some(&Type::Bool) {
                diagnostics.push(Diagnostic::error(
                    "bool match pattern requires a bool scrutinee",
                ));
            }
        }
        MatchPattern::EnumVariant { enum_name, variant } => {
            let Some(enum_decl) = enums.get(enum_name.as_str()) else {
                diagnostics.push(Diagnostic::error(format!("unknown enum '{enum_name}'")));
                return;
            };
            if !enum_decl
                .variants
                .iter()
                .any(|candidate| candidate.name == *variant)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "unknown variant '{variant}' on enum '{enum_name}'"
                )));
                return;
            }
            if value_ty != Some(&Type::Named(enum_name.clone())) {
                diagnostics.push(Diagnostic::error("enum match pattern type mismatch"));
            }
        }
    }
}

fn binary_expr_type(
    op: BinaryOp,
    left_ty: Option<Type>,
    right_ty: Option<Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Type> {
    match op {
        BinaryOp::And | BinaryOp::Or => {
            if left_ty == Some(Type::Bool) && right_ty == Some(Type::Bool) {
                Some(Type::Bool)
            } else {
                diagnostics.push(Diagnostic::error("logical operands must be bool"));
                None
            }
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
            if left_ty.as_ref().is_some_and(is_integer_type) && left_ty == right_ty {
                left_ty
            } else {
                diagnostics.push(Diagnostic::error(
                    "bitwise operands must be matching integer types",
                ));
                None
            }
        }
        BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
            if left_ty.as_ref().is_some_and(is_integer_type) && left_ty == right_ty {
                left_ty
            } else {
                diagnostics.push(Diagnostic::error(
                    "shift operands must be matching integer types",
                ));
                None
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            if op == BinaryOp::Add
                && left_ty == Some(Type::String)
                && right_ty == Some(Type::String)
            {
                Some(Type::String)
            } else if left_ty.as_ref().is_some_and(is_integer_type) && left_ty == right_ty {
                left_ty
            } else {
                diagnostics.push(Diagnostic::error(
                    "arithmetic operands must be matching integer types",
                ));
                None
            }
        }
        BinaryOp::Equal | BinaryOp::NotEqual => {
            if left_ty.is_some() && left_ty == right_ty {
                Some(Type::Bool)
            } else {
                diagnostics.push(Diagnostic::error("comparison operands must match"));
                None
            }
        }
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
            if left_ty.as_ref().is_some_and(is_integer_type) && left_ty == right_ty {
                Some(Type::Bool)
            } else {
                diagnostics.push(Diagnostic::error(
                    "comparison operands must be matching integer types",
                ));
                None
            }
        }
    }
}

fn pointer_binary_expr_type(
    op: BinaryOp,
    left_ty: Option<&Type>,
    right_ty: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Option<Type>> {
    match (op, left_ty, right_ty) {
        (BinaryOp::Add | BinaryOp::Sub, Some(Type::Pointer(inner)), Some(right))
            if is_integer_type(right) =>
        {
            Some(Some(Type::Pointer(inner.clone())))
        }
        (BinaryOp::Sub, Some(left), Some(right))
            if matches!(left, Type::Pointer(_)) && left == right =>
        {
            Some(Some(Type::Int))
        }
        (
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual,
            Some(left),
            Some(right),
        ) if matches!(left, Type::Pointer(_)) && left == right => Some(Some(Type::Bool)),
        (
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual,
            Some(Type::Pointer(_)),
            Some(Type::Pointer(_)),
        ) => {
            diagnostics.push(Diagnostic::error(
                "raw pointer comparison requires matching pointer types",
            ));
            Some(None)
        }
        (BinaryOp::Add, Some(Type::Pointer(_)), Some(Type::Pointer(_))) => {
            diagnostics.push(Diagnostic::error("cannot add two raw pointers"));
            Some(None)
        }
        (BinaryOp::Sub, Some(Type::Pointer(_)), Some(Type::Pointer(_))) => {
            diagnostics.push(Diagnostic::error(
                "raw pointer difference requires matching pointer types",
            ));
            Some(None)
        }
        (BinaryOp::Add | BinaryOp::Sub, Some(Type::Pointer(_)), Some(_)) => {
            diagnostics.push(Diagnostic::error(
                "raw pointer arithmetic requires an integer offset",
            ));
            Some(None)
        }
        _ => None,
    }
}

fn validate_type(
    ty: &Type,
    structs: &HashMap<&str, &StructDecl>,
    enums: &HashMap<&str, &EnumDecl>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match ty {
        Type::Array(inner) | Type::Slice(inner) | Type::Pointer(inner) => {
            validate_type(inner, structs, enums, diagnostics)
        }
        Type::Reference { inner, .. } => validate_type(inner, structs, enums, diagnostics),
        Type::Named(name) => {
            if !structs.contains_key(name.as_str()) && !enums.contains_key(name.as_str()) {
                diagnostics.push(Diagnostic::error(format!("unknown type '{name}'")));
            }
        }
        Type::Int
        | Type::Unit
        | Type::Bool
        | Type::Char
        | Type::String
        | Type::Usize
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64 => {}
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

fn type_matches_expr(expected: &Type, actual: Option<&Type>, expr: &Expr) -> bool {
    actual == Some(expected)
        || matches!((expected, actual), (Type::Slice(expected_inner), Some(Type::Array(actual_inner))) if expected_inner == actual_inner)
        || (matches!(expected, Type::Pointer(_)) && matches!(expr, Expr::Null))
        || (matches!(expected, Type::Pointer(_)) && matches!(expr, Expr::Int(0)))
        || (actual == Some(&Type::Int)
            && is_integer_type(expected)
            && integer_literal_value(expr)
                .is_some_and(|value| integer_value_fits_type(value, expected)))
}

fn assigned_local(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Field { base, .. } | Expr::Index { base, .. } => assigned_local(base),
        Expr::Var(name) => Some(name.clone()),
        _ => None,
    }
}

fn typed_integer_literal_type(
    expr: &Expr,
    expected: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Type> {
    let expected = expected.filter(|ty| is_integer_type(ty))?;
    let value = integer_literal_value(expr)?;
    if !integer_value_fits_type(value, expected) {
        diagnostics.push(Diagnostic::error(format!(
            "integer literal {value} does not fit in type {}",
            type_name(expected)
        )));
    }
    Some(expected.clone())
}

fn integer_literal_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Int(value) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => {
            let Expr::Int(value) = expr.as_ref() else {
                return None;
            };
            value.checked_neg()
        }
        _ => None,
    }
}

fn integer_value_fits_type(value: i64, ty: &Type) -> bool {
    match ty {
        Type::Int | Type::I64 => true,
        Type::I8 => i8::try_from(value).is_ok(),
        Type::I16 => i16::try_from(value).is_ok(),
        Type::I32 => i32::try_from(value).is_ok(),
        Type::Usize | Type::U64 => value >= 0,
        Type::U8 => u8::try_from(value).is_ok(),
        Type::U16 => u16::try_from(value).is_ok(),
        Type::U32 => u32::try_from(value).is_ok(),
        _ => false,
    }
}

fn type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Unit => "unit",
        Type::Int => "int",
        Type::Bool => "bool",
        Type::Char => "char",
        Type::String => "str",
        Type::Usize => "usize",
        Type::I8 => "i8",
        Type::I16 => "i16",
        Type::I32 => "i32",
        Type::I64 => "i64",
        Type::U8 => "u8",
        Type::U16 => "u16",
        Type::U32 => "u32",
        Type::U64 => "u64",
        Type::Array(_) => "array",
        Type::Slice(_) => "slice",
        Type::Reference { .. } => "reference",
        Type::Pointer(_) => "pointer",
        Type::Named(_) => "named",
    }
}
