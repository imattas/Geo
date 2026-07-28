use crate::ast::{
    BinaryOp, EnumDecl, EnumVariant, Expr, ExternFunction, Field, Function, Import, MatchArm,
    MatchPattern, Param, Program, Stmt, StructDecl, Type, TypeAlias, UnaryOp,
};
use crate::token::{Token, TokenKind};
use geo_diagnostics::Diagnostic;

pub fn parse(tokens: &[Token]) -> Result<Program, Vec<Diagnostic>> {
    Parser::new(tokens).parse_program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
    allow_struct_literals: bool,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            current: 0,
            allow_struct_literals: true,
        }
    }

    fn parse_program(&mut self) -> Result<Program, Vec<Diagnostic>> {
        let mut imports = Vec::new();
        let mut type_aliases = Vec::new();
        let mut consts = Vec::new();
        let mut structs = Vec::new();
        let mut enums = Vec::new();
        let mut externs = Vec::new();
        let mut functions = Vec::new();
        while !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::Import) {
                imports.push(self.parse_import()?);
            } else if self.at(&TokenKind::Type) {
                type_aliases.push(self.parse_type_alias()?);
            } else if self.at(&TokenKind::Const) {
                consts.push(self.parse_const()?);
            } else if self.at(&TokenKind::Extern) {
                externs.push(self.parse_extern_function()?);
            } else if self.at(&TokenKind::Struct) {
                structs.push(self.parse_struct()?);
            } else if self.at(&TokenKind::Enum) {
                enums.push(self.parse_enum()?);
            } else if self.at(&TokenKind::Eof) {
                break;
            } else {
                functions.push(self.parse_function()?);
            }
        }
        Ok(Program {
            imports,
            type_aliases,
            consts,
            structs,
            enums,
            externs,
            functions,
        })
    }

    fn parse_import(&mut self) -> Result<Import, Vec<Diagnostic>> {
        self.expect(&TokenKind::Import, "expected 'import'")?;
        let mut path = vec![self.expect_import_segment()?];
        while self.matches(&TokenKind::Dot) {
            path.push(self.expect_import_segment()?);
        }
        let alias = if self.matches(&TokenKind::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.consume_semicolons();
        Ok(Import { path, alias })
    }

    fn parse_type_alias(&mut self) -> Result<TypeAlias, Vec<Diagnostic>> {
        self.expect(&TokenKind::Type, "expected 'type'")?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Equal, "expected '='")?;
        let ty = self.parse_type()?;
        self.consume_semicolons();
        Ok(TypeAlias { name, ty })
    }

    fn parse_const(&mut self) -> Result<crate::ast::ConstDecl, Vec<Diagnostic>> {
        self.expect(&TokenKind::Const, "expected 'const'")?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Colon, "expected ':'")?;
        let ty = self.parse_type()?;
        self.expect(&TokenKind::Equal, "expected '='")?;
        let value = self.parse_expr()?;
        self.consume_semicolons();
        Ok(crate::ast::ConstDecl { name, ty, value })
    }

    fn parse_extern_function(&mut self) -> Result<ExternFunction, Vec<Diagnostic>> {
        self.expect(&TokenKind::Extern, "expected 'extern'")?;
        self.expect(&TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_ident()?;
        let params = self.parse_params()?;
        self.expect(&TokenKind::Arrow, "expected '->'")?;
        let return_type = self.parse_type()?;
        Ok(ExternFunction {
            name,
            params,
            return_type,
        })
    }

    fn parse_struct(&mut self) -> Result<StructDecl, Vec<Diagnostic>> {
        self.expect(&TokenKind::Struct, "expected 'struct'")?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::RightBrace) {
                break;
            }
            let name = self.expect_ident()?;
            self.expect(&TokenKind::Colon, "expected ':'")?;
            let ty = self.parse_type()?;
            fields.push(Field { name, ty });
            if !self.matches(&TokenKind::Comma) {
                self.consume_semicolons();
            }
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(StructDecl { name, fields })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, Vec<Diagnostic>> {
        self.expect(&TokenKind::Enum, "expected 'enum'")?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut variants = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::RightBrace) {
                break;
            }
            let variant_name = self.expect_ident()?;
            let value = if self.matches(&TokenKind::Equal) {
                let TokenKind::IntLiteral(value) = self.peek().kind else {
                    return Err(vec![Diagnostic::error(
                        "enum discriminant must be an integer literal",
                    )]);
                };
                self.advance();
                Some(value)
            } else {
                None
            };
            variants.push(EnumVariant {
                name: variant_name,
                value,
            });
            if !self.matches(&TokenKind::Comma) {
                self.consume_semicolons();
            }
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(EnumDecl { name, variants })
    }

    fn parse_function(&mut self) -> Result<Function, Vec<Diagnostic>> {
        self.expect(&TokenKind::Fn, "expected 'fn'")?;
        let name = self.expect_ident()?;
        let params = self.parse_params()?;
        let return_type = if self.matches(&TokenKind::Arrow) {
            self.parse_type()?
        } else {
            Type::Unit
        };
        let body = self.parse_block()?;
        Ok(Function {
            name,
            params,
            return_type,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Vec<Diagnostic>> {
        self.expect(&TokenKind::LeftParen, "expected '('")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let name = self.expect_ident()?;
                self.expect(&TokenKind::Colon, "expected ':'")?;
                let ty = self.parse_type()?;
                params.push(Param { name, ty });
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                if self.at(&TokenKind::RightParen) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, "expected ')'")?;
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Vec<Diagnostic>> {
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut body = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::RightBrace) {
                break;
            }
            body.push(self.parse_stmt()?);
            self.consume_semicolons();
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(body)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Vec<Diagnostic>> {
        if self.matches(&TokenKind::Return) {
            if self.at(&TokenKind::Semicolon) || self.at(&TokenKind::RightBrace) {
                return Ok(Stmt::Return(None));
            }
            return Ok(Stmt::Return(Some(self.parse_expr()?)));
        }
        if self.at(&TokenKind::Let) || self.at(&TokenKind::Var) {
            let mutable = self.matches(&TokenKind::Var);
            if !mutable {
                self.expect(&TokenKind::Let, "expected 'let'")?;
            }
            let name = self.expect_ident()?;
            let ty = if self.matches(&TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(&TokenKind::Equal, "expected '='")?;
            let value = self.parse_expr()?;
            return Ok(Stmt::Let {
                name,
                ty,
                mutable,
                value,
            });
        }
        if self.matches(&TokenKind::If) {
            return self.parse_if_stmt_after_if();
        }
        if self.matches(&TokenKind::While) {
            let condition = self.parse_control_condition_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::While { condition, body });
        }
        if self.matches(&TokenKind::For) {
            let name = self.expect_ident()?;
            self.expect(&TokenKind::In, "expected 'in'")?;
            let start = self.parse_expr()?;
            let inclusive = if self.matches(&TokenKind::DotDotEqual) {
                true
            } else {
                self.expect(&TokenKind::DotDot, "expected '..'")?;
                false
            };
            let end = self.parse_control_condition_expr()?;
            let body = self.parse_block()?;
            return Ok(Stmt::For {
                name,
                start,
                end,
                inclusive,
                body,
            });
        }
        if self.matches(&TokenKind::Loop) {
            return Ok(Stmt::Loop(self.parse_block()?));
        }
        if self.matches(&TokenKind::Unsafe) {
            return Ok(Stmt::Unsafe(self.parse_block()?));
        }
        if self.matches(&TokenKind::Break) {
            return Ok(Stmt::Break);
        }
        if self.matches(&TokenKind::Continue) {
            return Ok(Stmt::Continue);
        }

        if matches!(self.peek().kind, TokenKind::Ident(_)) && self.peek_next_is_assignment_op() {
            let name = self.expect_ident()?;
            let op = self.assignment_op()?;
            let value = self.parse_expr()?;
            return Ok(Stmt::Assign { name, op, value });
        }

        if self.matches(&TokenKind::Star) {
            let pointer = self.parse_unary()?;
            let op = self.assignment_op()?;
            let value = self.parse_expr()?;
            return Ok(Stmt::PointerAssign { pointer, op, value });
        }

        let expr = self.parse_expr()?;
        if self.is_assignment_op() {
            let op = self.assignment_op()?;
            let value = self.parse_expr()?;
            return Ok(Stmt::PlaceAssign {
                target: expr,
                op,
                value,
            });
        }

        Ok(Stmt::Expr(expr))
    }

    fn parse_if_stmt_after_if(&mut self) -> Result<Stmt, Vec<Diagnostic>> {
        let condition = self.parse_control_condition_expr()?;
        let then_body = self.parse_block()?;
        let else_body = if self.matches(&TokenKind::Else) {
            if self.matches(&TokenKind::If) {
                vec![self.parse_if_stmt_after_if()?]
            } else {
                self.parse_block()?
            }
        } else {
            Vec::new()
        };
        Ok(Stmt::If {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_type(&mut self) -> Result<Type, Vec<Diagnostic>> {
        if self.matches(&TokenKind::LeftBracket) {
            if self.matches(&TokenKind::RightBracket) {
                let inner = self.parse_type()?;
                return Ok(Type::Slice(Box::new(inner)));
            }
            let inner = self.parse_type()?;
            self.expect(&TokenKind::RightBracket, "expected ']'")?;
            return Ok(Type::Array(Box::new(inner)));
        }
        if self.matches(&TokenKind::Star) {
            let inner = self.parse_type()?;
            return Ok(Type::Pointer(Box::new(inner)));
        }
        if self.matches(&TokenKind::Ampersand) {
            let mutable = self.matches(&TokenKind::Mut);
            let inner = self.parse_type()?;
            return Ok(Type::Reference {
                mutable,
                inner: Box::new(inner),
            });
        }

        if self.matches(&TokenKind::Int) {
            Ok(Type::Int)
        } else if self.matches(&TokenKind::Bool) {
            Ok(Type::Bool)
        } else if self.matches(&TokenKind::Char) {
            Ok(Type::Char)
        } else if self.matches(&TokenKind::String) {
            Ok(Type::String)
        } else if self.matches(&TokenKind::Str) {
            Ok(Type::String)
        } else if self.matches(&TokenKind::Usize) {
            Ok(Type::Usize)
        } else if self.matches(&TokenKind::I8) {
            Ok(Type::I8)
        } else if self.matches(&TokenKind::I16) {
            Ok(Type::I16)
        } else if self.matches(&TokenKind::I32) {
            Ok(Type::I32)
        } else if self.matches(&TokenKind::I64) {
            Ok(Type::I64)
        } else if self.matches(&TokenKind::U8) {
            Ok(Type::U8)
        } else if self.matches(&TokenKind::U16) {
            Ok(Type::U16)
        } else if self.matches(&TokenKind::U32) {
            Ok(Type::U32)
        } else if self.matches(&TokenKind::U64) {
            Ok(Type::U64)
        } else if matches!(self.peek().kind, TokenKind::Ident(_)) {
            let name = self.parse_qualified_ident()?;
            Ok(Type::Named(name))
        } else {
            Err(vec![Diagnostic::error("expected type")])
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        self.parse_or()
    }

    fn parse_control_condition_expr(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let previous_allow_struct_literals = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let expr = self.parse_expr();
        self.allow_struct_literals = previous_allow_struct_literals;
        expr
    }

    fn parse_or(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_and()?;
        while self.matches(&TokenKind::PipePipe) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_bit_or()?;
        while self.matches(&TokenKind::AmpersandAmpersand) {
            let right = self.parse_bit_or()?;
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_bit_xor()?;
        while self.matches(&TokenKind::Pipe) {
            let right = self.parse_bit_xor()?;
            expr = Expr::Binary {
                op: BinaryOp::BitOr,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_bit_and()?;
        while self.matches(&TokenKind::Caret) {
            let right = self.parse_bit_and()?;
            expr = Expr::Binary {
                op: BinaryOp::BitXor,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_equality()?;
        while self.matches(&TokenKind::Ampersand) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                op: BinaryOp::BitAnd,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_comparison()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqual) {
                Some(BinaryOp::Equal)
            } else if self.matches(&TokenKind::BangEqual) {
                Some(BinaryOp::NotEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_shift()?;
        loop {
            let op = if self.matches(&TokenKind::Less) {
                Some(BinaryOp::Less)
            } else if self.matches(&TokenKind::LessEqual) {
                Some(BinaryOp::LessEqual)
            } else if self.matches(&TokenKind::Greater) {
                Some(BinaryOp::Greater)
            } else if self.matches(&TokenKind::GreaterEqual) {
                Some(BinaryOp::GreaterEqual)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_shift()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_shift(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_term()?;
        loop {
            let op = if self.matches(&TokenKind::ShiftLeft) {
                Some(BinaryOp::ShiftLeft)
            } else if self.matches(&TokenKind::ShiftRight) {
                Some(BinaryOp::ShiftRight)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_term()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_factor()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                Some(BinaryOp::Add)
            } else if self.matches(&TokenKind::Minus) {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_cast()?;
        loop {
            if self.starts_pointer_assignment() {
                break;
            }
            let op = if self.matches(&TokenKind::Star) {
                Some(BinaryOp::Mul)
            } else if self.matches(&TokenKind::Slash) {
                Some(BinaryOp::Div)
            } else if self.matches(&TokenKind::Percent) {
                Some(BinaryOp::Rem)
            } else {
                None
            };
            let Some(op) = op else { break };
            let right = self.parse_cast()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_cast(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = self.parse_unary()?;
        while self.matches(&TokenKind::As) {
            let ty = self.parse_type()?;
            expr = Expr::Cast {
                expr: Box::new(expr),
                ty,
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        if self.matches(&TokenKind::Minus) {
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.matches(&TokenKind::Bang) {
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.matches(&TokenKind::Tilde) {
            return Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.matches(&TokenKind::Ampersand) {
            let op = if self.matches(&TokenKind::Mut) {
                UnaryOp::MutableAddressOf
            } else {
                UnaryOp::AddressOf
            };
            return Ok(Expr::Unary {
                op,
                expr: Box::new(self.parse_unary()?),
            });
        }
        if self.matches(&TokenKind::Star) {
            return Ok(Expr::Unary {
                op: UnaryOp::Deref,
                expr: Box::new(self.parse_unary()?),
            });
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        let mut expr = match &self.peek().kind {
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Match => self.parse_match_expr(),
            TokenKind::Sizeof => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "expected '('")?;
                let ty = self.parse_type()?;
                self.expect(&TokenKind::RightParen, "expected ')'")?;
                Ok(Expr::SizeOf(ty))
            }
            TokenKind::Alignof => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "expected '('")?;
                let ty = self.parse_type()?;
                self.expect(&TokenKind::RightParen, "expected ')'")?;
                Ok(Expr::AlignOf(ty))
            }
            TokenKind::Offsetof => {
                self.advance();
                self.expect(&TokenKind::LeftParen, "expected '('")?;
                let ty = self.parse_type()?;
                self.expect(&TokenKind::Comma, "expected ','")?;
                let field = self.expect_ident()?;
                self.expect(&TokenKind::RightParen, "expected ')'")?;
                Ok(Expr::OffsetOf { ty, field })
            }
            TokenKind::IntLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(Expr::Int(value))
            }
            TokenKind::TypedIntLiteral(value, suffix) => {
                let value = *value;
                let ty = integer_suffix_type(suffix)?;
                self.advance();
                Ok(Expr::TypedInt { value, ty })
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Null)
            }
            TokenKind::CharLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(Expr::Char(value))
            }
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.advance();
                Ok(Expr::String(value))
            }
            TokenKind::Ident(_) => {
                let name = self.expect_ident()?;
                if self.at(&TokenKind::LeftParen) {
                    let args = self.parse_call_args()?;
                    Ok(Expr::Call { name, args })
                } else if let Some(name) = self.qualified_call_name(&name) {
                    let args = self.parse_call_args()?;
                    Ok(Expr::Call { name, args })
                } else if let Some(name) = self.qualified_struct_literal_name(&name) {
                    self.parse_struct_literal(name)
                } else if self.allow_struct_literals && self.at(&TokenKind::LeftBrace) {
                    self.parse_struct_literal(name)
                } else {
                    Ok(Expr::Var(name))
                }
            }
            TokenKind::LeftBracket => {
                self.advance();
                let mut values = Vec::new();
                if !self.at(&TokenKind::RightBracket) {
                    loop {
                        values.push(self.parse_expr()?);
                        if !self.matches(&TokenKind::Comma) {
                            self.consume_semicolons();
                            break;
                        }
                        if self.at(&TokenKind::RightBracket) {
                            break;
                        }
                    }
                }
                self.expect(&TokenKind::RightBracket, "expected ']'")?;
                Ok(Expr::Array(values))
            }
            TokenKind::LeftParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RightParen, "expected ')'")?;
                Ok(expr)
            }
            TokenKind::LeftBrace => self.parse_expr_block(),
            _ => Err(vec![Diagnostic::error("expected expression")]),
        }?;

        loop {
            if self.matches(&TokenKind::Dot) {
                let name = self.expect_ident()?;
                expr = Expr::Field {
                    base: Box::new(expr),
                    name,
                };
            } else if self.matches(&TokenKind::LeftBracket) {
                let index = self.parse_expr()?;
                self.expect(&TokenKind::RightBracket, "expected ']'")?;
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, Vec<Diagnostic>> {
        self.expect(&TokenKind::LeftParen, "expected '('")?;
        let mut args = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.matches(&TokenKind::Comma) {
                    break;
                }
                if self.at(&TokenKind::RightParen) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RightParen, "expected ')'")?;
        Ok(args)
    }

    fn parse_struct_literal(&mut self, name: String) -> Result<Expr, Vec<Diagnostic>> {
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::RightBrace) {
                break;
            }
            let field_name = self.expect_ident()?;
            let value = if self.matches(&TokenKind::Colon) {
                self.parse_expr()?
            } else {
                Expr::Var(field_name.clone())
            };
            fields.push((field_name, value));
            if !self.matches(&TokenKind::Comma) {
                self.consume_semicolons();
            }
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(Expr::Struct { name, fields })
    }

    fn qualified_struct_literal_name(&mut self, first: &str) -> Option<String> {
        if !self.allow_struct_literals || !self.at(&TokenKind::Dot) {
            return None;
        }

        let mut current = self.current;
        let mut segments = vec![first.to_string()];
        while matches!(
            self.tokens.get(current).map(|token| &token.kind),
            Some(TokenKind::Dot)
        ) {
            current += 1;
            let Some(TokenKind::Ident(segment)) = self.tokens.get(current).map(|token| &token.kind)
            else {
                return None;
            };
            segments.push(segment.clone());
            current += 1;
        }

        if !matches!(
            self.tokens.get(current).map(|token| &token.kind),
            Some(TokenKind::LeftBrace)
        ) {
            return None;
        }

        self.current = current;
        Some(segments.join("."))
    }

    fn qualified_call_name(&mut self, first: &str) -> Option<String> {
        if !self.at(&TokenKind::Dot) {
            return None;
        }

        let mut current = self.current;
        let mut segments = vec![first.to_string()];
        while matches!(
            self.tokens.get(current).map(|token| &token.kind),
            Some(TokenKind::Dot)
        ) {
            current += 1;
            let Some(TokenKind::Ident(segment)) = self.tokens.get(current).map(|token| &token.kind)
            else {
                return None;
            };
            segments.push(segment.clone());
            current += 1;
        }

        if !matches!(
            self.tokens.get(current).map(|token| &token.kind),
            Some(TokenKind::LeftParen)
        ) {
            return None;
        }

        self.current = current;
        Some(segments.join("."))
    }

    fn parse_match_expr(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        self.expect(&TokenKind::Match, "expected 'match'")?;
        let value = self.parse_control_condition_expr()?;
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut arms = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            self.consume_semicolons();
            if self.at(&TokenKind::RightBrace) {
                break;
            }
            let pattern = self.parse_match_pattern()?;
            self.expect(&TokenKind::FatArrow, "expected '=>'")?;
            let value = self.parse_expr()?;
            arms.push(MatchArm { pattern, value });
            if !self.matches(&TokenKind::Comma) {
                self.consume_semicolons();
            }
        }
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        Ok(Expr::Match {
            value: Box::new(value),
            arms,
        })
    }

    fn parse_if_expr(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        self.expect(&TokenKind::If, "expected 'if'")?;
        let condition = self.parse_control_condition_expr()?;
        let then_value = self.parse_expr_block()?;
        self.expect(&TokenKind::Else, "expected 'else' in if expression")?;
        let else_value = if self.at(&TokenKind::If) {
            self.parse_if_expr()?
        } else {
            self.parse_expr_block()?
        };
        Ok(Expr::If {
            condition: Box::new(condition),
            then_value: Box::new(then_value),
            else_value: Box::new(else_value),
        })
    }

    fn parse_expr_block(&mut self) -> Result<Expr, Vec<Diagnostic>> {
        self.expect(&TokenKind::LeftBrace, "expected '{'")?;
        let mut statements = Vec::new();
        while self.is_expr_block_setup_stmt() {
            statements.push(self.parse_stmt()?);
            self.consume_semicolons();
        }
        self.consume_semicolons();
        let expr = self.parse_expr()?;
        self.consume_semicolons();
        self.expect(&TokenKind::RightBrace, "expected '}'")?;
        if statements.is_empty() {
            Ok(expr)
        } else {
            Ok(Expr::Block {
                statements,
                value: Box::new(expr),
            })
        }
    }

    fn is_expr_block_setup_stmt(&self) -> bool {
        self.at(&TokenKind::Let)
            || self.at(&TokenKind::Var)
            || (matches!(self.peek().kind, TokenKind::Ident(_))
                && self.peek_next_is_assignment_op())
    }

    fn parse_match_pattern(&mut self) -> Result<MatchPattern, Vec<Diagnostic>> {
        match &self.peek().kind {
            TokenKind::Ident(name) if name == "_" => {
                self.advance();
                Ok(MatchPattern::Wildcard)
            }
            TokenKind::Ident(_) => {
                let segments = self.parse_qualified_ident_segments()?;
                if segments.len() < 2 {
                    return Err(vec![Diagnostic::error("expected enum variant pattern")]);
                }
                let variant = segments.last().expect("len checked").clone();
                let enum_name = segments[..segments.len() - 1].join(".");
                Ok(MatchPattern::EnumVariant { enum_name, variant })
            }
            TokenKind::IntLiteral(value) => {
                let value = *value;
                self.advance();
                Ok(MatchPattern::Int(value))
            }
            TokenKind::True => {
                self.advance();
                Ok(MatchPattern::Bool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(MatchPattern::Bool(false))
            }
            _ => Err(vec![Diagnostic::error("expected match pattern")]),
        }
    }

    fn expect_ident(&mut self) -> Result<String, Vec<Diagnostic>> {
        if let TokenKind::Ident(name) = &self.peek().kind {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(vec![Diagnostic::error("expected identifier")])
        }
    }

    fn parse_qualified_ident(&mut self) -> Result<String, Vec<Diagnostic>> {
        Ok(self.parse_qualified_ident_segments()?.join("."))
    }

    fn parse_qualified_ident_segments(&mut self) -> Result<Vec<String>, Vec<Diagnostic>> {
        let mut segments = vec![self.expect_ident()?];
        while self.matches(&TokenKind::Dot) {
            segments.push(self.expect_ident()?);
        }
        Ok(segments)
    }

    fn expect_import_segment(&mut self) -> Result<String, Vec<Diagnostic>> {
        match &self.peek().kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            TokenKind::String => {
                self.advance();
                Ok("string".to_string())
            }
            _ => Err(vec![Diagnostic::error("expected import path segment")]),
        }
    }

    fn expect(&mut self, kind: &TokenKind, message: &str) -> Result<(), Vec<Diagnostic>> {
        if self.matches(kind) {
            Ok(())
        } else {
            Err(vec![Diagnostic::error(message)])
        }
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.current += 1;
            true
        } else {
            false
        }
    }

    fn consume_semicolons(&mut self) {
        while self.matches(&TokenKind::Semicolon) {}
    }

    fn at(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.peek().kind) == std::mem::discriminant(kind)
    }

    fn peek_next_is_assignment_op(&self) -> bool {
        self.tokens
            .get(self.current + 1)
            .map(|token| {
                matches!(
                    token.kind,
                    TokenKind::Equal
                        | TokenKind::PlusEqual
                        | TokenKind::MinusEqual
                        | TokenKind::StarEqual
                        | TokenKind::SlashEqual
                        | TokenKind::PercentEqual
                        | TokenKind::AmpersandEqual
                        | TokenKind::PipeEqual
                        | TokenKind::CaretEqual
                        | TokenKind::ShiftLeftEqual
                        | TokenKind::ShiftRightEqual
                )
            })
            .unwrap_or(false)
    }

    fn starts_pointer_assignment(&self) -> bool {
        self.at(&TokenKind::Star)
            && matches!(
                self.tokens.get(self.current + 1).map(|token| &token.kind),
                Some(TokenKind::Ident(_))
            )
            && self.tokens.get(self.current + 2).is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Equal
                        | TokenKind::PlusEqual
                        | TokenKind::MinusEqual
                        | TokenKind::StarEqual
                        | TokenKind::SlashEqual
                        | TokenKind::PercentEqual
                        | TokenKind::AmpersandEqual
                        | TokenKind::PipeEqual
                        | TokenKind::CaretEqual
                        | TokenKind::ShiftLeftEqual
                        | TokenKind::ShiftRightEqual
                )
            })
    }

    fn is_assignment_op(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Equal
                | TokenKind::PlusEqual
                | TokenKind::MinusEqual
                | TokenKind::StarEqual
                | TokenKind::SlashEqual
                | TokenKind::PercentEqual
                | TokenKind::AmpersandEqual
                | TokenKind::PipeEqual
                | TokenKind::CaretEqual
                | TokenKind::ShiftLeftEqual
                | TokenKind::ShiftRightEqual
        )
    }

    fn assignment_op(&mut self) -> Result<Option<BinaryOp>, Vec<Diagnostic>> {
        if self.matches(&TokenKind::Equal) {
            Ok(None)
        } else if self.matches(&TokenKind::PlusEqual) {
            Ok(Some(BinaryOp::Add))
        } else if self.matches(&TokenKind::MinusEqual) {
            Ok(Some(BinaryOp::Sub))
        } else if self.matches(&TokenKind::StarEqual) {
            Ok(Some(BinaryOp::Mul))
        } else if self.matches(&TokenKind::SlashEqual) {
            Ok(Some(BinaryOp::Div))
        } else if self.matches(&TokenKind::PercentEqual) {
            Ok(Some(BinaryOp::Rem))
        } else if self.matches(&TokenKind::AmpersandEqual) {
            Ok(Some(BinaryOp::BitAnd))
        } else if self.matches(&TokenKind::PipeEqual) {
            Ok(Some(BinaryOp::BitOr))
        } else if self.matches(&TokenKind::CaretEqual) {
            Ok(Some(BinaryOp::BitXor))
        } else if self.matches(&TokenKind::ShiftLeftEqual) {
            Ok(Some(BinaryOp::ShiftLeft))
        } else if self.matches(&TokenKind::ShiftRightEqual) {
            Ok(Some(BinaryOp::ShiftRight))
        } else {
            Err(vec![Diagnostic::error("expected assignment operator")])
        }
    }

    fn advance(&mut self) -> &'a Token {
        let token = self.peek();
        if !self.at(&TokenKind::Eof) {
            self.current += 1;
        }
        token
    }

    fn peek(&self) -> &'a Token {
        &self.tokens[self.current]
    }
}

fn integer_suffix_type(suffix: &str) -> Result<Type, Vec<Diagnostic>> {
    match suffix {
        "int" => Ok(Type::Int),
        "usize" => Ok(Type::Usize),
        "i8" => Ok(Type::I8),
        "i16" => Ok(Type::I16),
        "i32" => Ok(Type::I32),
        "i64" => Ok(Type::I64),
        "u8" => Ok(Type::U8),
        "u16" => Ok(Type::U16),
        "u32" => Ok(Type::U32),
        "u64" => Ok(Type::U64),
        _ => Err(vec![Diagnostic::error(format!(
            "unknown integer literal suffix '{suffix}'"
        ))]),
    }
}
