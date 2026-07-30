use crate::ast::{BinaryOp, Expr, MatchPattern, Program, Stmt, Type, UnaryOp};
use std::fmt::Write;

/// Format a parsed Geo program using the canonical source layout.
pub fn format_program(program: &Program) -> String {
    Formatter::new().format_program(program)
}

struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn format_program(mut self, program: &Program) -> String {
        for import in &program.imports {
            self.line(&format!(
                "import {}{}",
                import.path.join("."),
                import
                    .alias
                    .as_deref()
                    .map(|alias| format!(" as {alias}"))
                    .unwrap_or_default()
            ));
        }
        self.blank_after_section(!program.imports.is_empty());

        for alias in &program.type_aliases {
            self.line(&format!("type {} = {}", alias.name, format_type(&alias.ty)));
        }
        for constant in &program.consts {
            self.line(&format!(
                "const {}: {} = {}",
                constant.name,
                format_type(&constant.ty),
                self.expr(&constant.value, 0)
            ));
        }
        self.format_structs(&program.structs);
        self.format_enums(&program.enums);
        for external in &program.externs {
            self.line(&format!(
                "extern fn {}({}) -> {}",
                external.name,
                format_params(&external.params),
                format_type(&external.return_type)
            ));
        }
        self.blank_after_section(
            !program.type_aliases.is_empty()
                || !program.consts.is_empty()
                || !program.structs.is_empty()
                || !program.enums.is_empty()
                || !program.externs.is_empty(),
        );

        for (index, function) in program.functions.iter().enumerate() {
            if index != 0 {
                self.blank_line();
            }
            self.format_function(function);
        }

        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }

    fn blank_after_section(&mut self, present: bool) {
        if present && !self.output.is_empty() && !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn blank_line(&mut self) {
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn format_structs(&mut self, structs: &[crate::ast::StructDecl]) {
        for structure in structs {
            self.line(&format!("struct {} {{", structure.name));
            self.indent += 1;
            for field in &structure.fields {
                let visibility = if field.is_public { "pub " } else { "" };
                self.line(&format!(
                    "{}{}: {}",
                    visibility,
                    field.name,
                    format_type(&field.ty)
                ));
            }
            self.indent -= 1;
            self.line("}");
            self.blank_line();
        }
    }

    fn format_enums(&mut self, enums: &[crate::ast::EnumDecl]) {
        for enumeration in enums {
            self.line(&format!("enum {} {{", enumeration.name));
            self.indent += 1;
            for variant in &enumeration.variants {
                let value = variant
                    .value
                    .map(|value| format!(" = {value}"))
                    .unwrap_or_default();
                self.line(&format!("{}{value},", variant.name));
            }
            self.indent -= 1;
            self.line("}");
            self.blank_line();
        }
    }

    fn format_function(&mut self, function: &crate::ast::Function) {
        let return_type = if function.return_type == Type::Unit {
            String::new()
        } else {
            format!(" -> {}", format_type(&function.return_type))
        };
        self.line(&format!(
            "fn {}({}){} {{",
            function.name,
            format_params(&function.params),
            return_type
        ));
        self.indent += 1;
        for statement in &function.body {
            self.format_stmt(statement);
        }
        self.indent -= 1;
        self.line("}");
    }

    fn format_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Return(value) => {
                self.line(&match value {
                    Some(value) => format!("return {}", self.expr(value, 0)),
                    None => "return".to_string(),
                });
            }
            Stmt::Let {
                name,
                ty,
                mutable,
                value,
            } => {
                let keyword = if *mutable { "var" } else { "let" };
                let annotation = ty
                    .as_ref()
                    .map(|ty| format!(": {}", format_type(ty)))
                    .unwrap_or_default();
                self.line(&format!(
                    "{keyword} {name}{annotation} = {}",
                    self.expr(value, 0)
                ));
            }
            Stmt::Assign { name, op, value } => self.line(&format!(
                "{name} {} {}",
                format_assignment_op(*op),
                self.expr(value, 0)
            )),
            Stmt::PointerAssign { pointer, op, value } => self.line(&format!(
                "{} {} {}",
                self.expr(pointer, 0),
                format_assignment_op(*op),
                self.expr(value, 0)
            )),
            Stmt::PlaceAssign { target, op, value } => self.line(&format!(
                "{} {} {}",
                self.expr(target, 0),
                format_assignment_op(*op),
                self.expr(value, 0)
            )),
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                self.line(&format!("if {} {{", self.expr(condition, 0)));
                self.indent += 1;
                for statement in then_body {
                    self.format_stmt(statement);
                }
                self.indent -= 1;
                if else_body.is_empty() {
                    self.line("}");
                } else if else_body.len() == 1 {
                    if let Stmt::If { .. } = &else_body[0] {
                        self.output.push_str("} else ");
                        self.format_stmt(&else_body[0]);
                    } else {
                        self.line("} else {");
                        self.indent += 1;
                        self.format_stmt(&else_body[0]);
                        for statement in &else_body[1..] {
                            self.format_stmt(statement);
                        }
                        self.indent -= 1;
                        self.line("}");
                    }
                } else {
                    self.line("} else {");
                    self.indent += 1;
                    for statement in else_body {
                        self.format_stmt(statement);
                    }
                    self.indent -= 1;
                    self.line("}");
                }
            }
            Stmt::While { condition, body } => {
                self.line(&format!("while {} {{", self.expr(condition, 0)));
                self.indent += 1;
                for statement in body {
                    self.format_stmt(statement);
                }
                self.indent -= 1;
                self.line("}");
            }
            Stmt::For {
                name,
                start,
                end,
                inclusive,
                body,
            } => {
                let range = if *inclusive { "..=" } else { ".." };
                self.line(&format!(
                    "for {name} in {}{range}{} {{",
                    self.expr(start, 0),
                    self.expr(end, 0)
                ));
                self.indent += 1;
                for statement in body {
                    self.format_stmt(statement);
                }
                self.indent -= 1;
                self.line("}");
            }
            Stmt::Loop(body) | Stmt::Unsafe(body) => {
                let keyword = if matches!(statement, Stmt::Loop(_)) {
                    "loop"
                } else {
                    "unsafe"
                };
                self.line(&format!("{keyword} {{"));
                self.indent += 1;
                for statement in body {
                    self.format_stmt(statement);
                }
                self.indent -= 1;
                self.line("}");
            }
            Stmt::Break => self.line("break"),
            Stmt::Continue => self.line("continue"),
            Stmt::Expr(value) => self.line(&self.expr(value, 0)),
        }
    }

    fn expr(&self, expression: &Expr, parent_precedence: u8) -> String {
        let (text, precedence) = match expression {
            Expr::Int(value) => (value.to_string(), 100),
            Expr::TypedInt { value, ty } => (format!("{value}{}", format_type(ty)), 100),
            Expr::Bool(value) => (value.to_string(), 100),
            Expr::Char(value) => (quote_char(*value), 100),
            Expr::String(value) => (quote_string(value), 100),
            Expr::Null => ("null".to_string(), 100),
            Expr::Var(name) => (name.clone(), 100),
            Expr::Unary { op, expr } => (
                format!("{}{}", format_unary_op(*op), self.expr(expr, 90)),
                90,
            ),
            Expr::Cast { expr, ty } => (
                format!("{} as {}", self.expr(expr, 25), format_type(ty)),
                25,
            ),
            Expr::SizeOf(ty) => (format!("sizeof({})", format_type(ty)), 100),
            Expr::AlignOf(ty) => (format!("alignof({})", format_type(ty)), 100),
            Expr::OffsetOf { ty, field } => {
                (format!("offsetof({}, {field})", format_type(ty)), 100)
            }
            Expr::Match { value, arms } => {
                let mut text = format!("match {} {{", self.expr(value, 0));
                for arm in arms {
                    write!(
                        text,
                        " {} => {},",
                        format_pattern(&arm.pattern),
                        self.expr(&arm.value, 0)
                    )
                    .expect("writing to a String cannot fail");
                }
                text.push_str(" }");
                (text, 10)
            }
            Expr::If {
                condition,
                then_value,
                else_value,
            } => (
                format!(
                    "if {} {} else {}",
                    self.expr(condition, 0),
                    self.expr(then_value, 0),
                    self.expr(else_value, 0)
                ),
                10,
            ),
            Expr::Block { statements, value } => {
                let mut text = "{ ".to_string();
                for statement in statements {
                    write!(text, "{}; ", self.inline_stmt(statement))
                        .expect("writing to a String cannot fail");
                }
                write!(text, "{} }}", self.expr(value, 0))
                    .expect("writing to a String cannot fail");
                (text, 10)
            }
            Expr::Binary { op, left, right } => {
                let precedence = binary_precedence(*op);
                (
                    format!(
                        "{} {} {}",
                        self.expr(left, precedence),
                        format_binary_op(*op),
                        self.expr(right, precedence + 1)
                    ),
                    precedence,
                )
            }
            Expr::Call { name, args } => (
                format!(
                    "{name}({})",
                    args.iter()
                        .map(|arg| self.expr(arg, 0))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                100,
            ),
            Expr::Struct { name, fields } => (
                format!(
                    "{name} {{ {} }}",
                    fields
                        .iter()
                        .map(|(field, value)| format!("{field}: {}", self.expr(value, 0)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                100,
            ),
            Expr::Array(values) => (
                format!(
                    "[{}]",
                    values
                        .iter()
                        .map(|value| self.expr(value, 0))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                100,
            ),
            Expr::Field { base, name } => (format!("{}.{}", self.expr(base, 100), name), 100),
            Expr::Index { base, index } => (
                format!("{}[{}]", self.expr(base, 100), self.expr(index, 0)),
                100,
            ),
        };

        if precedence < parent_precedence {
            format!("({text})")
        } else {
            text
        }
    }

    fn inline_stmt(&self, statement: &Stmt) -> String {
        match statement {
            Stmt::Expr(value) => self.expr(value, 0),
            Stmt::Return(value) => match value {
                Some(value) => format!("return {}", self.expr(value, 0)),
                None => "return".to_string(),
            },
            Stmt::Let {
                name,
                ty,
                mutable,
                value,
            } => {
                let keyword = if *mutable { "var" } else { "let" };
                let annotation = ty
                    .as_ref()
                    .map(|ty| format!(": {}", format_type(ty)))
                    .unwrap_or_default();
                format!("{keyword} {name}{annotation} = {}", self.expr(value, 0))
            }
            Stmt::Assign { name, op, value } => format!(
                "{name} {} {}",
                format_assignment_op(*op),
                self.expr(value, 0)
            ),
            Stmt::PointerAssign { pointer, op, value } => format!(
                "{} {} {}",
                self.expr(pointer, 0),
                format_assignment_op(*op),
                self.expr(value, 0)
            ),
            Stmt::PlaceAssign { target, op, value } => format!(
                "{} {} {}",
                self.expr(target, 0),
                format_assignment_op(*op),
                self.expr(value, 0)
            ),
            _ => "/* unsupported expression-block statement */".to_string(),
        }
    }
}

fn format_params(params: &[crate::ast::Param]) -> String {
    params
        .iter()
        .map(|param| format!("{}: {}", param.name, format_type(&param.ty)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Unit => "void".to_string(),
        Type::Int => "int".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Char => "char".to_string(),
        Type::String => "str".to_string(),
        Type::Usize => "usize".to_string(),
        Type::I8 => "i8".to_string(),
        Type::I16 => "i16".to_string(),
        Type::I32 => "i32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U8 => "u8".to_string(),
        Type::U16 => "u16".to_string(),
        Type::U32 => "u32".to_string(),
        Type::U64 => "u64".to_string(),
        Type::Array(inner) => format!("[{}]", format_type(inner)),
        Type::Slice(inner) => format!("[]{}", format_type(inner)),
        Type::Reference { mutable, inner } => {
            format!(
                "&{}{}",
                if *mutable { "mut " } else { "" },
                format_type(inner)
            )
        }
        Type::Pointer(inner) => format!("*{}", format_type(inner)),
        Type::Named(name) => name.clone(),
    }
}

fn format_pattern(pattern: &MatchPattern) -> String {
    match pattern {
        MatchPattern::Wildcard => "_".to_string(),
        MatchPattern::Int(value) => value.to_string(),
        MatchPattern::Bool(value) => value.to_string(),
        MatchPattern::EnumVariant { enum_name, variant } => format!("{enum_name}.{variant}"),
    }
}

fn format_unary_op(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
        UnaryOp::BitNot => "~",
        UnaryOp::AddressOf => "&",
        UnaryOp::MutableAddressOf => "&mut ",
        UnaryOp::Deref => "*",
    }
}

fn format_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::BitAnd => "&",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}

fn format_assignment_op(op: Option<BinaryOp>) -> &'static str {
    match op {
        None => "=",
        Some(BinaryOp::Add) => "+=",
        Some(BinaryOp::Sub) => "-=",
        Some(BinaryOp::Mul) => "*=",
        Some(BinaryOp::Div) => "/=",
        Some(BinaryOp::Rem) => "%=",
        Some(BinaryOp::BitAnd) => "&=",
        Some(BinaryOp::BitOr) => "|=",
        Some(BinaryOp::BitXor) => "^=",
        Some(BinaryOp::ShiftLeft) => "<<=",
        Some(BinaryOp::ShiftRight) => ">>=",
        Some(other) => format_binary_op(other),
    }
}

fn binary_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Or => 10,
        BinaryOp::And => 20,
        BinaryOp::BitOr => 30,
        BinaryOp::BitXor => 40,
        BinaryOp::BitAnd => 50,
        BinaryOp::Equal | BinaryOp::NotEqual => 60,
        BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => 70,
        BinaryOp::ShiftLeft | BinaryOp::ShiftRight => 80,
        BinaryOp::Add | BinaryOp::Sub => 90,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 95,
    }
}

fn quote_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            character if character.is_control() => {
                write!(output, "\\u{{{:x}}}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn quote_char(value: char) -> String {
    let quoted = quote_string(&value.to_string());
    format!("'{}'", &quoted[1..quoted.len() - 1])
}
