use crate::ast::*;
use crate::error::PinkerError;
use crate::token::{Span, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

fn merge_span(a: Span, b: Span) -> Span {
    a.merge(b)
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens
            .get(self.current)
            .filter(|token| token.kind != TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.current)
            .map(|token| token.span)
            .or_else(|| self.tokens.last().map(|token| token.span))
            .unwrap_or_else(|| Span::single(crate::token::Position::new(1, 1)))
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.current >= self.tokens.len() {
            return None;
        }

        let token = &self.tokens[self.current];
        self.current += 1;
        if token.kind == TokenKind::Eof {
            None
        } else {
            Some(token)
        }
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().map(|token| token.kind == kind).unwrap_or(false)
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind, expected: &str) -> Result<&Token, PinkerError> {
        if self.check(kind) {
            Ok(self.advance().unwrap())
        } else {
            let found = self
                .peek()
                .map(|token| token.lexeme.clone())
                .unwrap_or_default();
            Err(PinkerError::Expected {
                expected: expected.to_string(),
                found,
                span: self.peek_span(),
            })
        }
    }

    pub fn parse(&mut self) -> Result<Program, PinkerError> {
        let package = if self.match_token(TokenKind::KwPacote) {
            let start_span = self.previous().span;
            let name = self
                .consume(TokenKind::Ident, "nome do pacote")?
                .lexeme
                .clone();
            self.consume(TokenKind::Semi, ";")?;
            Some(PackageDecl {
                name,
                span: merge_span(start_span, self.previous().span),
            })
        } else {
            None
        };

        let mut items = Vec::new();
        while self.peek().is_some() {
            items.push(self.parse_item()?);
        }

        Ok(Program { package, items })
    }

    fn parse_item(&mut self) -> Result<Item, PinkerError> {
        if self.match_token(TokenKind::KwCarinho) {
            Ok(Item::Function(self.parse_function()?))
        } else if self.match_token(TokenKind::KwEterno) {
            Ok(Item::Const(self.parse_const()?))
        } else if self.match_token(TokenKind::KwApelido) {
            Ok(Item::TypeAlias(self.parse_type_alias()?))
        } else if self.match_token(TokenKind::KwNinho) {
            Ok(Item::Struct(self.parse_struct_decl()?))
        } else {
            Err(PinkerError::Expected {
                expected: "carinho, eterno, apelido ou ninho".to_string(),
                found: self
                    .peek()
                    .map(|token| token.lexeme.clone())
                    .unwrap_or_default(),
                span: self.peek_span(),
            })
        }
    }

    fn parse_type(&mut self) -> Result<Type, PinkerError> {
        let span = self.peek_span();
        if self.match_token(TokenKind::KwFragil) {
            let qualifier_span = self.previous().span;
            let ty = self.parse_type()?;
            return match ty {
                Type::Pointer {
                    base,
                    is_volatile: false,
                    span: pointer_span,
                } => Ok(Type::Pointer {
                    base,
                    is_volatile: true,
                    span: merge_span(qualifier_span, pointer_span),
                }),
                Type::Pointer {
                    is_volatile: true,
                    span: pointer_span,
                    ..
                } => Err(PinkerError::Expected {
                    expected: "tipo seta sem qualificador repetido".to_string(),
                    found: "fragil".to_string(),
                    span: merge_span(qualifier_span, pointer_span),
                }),
                _ => Err(PinkerError::Expected {
                    expected: "'fragil' só pode qualificar tipo seta (ex.: fragil seta<u8>)"
                        .to_string(),
                    found: ty.name().to_string(),
                    span: ty.span(),
                }),
            };
        }
        if self.match_token(TokenKind::LBracket) {
            let start_span = self.previous().span;
            let element = self.parse_type()?;
            self.consume(TokenKind::Semi, ";")?;
            let size_token = self.consume(TokenKind::IntLit, "tamanho inteiro do array fixo")?;
            let size = size_token
                .lexeme
                .parse::<u64>()
                .map_err(|_| PinkerError::Expected {
                    expected: "tamanho inteiro válido do array fixo".to_string(),
                    found: size_token.lexeme.clone(),
                    span: size_token.span,
                })?;
            self.consume(TokenKind::RBracket, "]")?;
            return Ok(Type::FixedArray {
                element: Box::new(element),
                size,
                span: merge_span(start_span, self.previous().span),
            });
        }
        if self.match_token(TokenKind::KwSeta) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Less, "<")?;
            let base = self.parse_type()?;
            self.consume(TokenKind::Greater, ">")?;
            return Ok(Type::Pointer {
                base: Box::new(base),
                is_volatile: false,
                span: merge_span(start_span, self.previous().span),
            });
        }

        if self.match_token(TokenKind::KwBombom) {
            Ok(Type::Bombom(span))
        } else if self.match_token(TokenKind::KwU8) {
            Ok(Type::U8(span))
        } else if self.match_token(TokenKind::KwU16) {
            Ok(Type::U16(span))
        } else if self.match_token(TokenKind::KwU32) {
            Ok(Type::U32(span))
        } else if self.match_token(TokenKind::KwU64) {
            Ok(Type::U64(span))
        } else if self.match_token(TokenKind::KwI8) {
            Ok(Type::I8(span))
        } else if self.match_token(TokenKind::KwI16) {
            Ok(Type::I16(span))
        } else if self.match_token(TokenKind::KwI32) {
            Ok(Type::I32(span))
        } else if self.match_token(TokenKind::KwI64) {
            Ok(Type::I64(span))
        } else if self.match_token(TokenKind::KwLogica) {
            Ok(Type::Logica(span))
        } else if self.match_token(TokenKind::Ident) {
            Ok(Type::Alias {
                name: self.previous().lexeme.clone(),
                span,
            })
        } else {
            Err(PinkerError::Expected {
                expected:
                    "tipo válido (ex.: bombom, logica, alias, [tipo; N], seta<tipo> ou fragil seta<tipo>)"
                        .to_string(),
                found: self
                    .peek()
                    .map(|token| token.lexeme.clone())
                    .unwrap_or_default(),
                span,
            })
        }
    }

    fn parse_type_alias(&mut self) -> Result<TypeAliasDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome do alias de tipo")?
            .lexeme
            .clone();
        self.consume(TokenKind::Eq, "=")?;
        let target = self.parse_type()?;
        self.consume(TokenKind::Semi, ";")?;
        Ok(TypeAliasDecl {
            name,
            target,
            span: merge_span(start_span, self.previous().span),
        })
    }

    fn parse_struct_decl(&mut self) -> Result<StructDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da struct")?
            .lexeme
            .clone();
        self.consume(TokenKind::LBrace, "{")?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) {
            let field_name = self
                .consume(TokenKind::Ident, "nome do campo da struct")?
                .lexeme
                .clone();
            let field_start = self.previous().span;
            self.consume(TokenKind::Colon, ":")?;
            let ty = self.parse_type()?;
            self.consume(TokenKind::Semi, ";")?;
            fields.push(StructField {
                name: field_name,
                ty,
                span: merge_span(field_start, self.previous().span),
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        Ok(StructDecl {
            name,
            fields,
            span: merge_span(start_span, self.previous().span),
        })
    }

    fn parse_function(&mut self) -> Result<FunctionDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da função")?
            .lexeme
            .clone();

        self.consume(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                let name = self
                    .consume(TokenKind::Ident, "nome do parâmetro")?
                    .lexeme
                    .clone();
                let param_start = self.previous().span;
                self.consume(TokenKind::Colon, ":")?;
                let ty = self.parse_type()?;
                params.push(Param {
                    name,
                    ty,
                    span: merge_span(param_start, self.previous().span),
                });
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, ")")?;

        let ret_type = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        Ok(FunctionDecl {
            name,
            params,
            ret_type,
            span: merge_span(start_span, body.span),
            body,
        })
    }

    fn parse_const(&mut self) -> Result<ConstDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da constante")?
            .lexeme
            .clone();
        self.consume(TokenKind::Colon, ":")?;
        let ty = self.parse_type()?;
        self.consume(TokenKind::Eq, "=")?;
        let init = self.parse_expr()?;
        self.consume(TokenKind::Semi, ";")?;

        Ok(ConstDecl {
            name,
            ty,
            init,
            span: merge_span(start_span, self.previous().span),
        })
    }

    fn parse_block(&mut self) -> Result<Block, PinkerError> {
        let start_span = self.consume(TokenKind::LBrace, "{")?.span;
        let mut stmts = Vec::new();

        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            stmts.push(self.parse_stmt()?);
        }

        self.consume(TokenKind::RBrace, "}")?;
        Ok(Block {
            stmts,
            span: merge_span(start_span, self.previous().span),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, PinkerError> {
        if self.match_token(TokenKind::KwNova) {
            let start_span = self.previous().span;
            let is_mut = self.match_token(TokenKind::KwMut);
            let name = self
                .consume(TokenKind::Ident, "nome da variável")?
                .lexeme
                .clone();
            let ty = if self.match_token(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            self.consume(TokenKind::Eq, "=")?;
            let init = self.parse_expr()?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Let(LetStmt {
                name,
                is_mut,
                ty,
                init,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwMimo) {
            let start_span = self.previous().span;
            let expr = if self.check(TokenKind::Semi) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Return(ReturnStmt {
                expr,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwQuebrar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Break(BreakStmt {
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwContinuar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Continue(ContinueStmt {
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwSussurro) {
            let start_span = self.previous().span;
            self.consume(TokenKind::LParen, "(")?;
            let mut chunks = Vec::new();
            loop {
                let chunk_token = self.consume(
                    TokenKind::StringLit,
                    "string literal em sussurro (ex.: \"mov rax, 60\")",
                )?;
                chunks.push(chunk_token.lexeme.clone());
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::InlineAsm(InlineAsmStmt {
                chunks,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwSempre) {
            let start_span = self.previous().span;
            self.consume(TokenKind::KwQue, "que")?;
            let condition = self.parse_expr()?;
            let body = self.parse_block()?;
            let span = merge_span(start_span, body.span);
            return Ok(Stmt::While(WhileStmt {
                condition,
                body,
                span,
            }));
        }

        if self.match_token(TokenKind::KwTalvez) {
            let start_span = self.previous().span;
            let condition = self.parse_expr()?;
            let then_branch = self.parse_block()?;
            let else_branch = if self.match_token(TokenKind::KwSenao) {
                if self.check(TokenKind::KwTalvez) {
                    let nested = self.parse_stmt()?;
                    match nested {
                        Stmt::If(if_stmt) => Some(ElseBlock::If(Box::new(if_stmt))),
                        _ => unreachable!("parse_stmt após 'senao talvez' deve retornar If"),
                    }
                } else {
                    Some(ElseBlock::Block(self.parse_block()?))
                }
            } else {
                None
            };

            let end_span = else_branch
                .as_ref()
                .map(ElseBlock::span)
                .unwrap_or(then_branch.span);

            return Ok(Stmt::If(IfStmt {
                condition,
                then_branch,
                else_branch,
                span: merge_span(start_span, end_span),
            }));
        }

        let expr = self.parse_expr()?;
        if self.match_token(TokenKind::Eq) {
            if let ExprKind::Ident(name) = &expr.kind {
                let rhs = self.parse_expr()?;
                self.consume(TokenKind::Semi, ";")?;
                return Ok(Stmt::Assign(AssignStmt {
                    name: name.clone(),
                    expr: rhs,
                    span: merge_span(expr.span, self.previous().span),
                }));
            }
            return Err(PinkerError::Parse {
                msg: "atribuição inválida: o lado esquerdo deve ser um identificador".to_string(),
                span: expr.span,
            });
        }

        self.consume(TokenKind::Semi, ";")?;
        Ok(Stmt::Expr(Expr {
            kind: expr.kind,
            span: merge_span(expr.span, self.previous().span),
        }))
    }

    fn parse_expr(&mut self) -> Result<Expr, PinkerError> {
        self.parse_expr_binary(0)
    }

    fn parse_expr_binary(&mut self, min_prec: u8) -> Result<Expr, PinkerError> {
        let mut lhs = self.parse_expr_unary()?;

        while let Some(token) = self.peek() {
            let op = match BinaryOp::from_token(token.kind) {
                Some(op) => op,
                None => break,
            };

            let prec = Self::precedence(op);
            if prec < min_prec {
                break;
            }

            self.advance();
            let rhs = self.parse_expr_binary(prec + 1)?;
            lhs = Expr {
                span: merge_span(lhs.span, rhs.span),
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
            };
        }

        Ok(lhs)
    }

    fn precedence(op: BinaryOp) -> u8 {
        match op {
            BinaryOp::LogicalOr => 1,
            BinaryOp::LogicalAnd => 2,
            BinaryOp::BitOr => 3,
            BinaryOp::BitXor => 4,
            BinaryOp::BitAnd => 5,
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Gte => 6,
            BinaryOp::Shl | BinaryOp::Shr => 7,
            BinaryOp::Add | BinaryOp::Sub => 8,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 9,
        }
    }

    fn parse_expr_unary(&mut self) -> Result<Expr, PinkerError> {
        if let Some(token) = self.peek() {
            if token.kind == TokenKind::Minus || token.kind == TokenKind::Bang {
                let op_span = token.span;
                let token_kind = token.kind;
                self.advance();
                let operand = self.parse_expr_unary()?;
                let unary_expr = Expr {
                    span: merge_span(op_span, operand.span),
                    kind: ExprKind::Unary(
                        if token_kind == TokenKind::Minus {
                            UnaryOp::Neg
                        } else {
                            UnaryOp::Not
                        },
                        Box::new(operand),
                    ),
                };
                return self.parse_cast_suffix(unary_expr);
            }
        }

        let expr = self.parse_expr_primary()?;
        self.parse_cast_suffix(expr)
    }

    fn parse_expr_primary(&mut self) -> Result<Expr, PinkerError> {
        let eof_span = self.peek_span();
        let token = self.advance().ok_or(PinkerError::Parse {
            msg: "fim inesperado da expressão".to_string(),
            span: eof_span,
        })?;

        let base = match token.kind {
            TokenKind::IntLit => Ok(Expr {
                kind: ExprKind::IntLit(token.lexeme.parse().unwrap()),
                span: token.span,
            }),
            TokenKind::KwVerdade => Ok(Expr {
                kind: ExprKind::BoolLit(true),
                span: token.span,
            }),
            TokenKind::KwFalso => Ok(Expr {
                kind: ExprKind::BoolLit(false),
                span: token.span,
            }),
            TokenKind::Ident => Ok(Expr {
                kind: ExprKind::Ident(token.lexeme.clone()),
                span: token.span,
            }),
            TokenKind::LParen => {
                let lparen_span = token.span;
                let expr = self.parse_expr()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: expr.kind,
                    span: merge_span(lparen_span, self.previous().span),
                })
            }
            TokenKind::KwPeso => {
                let start_span = token.span;
                self.consume(TokenKind::LParen, "(")?;
                let target = self.parse_type()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: ExprKind::SizeOfType { target },
                    span: merge_span(start_span, self.previous().span),
                })
            }
            TokenKind::KwAlinhamento => {
                let start_span = token.span;
                self.consume(TokenKind::LParen, "(")?;
                let target = self.parse_type()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: ExprKind::AlignOfType { target },
                    span: merge_span(start_span, self.previous().span),
                })
            }
            _ => Err(PinkerError::Parse {
                msg: format!("expressão inválida: '{}'", token.lexeme),
                span: token.span,
            }),
        }?;

        self.parse_postfix_suffix(base)
    }

    fn parse_postfix_suffix(&mut self, mut expr: Expr) -> Result<Expr, PinkerError> {
        loop {
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, ")")?;
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::Call(Box::new(expr), args),
                };
                continue;
            }
            if self.match_token(TokenKind::Dot) {
                let field = self
                    .consume(TokenKind::Ident, "nome do campo após '.'")?
                    .lexeme
                    .clone();
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::FieldAccess {
                        base: Box::new(expr),
                        field,
                    },
                };
                continue;
            }
            if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.consume(TokenKind::RBracket, "]")?;
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_cast_suffix(&mut self, mut expr: Expr) -> Result<Expr, PinkerError> {
        while self.match_token(TokenKind::KwVirar) {
            let target = self.parse_type()?;
            expr = Expr {
                span: merge_span(expr.span, target.span()),
                kind: ExprKind::Cast {
                    expr: Box::new(expr),
                    target,
                },
            };
        }
        Ok(expr)
    }
}
