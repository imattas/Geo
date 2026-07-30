use crate::ast::{Expr, Import, MatchPattern, Program, Stmt, Type};
use crate::diagnostics::Diagnostic;
use crate::lexer::lex;
use crate::parser::parse;
use crate::source::{module_path_to_file, SourceFile};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn load_package_entry(path: &Path) -> Result<Program, Vec<Diagnostic>> {
    let entry = package_entry(path)?;
    let root = entry.parent().unwrap_or_else(|| Path::new("."));
    let mut ctx = ResolveCtx {
        root: root.to_path_buf(),
        visiting: HashSet::new(),
        visited: HashSet::new(),
    };
    ctx.load_file(&entry)
}

fn package_entry(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    if path.is_dir() {
        let entry = path.join("main.geo");
        if !entry.is_file() {
            return Err(vec![Diagnostic::error(format!(
                "package '{}' has no main.geo entry",
                path.display()
            ))]);
        }
        return Ok(entry);
    }

    Ok(path.to_path_buf())
}

struct ResolveCtx {
    root: PathBuf,
    visiting: HashSet<PathBuf>,
    visited: HashSet<PathBuf>,
}

impl ResolveCtx {
    fn load_file(&mut self, path: &Path) -> Result<Program, Vec<Diagnostic>> {
        let key = canonical_key(path);
        if self.visiting.contains(&key) {
            return Err(vec![Diagnostic::error(format!(
                "circular import involving '{}'",
                path.display()
            ))]);
        }
        if self.visited.contains(&key) {
            return Ok(empty_program());
        }

        self.visiting.insert(key.clone());
        let source = SourceFile::load(path)?;
        let mut program = parse_source(&source)?;
        if !has_explicit_visibility(&program) {
            for function in &mut program.functions {
                function.is_public = true;
            }
            for function in &mut program.externs {
                function.is_public = true;
            }
            for alias in &mut program.type_aliases {
                alias.is_public = true;
            }
            for constant in &mut program.consts {
                constant.is_public = true;
            }
            for structure in &mut program.structs {
                structure.is_public = true;
                for field in &mut structure.fields {
                    field.is_public = true;
                }
            }
            for enumeration in &mut program.enums {
                enumeration.is_public = true;
            }
        }
        let mut merged = empty_program();

        for import in program.imports.clone() {
            if is_std_import(&import) {
                continue;
            }
            let module_path = module_path_to_file(&self.root, &import.path);
            let imported = self.load_file(&module_path)?;
            rewrite_qualified_imports(&mut program, &import, &imported);
            merge_program(&mut merged, imported);
        }

        self.visiting.remove(&key);
        self.visited.insert(key);
        merge_program(&mut merged, take_declarations(&mut program));
        Ok(merged)
    }
}

fn has_explicit_visibility(program: &Program) -> bool {
    program.functions.iter().any(|function| function.is_public)
        || program.externs.iter().any(|function| function.is_public)
        || program.type_aliases.iter().any(|alias| alias.is_public)
        || program.consts.iter().any(|constant| constant.is_public)
        || program.structs.iter().any(|structure| structure.is_public)
        || program
            .enums
            .iter()
            .any(|enumeration| enumeration.is_public)
}

fn parse_source(source: &SourceFile) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lex(&source.text).map_err(|diagnostics| source.attach_diagnostics(diagnostics))?;
    let mut program =
        parse(&tokens).map_err(|diagnostics| source.attach_diagnostics(diagnostics))?;
    for function in &mut program.functions {
        function.source_path = Some(source.path.clone());
    }
    for function in &mut program.externs {
        function.source_path = Some(source.path.clone());
    }
    for structure in &mut program.structs {
        structure.source_path = Some(source.path.clone());
    }
    for alias in &mut program.type_aliases {
        alias.source_path = Some(source.path.clone());
    }
    for constant in &mut program.consts {
        constant.source_path = Some(source.path.clone());
    }
    for enumeration in &mut program.enums {
        enumeration.source_path = Some(source.path.clone());
    }
    Ok(program)
}

fn is_std_import(import: &Import) -> bool {
    import.path.first().is_some_and(|segment| segment == "std")
}

fn import_source_prefix(import: &Import) -> String {
    import
        .alias
        .clone()
        .unwrap_or_else(|| import.path.join("."))
}

fn rewrite_qualified_imports(program: &mut Program, import: &Import, imported: &Program) {
    let prefix = import_source_prefix(import);
    let mut call_names = HashSet::new();
    let mut type_names = HashSet::new();
    let mut const_names = HashSet::new();
    let mut enum_variant_names = HashSet::new();
    for function in &imported.functions {
        if function.is_public {
            call_names.insert(format!("{prefix}.{}", function.name));
        }
    }
    for extern_function in &imported.externs {
        if extern_function.is_public {
            call_names.insert(format!("{prefix}.{}", extern_function.name));
        }
    }
    for struct_decl in &imported.structs {
        if struct_decl.is_public {
            type_names.insert(format!("{prefix}.{}", struct_decl.name));
        }
    }
    for enum_decl in &imported.enums {
        if enum_decl.is_public {
            type_names.insert(format!("{prefix}.{}", enum_decl.name));
            for variant in &enum_decl.variants {
                enum_variant_names.insert(format!("{prefix}.{}.{}", enum_decl.name, variant.name));
            }
        }
    }
    for alias in &imported.type_aliases {
        if alias.is_public {
            type_names.insert(format!("{prefix}.{}", alias.name));
        }
    }
    for const_decl in &imported.consts {
        if const_decl.is_public {
            const_names.insert(format!("{prefix}.{}", const_decl.name));
        }
    }

    for alias in &mut program.type_aliases {
        rewrite_qualified_type(&mut alias.ty, &type_names, &prefix);
    }
    for const_decl in &mut program.consts {
        rewrite_qualified_type(&mut const_decl.ty, &type_names, &prefix);
        rewrite_qualified_imports_in_expr(
            &mut const_decl.value,
            &call_names,
            &type_names,
            &enum_variant_names,
            &prefix,
        );
        rewrite_qualified_consts_in_expr(&mut const_decl.value, &const_names, &prefix);
    }
    for struct_decl in &mut program.structs {
        for field in &mut struct_decl.fields {
            rewrite_qualified_type(&mut field.ty, &type_names, &prefix);
        }
    }
    for extern_function in &mut program.externs {
        for param in &mut extern_function.params {
            rewrite_qualified_type(&mut param.ty, &type_names, &prefix);
        }
        rewrite_qualified_type(&mut extern_function.return_type, &type_names, &prefix);
    }
    for function in &mut program.functions {
        for param in &mut function.params {
            rewrite_qualified_type(&mut param.ty, &type_names, &prefix);
        }
        rewrite_qualified_type(&mut function.return_type, &type_names, &prefix);
        for stmt in &mut function.body {
            rewrite_qualified_imports_in_stmt(
                stmt,
                &call_names,
                &type_names,
                &enum_variant_names,
                &prefix,
            );
            rewrite_qualified_consts_in_stmt(stmt, &const_names, &prefix);
        }
    }
}

fn rewrite_qualified_imports_in_stmt(
    stmt: &mut Stmt,
    call_names: &HashSet<String>,
    type_names: &HashSet<String>,
    enum_variant_names: &HashSet<String>,
    prefix: &str,
) {
    match stmt {
        Stmt::Return(Some(value))
        | Stmt::Assign { value, .. }
        | Stmt::PointerAssign { value, .. }
        | Stmt::PlaceAssign { value, .. }
        | Stmt::Expr(value) => rewrite_qualified_imports_in_expr(
            value,
            call_names,
            type_names,
            enum_variant_names,
            prefix,
        ),
        Stmt::Let { ty, value, .. } => {
            if let Some(ty) = ty {
                rewrite_qualified_type(ty, type_names, prefix);
            }
            rewrite_qualified_imports_in_expr(
                value,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            rewrite_qualified_imports_in_expr(
                condition,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            for stmt in then_body {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
            for stmt in else_body {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Stmt::While { condition, body } => {
            rewrite_qualified_imports_in_expr(
                condition,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            for stmt in body {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Stmt::For {
            start, end, body, ..
        } => {
            rewrite_qualified_imports_in_expr(
                start,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_imports_in_expr(
                end,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            for stmt in body {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Stmt::Loop(body) | Stmt::Unsafe(body) => {
            for stmt in body {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
    }
}

fn rewrite_qualified_imports_in_expr(
    expr: &mut Expr,
    call_names: &HashSet<String>,
    type_names: &HashSet<String>,
    enum_variant_names: &HashSet<String>,
    prefix: &str,
) {
    match expr {
        Expr::Field { .. } => {
            if let Some((enum_name, variant)) =
                qualified_enum_variant_expr(expr, enum_variant_names, prefix)
            {
                *expr = Expr::Field {
                    base: Box::new(Expr::Var(enum_name)),
                    name: variant,
                };
                return;
            }
            if let Expr::Field { base, .. } = expr {
                rewrite_qualified_imports_in_expr(
                    base,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Expr::Call { name, args } => {
            if let Some(unqualified) = imported_unqualified_name(name, call_names, prefix) {
                *name = unqualified;
            }
            for arg in args {
                rewrite_qualified_imports_in_expr(
                    arg,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Expr::Unary { expr, .. } => {
            rewrite_qualified_imports_in_expr(
                expr,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Expr::Cast { expr, ty } => {
            rewrite_qualified_imports_in_expr(
                expr,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_type(ty, type_names, prefix);
        }
        Expr::SizeOf(ty) | Expr::AlignOf(ty) => {
            rewrite_qualified_type(ty, type_names, prefix);
        }
        Expr::OffsetOf { ty, .. } => {
            rewrite_qualified_type(ty, type_names, prefix);
        }
        Expr::Match { value, arms } => {
            rewrite_qualified_imports_in_expr(
                value,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            for arm in arms {
                rewrite_qualified_match_pattern(&mut arm.pattern, enum_variant_names, prefix);
                rewrite_qualified_imports_in_expr(
                    &mut arm.value,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            rewrite_qualified_imports_in_expr(
                condition,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_imports_in_expr(
                then_value,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_imports_in_expr(
                else_value,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Expr::Block { statements, value } => {
            for stmt in statements {
                rewrite_qualified_imports_in_stmt(
                    stmt,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
            rewrite_qualified_imports_in_expr(
                value,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Expr::Binary { left, right, .. } => {
            rewrite_qualified_imports_in_expr(
                left,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_imports_in_expr(
                right,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Expr::Struct { name, fields } => {
            if let Some(unqualified) = imported_unqualified_name(name, type_names, prefix) {
                *name = unqualified;
            }
            for (_, value) in fields {
                rewrite_qualified_imports_in_expr(
                    value,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Expr::Array(values) => {
            for value in values {
                rewrite_qualified_imports_in_expr(
                    value,
                    call_names,
                    type_names,
                    enum_variant_names,
                    prefix,
                );
            }
        }
        Expr::Index { base, index } => {
            rewrite_qualified_imports_in_expr(
                base,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
            rewrite_qualified_imports_in_expr(
                index,
                call_names,
                type_names,
                enum_variant_names,
                prefix,
            );
        }
        Expr::Int(_)
        | Expr::TypedInt { .. }
        | Expr::Bool(_)
        | Expr::Char(_)
        | Expr::String(_)
        | Expr::Null
        | Expr::Var(_) => {}
    }
}

fn rewrite_qualified_consts_in_stmt(stmt: &mut Stmt, const_names: &HashSet<String>, prefix: &str) {
    match stmt {
        Stmt::Return(Some(value))
        | Stmt::Assign { value, .. }
        | Stmt::PointerAssign { value, .. }
        | Stmt::PlaceAssign { value, .. }
        | Stmt::Expr(value) => rewrite_qualified_consts_in_expr(value, const_names, prefix),
        Stmt::Let { value, .. } => rewrite_qualified_consts_in_expr(value, const_names, prefix),
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            rewrite_qualified_consts_in_expr(condition, const_names, prefix);
            for stmt in then_body {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
            for stmt in else_body {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
        }
        Stmt::While { condition, body } => {
            rewrite_qualified_consts_in_expr(condition, const_names, prefix);
            for stmt in body {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
        }
        Stmt::For {
            start, end, body, ..
        } => {
            rewrite_qualified_consts_in_expr(start, const_names, prefix);
            rewrite_qualified_consts_in_expr(end, const_names, prefix);
            for stmt in body {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
        }
        Stmt::Loop(body) | Stmt::Unsafe(body) => {
            for stmt in body {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
        }
        Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
    }
}

fn rewrite_qualified_consts_in_expr(expr: &mut Expr, const_names: &HashSet<String>, prefix: &str) {
    match expr {
        Expr::Field { .. } => {
            if let Some(qualified) = field_chain_name(expr) {
                if let Some(unqualified) =
                    imported_unqualified_name(&qualified, const_names, prefix)
                {
                    *expr = Expr::Var(unqualified);
                    return;
                }
            }
            if let Expr::Field { base, .. } = expr {
                rewrite_qualified_consts_in_expr(base, const_names, prefix);
            }
        }
        Expr::Call { args, .. } => {
            for arg in args {
                rewrite_qualified_consts_in_expr(arg, const_names, prefix);
            }
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            rewrite_qualified_consts_in_expr(expr, const_names, prefix);
        }
        Expr::Match { value, arms } => {
            rewrite_qualified_consts_in_expr(value, const_names, prefix);
            for arm in arms {
                rewrite_qualified_consts_in_expr(&mut arm.value, const_names, prefix);
            }
        }
        Expr::If {
            condition,
            then_value,
            else_value,
        } => {
            rewrite_qualified_consts_in_expr(condition, const_names, prefix);
            rewrite_qualified_consts_in_expr(then_value, const_names, prefix);
            rewrite_qualified_consts_in_expr(else_value, const_names, prefix);
        }
        Expr::Block { statements, value } => {
            for stmt in statements {
                rewrite_qualified_consts_in_stmt(stmt, const_names, prefix);
            }
            rewrite_qualified_consts_in_expr(value, const_names, prefix);
        }
        Expr::Binary { left, right, .. } => {
            rewrite_qualified_consts_in_expr(left, const_names, prefix);
            rewrite_qualified_consts_in_expr(right, const_names, prefix);
        }
        Expr::Struct { fields, .. } => {
            for (_, value) in fields {
                rewrite_qualified_consts_in_expr(value, const_names, prefix);
            }
        }
        Expr::Array(values) => {
            for value in values {
                rewrite_qualified_consts_in_expr(value, const_names, prefix);
            }
        }
        Expr::Index { base, index } => {
            rewrite_qualified_consts_in_expr(base, const_names, prefix);
            rewrite_qualified_consts_in_expr(index, const_names, prefix);
        }
        Expr::SizeOf(_)
        | Expr::AlignOf(_)
        | Expr::OffsetOf { .. }
        | Expr::Int(_)
        | Expr::TypedInt { .. }
        | Expr::Bool(_)
        | Expr::Char(_)
        | Expr::String(_)
        | Expr::Null
        | Expr::Var(_) => {}
    }
}

fn rewrite_qualified_match_pattern(
    pattern: &mut MatchPattern,
    enum_variant_names: &HashSet<String>,
    prefix: &str,
) {
    let MatchPattern::EnumVariant { enum_name, variant } = pattern else {
        return;
    };
    let qualified = format!("{enum_name}.{variant}");
    if let Some((unqualified_enum, unqualified_variant)) =
        imported_unqualified_enum_variant(&qualified, enum_variant_names, prefix)
    {
        *enum_name = unqualified_enum;
        *variant = unqualified_variant;
    }
}

fn qualified_enum_variant_expr(
    expr: &Expr,
    enum_variant_names: &HashSet<String>,
    prefix: &str,
) -> Option<(String, String)> {
    let qualified = field_chain_name(expr)?;
    imported_unqualified_enum_variant(&qualified, enum_variant_names, prefix)
}

fn field_chain_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::Field { base, name } => Some(format!("{}.{}", field_chain_name(base)?, name)),
        _ => None,
    }
}

fn imported_unqualified_enum_variant(
    name: &str,
    enum_variant_names: &HashSet<String>,
    prefix: &str,
) -> Option<(String, String)> {
    let unqualified = imported_unqualified_name(name, enum_variant_names, prefix)?;
    let (enum_name, variant) = unqualified.rsplit_once('.')?;
    Some((enum_name.to_string(), variant.to_string()))
}

fn rewrite_qualified_type(ty: &mut Type, type_names: &HashSet<String>, prefix: &str) {
    match ty {
        Type::Named(name) => {
            if let Some(unqualified) = imported_unqualified_name(name, type_names, prefix) {
                *name = unqualified;
            }
        }
        Type::Array(inner) | Type::Slice(inner) | Type::Pointer(inner) => {
            rewrite_qualified_type(inner, type_names, prefix)
        }
        Type::Reference { inner, .. } => rewrite_qualified_type(inner, type_names, prefix),
        Type::Unit
        | Type::Int
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

fn imported_unqualified_name(
    name: &str,
    imported_names: &HashSet<String>,
    prefix: &str,
) -> Option<String> {
    if !imported_names.contains(name) {
        return None;
    }
    Some(
        name.strip_prefix(prefix)
            .and_then(|name| name.strip_prefix('.'))
            .expect("imported names are built from this prefix")
            .to_string(),
    )
}

fn merge_program(target: &mut Program, source: Program) {
    for import in source.imports {
        if !target
            .imports
            .iter()
            .any(|existing| existing.path == import.path && existing.alias == import.alias)
        {
            target.imports.push(import);
        }
    }
    target.structs.extend(source.structs);
    target.enums.extend(source.enums);
    target.type_aliases.extend(source.type_aliases);
    target.consts.extend(source.consts);
    target.externs.extend(source.externs);
    target.functions.extend(source.functions);
}

fn take_declarations(program: &mut Program) -> Program {
    Program {
        imports: std::mem::take(&mut program.imports)
            .into_iter()
            .filter(is_std_import)
            .collect(),
        type_aliases: std::mem::take(&mut program.type_aliases),
        consts: std::mem::take(&mut program.consts),
        structs: std::mem::take(&mut program.structs),
        enums: std::mem::take(&mut program.enums),
        externs: std::mem::take(&mut program.externs),
        functions: std::mem::take(&mut program.functions),
    }
}

fn empty_program() -> Program {
    Program {
        imports: Vec::new(),
        type_aliases: Vec::new(),
        consts: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
        externs: Vec::new(),
        functions: Vec::new(),
    }
}

fn canonical_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
