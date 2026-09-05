use super::*;

impl Parser {
    // @pinker-nav:start parser.comandos.bloco
    // @pinker-nav:domain comandos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Blocos e comandos: reconhece `{ ... }`, declarações locais (`nova`/`muda`), atribuições, `mimo` (retorno), `talvez`/`senao`, laços (`sempre`/`repetir`), `quebrar`/`continuar`, `falar` e asm inline, produzindo `ast::Block`/`ast::Stmt`.
    pub(super) fn parse_block(&mut self) -> Result<Block, PinkerError> {
        let start_span = self.consume(TokenKind::LBrace, "{")?.span;
        self.function_value_scopes.push(HashMap::new());
        self.value_type_scopes.push(HashMap::new());
        // Parte G: o bloco é a unidade de escopo léxico das ligações locais, e
        // o corpo vive numa função à parte para que a pilha desempilhe também
        // no caminho de erro — simetria que `value_type_scopes` já tinha.
        self.abrir_escopo_local();
        let corpo = self.parse_block_stmts();
        self.fechar_escopo_local();
        self.value_type_scopes.pop();
        self.function_value_scopes.pop();
        let stmts = corpo?;
        Ok(Block {
            stmts,
            span: merge_span(start_span, self.previous().span),
        })
    }

    /// Comandos de um bloco, até o `}` que o fecha.
    ///
    /// Separado de `parse_block` só para que o escopo léxico da Parte G tenha
    /// um ponto único de fechamento.
    fn parse_block_stmts(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        let mut stmts = Vec::new();
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            let ja_produzidos = stmts.len();
            if self.check(TokenKind::KwPara) {
                stmts.extend(self.parse_for_stmt_desugared()?);
            } else if self.check(TokenKind::KwEncaixe) {
                stmts.extend(self.parse_encaixe()?);
            } else if self.check(TokenKind::KwTentar) {
                stmts.extend(self.parse_tentar_desugared()?);
            } else if self.check(TokenKind::KwPropagar) {
                stmts.extend(self.parse_propagar_desugared()?);
            } else if self.starts_function_value_let() {
                if let Some(stmt) = self.parse_function_value_let()? {
                    stmts.push(stmt);
                }
            } else {
                let stmt = self.parse_stmt()?;
                if let Stmt::Let(let_stmt) = &stmt {
                    self.current_function_value_scope_mut()
                        .remove(&let_stmt.name);
                }
                stmts.push(stmt);
            }
            // Parte G: o que este comando acabou de ligar passa a ser visível
            // do ponto seguinte em diante, e só dentro deste bloco. Ler o
            // resultado do parser cobre `nova`/`muda`, alias de função e todo
            // local nascido de desugaring sem repetir a lista de construtos.
            self.registrar_ligacoes_de_stmts(&stmts[ja_produzidos..]);
        }
        self.consume(TokenKind::RBrace, "}")?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, PinkerError> {
        if self.match_token(TokenKind::KwNova) {
            let start_span = self.previous().span;
            let is_mut = self.match_token(TokenKind::KwMuda);
            let name = self
                .consume(TokenKind::Ident, "nome da variável")?
                .lexeme
                .clone();
            let ty = if self.match_token(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            // Registra tipo de coleção para dispatch em `para cada`.
            if let Some(declared_ty) = &ty {
                match declared_ty {
                    Type::ListBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListBombom);
                    }
                    Type::ListVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListVerso);
                    }
                    Type::ListEnum { element, .. } => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListEnum(element.clone()));
                    }
                    Type::MapVersoBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapVersoBombom);
                    }
                    Type::MapVersoVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapVersoVerso);
                    }
                    Type::MapBombomBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapBombomBombom);
                    }
                    Type::MapBombomVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapBombomVerso);
                    }
                    Type::Map { key, .. } => {
                        self.collection_types.insert(
                            name.clone(),
                            CollectionKind::Map {
                                key: key.as_ref().clone(),
                            },
                        );
                    }
                    _ => {}
                }
            }
            self.consume(TokenKind::Eq, "=")?;
            self.declarando.push(name.clone());
            let init = self.parse_expr();
            self.declarando.pop();
            let init = init?;
            self.consume(TokenKind::Semi, ";")?;
            if let Some(value_ty) = ty.clone().or_else(|| self.infer_local_expr_type(&init)) {
                self.register_value_type(&name, value_ty);
            }
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

        if self.match_token(TokenKind::KwFalar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::LParen, "(")?;
            let mut args = vec![self.parse_expr()?];
            while self.match_token(TokenKind::Comma) {
                args.push(self.parse_expr()?);
            }
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Falar(FalarStmt {
                args,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwSussurro) {
            let start_span = self.previous().span;
            self.consume(TokenKind::LParen, "(")?;
            let mut chunks = Vec::new();
            let first_chunk = self.consume(
                TokenKind::StringLit,
                "string literal em sussurro (ex.: \"mov rax, 60\")",
            )?;
            chunks.push(first_chunk.lexeme.clone());
            while self.check(TokenKind::Comma) && self.check_at(1, TokenKind::StringLit) {
                self.advance();
                let chunk = self
                    .consume(TokenKind::StringLit, "string literal em sussurro")?
                    .clone();
                chunks.push(chunk.lexeme);
            }

            let mut operands = Vec::new();
            let mut clobbers = Vec::new();
            if self.match_token(TokenKind::Semi) {
                while !self.check(TokenKind::RParen) {
                    let clause = self
                        .consume(
                            TokenKind::Ident,
                            "'entrada', 'saida' ou 'destroi' em sussurro",
                        )?
                        .clone();
                    if clause.lexeme == "destroi" {
                        self.consume(TokenKind::LParen, "(")?;
                        if !self.check(TokenKind::RParen) {
                            loop {
                                let clobber = self
                                    .consume(TokenKind::Ident, "efeito destruído por sussurro")?
                                    .clone();
                                clobbers.push(InlineAsmClobber {
                                    name: clobber.lexeme,
                                    span: clobber.span,
                                });
                                if !self.match_token(TokenKind::Comma) {
                                    break;
                                }
                            }
                        }
                        self.consume(TokenKind::RParen, ")")?;
                    } else {
                        let direction = match clause.lexeme.as_str() {
                            "entrada" => InlineAsmDirection::Input,
                            "saida" => InlineAsmDirection::Output,
                            _ => InlineAsmDirection::Unknown(clause.lexeme.clone()),
                        };
                        let name = self
                            .consume(TokenKind::Ident, "nome do operando de sussurro")?
                            .clone();
                        self.consume(TokenKind::Colon, ":")?;
                        let constraint = self
                            .consume(TokenKind::Ident, "constraint do operando de sussurro")?
                            .clone();
                        self.consume(TokenKind::Eq, "=")?;
                        let value = self.parse_expr()?;
                        operands.push(InlineAsmOperand {
                            name: name.lexeme,
                            direction,
                            constraint: constraint.lexeme,
                            span: merge_span(clause.span, value.span),
                            value,
                        });
                    }
                    if !self.match_token(TokenKind::Semi) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::InlineAsm(InlineAsmStmt {
                chunks,
                operands,
                clobbers,
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

        if self.match_token(TokenKind::KwRepetir) {
            let start_span = self.previous().span;
            let body = self.parse_block()?;
            self.consume(TokenKind::KwAte, "ate")?;
            let condition = self.parse_expr()?;
            self.consume(TokenKind::Semi, ";")?;
            let loop_span = merge_span(start_span, self.previous().span);
            let break_stmt = Stmt::If(IfStmt {
                condition,
                then_branch: Block {
                    stmts: vec![Stmt::Break(BreakStmt { span: loop_span })],
                    span: loop_span,
                },
                else_branch: None,
                span: loop_span,
            });
            let mut while_body = body.stmts;
            while_body.push(break_stmt);
            return Ok(Stmt::While(WhileStmt {
                condition: Expr {
                    kind: ExprKind::BoolLit(true),
                    span: loop_span,
                },
                body: Block {
                    stmts: while_body,
                    span: loop_span,
                },
                span: loop_span,
            }));
        }

        if self.match_token(TokenKind::KwEscolha) {
            let start_span = self.previous().span;
            let scrutinee = self.parse_expr()?;
            self.consume(TokenKind::LBrace, "{")?;

            let mut cases: Vec<(Expr, Block)> = Vec::new();
            let mut default_block: Option<Block> = None;

            while !self.check(TokenKind::RBrace) && self.peek().is_some() {
                if self.match_token(TokenKind::KwCaso) {
                    let pattern = self.parse_expr()?;
                    let body = self.parse_block()?;
                    cases.push((pattern, body));
                } else if self.match_token(TokenKind::KwSenao) {
                    default_block = Some(self.parse_block()?);
                    break;
                } else {
                    return Err(PinkerError::Parse {
                        msg: "esperado 'caso' ou 'senao' dentro de 'escolha'".to_string(),
                        span: self.peek_span(),
                    });
                }
            }
            self.consume(TokenKind::RBrace, "}")?;
            let end_span = self.previous().span;

            let mut result: Option<Stmt> = default_block.map(|blk| {
                Stmt::If(IfStmt {
                    condition: Expr {
                        kind: ExprKind::BoolLit(true),
                        span: blk.span,
                    },
                    then_branch: blk.clone(),
                    else_branch: None,
                    span: blk.span,
                })
            });

            for (pattern, body) in cases.into_iter().rev() {
                let cond = Expr {
                    kind: ExprKind::Binary(
                        Box::new(scrutinee.clone()),
                        BinaryOp::Eq,
                        Box::new(pattern),
                    ),
                    span: body.span,
                };
                let else_branch = result.map(|stmt| match stmt {
                    Stmt::If(if_stmt) => ElseBlock::If(Box::new(if_stmt)),
                    _ => unreachable!(),
                });
                result = Some(Stmt::If(IfStmt {
                    condition: cond,
                    then_branch: body,
                    else_branch,
                    span: merge_span(start_span, end_span),
                }));
            }

            return Ok(result.unwrap_or_else(|| {
                Stmt::Expr(Expr {
                    kind: ExprKind::IntLit(0),
                    span: merge_span(start_span, end_span),
                })
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

        let compound_op = if self.match_token(TokenKind::PlusEq) {
            Some(BinaryOp::Add)
        } else if self.match_token(TokenKind::MinusEq) {
            Some(BinaryOp::Sub)
        } else if self.match_token(TokenKind::StarEq) {
            Some(BinaryOp::Mul)
        } else if self.match_token(TokenKind::SlashEq) {
            Some(BinaryOp::Div)
        } else if self.match_token(TokenKind::PercentEq) {
            Some(BinaryOp::Mod)
        } else {
            None
        };

        if compound_op.is_some() || self.match_token(TokenKind::Eq) {
            let target = match &expr.kind {
                ExprKind::Ident(name) => AssignTarget::Ident(name.clone()),
                ExprKind::Unary(UnaryOp::Deref, ptr_expr) => {
                    AssignTarget::Deref(Box::new((**ptr_expr).clone()))
                }
                ExprKind::FieldAccess { base, field } => AssignTarget::FieldDeref {
                    base: base.clone(),
                    field: field.clone(),
                },
                ExprKind::Index { base, index } => AssignTarget::Index {
                    base: base.clone(),
                    index: index.clone(),
                },
                _ => {
                    return Err(PinkerError::Parse {
                        msg: "atribuição inválida: o lado esquerdo deve ser um identificador, dereferência '*expr', acesso a campo '(*ptr).campo' ou indexação 'base[índice]'".to_string(),
                        span: expr.span,
                    });
                }
            };
            let rhs = self.parse_expr()?;
            let final_rhs = if let Some(op) = compound_op {
                Expr {
                    kind: ExprKind::Binary(Box::new(expr.clone()), op, Box::new(rhs)),
                    span: expr.span,
                }
            } else {
                rhs
            };
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Assign(AssignStmt {
                target,
                expr: final_rhs,
                span: merge_span(expr.span, self.previous().span),
            }));
        }

        self.consume(TokenKind::Semi, ";")?;
        Ok(Stmt::Expr(Expr {
            kind: expr.kind,
            span: merge_span(expr.span, self.previous().span),
        }))
    }

    // @pinker-nav:end parser.comandos.bloco
}
